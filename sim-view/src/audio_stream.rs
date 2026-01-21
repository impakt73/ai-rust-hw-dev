use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use cpu_sim::AudioConfig;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

// Audio buffer size limit (0.5 seconds at 48kHz mono/stereo)
const MAX_AUDIO_BUFFER_SAMPLES: usize = 48000;

pub struct AudioStream {
    /// CPAL audio device
    device: cpal::Device,

    /// Active audio output stream
    stream: cpal::Stream,

    /// Sample buffer queue (thread-safe)
    sample_buffer: Arc<Mutex<VecDeque<i16>>>,

    /// Current audio configuration
    current_config: Option<AudioConfig>,
}

impl AudioStream {
    pub fn new() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("No audio output device available")?;

        log::info!(
            "Using audio device: {}",
            device.name().unwrap_or_else(|_| "Unknown".to_string())
        );

        // Shared buffer for audio samples
        let sample_buffer = Arc::new(Mutex::new(VecDeque::new()));

        // Create initial stream with default config
        let stream = Self::create_stream(&device, None, Arc::clone(&sample_buffer))?;

        // Start playback
        stream
            .play()
            .map_err(|e| format!("Failed to start audio stream: {}", e))?;

        Ok(AudioStream {
            device,
            stream,
            sample_buffer,
            current_config: None,
        })
    }

    /// Recreate the audio stream with a new configuration
    pub fn set_config(&mut self, config: &AudioConfig) -> Result<(), String> {
        log::info!(
            "Reconfiguring audio stream: {} Hz, {:?}, {} samples",
            config.sample_rate.to_hz(),
            config.channels,
            config.sample_count
        );

        // Store the new configuration
        self.current_config = Some(*config);

        // Pause the existing stream to prevent it from consuming samples during transition
        if let Err(e) = self.stream.pause() {
            log::warn!("Failed to pause audio stream during reconfiguration: {}", e);
        }

        // Clear any samples remaining from the previous configuration to avoid
        // playing data with mismatched parameters after reconfiguration.
        if let Ok(mut buffer) = self.sample_buffer.lock() {
            buffer.clear();
        }

        // Create new stream with the specified configuration
        let new_stream =
            Self::create_stream(&self.device, Some(config), Arc::clone(&self.sample_buffer))?;

        // Start the new stream
        new_stream
            .play()
            .map_err(|e| format!("Failed to start reconfigured audio stream: {}", e))?;

        // Replace the old stream (this will drop and stop it)
        self.stream = new_stream;

        log::info!("Audio stream reconfigured successfully");
        Ok(())
    }

    /// Create a cpal stream with the given configuration
    /// If config is None, uses the device's default configuration
    fn create_stream(
        device: &cpal::Device,
        config: Option<&AudioConfig>,
        sample_buffer: Arc<Mutex<VecDeque<i16>>>,
    ) -> Result<cpal::Stream, String> {
        let (stream_config, sample_format) = if let Some(cfg) = config {
            // Build desired configuration from AudioConfig
            let sample_rate = cpal::SampleRate(cfg.sample_rate.to_hz());
            let channels = cfg.channels.count() as u16;

            // Query device for supported output configurations and try to find a match
            let supported_configs = device
                .supported_output_configs()
                .map_err(|e| format!("Failed to query supported audio configs: {}", e))?;

            let mut matching_range = None;
            for range in supported_configs {
                if range.channels() == channels
                    && range.min_sample_rate().0 <= sample_rate.0
                    && range.max_sample_rate().0 >= sample_rate.0
                {
                    matching_range = Some(range);
                    break;
                }
            }

            if let Some(range) = matching_range {
                // Use a supported configuration that matches the requested parameters
                let supported_config = range.with_sample_rate(sample_rate);
                let sample_format = supported_config.sample_format();
                let config: StreamConfig = supported_config.into();

                log::info!(
                    "Using requested audio config: {} Hz, {:?}, format: {:?}",
                    cfg.sample_rate.to_hz(),
                    cfg.channels,
                    sample_format
                );

                (config, sample_format)
            } else {
                // Fall back to device default if the requested config is not supported
                let supported_config = device
                    .default_output_config()
                    .map_err(|e| format!("Failed to get default audio config: {}", e))?;

                log::warn!(
                    "Requested audio config {} Hz, {:?} is not supported; \
                     falling back to device default: {} Hz, {} channels, format: {:?}",
                    cfg.sample_rate.to_hz(),
                    cfg.channels,
                    supported_config.sample_rate().0,
                    supported_config.channels(),
                    supported_config.sample_format()
                );

                let sample_format = supported_config.sample_format();
                let config: StreamConfig = supported_config.into();

                (config, sample_format)
            }
        } else {
            // Use default configuration
            let supported_config = device
                .default_output_config()
                .map_err(|e| format!("Failed to get default audio config: {}", e))?;

            let sample_format = supported_config.sample_format();
            let config: StreamConfig = supported_config.into();

            log::info!(
                "Audio config: {} Hz, {} channels, format: {:?}",
                config.sample_rate.0,
                config.channels,
                sample_format
            );

            (config, sample_format)
        };

        // Build output stream based on sample format
        let stream = match sample_format {
            SampleFormat::I16 => {
                Self::build_i16_stream(device, &stream_config, Arc::clone(&sample_buffer))?
            }
            SampleFormat::F32 => {
                Self::build_f32_stream(device, &stream_config, Arc::clone(&sample_buffer))?
            }
            SampleFormat::U16 => {
                Self::build_u16_stream(device, &stream_config, Arc::clone(&sample_buffer))?
            }
            _ => {
                return Err(format!("Unsupported sample format: {:?}", sample_format));
            }
        };

        Ok(stream)
    }

    /// Push audio samples to the buffer for playback
    /// This is called by the main viewer loop with samples from the simulator
    pub fn push_samples(&self, samples: &[i16]) {
        let mut buf = self.sample_buffer.lock().unwrap();

        // Add samples to buffer
        for &sample in samples {
            buf.push_back(sample);
        }

        // Limit buffer size to prevent unbounded growth
        while buf.len() > MAX_AUDIO_BUFFER_SAMPLES {
            buf.pop_front();
        }
    }

    /// Fill `data` from the shared sample `buffer`, invoking `conv` for each sample (or when the buffer is empty).
    ///
    /// This helper centralizes underrun counting and logging so the callbacks remain small and consistent.
    fn fill_from_buffer<T, F>(data: &mut [T], buffer: &Arc<Mutex<VecDeque<i16>>>, mut conv: F)
    where
        F: FnMut(Option<i16>) -> T,
    {
        let total = data.len();
        let mut underruns = 0usize;

        let mut buf = buffer.lock().unwrap();
        for slot in data.iter_mut() {
            match buf.pop_front() {
                Some(v) => *slot = conv(Some(v)),
                None => {
                    *slot = conv(None);
                    underruns += 1;
                }
            }
        }

        if underruns > 0 {
            let available = total - underruns;
            log::warn!(
                "Audio output buffer underrun: {}/{} samples available, injecting {} silent sample(s)",
                available,
                total,
                underruns
            );
        }
    }

    /// Build i16 output stream
    fn build_i16_stream(
        device: &cpal::Device,
        config: &StreamConfig,
        buffer: Arc<Mutex<VecDeque<i16>>>,
    ) -> Result<cpal::Stream, String> {
        device
            .build_output_stream(
                config,
                move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                    Self::fill_from_buffer(data, &buffer, |opt| opt.unwrap_or(0));
                },
                |err| eprintln!("Audio stream error: {}", err),
                None,
            )
            .map_err(|e| format!("Failed to build i16 stream: {}", e))
    }

    /// Build f32 output stream
    fn build_f32_stream(
        device: &cpal::Device,
        config: &StreamConfig,
        buffer: Arc<Mutex<VecDeque<i16>>>,
    ) -> Result<cpal::Stream, String> {
        device
            .build_output_stream(
                config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    Self::fill_from_buffer(data, &buffer, |opt| {
                        opt.map(|s| s as f32 / 32768.0).unwrap_or(0.0)
                    });
                },
                |err| eprintln!("Audio stream error: {}", err),
                None,
            )
            .map_err(|e| format!("Failed to build f32 stream: {}", e))
    }

    /// Build u16 output stream
    fn build_u16_stream(
        device: &cpal::Device,
        config: &StreamConfig,
        buffer: Arc<Mutex<VecDeque<i16>>>,
    ) -> Result<cpal::Stream, String> {
        device
            .build_output_stream(
                config,
                move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                    let center = ((i16::MAX as i32) + 1) as u16;
                    Self::fill_from_buffer(data, &buffer, |opt| match opt {
                        Some(sample_i16) => {
                            // Convert i16 to u16 (shift range)
                            let shifted = (sample_i16 as i32) + (i16::MAX as i32) + 1;
                            shifted.clamp(0, u16::MAX as i32) as u16
                        }
                        None => center,
                    });
                },
                |err| eprintln!("Audio stream error: {}", err),
                None,
            )
            .map_err(|e| format!("Failed to build u16 stream: {}", e))
    }
}

// Stream is automatically stopped when dropped
impl Drop for AudioStream {
    fn drop(&mut self) {
        log::debug!("Audio stream dropped");
    }
}
