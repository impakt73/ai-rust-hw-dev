use crate::bus_device::{BusDevice, BusDeviceError, SystemContext};

// Re-export types from riscv_shared for backward compatibility
pub use riscv_shared::audio::{AudioChannels, AudioConfig, AudioSampleRate};

/// Active DMA operation state
#[derive(Debug, Clone)]
struct ActiveDma {
    /// Memory address to read from (incremented during operation)
    current_addr: u32,
    /// Number of bytes remaining to read
    bytes_remaining: u32,
    /// Buffer collecting sample data
    sample_data: Vec<u8>,
    /// Configuration for this DMA operation
    config: AudioConfig,
}

/// Audio device providing audio/sound functionality
///
/// This device simulates an audio controller with DMA-based transfers.
/// The CPU configures a buffer address and audio settings, then triggers
/// a DMA operation to read multiple audio samples from memory in one batch.
///
/// Register Map (all word-aligned):
/// - 0x00: AUDIO_ADDR       - Buffer address in memory (read/write)
/// - 0x04: AUDIO_CONFIG     - Audio configuration (read/write)
///   Bits [1:0]   = sample_rate (0=48000Hz, 1=44100Hz, 2=22050Hz)
///   Bit 2        = channels (0=mono, 1=stereo)
///   Bits [18:3]  = sample_count - 1 (16 bits, allows 1-65536 samples with +1 bias)
/// - 0x08: AUDIO_STATUS     - Status register (read-only)
///   Bit 0: DMA_READY (1 = can trigger new DMA operation)
///   Bit 1: SAMPLE_BUFFER_READY (1 = sample buffer has space for more samples)
/// - 0x0C: AUDIO_DMA        - Trigger DMA (write-only, write any value to start)
///
/// The device operates as follows:
/// 1. CPU writes buffer address to AUDIO_ADDR
/// 2. CPU writes audio configuration to AUDIO_CONFIG (sets sample count to read)
/// 3. CPU polls AUDIO_STATUS until DMA_READY is set
/// 4. CPU writes to AUDIO_DMA to trigger DMA read operation
/// 5. Device reads memory (multiple bytes per cycle) and builds sample buffer
/// 6. Device invokes sample callback when complete DMA operation finishes
/// 7. Device invokes config callback when AUDIO_CONFIG changes
/// 8. AUDIO_STATUS.DMA_READY is set again when operation completes
///
/// Note: Changes to registers do not affect in-flight DMA operations.
/// All required state is captured when the DMA operation starts.
pub struct Audio<S = fn(&[i16]), C = fn(&AudioConfig)>
where
    S: FnMut(&[i16]),
    C: FnMut(&AudioConfig),
{
    /// Buffer address configuration register
    audio_addr: u32,
    /// Audio configuration register
    audio_config: u32,
    /// Active DMA operation (None = idle)
    active_dma: Option<ActiveDma>,
    /// Optional callback invoked when DMA operation completes with full sample buffer
    sample_callback: Option<S>,
    /// Optional callback invoked when configuration changes
    config_callback: Option<C>,
    /// Number of samples returned by callback within the current time window
    samples_in_current_window: u32,
    /// Elapsed time (microseconds) when the current time window started
    window_start_time_us: Option<u64>,
    /// Multiplier for buffer ahead threshold (default 2.0)
    /// Max samples per second = sample_rate * buffer_ahead_multiplier
    buffer_ahead_multiplier: f32,
}

