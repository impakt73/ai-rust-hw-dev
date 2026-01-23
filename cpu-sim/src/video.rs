use crate::bus_device::{BusDevice, BusDeviceError, SystemContext};

// Re-export types from riscv_shared for backward compatibility
pub use riscv_shared::video::{VideoConfig, VideoFormat};

/// Active present operation state
#[derive(Debug, Clone)]
struct ActivePresent {
    /// Memory address to read from (incremented during operation)
    current_addr: u32,
    /// Number of bytes remaining to read
    bytes_remaining: u32,
    /// Buffer collecting pixel data
    pixel_data: Vec<u8>,
    /// Configuration for this present operation
    config: VideoConfig,
}

/// Video device providing display/graphics functionality
///
/// This device simulates a simple video controller that can read framebuffer
/// data from memory and invoke a callback when the data is ready. It provides
/// frame pacing to ensure consistent frame rates based on elapsed host time.
///
/// Register Map (all word-aligned):
/// - 0x00: VIDEO_ADDR    - Framebuffer address in memory (read/write)
/// - 0x04: VIDEO_CONFIG  - Image configuration (read/write)
///   Bits [11:0]   = width - 1  (12 bits, 1-4096 pixels)
///   Bits [23:12]  = height - 1 (12 bits, 1-4096 pixels)
///   Bits [31:24]  = format (0=RGBA8, 1=RGB8, 2=RGB565, 3=R8)
/// - 0x08: VIDEO_STATUS  - Status register (read-only)
///   Bit 0: FRAME_READY   (1 = can start rendering new frame)
///   Bit 1: PRESENT_READY (1 = can trigger present operation)
/// - 0x0C: VIDEO_PRESENT - Trigger present (write-only, write 0 to trigger)
///
/// The device operates as follows:
/// 1. CPU writes framebuffer address to VIDEO_ADDR
/// 2. CPU writes image configuration to VIDEO_CONFIG
/// 3. CPU polls VIDEO_STATUS until FRAME_READY is set
/// 4. CPU renders frame data to memory at VIDEO_ADDR
/// 5. CPU polls VIDEO_STATUS until PRESENT_READY is set
/// 6. CPU writes 0 to VIDEO_PRESENT to trigger present operation
/// 7. Device reads memory one byte per cycle and invokes callback when complete
/// 8. After callback completes, FRAME_READY is set after frame pacing delay
pub struct Video<F = fn(&[u8], &VideoConfig)>
where
    F: FnMut(&[u8], &VideoConfig),
{
    /// Framebuffer address configuration register
    video_addr: u32,
    /// Image configuration register
    video_config: u32,
    /// Active present operation (None = idle)
    active_present: Option<ActivePresent>,
    /// Target frame rate in frames per second
    target_fps: u32,
    /// Elapsed time (microseconds) when last frame was completed (for frame pacing)
    last_frame_time_us: Option<u64>,
    /// Optional callback invoked when present data is fully available
    present_callback: Option<F>,
}

