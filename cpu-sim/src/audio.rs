use crate::bus_device::{BusDevice, BusDeviceError, SystemContext};

// Re-export types from riscv_shared for backward compatibility
pub use riscv_shared::audio::{AudioChannels, AudioConfig, AudioSampleRate};

/// Active read operation state
#[derive(Debug, Clone)]
struct ActiveRead {
    /// Current configuration for this read operation
    config: AudioConfig,
    /// Buffer collecting sample data bytes (max 4 bytes: 2 channels × 2 bytes/sample)
    sample_buffer: [u8; 4],
    /// Number of bytes currently in the buffer
    bytes_in_buffer: usize,
    /// Current read address
    current_addr: u32,
}

/// Audio device providing audio/sound functionality
///
/// This device simulates an audio controller with a free-running circular buffer
/// that the CPU writes audio samples to. The device reads samples from memory
/// when they become available based on read/write pointer positions.
///
/// Register Map (all word-aligned):
/// - 0x00: AUDIO_ADDR       - Ring buffer address in memory (read/write)
/// - 0x04: AUDIO_CONFIG     - Audio configuration (read/write)
///   Bits [1:0]   = sample_rate (0=48000Hz, 1=44100Hz, 2=22050Hz)
///   Bit 2        = channels (0=mono, 1=stereo)
///   Bits [7:3]   = log2(sample_count) (5 bits)
/// - 0x08: AUDIO_READ_PTR   - Current read pointer offset (read-only)
/// - 0x0C: AUDIO_WRITE_PTR  - Current write pointer offset (read/write)
///
/// The device operates as follows:
/// 1. CPU writes ring buffer address to AUDIO_ADDR
/// 2. CPU writes audio configuration to AUDIO_CONFIG
/// 3. CPU writes audio samples to memory at AUDIO_ADDR + offset
/// 4. CPU updates AUDIO_WRITE_PTR when new samples are available
/// 5. Device reads memory one byte per cycle when read_ptr != write_ptr
/// 6. Device invokes sample callback when a complete sample is read
/// 7. Device invokes config callback when AUDIO_CONFIG changes
pub struct Audio<S = fn(&[i16]), C = fn(&AudioConfig)>
where
    S: FnMut(&[i16]),
    C: FnMut(&AudioConfig),
{
    /// Ring buffer address configuration register
    audio_addr: u32,
    /// Audio configuration register
    audio_config: u32,
    /// Current read pointer offset (relative to audio_addr)
    read_ptr: u32,
    /// Current write pointer offset (relative to audio_addr)
    write_ptr: u32,
    /// Active read operation (None = idle)
    active_read: Option<ActiveRead>,
    /// Optional callback invoked when a complete sample is available
    sample_callback: Option<S>,
    /// Optional callback invoked when configuration changes
    config_callback: Option<C>,
}