impl<S, C> Audio<S, C>
where
    S: FnMut(&[i16]),
    C: FnMut(&AudioConfig),
{
    /// Create a new Audio device with optional callbacks and default buffer ahead multiplier (2.0)
    pub fn new(sample_callback: Option<S>, config_callback: Option<C>) -> Self {
        Self::with_buffer_ahead_multiplier(2.0, sample_callback, config_callback)
    }

    /// Create a new Audio device with custom buffer ahead multiplier
    ///
    /// The buffer ahead multiplier controls the back pressure mechanism. The SAMPLE_BUFFER_READY
    /// status bit is set when the number of samples returned within the current 1-second time
    /// window is below:
    /// max_samples_per_second = sample_rate * buffer_ahead_multiplier
    ///
    /// Each time window spans 1 second, and the sample count resets when a new window begins
    /// (based on elapsed time).
    ///
    /// # Arguments
    /// * `buffer_ahead_multiplier` - Multiplier for buffer ahead threshold (typically 1.0-4.0)
    /// * `sample_callback` - Optional callback invoked when DMA completes
    /// * `config_callback` - Optional callback invoked when config changes
    pub fn with_buffer_ahead_multiplier(
        buffer_ahead_multiplier: f32,
        sample_callback: Option<S>,
        config_callback: Option<C>,
    ) -> Self {
        Audio {
            audio_addr: 0,
            audio_config: 0,
            active_dma: None,
            sample_callback,
            config_callback,
            samples_in_current_window: 0,
            window_start_time_us: None,
            buffer_ahead_multiplier,
        }
    }

    /// Check if a DMA operation is currently in progress
    fn is_dma_active(&self) -> bool {
        self.active_dma.is_some()
    }

    /// Check if sample buffer has space for more samples
    ///
    /// Sample buffer pacing is based on elapsed host time (not simulation cycles).
    /// The buffer is ready when samples returned in the current 1-second window is below:
    ///   max_samples_per_second = sample_rate * buffer_ahead_multiplier
    ///
    /// The time window resets every second based on elapsed time.
    fn is_sample_buffer_ready(&self, current_time_us: u64) -> bool {
        let Some(config) = AudioConfig::from_register(self.audio_config) else {
            // No valid config, return true to allow initial setup
            return true;
        };

        // Check if we've moved into a new time window (1 second = 1,000,000 microseconds)
        let window_elapsed = match self.window_start_time_us {
            None => 0, // No window started yet, treat as new window
            Some(start_time) => current_time_us.saturating_sub(start_time),
        };

        // If we've passed 1 second, the window has reset (will be handled in callback)
        // For now, check against samples in current window
        if window_elapsed >= 1_000_000 {
            // New window has started, so buffer is ready
            return true;
        }

        // Calculate max samples allowed in current window
        let sample_rate_hz = config.sample_rate.to_hz();
        let max_samples_per_second = (sample_rate_hz as f32 * self.buffer_ahead_multiplier) as u32;

        self.samples_in_current_window < max_samples_per_second
    }

    /// Start a DMA read operation using the configured registers
    fn start_dma(&mut self) {
        if self.is_dma_active() {
            log::warn!("Audio: DMA attempted while operation already in progress - ignoring");
            return;
        }

        let Some(config) = AudioConfig::from_register(self.audio_config) else {
            log::warn!("Audio: Invalid configuration for DMA operation - ignoring");
            return;
        };

        // Calculate total bytes to read (sample_count * bytes_per_sample), guarding against overflow
        let Some(total_bytes) = config.sample_count.checked_mul(config.bytes_per_sample()) else {
            log::warn!(
                "Audio: DMA size overflow for sample_count={} bytes_per_sample={}",
                config.sample_count,
                config.bytes_per_sample()
            );
            return;
        };
        if total_bytes == 0 {
            log::warn!("Audio: DMA attempted with zero size - ignoring");
            return;
        }

        log::debug!(
            "Audio: Starting DMA from 0x{:08x}, {} samples, {}Hz, {:?}, {} bytes",
            self.audio_addr,
            config.sample_count,
            config.sample_rate.to_hz(),
            config.channels,
            total_bytes
        );

        // Pre-allocate buffer for DMA operation using fallible allocation
        let total_bytes_usize = total_bytes as usize;
        let mut sample_data = Vec::new();
        if let Err(err) = sample_data.try_reserve_exact(total_bytes_usize) {
            log::error!(
                "Audio: Failed to allocate DMA buffer of {} bytes: {:?} - aborting DMA start",
                total_bytes_usize,
                err
            );
            return;
        }

        // Start active DMA operation - capture all state now
        self.active_dma = Some(ActiveDma {
            current_addr: self.audio_addr,
            bytes_remaining: total_bytes,
            sample_data,
            config,
        });
    }

    /// Read the largest possible chunk from memory during DMA operation
    fn dma_chunk(&mut self, ctx: &mut SystemContext) {
        let dma = match self.active_dma.as_mut() {
            Some(d) => d,
            None => return,
        };

        // Read chunk using shared helper
        let (bytes, read_size) =
            crate::bus_device::read_memory_chunk(ctx, dma.current_addr, dma.bytes_remaining);

        // Append bytes to sample data
        dma.sample_data
            .extend_from_slice(&bytes[..read_size as usize]);
        dma.current_addr = dma.current_addr.wrapping_add(read_size);
        dma.bytes_remaining -= read_size;

        // Check if DMA is complete
        if dma.bytes_remaining == 0 {
            let dma_data = self.active_dma.take().unwrap();
            let current_time_us = ctx.elapsed_time_us();
            self.invoke_sample_callback(dma_data, current_time_us);
        }
    }

    /// Invoke the sample callback with the collected audio data
    fn invoke_sample_callback(&mut self, dma: ActiveDma, current_time_us: u64) {
        log::info!(
            "Audio: DMA complete ({} samples, {}Hz, {:?}, {} bytes)",
            dma.config.sample_count,
            dma.config.sample_rate.to_hz(),
            dma.config.channels,
            dma.sample_data.len()
        );

        // Convert byte buffer to i16 samples
        let channel_count = dma.config.channels.count();
        let sample_count = dma.config.sample_count as usize;
        let expected_bytes = sample_count * channel_count * 2;

        // Warn if buffer is incomplete
        if dma.sample_data.len() < expected_bytes {
            log::warn!(
                "Audio: DMA buffer incomplete - expected {} bytes, got {} bytes. Some samples will be skipped.",
                expected_bytes,
                dma.sample_data.len()
            );
        }

        let mut samples = Vec::with_capacity(sample_count * channel_count);

        // Parse all samples from the byte buffer
        for sample_idx in 0..sample_count {
            for channel_idx in 0..channel_count {
                let byte_offset = (sample_idx * channel_count + channel_idx) * 2;
                if byte_offset + 1 < dma.sample_data.len() {
                    let sample = i16::from_le_bytes([
                        dma.sample_data[byte_offset],
                        dma.sample_data[byte_offset + 1],
                    ]);
                    samples.push(sample);
                } else {
                    // Log when samples are skipped due to incomplete buffer
                    log::debug!(
                        "Audio: Skipping sample at index {} (channel {}) due to incomplete buffer",
                        sample_idx,
                        channel_idx
                    );
                }
            }
        }

        // Check if we need to reset the time window (every 1 second)
        let window_elapsed = match self.window_start_time_us {
            None => u64::MAX, // Force window reset on first callback
            Some(start_time) => current_time_us.saturating_sub(start_time),
        };

        if window_elapsed >= 1_000_000 {
            // New second has begun, reset the window
            self.window_start_time_us = Some(current_time_us);
            self.samples_in_current_window = sample_count as u32;
            log::debug!(
                "Audio: New time window started at {}us, samples in window reset to {}",
                current_time_us,
                sample_count
            );
        } else {
            // Same window, increment sample count
            self.samples_in_current_window += sample_count as u32;
            log::debug!(
                "Audio: Samples in current window: {} (window started at {}us)",
                self.samples_in_current_window,
                self.window_start_time_us.unwrap_or(0)
            );
        }

        // Invoke callback with full buffer
        if let Some(ref mut callback) = self.sample_callback {
            callback(&samples);
        }
    }

    /// Handle configuration register write
    fn handle_config_write(&mut self, new_config: u32) {
        // Configuration changes do not affect in-flight DMA operations
        self.audio_config = new_config;

        // Reset sample window when config changes (indicates new audio stream)
        self.samples_in_current_window = 0;
        self.window_start_time_us = None;

        // Notify on config write (treat all writes as changes)
        if let Some(config) = AudioConfig::from_register(new_config) {
            log::info!(
                "Audio: Config changed - {}Hz, {:?}, {} samples (window reset)",
                config.sample_rate.to_hz(),
                config.channels,
                config.sample_count
            );

            // Invoke config callback
            if let Some(ref mut callback) = self.config_callback {
                callback(&config);
            }
        }
    }
}