impl<F> Video<F>
where
    F: FnMut(&[u8], &VideoConfig),
{
    /// Create a new Video device with default 60 FPS frame rate
    pub fn new(present_callback: Option<F>) -> Self {
        Self::with_fps(60, present_callback)
    }

    /// Create a new Video device with specified frame rate
    pub fn with_fps(fps: u32, present_callback: Option<F>) -> Self {
        Video {
            video_addr: 0,
            video_config: 0,
            active_present: None,
            target_fps: fps,
            last_frame_time_us: None,
            present_callback,
        }
    }

    /// Check if a present operation is currently in progress
    fn is_present_active(&self) -> bool {
        self.active_present.is_some()
    }

    /// Check if enough time has passed since last frame for frame pacing
    ///
    /// Frame pacing is based on elapsed host time (not simulation cycles).
    /// For 60 FPS: frame_time = 1,000,000 / 60 = ~16,667 microseconds.
    ///
    /// The frame time is computed as:
    ///
    ///   us_per_frame = 1_000_000 / target_fps
    ///
    /// This uses integer division and microsecond resolution, so:
    ///   - The minimum non-zero frame time is 1 microsecond (target_fps ≈ 1,000,000).
    ///   - For target_fps > 1_000,000, us_per_frame becomes 0, which effectively
    ///     disables frame pacing and treats every frame as ready.
    ///
    /// In practice, very high target_fps values are limited by the host timer
    /// granularity and should be treated as "uncapped" rather than precise frame
    /// rates.
    fn is_frame_ready(&self, current_time_us: u64) -> bool {
        match self.last_frame_time_us {
            None => true, // Always ready initially
            Some(last_time_us) => {
                // Calculate microseconds per frame
                let us_per_frame = 1_000_000 / self.target_fps as u64;
                let elapsed_us = current_time_us.saturating_sub(last_time_us);

                // Frame is ready if enough time has passed
                elapsed_us >= us_per_frame
            }
        }
    }

    /// Start a present operation using the configured registers
    fn start_present(&mut self) {
        if self.is_present_active() {
            log::warn!("Video: Present attempted while operation already in progress - ignoring");
            return;
        }

        let Some(config) = VideoConfig::from_register(self.video_config) else {
            log::warn!("Video: Invalid configuration for present operation - ignoring");
            return;
        };

        let total_bytes = config.total_bytes();
        if total_bytes == 0 {
            log::warn!("Video: Present attempted with zero size - ignoring");
            return;
        }

        log::debug!(
            "Video: Starting present from 0x{:08x}, {}x{} pixels, format {:?}, {} bytes",
            self.video_addr,
            config.width,
            config.height,
            config.format,
            total_bytes
        );

        // Start active present operation
        self.active_present = Some(ActivePresent {
            current_addr: self.video_addr,
            bytes_remaining: total_bytes,
            pixel_data: Vec::with_capacity(total_bytes as usize),
            config,
        });
    }

    /// Read the largest possible chunk from memory during present operation
    fn present_chunk(&mut self, ctx: &mut SystemContext) {
        let present = match self.active_present.as_mut() {
            Some(p) => p,
            None => return,
        };

        // Read chunk using shared helper
        let (bytes, read_size) = crate::bus_device::read_memory_chunk(
            ctx,
            present.current_addr,
            present.bytes_remaining,
        );

        // Append bytes to pixel data
        present
            .pixel_data
            .extend_from_slice(&bytes[..read_size as usize]);
        present.current_addr = present.current_addr.wrapping_add(read_size);
        present.bytes_remaining -= read_size;

        // Check if present is complete
        if present.bytes_remaining == 0 {
            let present_data = self.active_present.take().unwrap();
            let current_time_us = ctx.elapsed_time_us();
            self.invoke_present_callback(present_data, current_time_us);
        }
    }

    /// Invoke the present callback with the collected frame data
    fn invoke_present_callback(&mut self, present: ActivePresent, current_time_us: u64) {
        log::info!(
            "Video: Present complete ({}x{} {:?}, {} bytes)",
            present.config.width,
            present.config.height,
            present.config.format,
            present.pixel_data.len()
        );

        // Invoke the callback with the pixel data and configuration if present
        if let Some(ref mut callback) = self.present_callback {
            callback(&present.pixel_data, &present.config);
        }

        // Update frame pacing state
        self.last_frame_time_us = Some(current_time_us);
    }
}

