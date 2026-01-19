use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

// Audio buffer size limit (0.5 seconds at 48kHz mono/stereo)
const MAX_AUDIO_BUFFER_SAMPLES: usize = 48000;

pub struct AudioStream {
    /// Active audio output stream
    _stream: cpal::Stream,

    /// Sample buffer queue (thread-safe)
    sample_buffer: Arc<Mutex<VecDeque<i16>>>,
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

        // Get default configuration
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

        // Shared buffer for audio samples
        let sample_buffer = Arc::new(Mutex::new(VecDeque::new()));

        // Build output stream based on sample format
        let stream = match sample_format {
            SampleFormat::I16 => {
                Self::build_i16_stream(&device, &config, Arc::clone(&sample_buffer))?
            }
            SampleFormat::F32 => {
                Self::build_f32_stream(&device, &config, Arc::clone(&sample_buffer))?
            }
            SampleFormat::U16 => {
                Self::build_u16_stream(&device, &config, Arc::clone(&sample_buffer))?
            }
            _ => {
                return Err(format!("Unsupported sample format: {:?}", sample_format));
            }
        };

        // Start playback
        stream
            .play()
            .map_err(|e| format!("Failed to start audio stream: {}", e))?;

        Ok(AudioStream {
            _stream: stream,
            sample_buffer,
        })
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
                    let mut buf = buffer.lock().unwrap();

                    for sample_slot in data.iter_mut() {
                        *sample_slot = buf.pop_front().unwrap_or(0);
                    }
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
                    let mut buf = buffer.lock().unwrap();

                    for sample_slot in data.iter_mut() {
                        let sample_i16 = buf.pop_front().unwrap_or(0);
                        // Convert i16 to f32 in range [-1.0, 1.0]
                        // Using 32768.0 ensures proper normalization for both positive and negative values
                        *sample_slot = sample_i16 as f32 / 32768.0;
                    }
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
                    let mut buf = buffer.lock().unwrap();

                    for sample_slot in data.iter_mut() {
                        let sample_i16 = buf.pop_front().unwrap_or(0);
                        // Convert i16 to u16 (shift range)
                        let shifted = (sample_i16 as i32) + (i16::MAX as i32) + 1;
                        *sample_slot = shifted.clamp(0, u16::MAX as i32) as u16;
                    }
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