impl<S, C> BusDevice for Audio<S, C>
where
    S: FnMut(&[i16]),
    C: FnMut(&AudioConfig),
{
    fn read_word(&mut self, ctx: &mut SystemContext, offset: u32) -> Result<u32, BusDeviceError> {
        match offset {
            0x00 => Ok(self.audio_addr),
            0x04 => Ok(self.audio_config),
            0x08 => {
                // AUDIO_STATUS register
                let mut status = 0u32;

                // Bit 0: DMA_READY (inverse of DMA active)
                if !self.is_dma_active() {
                    status |= 1 << 0;
                }

                // Bit 1: SAMPLE_BUFFER_READY
                if self.is_sample_buffer_ready(ctx.elapsed_time_us()) {
                    status |= 1 << 1;
                }

                Ok(status)
            }
            0x0C => {
                // AUDIO_DMA register is write-only
                Err(BusDeviceError::ReadFromWriteOnly { offset })
            }
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }

    fn write_word(
        &mut self,
        _ctx: &mut SystemContext,
        offset: u32,
        value: u32,
    ) -> Result<(), BusDeviceError> {
        match offset {
            0x00 => {
                // AUDIO_ADDR - address changes don't affect in-flight DMA
                self.audio_addr = value;
                Ok(())
            }
            0x04 => {
                self.handle_config_write(value);
                Ok(())
            }
            0x08 => {
                // AUDIO_STATUS register is read-only
                Err(BusDeviceError::WriteToReadOnly { offset })
            }
            0x0C => {
                // AUDIO_DMA register - writing any value starts the DMA
                self.start_dma();
                Ok(())
            }
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }

    fn size(&self) -> u32 {
        // 4 registers × 4 bytes each = 16 bytes
        16
    }

    fn name(&self) -> &str {
        "Audio"
    }

    fn reset(&mut self, _ctx: &mut SystemContext) {
        self.audio_addr = 0;
        self.audio_config = 0;
        self.active_dma = None;
        self.samples_in_current_window = 0;
        self.window_start_time_us = None;
    }

    fn clock_cycle(&mut self, ctx: &mut SystemContext) {
        // Read the largest possible chunk per clock cycle if a DMA operation is in progress
        self.dma_chunk(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::Memory;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn test_audio_sample_rate() {
        assert_eq!(AudioSampleRate::Hz48000.to_hz(), 48000);
        assert_eq!(AudioSampleRate::Hz44100.to_hz(), 44100);
        assert_eq!(AudioSampleRate::Hz22050.to_hz(), 22050);
    }

    #[test]
    fn test_audio_channels_count() {
        assert_eq!(AudioChannels::Mono.count(), 1);
        assert_eq!(AudioChannels::Stereo.count(), 2);
    }

    #[test]
    fn test_audio_config_parsing() {
        // Test: 48000Hz, Mono, 256 samples
        // sample_rate=0, channels=0, sample_count=256 (stored as 255)
        let config_value = 255 << 3;
        let config = AudioConfig::from_register(config_value).unwrap();
        assert_eq!(config.sample_rate, AudioSampleRate::Hz48000);
        assert_eq!(config.channels, AudioChannels::Mono);
        assert_eq!(config.sample_count, 256);
        assert_eq!(config.buffer_bytes(), 256 * 2); // 256 samples × 2 bytes

        // Test round-trip
        assert_eq!(config.to_register(), config_value);
    }

    #[test]
    fn test_audio_config_stereo() {
        // Test: 44100Hz, Stereo, 512 samples
        let config_value = 1 | (1 << 2) | (511 << 3);
        let config = AudioConfig::from_register(config_value).unwrap();
        assert_eq!(config.sample_rate, AudioSampleRate::Hz44100);
        assert_eq!(config.channels, AudioChannels::Stereo);
        assert_eq!(config.sample_count, 512);
        assert_eq!(config.buffer_bytes(), 512 * 2 * 2); // 512 samples × 2 channels × 2 bytes
    }

    #[test]
    fn test_audio_register_access() {
        let mut audio = Audio::new(None::<fn(&[i16])>, None::<fn(&AudioConfig)>);
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        // Write to registers (9 samples stored as 8)
        audio.write_word(&mut ctx, 0x00, 0x8000_1000).unwrap();
        audio.write_word(&mut ctx, 0x04, 8 << 3).unwrap();

        // Read back registers
        assert_eq!(audio.read_word(&mut ctx, 0x00).unwrap(), 0x8000_1000);
        assert_eq!(audio.read_word(&mut ctx, 0x04).unwrap(), (8 << 3));

        // Status should show DMA_READY and SAMPLE_BUFFER_READY (bits 0 and 1)
        assert_eq!(audio.read_word(&mut ctx, 0x08).unwrap(), 0b11);
    }

    #[test]
    fn test_audio_status_read_only() {
        let mut audio = Audio::new(None::<fn(&[i16])>, None::<fn(&AudioConfig)>);
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        let result = audio.write_word(&mut ctx, 0x08, 0x100);
        assert!(matches!(
            result,
            Err(BusDeviceError::WriteToReadOnly { offset: 0x08 })
        ));
    }

    #[test]
    fn test_audio_dma_write_only() {
        let mut audio = Audio::new(None::<fn(&[i16])>, None::<fn(&AudioConfig)>);
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        let result = audio.read_word(&mut ctx, 0x0C);
        assert!(matches!(
            result,
            Err(BusDeviceError::ReadFromWriteOnly { offset: 0x0C })
        ));
    }

    #[test]
    fn test_audio_mono_dma_read() {
        // Use Rc<RefCell<>> to capture callback data
        let sample_data: Rc<RefCell<Vec<Vec<i16>>>> = Rc::new(RefCell::new(Vec::new()));
        let sample_data_clone = sample_data.clone();

        let mut audio = Audio::new(
            Some(move |samples: &[i16]| {
                sample_data_clone.borrow_mut().push(samples.to_vec());
            }),
            None::<fn(&AudioConfig)>,
        );
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        // Set up test audio data in memory (4 mono samples)
        let audio_addr = 0x8000_1000;
        let test_samples = [0x1234i16, 0x5678i16, -100i16, 200i16];

        for (i, &sample) in test_samples.iter().enumerate() {
            let bytes = sample.to_le_bytes();
            ctx.write_byte(audio_addr + (i * 2) as u32, bytes[0]);
            ctx.write_byte(audio_addr + (i * 2 + 1) as u32, bytes[1]);
        }

        // Configure audio device for mono, 4 samples
        audio.write_word(&mut ctx, 0x00, audio_addr).unwrap();
        let config = 3 << 3; // 48000Hz, Mono, 4 samples (stored as 3)
        audio.write_word(&mut ctx, 0x04, config).unwrap();

        // Check status: DMA should be ready and buffer ready
        assert_eq!(audio.read_word(&mut ctx, 0x08).unwrap(), 0b11);

        // Trigger DMA
        audio.write_word(&mut ctx, 0x0C, 0).unwrap();

        // Check status: DMA should not be ready (operation in progress), but buffer is ready
        assert_eq!(audio.read_word(&mut ctx, 0x08).unwrap(), 0b10);

        // Run clock cycles to complete DMA (4 samples × 2 bytes = 8 bytes)
        for _ in 0..8 {
            audio.clock_cycle(&mut ctx);
        }

        // Check status: DMA should be ready again (and buffer ready)
        assert_eq!(audio.read_word(&mut ctx, 0x08).unwrap(), 0b11);

        // Verify callback was invoked once with all samples
        let captured = sample_data.borrow();
        assert_eq!(captured.len(), 1, "Should have received 1 callback");
        assert_eq!(captured[0].len(), 4, "Should have 4 samples");
        assert_eq!(captured[0][0], 0x1234);
        assert_eq!(captured[0][1], 0x5678);
        assert_eq!(captured[0][2], -100);
        assert_eq!(captured[0][3], 200);
    }

    #[test]
    fn test_audio_stereo_dma_read() {
        let sample_data: Rc<RefCell<Vec<Vec<i16>>>> = Rc::new(RefCell::new(Vec::new()));
        let sample_data_clone = sample_data.clone();

        let mut audio = Audio::new(
            Some(move |samples: &[i16]| {
                sample_data_clone.borrow_mut().push(samples.to_vec());
            }),
            None::<fn(&AudioConfig)>,
        );
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        // Set up test audio data in memory (2 stereo samples: left, right)
        let audio_addr = 0x8000_1000;
        let test_samples = [
            (100i16, 200i16),  // Sample 0: left=100, right=200
            (-500i16, 300i16), // Sample 1: left=-500, right=300
        ];

        for (i, &(left, right)) in test_samples.iter().enumerate() {
            let left_bytes = left.to_le_bytes();
            let right_bytes = right.to_le_bytes();
            let base = (i * 4) as u32;
            ctx.write_byte(audio_addr + base, left_bytes[0]);
            ctx.write_byte(audio_addr + base + 1, left_bytes[1]);
            ctx.write_byte(audio_addr + base + 2, right_bytes[0]);
            ctx.write_byte(audio_addr + base + 3, right_bytes[1]);
        }

        // Configure audio device for stereo, 2 samples
        audio.write_word(&mut ctx, 0x00, audio_addr).unwrap();
        let config = (1 << 2) | (1 << 3); // 48000Hz, Stereo, 2 samples (stored as 1)
        audio.write_word(&mut ctx, 0x04, config).unwrap();

        // Trigger DMA
        audio.write_word(&mut ctx, 0x0C, 0).unwrap();

        // Run clock cycles to complete DMA (2 samples × 2 channels × 2 bytes = 8 bytes)
        for _ in 0..8 {
            audio.clock_cycle(&mut ctx);
        }

        // Verify callback was invoked once with all samples
        let captured = sample_data.borrow();
        assert_eq!(captured.len(), 1, "Should have received 1 callback");
        assert_eq!(
            captured[0].len(),
            4,
            "Should have 4 channel samples (2 stereo samples)"
        );
        assert_eq!(captured[0][0], 100); // Sample 0 left
        assert_eq!(captured[0][1], 200); // Sample 0 right
        assert_eq!(captured[0][2], -500); // Sample 1 left
        assert_eq!(captured[0][3], 300); // Sample 1 right
    }

    #[test]
    fn test_audio_config_callback() {
        let config_data: Rc<RefCell<Vec<AudioConfig>>> = Rc::new(RefCell::new(Vec::new()));
        let config_data_clone = config_data.clone();

        let mut audio = Audio::new(
            None::<fn(&[i16])>,
            Some(move |config: &AudioConfig| {
                config_data_clone.borrow_mut().push(*config);
            }),
        );
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        // Write first config
        let config1 = 255 << 3; // 48000Hz, Mono, 256 samples (stored as 255)
        audio.write_word(&mut ctx, 0x04, config1).unwrap();

        // Write different config
        let config2 = 1 | (1 << 2) | (511 << 3); // 44100Hz, Stereo, 512 samples (stored as 511)
        audio.write_word(&mut ctx, 0x04, config2).unwrap();

        // Verify callback was invoked twice
        let captured = config_data.borrow();
        assert_eq!(captured.len(), 2, "Should have received 2 config changes");
        assert_eq!(captured[0].sample_rate, AudioSampleRate::Hz48000);
        assert_eq!(captured[0].channels, AudioChannels::Mono);
        assert_eq!(captured[0].sample_count, 256);
        assert_eq!(captured[1].sample_rate, AudioSampleRate::Hz44100);
        assert_eq!(captured[1].channels, AudioChannels::Stereo);
        assert_eq!(captured[1].sample_count, 512);
    }

    #[test]
    fn test_audio_multiple_dma_rejected() {
        let mut audio = Audio::new(None::<fn(&[i16])>, None::<fn(&AudioConfig)>);
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        // Configure for buffer that will take multiple cycles to complete
        // Use stereo, 8 samples = 8 × 2 channels × 2 bytes = 32 bytes
        audio.write_word(&mut ctx, 0x00, 0x8000_1000).unwrap();
        audio
            .write_word(&mut ctx, 0x04, (1 << 2) | (7 << 3))
            .unwrap(); // Stereo, 8 samples (stored as 7)

        // Trigger first DMA
        audio.write_word(&mut ctx, 0x0C, 0).unwrap();
        assert!(audio.is_dma_active());

        // Attempt to trigger second DMA (should be rejected)
        audio.write_word(&mut ctx, 0x0C, 0).unwrap();

        // Run a few cycles (not enough to complete the full 32-byte transfer)
        for _ in 0..4 {
            audio.clock_cycle(&mut ctx);
        }

        // Should still have active DMA
        assert!(audio.is_dma_active());
    }

    #[test]
    fn test_audio_reset() {
        let mut audio = Audio::new(None::<fn(&[i16])>, None::<fn(&AudioConfig)>);
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        // Configure and start DMA
        audio.write_word(&mut ctx, 0x00, 0x8000_1000).unwrap();
        audio.write_word(&mut ctx, 0x04, 8 << 3).unwrap();
        audio.write_word(&mut ctx, 0x0C, 0).unwrap();

        // Reset
        audio.reset(&mut ctx);

        // All registers should be zero
        assert_eq!(audio.read_word(&mut ctx, 0x00).unwrap(), 0);
        assert_eq!(audio.read_word(&mut ctx, 0x04).unwrap(), 0);

        // Status should show DMA_READY and SAMPLE_BUFFER_READY
        assert_eq!(audio.read_word(&mut ctx, 0x08).unwrap(), 0b11);

        // Active DMA should be cleared
        assert!(!audio.is_dma_active());
    }

    #[test]
    fn test_audio_buffer_ready_status() {
        let sample_data: Rc<RefCell<Vec<Vec<i16>>>> = Rc::new(RefCell::new(Vec::new()));
        let sample_data_clone = sample_data.clone();

        let mut audio = Audio::with_buffer_ahead_multiplier(
            2.0,
            Some(move |samples: &[i16]| {
                sample_data_clone.borrow_mut().push(samples.to_vec());
            }),
            None::<fn(&AudioConfig)>,
        );
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        // Configure for 48000Hz, mono, 10 samples per batch
        // With multiplier 2.0: max = 48000 * 2.0 = 96000 samples
        audio.write_word(&mut ctx, 0x00, 0x8000_1000).unwrap();
        audio.write_word(&mut ctx, 0x04, 9 << 3).unwrap(); // 10 samples

        // Initially buffer should be ready (bit 1 set)
        let status = audio.read_word(&mut ctx, 0x08).unwrap();
        assert_eq!(
            status & 0b10,
            0b10,
            "SAMPLE_BUFFER_READY should be set initially"
        );

        // Fill memory with test data
        for i in 0..10 {
            let sample = (i * 100) as i16;
            let bytes = sample.to_le_bytes();
            ctx.write_byte(0x8000_1000 + i * 2, bytes[0]);
            ctx.write_byte(0x8000_1000 + i * 2 + 1, bytes[1]);
        }

        // Trigger DMA and complete it
        audio.write_word(&mut ctx, 0x0C, 0).unwrap();
        for _ in 0..20 {
            audio.clock_cycle(&mut ctx);
        }

        // Buffer should still be ready (only 10 samples out of 96000)
        let status = audio.read_word(&mut ctx, 0x08).unwrap();
        assert_eq!(
            status & 0b10,
            0b10,
            "SAMPLE_BUFFER_READY should still be set"
        );
    }

    #[test]
    fn test_audio_buffer_ready_threshold() {
        let sample_data: Rc<RefCell<Vec<Vec<i16>>>> = Rc::new(RefCell::new(Vec::new()));
        let sample_data_clone = sample_data.clone();

        // Use small multiplier to test threshold
        let mut audio = Audio::with_buffer_ahead_multiplier(
            0.0001, // Very small multiplier: 48000 * 0.0001 = 4.8 samples (rounds to 4)
            Some(move |samples: &[i16]| {
                sample_data_clone.borrow_mut().push(samples.to_vec());
            }),
            None::<fn(&AudioConfig)>,
        );
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        // Configure for 48000Hz, mono, 10 samples per batch
        audio.write_word(&mut ctx, 0x00, 0x8000_1000).unwrap();
        audio.write_word(&mut ctx, 0x04, 9 << 3).unwrap(); // 10 samples

        // Fill memory with test data
        for i in 0..10 {
            let sample = (i * 100) as i16;
            let bytes = sample.to_le_bytes();
            ctx.write_byte(0x8000_1000 + i * 2, bytes[0]);
            ctx.write_byte(0x8000_1000 + i * 2 + 1, bytes[1]);
        }

        // Trigger DMA and complete it
        audio.write_word(&mut ctx, 0x0C, 0).unwrap();
        for _ in 0..20 {
            audio.clock_cycle(&mut ctx);
        }

        // After 10 samples, should exceed threshold (4.8), so buffer not ready
        let status = audio.read_word(&mut ctx, 0x08).unwrap();
        assert_eq!(
            status & 0b10,
            0,
            "SAMPLE_BUFFER_READY should be clear after exceeding threshold"
        );
    }

    #[test]
    fn test_audio_buffer_ready_reset_on_config_change() {
        let sample_data: Rc<RefCell<Vec<Vec<i16>>>> = Rc::new(RefCell::new(Vec::new()));
        let sample_data_clone = sample_data.clone();

        let mut audio = Audio::with_buffer_ahead_multiplier(
            0.0001, // Small threshold to trigger quickly
            Some(move |samples: &[i16]| {
                sample_data_clone.borrow_mut().push(samples.to_vec());
            }),
            None::<fn(&AudioConfig)>,
        );
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        // Configure and generate samples to exceed threshold
        audio.write_word(&mut ctx, 0x00, 0x8000_1000).unwrap();
        audio.write_word(&mut ctx, 0x04, 9 << 3).unwrap(); // 10 samples

        // Fill memory and trigger DMA
        for i in 0..10 {
            let sample = (i * 100) as i16;
            let bytes = sample.to_le_bytes();
            ctx.write_byte(0x8000_1000 + i * 2, bytes[0]);
            ctx.write_byte(0x8000_1000 + i * 2 + 1, bytes[1]);
        }
        audio.write_word(&mut ctx, 0x0C, 0).unwrap();
        for _ in 0..20 {
            audio.clock_cycle(&mut ctx);
        }

        // Verify buffer not ready
        let status = audio.read_word(&mut ctx, 0x08).unwrap();
        assert_eq!(status & 0b10, 0, "SAMPLE_BUFFER_READY should be clear");

        // Change config (should reset sample count)
        audio
            .write_word(&mut ctx, 0x04, (1 << 2) | (9 << 3))
            .unwrap(); // Change to stereo

        // Buffer should be ready again
        let status = audio.read_word(&mut ctx, 0x08).unwrap();
        assert_eq!(
            status & 0b10,
            0b10,
            "SAMPLE_BUFFER_READY should be set after config change"
        );
    }
}