impl<F> BusDevice for Video<F>
where
    F: FnMut(&[u8], &VideoConfig),
{
    fn read_word(&mut self, ctx: &mut SystemContext, offset: u32) -> Result<u32, BusDeviceError> {
        match offset {
            0x00 => Ok(self.video_addr),
            0x04 => Ok(self.video_config),
            0x08 => {
                // VIDEO_STATUS register
                let mut status = 0u32;

                // Bit 0: FRAME_READY
                if self.is_frame_ready(ctx.elapsed_time_us()) {
                    status |= 1 << 0;
                }

                // Bit 1: PRESENT_READY (inverse of present active)
                if !self.is_present_active() {
                    status |= 1 << 1;
                }

                Ok(status)
            }
            0x0C => {
                // VIDEO_PRESENT register is write-only
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
                self.video_addr = value;
                Ok(())
            }
            0x04 => {
                self.video_config = value;
                Ok(())
            }
            0x08 => {
                // VIDEO_STATUS register is read-only
                Err(BusDeviceError::WriteToReadOnly { offset })
            }
            0x0C => {
                // VIDEO_PRESENT register - writing any value starts the present
                self.start_present();
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
        "Video"
    }

    fn reset(&mut self, _ctx: &mut SystemContext) {
        self.video_addr = 0;
        self.video_config = 0;
        self.active_present = None;
        self.last_frame_time_us = None;
        // Note: frame_index is not reset so frames continue numbering across resets
    }

    fn clock_cycle(&mut self, ctx: &mut SystemContext) {
        // Read the largest possible chunk per clock cycle if a present operation is in progress
        self.present_chunk(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::Memory;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn test_video_format_bytes_per_pixel() {
        assert_eq!(VideoFormat::Rgba8.bytes_per_pixel(), 4);
        assert_eq!(VideoFormat::Rgb8.bytes_per_pixel(), 3);
        assert_eq!(VideoFormat::Rgb565.bytes_per_pixel(), 2);
        assert_eq!(VideoFormat::R8.bytes_per_pixel(), 1);
    }

    #[test]
    fn test_video_config_parsing() {
        // Test: 640x480 RGBA8
        // width-1 = 639 (0x27F), height-1 = 479 (0x1DF), format = 0
        let config_value = 0x27F | (0x1DF << 12);
        let config = VideoConfig::from_register(config_value).unwrap();
        assert_eq!(config.width, 640);
        assert_eq!(config.height, 480);
        assert_eq!(config.format, VideoFormat::Rgba8);
        assert_eq!(config.total_bytes(), 640 * 480 * 4);

        // Test round-trip
        assert_eq!(config.to_register(), config_value);
    }

    #[test]
    fn test_video_config_max_dimensions() {
        // Test: 4096x4096 RGBA8
        let config_value = 0xFFF | (0xFFF << 12);
        let config = VideoConfig::from_register(config_value).unwrap();
        assert_eq!(config.width, 4096);
        assert_eq!(config.height, 4096);
    }

    #[test]
    fn test_video_config_min_dimensions() {
        // Test: 1x1 RGB565
        let config_value = 2 << 24;
        let config = VideoConfig::from_register(config_value).unwrap();
        assert_eq!(config.width, 1);
        assert_eq!(config.height, 1);
        assert_eq!(config.format, VideoFormat::Rgb565);
        assert_eq!(config.total_bytes(), 2);
    }

    #[test]
    fn test_video_register_access() {
        let mut video = Video::new(None::<fn(&[u8], &VideoConfig)>);
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        // Write to registers
        video.write_word(&mut ctx, 0x00, 0x8000_1000).unwrap();
        video
            .write_word(&mut ctx, 0x04, 0x27F | (0x1DF << 12))
            .unwrap();

        // Read back registers
        assert_eq!(video.read_word(&mut ctx, 0x00).unwrap(), 0x8000_1000);
        assert_eq!(
            video.read_word(&mut ctx, 0x04).unwrap(),
            0x27F | (0x1DF << 12)
        );

        // Status should show FRAME_READY and PRESENT_READY
        assert_eq!(video.read_word(&mut ctx, 0x08).unwrap(), 0b11);
    }

    #[test]
    fn test_video_status_register_read_only() {
        let mut video = Video::new(None::<fn(&[u8], &VideoConfig)>);
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        let result = video.write_word(&mut ctx, 0x08, 1);
        assert!(matches!(
            result,
            Err(BusDeviceError::WriteToReadOnly { offset: 0x08 })
        ));
    }

    #[test]
    fn test_video_present_register_write_only() {
        let mut video = Video::new(None::<fn(&[u8], &VideoConfig)>);
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        let result = video.read_word(&mut ctx, 0x0C);
        assert!(matches!(
            result,
            Err(BusDeviceError::ReadFromWriteOnly { offset: 0x0C })
        ));
    }

    #[test]
    fn test_video_present_operation() {
        // Use Rc<RefCell<>> to capture callback data
        type CallbackData = Rc<RefCell<Option<(Vec<u8>, VideoConfig)>>>;
        let callback_data: CallbackData = Rc::new(RefCell::new(None));
        let callback_data_clone = callback_data.clone();

        let mut video = Video::new(Some(move |data: &[u8], config: &VideoConfig| {
            *callback_data_clone.borrow_mut() = Some((data.to_vec(), *config));
        }));
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        // Set up test image data in memory (2x2 RGBA8)
        let img_addr = 0x8000_1000;
        let test_data = [
            0xFF, 0x00, 0x00, 0xFF, // Red pixel
            0x00, 0xFF, 0x00, 0xFF, // Green pixel
            0x00, 0x00, 0xFF, 0xFF, // Blue pixel
            0xFF, 0xFF, 0xFF, 0xFF, // White pixel
        ];

        for (i, &byte) in test_data.iter().enumerate() {
            ctx.write_byte(img_addr + i as u32, byte);
        }

        // Configure video device for 2x2 RGBA8
        video.write_word(&mut ctx, 0x00, img_addr).unwrap();
        let config = 1 | (1 << 12); // 2x2 RGBA8
        video.write_word(&mut ctx, 0x04, config).unwrap();

        // Check initial status: both FRAME_READY and PRESENT_READY
        assert_eq!(video.read_word(&mut ctx, 0x08).unwrap(), 0b11);

        // Trigger present
        video.write_word(&mut ctx, 0x0C, 0).unwrap();

        // Check status: PRESENT_READY should be clear, FRAME_READY still set
        let status = video.read_word(&mut ctx, 0x08).unwrap();
        assert_eq!(status & 0b10, 0); // PRESENT_READY = 0

        // Run clock cycles to complete the transfer
        for _ in 0..test_data.len() {
            video.clock_cycle(&mut ctx);
        }

        // After completion, PRESENT_READY should be set again
        let status = video.read_word(&mut ctx, 0x08).unwrap();
        assert_eq!(status & 0b10, 0b10); // PRESENT_READY = 1

        // Verify callback was invoked with correct data
        let captured = callback_data.borrow();
        let (data, cfg) = captured.as_ref().unwrap();
        assert_eq!(data.as_slice(), &test_data);
        assert_eq!(cfg.width, 2);
        assert_eq!(cfg.height, 2);
        assert_eq!(cfg.format, VideoFormat::Rgba8);
    }

    #[test]
    fn test_video_invalid_config() {
        let mut video = Video::new(None::<fn(&[u8], &VideoConfig)>);
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        // Configure with invalid format
        video.write_word(&mut ctx, 0x00, 0x8000_1000).unwrap();
        video
            .write_word(&mut ctx, 0x04, 0x100 | (0x100 << 12) | (0xFF << 24))
            .unwrap();

        // Trigger present (should be ignored due to invalid format)
        video.write_word(&mut ctx, 0x0C, 0).unwrap();

        // Status should still show PRESENT_READY (no operation started)
        assert_eq!(video.read_word(&mut ctx, 0x08).unwrap(), 0b11);
    }

    #[test]
    fn test_video_multiple_present_rejected() {
        let mut video = Video::new(None::<fn(&[u8], &VideoConfig)>);
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        // Configure for small image
        video.write_word(&mut ctx, 0x00, 0x8000_1000).unwrap();
        video.write_word(&mut ctx, 0x04, 1 | (1 << 12)).unwrap();

        // Trigger first present
        video.write_word(&mut ctx, 0x0C, 0).unwrap();
        assert!(video.is_present_active());

        // Attempt to trigger second present (should be rejected)
        video.write_word(&mut ctx, 0x0C, 0).unwrap();

        // Run one cycle
        video.clock_cycle(&mut ctx);

        // Should still have active present
        assert!(video.is_present_active());
    }

    #[test]
    fn test_video_reset() {
        let mut video = Video::new(None::<fn(&[u8], &VideoConfig)>);
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        // Configure and start present
        video.write_word(&mut ctx, 0x00, 0x8000_1000).unwrap();
        video.write_word(&mut ctx, 0x04, 1 | (1 << 12)).unwrap();
        video.write_word(&mut ctx, 0x0C, 0).unwrap();

        // Reset
        video.reset(&mut ctx);

        // All registers should be zero
        assert_eq!(video.read_word(&mut ctx, 0x00).unwrap(), 0);
        assert_eq!(video.read_word(&mut ctx, 0x04).unwrap(), 0);

        // Status should show both ready bits
        assert_eq!(video.read_word(&mut ctx, 0x08).unwrap(), 0b11);

        // Active present should be cleared
        assert!(!video.is_present_active());
    }

    #[test]
    fn test_video_callback_receives_rgb8_data() {
        type CallbackData = Rc<RefCell<Option<(Vec<u8>, VideoConfig)>>>;
        let callback_data: CallbackData = Rc::new(RefCell::new(None));
        let callback_data_clone = callback_data.clone();

        let mut video = Video::new(Some(move |data: &[u8], config: &VideoConfig| {
            *callback_data_clone.borrow_mut() = Some((data.to_vec(), *config));
        }));
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        // Set up test image data in memory (2x2 RGB8)
        let img_addr = 0x8000_1000;
        let test_data = [
            0xFF, 0x00, 0x00, // Red pixel
            0x00, 0xFF, 0x00, // Green pixel
            0x00, 0x00, 0xFF, // Blue pixel
            0x80, 0x40, 0x20, // Gray pixel
        ];

        for (i, &byte) in test_data.iter().enumerate() {
            ctx.write_byte(img_addr + i as u32, byte);
        }

        // Configure video device for 2x2 RGB8
        video.write_word(&mut ctx, 0x00, img_addr).unwrap();
        let config = 1 | (1 << 12) | (1 << 24); // 2x2 RGB8 (format=1)
        video.write_word(&mut ctx, 0x04, config).unwrap();

        // Trigger present
        video.write_word(&mut ctx, 0x0C, 0).unwrap();

        // Run clock cycles to complete the transfer
        for _ in 0..test_data.len() {
            video.clock_cycle(&mut ctx);
        }

        // Verify callback was invoked with RGB8 data
        let captured = callback_data.borrow();
        let (data, cfg) = captured.as_ref().unwrap();
        assert_eq!(data.as_slice(), &test_data);
        assert_eq!(cfg.width, 2);
        assert_eq!(cfg.height, 2);
        assert_eq!(cfg.format, VideoFormat::Rgb8);
    }

    #[test]
    fn test_video_callback_receives_rgb565_data() {
        type CallbackData = Rc<RefCell<Option<(Vec<u8>, VideoConfig)>>>;
        let callback_data: CallbackData = Rc::new(RefCell::new(None));
        let callback_data_clone = callback_data.clone();

        let mut video = Video::new(Some(move |data: &[u8], config: &VideoConfig| {
            *callback_data_clone.borrow_mut() = Some((data.to_vec(), *config));
        }));
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        // Set up test image data in memory (2x2 RGB565)
        let img_addr = 0x8000_1000;

        // RGB565 test values:
        // Pure red: R=31, G=0, B=0 -> 0xF800
        // Pure green: R=0, G=63, B=0 -> 0x07E0
        // Pure blue: R=0, G=0, B=31 -> 0x001F
        // White: R=31, G=63, B=31 -> 0xFFFF
        let test_pixels = [
            0xF800u16, // Red
            0x07E0u16, // Green
            0x001Fu16, // Blue
            0xFFFFu16, // White
        ];

        for (i, &pixel) in test_pixels.iter().enumerate() {
            let bytes = pixel.to_le_bytes();
            ctx.write_byte(img_addr + (i * 2) as u32, bytes[0]);
            ctx.write_byte(img_addr + (i * 2 + 1) as u32, bytes[1]);
        }

        // Configure video device for 2x2 RGB565
        video.write_word(&mut ctx, 0x00, img_addr).unwrap();
        let config = 1 | (1 << 12) | (2 << 24); // 2x2 RGB565 (format=2)
        video.write_word(&mut ctx, 0x04, config).unwrap();

        // Trigger present
        video.write_word(&mut ctx, 0x0C, 0).unwrap();

        // Run clock cycles to complete the transfer
        for _ in 0..(test_pixels.len() * 2) {
            video.clock_cycle(&mut ctx);
        }

        // Verify callback was invoked with RGB565 data
        let captured = callback_data.borrow();
        let (data, cfg) = captured.as_ref().unwrap();
        assert_eq!(data.len(), 8); // 4 pixels × 2 bytes
        assert_eq!(cfg.width, 2);
        assert_eq!(cfg.height, 2);
        assert_eq!(cfg.format, VideoFormat::Rgb565);
    }

    #[test]
    fn test_video_callback_receives_r8_data() {
        type CallbackData = Rc<RefCell<Option<(Vec<u8>, VideoConfig)>>>;
        let callback_data: CallbackData = Rc::new(RefCell::new(None));
        let callback_data_clone = callback_data.clone();

        let mut video = Video::new(Some(move |data: &[u8], config: &VideoConfig| {
            *callback_data_clone.borrow_mut() = Some((data.to_vec(), *config));
        }));
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        // Set up test image data in memory (2x2 R8 grayscale)
        let img_addr = 0x8000_1000;
        let test_data = [
            0x00, // Black
            0x80, // Mid-gray
            0xFF, // White
            0x40, // Dark gray
        ];

        for (i, &byte) in test_data.iter().enumerate() {
            ctx.write_byte(img_addr + i as u32, byte);
        }

        // Configure video device for 2x2 R8
        video.write_word(&mut ctx, 0x00, img_addr).unwrap();
        let config = 1 | (1 << 12) | (3 << 24); // 2x2 R8 (format=3)
        video.write_word(&mut ctx, 0x04, config).unwrap();

        // Trigger present
        video.write_word(&mut ctx, 0x0C, 0).unwrap();

        // Run clock cycles to complete the transfer
        for _ in 0..test_data.len() {
            video.clock_cycle(&mut ctx);
        }

        // Verify callback was invoked with R8 data
        let captured = callback_data.borrow();
        let (data, cfg) = captured.as_ref().unwrap();
        assert_eq!(data.as_slice(), &test_data);
        assert_eq!(cfg.width, 2);
        assert_eq!(cfg.height, 2);
        assert_eq!(cfg.format, VideoFormat::R8);
    }

    #[test]
    fn test_rgb565_scaling_precision() {
        // Test the RGB565 to RGBA8 scaling logic directly
        // This verifies the bit manipulation is correct

        // Test case 1: Max red (R=31, 5-bit)
        let r_5bit = 31u8;
        let r_8bit = (r_5bit << 3) | (r_5bit >> 2);
        assert_eq!(r_8bit, 255, "Max 5-bit red should scale to 255");

        // Test case 2: Mid red (R=16, 5-bit)
        let r_5bit = 16u8;
        let r_8bit = (r_5bit << 3) | (r_5bit >> 2);
        assert_eq!(r_8bit, 132, "Mid 5-bit red should scale to 132");

        // Test case 3: Max green (G=63, 6-bit)
        let g_6bit = 63u8;
        let g_8bit = (g_6bit << 2) | (g_6bit >> 4);
        assert_eq!(g_8bit, 255, "Max 6-bit green should scale to 255");

        // Test case 4: Mid green (G=32, 6-bit)
        let g_6bit = 32u8;
        let g_8bit = (g_6bit << 2) | (g_6bit >> 4);
        assert_eq!(g_8bit, 130, "Mid 6-bit green should scale to 130");

        // Test case 5: Min values should scale to 0
        let r_5bit = 0u8;
        let r_8bit = (r_5bit << 3) | (r_5bit >> 2);
        assert_eq!(r_8bit, 0, "Zero 5-bit should scale to 0");

        let g_6bit = 0u8;
        let g_8bit = (g_6bit << 2) | (g_6bit >> 4);
        assert_eq!(g_8bit, 0, "Zero 6-bit should scale to 0");
    }
}