impl<S, C> Audio<S, C>
where
    S: FnMut(&[i16]),
    C: FnMut(&AudioConfig),
{
    /// Create a new Audio device with optional callbacks
    pub fn new(sample_callback: Option<S>, config_callback: Option<C>) -> Self {
        Audio {
            audio_addr: 0,
            audio_config: 0,
            read_ptr: 0,
            write_ptr: 0,
            active_read: None,
            sample_callback,
            config_callback,
        }
    }

    /// Check if a read operation is currently in progress
    fn is_read_active(&self) -> bool {
        self.active_read.is_some()
    }

    /// Start reading a sample from memory if data is available
    fn start_read_if_available(&mut self) {
        // Don't start if already reading
        if self.is_read_active() {
            return;
        }

        // Parse current configuration
        let Some(config) = AudioConfig::from_register(self.audio_config) else {
            return;
        };

        // Check if data is available (read_ptr != write_ptr)
        if self.read_ptr == self.write_ptr {
            return;
        }

        // Validate pointers are within buffer bounds
        // Note: write_ptr can be == buffer_bytes (full buffer wraps to 0)
        let buffer_bytes = config.buffer_bytes();
        if self.read_ptr >= buffer_bytes || self.write_ptr > buffer_bytes {
            log::warn!(
                "Audio: Invalid pointers (read=0x{:x}, write=0x{:x}, buffer_bytes=0x{:x})",
                self.read_ptr,
                self.write_ptr,
                buffer_bytes
            );
            return;
        }

        log::debug!(
            "Audio: Starting sample read at offset 0x{:x} (addr=0x{:08x})",
            self.read_ptr,
            self.audio_addr.wrapping_add(self.read_ptr)
        );

        // Start active read operation
        self.active_read = Some(ActiveRead {
            config,
            sample_buffer: [0u8; 4],
            bytes_in_buffer: 0,
            current_addr: self.audio_addr.wrapping_add(self.read_ptr),
        });
    }

    /// Read the largest possible chunk from memory during active read operation
    fn read_chunk(&mut self, ctx: &mut SystemContext) {
        let read = match self.active_read.as_mut() {
            Some(r) => r,
            None => return,
        };

        let bytes_per_sample = read.config.bytes_per_sample() as usize;
        let bytes_remaining = (bytes_per_sample - read.bytes_in_buffer) as u32;

        // Read chunk using shared helper
        let (bytes, read_size) =
            crate::bus_device::read_memory_chunk(ctx, read.current_addr, bytes_remaining);
        let read_size = read_size as usize;

        // Copy bytes to sample buffer
        read.sample_buffer[read.bytes_in_buffer..read.bytes_in_buffer + read_size]
            .copy_from_slice(&bytes[..read_size]);
        read.bytes_in_buffer += read_size;
        read.current_addr = read.current_addr.wrapping_add(read_size as u32);

        // Check if sample is complete
        if read.bytes_in_buffer >= bytes_per_sample {
            let read_data = self.active_read.take().unwrap();
            self.process_complete_sample(read_data);
        }
    }

    /// Process a complete sample and invoke callback
    fn process_complete_sample(&mut self, read: ActiveRead) {
        // Convert byte buffer to i16 samples using fixed-size array
        let channel_count = read.config.channels.count();
        let mut samples = [0i16; 2]; // Max 2 channels
        let mut sample_count = 0;

        for i in 0..channel_count {
            let offset = i * 2;
            if offset + 1 < read.bytes_in_buffer {
                samples[sample_count] = i16::from_le_bytes([
                    read.sample_buffer[offset],
                    read.sample_buffer[offset + 1],
                ]);
                sample_count += 1;
            }
        }

        // Update read pointer (wrapping within ring buffer)
        let bytes_read = read.config.bytes_per_sample();
        let new_read_ptr = self.read_ptr.wrapping_add(bytes_read);
        let buffer_bytes = read.config.buffer_bytes();
        self.read_ptr = new_read_ptr % buffer_bytes;

        log::debug!(
            "Audio: Sample complete ({} channels, read_ptr now 0x{:x})",
            sample_count,
            self.read_ptr
        );

        // Invoke sample callback with slice of actual samples
        if let Some(ref mut callback) = self.sample_callback {
            callback(&samples[..sample_count]);
        }
    }

    /// Handle configuration register write
    fn handle_config_write(&mut self, new_config: u32) {
        // If config changes during active read, abort the read
        if self.is_read_active() {
            log::warn!("Audio: Configuration changed during active read, aborting read");
            self.active_read = None;
        }

        self.audio_config = new_config;

        // Always notify on config write (treat all writes as changes)
        if let Some(config) = AudioConfig::from_register(new_config) {
            log::info!(
                "Audio: Config changed - {}Hz, {:?}, {} samples",
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

    /// Handle ring buffer address write
    fn handle_addr_write(&mut self, new_addr: u32) {
        // If address changes during active read, abort the read
        if self.audio_addr != new_addr && self.is_read_active() {
            log::warn!("Audio: Ring buffer address changed during active read, aborting read");
            self.active_read = None;
        }

        self.audio_addr = new_addr;
    }
}

impl<S, C> BusDevice for Audio<S, C>
where
    S: FnMut(&[i16]),
    C: FnMut(&AudioConfig),
{
    fn read_word(&mut self, _ctx: &mut SystemContext, offset: u32) -> Result<u32, BusDeviceError> {
        match offset {
            0x00 => Ok(self.audio_addr),
            0x04 => Ok(self.audio_config),
            0x08 => Ok(self.read_ptr),
            0x0C => Ok(self.write_ptr),
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
                self.handle_addr_write(value);
                Ok(())
            }
            0x04 => {
                self.handle_config_write(value);
                Ok(())
            }
            0x08 => {
                // AUDIO_READ_PTR register is read-only
                Err(BusDeviceError::WriteToReadOnly { offset })
            }
            0x0C => {
                self.write_ptr = value;
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
        self.read_ptr = 0;
        self.write_ptr = 0;
        self.active_read = None;
    }

    fn clock_cycle(&mut self, ctx: &mut SystemContext) {
        // Read the largest possible chunk per clock cycle if an active read is in progress
        if self.is_read_active() {
            self.read_chunk(ctx);
        } else {
            // Try to start a new read if data is available
            self.start_read_if_available();

            // If we just started a read, process one chunk this cycle
            if self.is_read_active() {
                self.read_chunk(ctx);
            }
        }
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
        // Test: 48000Hz, Mono, 256 samples (log2=8)
        // sample_rate=0, channels=0, log2_count=8
        let config_value = 0 | (0 << 2) | (8 << 3);
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
        // Test: 44100Hz, Stereo, 512 samples (log2=9)
        let config_value = 1 | (1 << 2) | (9 << 3);
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

        // Write to registers
        audio.write_word(&mut ctx, 0x00, 0x8000_1000).unwrap();
        audio
            .write_word(&mut ctx, 0x04, 0 | (0 << 2) | (8 << 3))
            .unwrap();
        audio.write_word(&mut ctx, 0x0C, 0x100).unwrap();

        // Read back registers
        assert_eq!(audio.read_word(&mut ctx, 0x00).unwrap(), 0x8000_1000);
        assert_eq!(
            audio.read_word(&mut ctx, 0x04).unwrap(),
            0 | (0 << 2) | (8 << 3)
        );
        assert_eq!(audio.read_word(&mut ctx, 0x08).unwrap(), 0); // read_ptr starts at 0
        assert_eq!(audio.read_word(&mut ctx, 0x0C).unwrap(), 0x100);
    }

    #[test]
    fn test_audio_read_ptr_read_only() {
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
    fn test_audio_mono_sample_read() {
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

        // Set up test audio data in memory (mono samples)
        let audio_addr = 0x8000_1000;
        let test_samples = [0x1234i16, 0x5678i16, -100i16, 200i16];

        for (i, &sample) in test_samples.iter().enumerate() {
            let bytes = sample.to_le_bytes();
            ctx.write_byte(audio_addr + (i * 2) as u32, bytes[0]);
            ctx.write_byte(audio_addr + (i * 2 + 1) as u32, bytes[1]);
        }

        // Configure audio device for mono, 4 samples
        audio.write_word(&mut ctx, 0x00, audio_addr).unwrap();
        let config = 0 | (0 << 2) | (2 << 3); // 48000Hz, Mono, 4 samples (log2=2)
        audio.write_word(&mut ctx, 0x04, config).unwrap();

        // Set write pointer to indicate 4 samples available (4 samples × 2 bytes = 8 bytes)
        audio.write_word(&mut ctx, 0x0C, 8).unwrap();

        // Run clock cycles to read all samples
        // With optimization, each 2-byte mono sample can be read in 1 cycle (halfword)
        // 4 samples = 4 cycles
        for _ in 0..test_samples.len() {
            audio.clock_cycle(&mut ctx);
        }

        // Verify callback was invoked with correct samples
        let captured = sample_data.borrow();
        assert_eq!(captured.len(), 4, "Should have received 4 samples");
        assert_eq!(captured[0], vec![0x1234]);
        assert_eq!(captured[1], vec![0x5678]);
        assert_eq!(captured[2], vec![-100]);
        assert_eq!(captured[3], vec![200]);

        // Verify read pointer wrapped to 0 (read all 8 bytes from 8-byte buffer)
        assert_eq!(audio.read_word(&mut ctx, 0x08).unwrap(), 0);
    }

    #[test]
    fn test_audio_stereo_sample_read() {
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

        // Set up test audio data in memory (stereo samples: left, right)
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
        let config = 0 | (1 << 2) | (1 << 3); // 48000Hz, Stereo, 2 samples (log2=1)
        audio.write_word(&mut ctx, 0x04, config).unwrap();

        // Set write pointer to indicate 2 samples available (2 samples × 2 channels × 2 bytes = 8 bytes)
        audio.write_word(&mut ctx, 0x0C, 8).unwrap();

        // Run clock cycles to read all samples
        // With optimization, each 4-byte stereo sample can be read in 1 cycle (word)
        // 2 samples = 2 cycles
        for _ in 0..test_samples.len() {
            audio.clock_cycle(&mut ctx);
        }

        // Verify callback was invoked with correct samples
        let captured = sample_data.borrow();
        assert_eq!(captured.len(), 2, "Should have received 2 stereo samples");
        assert_eq!(captured[0], vec![100, 200]);
        assert_eq!(captured[1], vec![-500, 300]);

        // Verify read pointer wrapped to 0 (read all 8 bytes from 8-byte buffer)
        assert_eq!(audio.read_word(&mut ctx, 0x08).unwrap(), 0);
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
        let config1 = 0 | (0 << 2) | (8 << 3); // 48000Hz, Mono, 256 samples
        audio.write_word(&mut ctx, 0x04, config1).unwrap();

        // Write different config
        let config2 = 1 | (1 << 2) | (9 << 3); // 44100Hz, Stereo, 512 samples
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
    fn test_audio_ring_buffer_wrap() {
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

        // Set up small ring buffer (4 samples = 8 bytes for mono)
        let audio_addr = 0x8000_1000;
        let ring_buffer_size = 8u32; // 4 samples × 2 bytes

        // Fill entire ring buffer with test samples
        for i in 0..4 {
            let sample = (i * 100) as i16;
            let bytes = sample.to_le_bytes();
            ctx.write_byte(audio_addr + (i * 2), bytes[0]);
            ctx.write_byte(audio_addr + (i * 2 + 1), bytes[1]);
        }

        // Configure audio device
        audio.write_word(&mut ctx, 0x00, audio_addr).unwrap();
        let config = 0 | (0 << 2) | (2 << 3); // 48000Hz, Mono, 4 samples (log2=2)
        audio.write_word(&mut ctx, 0x04, config).unwrap();

        // Read all 4 samples (should wrap read pointer to 0)
        audio.write_word(&mut ctx, 0x0C, ring_buffer_size).unwrap();
        // With optimization, we need fewer cycles: 4 samples at 1 cycle each (halfword-aligned)
        for _ in 0..4 {
            audio.clock_cycle(&mut ctx);
        }

        // Read pointer should have wrapped to 0
        assert_eq!(audio.read_word(&mut ctx, 0x08).unwrap(), 0);

        // Verify all samples were read
        let captured = sample_data.borrow();
        assert_eq!(captured.len(), 4);
    }

    #[test]
    fn test_audio_reset() {
        let mut audio = Audio::new(None::<fn(&[i16])>, None::<fn(&AudioConfig)>);
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        // Configure and set pointers
        audio.write_word(&mut ctx, 0x00, 0x8000_1000).unwrap();
        audio
            .write_word(&mut ctx, 0x04, 0 | (0 << 2) | (8 << 3))
            .unwrap();
        audio.write_word(&mut ctx, 0x0C, 0x100).unwrap();

        // Reset
        audio.reset(&mut ctx);

        // All registers should be zero
        assert_eq!(audio.read_word(&mut ctx, 0x00).unwrap(), 0);
        assert_eq!(audio.read_word(&mut ctx, 0x04).unwrap(), 0);
        assert_eq!(audio.read_word(&mut ctx, 0x08).unwrap(), 0);
        assert_eq!(audio.read_word(&mut ctx, 0x0C).unwrap(), 0);

        // Active read should be cleared
        assert!(!audio.is_read_active());
    }
}
