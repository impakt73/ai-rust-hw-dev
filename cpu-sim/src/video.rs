use crate::bus_device::{BusDevice, BusDeviceError, SystemContext};

/// Video format enumeration for pixel data interpretation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoFormat {
    /// 4 bytes per pixel: Red, Green, Blue, Alpha (8 bits each)
    Rgba8,
    /// 3 bytes per pixel: Red, Green, Blue (8 bits each)
    Rgb8,
    /// 2 bytes per pixel: 5 bits Red, 6 bits Green, 5 bits Blue
    Rgb565,
    /// 1 byte per pixel: Grayscale (8 bits)
    R8,
}

impl VideoFormat {
    /// Get the number of bytes per pixel for this format
    fn bytes_per_pixel(&self) -> u32 {
        match self {
            VideoFormat::Rgba8 => 4,
            VideoFormat::Rgb8 => 3,
            VideoFormat::Rgb565 => 2,
            VideoFormat::R8 => 1,
        }
    }

    /// Parse format from 8-bit format field
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(VideoFormat::Rgba8),
            1 => Some(VideoFormat::Rgb8),
            2 => Some(VideoFormat::Rgb565),
            3 => Some(VideoFormat::R8),
            _ => None,
        }
    }

    /// Convert format to 8-bit format field
    #[allow(dead_code)]
    fn to_u8(self) -> u8 {
        match self {
            VideoFormat::Rgba8 => 0,
            VideoFormat::Rgb8 => 1,
            VideoFormat::Rgb565 => 2,
            VideoFormat::R8 => 3,
        }
    }
}

/// Configuration parsed from VIDEO_CONFIG register
#[derive(Debug, Clone, Copy)]
struct VideoConfig {
    /// Image width in pixels (1-4096, stored with +1 bias)
    width: u32,
    /// Image height in pixels (1-4096, stored with +1 bias)
    height: u32,
    /// Pixel format
    format: VideoFormat,
}

impl VideoConfig {
    /// Parse configuration from register value
    /// Bits [11:0]   = width - 1  (12 bits, allowing 1-4096)
    /// Bits [23:12]  = height - 1 (12 bits, allowing 1-4096)
    /// Bits [31:24]  = format     (8 bits)
    fn from_register(value: u32) -> Option<Self> {
        let width_minus_1 = value & 0xFFF;
        let height_minus_1 = (value >> 12) & 0xFFF;
        let format_field = ((value >> 24) & 0xFF) as u8;

        let format = VideoFormat::from_u8(format_field)?;

        Some(VideoConfig {
            width: width_minus_1 + 1,
            height: height_minus_1 + 1,
            format,
        })
    }

    /// Convert configuration to register value
    #[allow(dead_code)]
    fn to_register(self) -> u32 {
        let width_minus_1 = self.width - 1;
        let height_minus_1 = self.height - 1;
        let format_field = self.format.to_u8() as u32;
        
        width_minus_1 | (height_minus_1 << 12) | (format_field << 24)
    }

    /// Calculate total number of bytes needed for this configuration
    fn total_bytes(&self) -> u32 {
        self.width * self.height * self.format.bytes_per_pixel()
    }
}

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
/// data from memory and save it as PNG images. It provides frame pacing to
/// ensure consistent frame rates based on simulation cycles.
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
/// 7. Device reads memory one byte per cycle and saves as PNG
/// 8. After save completes, FRAME_READY is set after frame pacing delay
pub struct Video {
    /// Framebuffer address configuration register
    video_addr: u32,
    /// Image configuration register
    video_config: u32,
    /// Active present operation (None = idle)
    active_present: Option<ActivePresent>,
    /// Target frame rate in frames per second
    target_fps: u32,
    /// Cycle count when last frame was completed (for frame pacing)
    last_frame_cycle: Option<u64>,
    /// Current cycle count
    current_cycle: u64,
    /// Current frame index (for generating filenames)
    frame_index: u32,
}

impl Video {
    /// Create a new Video device with default 60 FPS frame rate
    pub fn new() -> Self {
        Self::with_fps(60)
    }

    /// Create a new Video device with specified frame rate
    pub fn with_fps(fps: u32) -> Self {
        Video {
            video_addr: 0,
            video_config: 0,
            active_present: None,
            target_fps: fps,
            last_frame_cycle: None,
            current_cycle: 0,
            frame_index: 0,
        }
    }

    /// Check if a present operation is currently in progress
    fn is_present_active(&self) -> bool {
        self.active_present.is_some()
    }

    /// Check if enough cycles have passed since last frame for frame pacing
    ///
    /// Frame pacing is based on cycles. For a 100MHz CPU and 60 FPS:
    /// - Cycles per frame = 100_000_000 / 60 = ~1,666,667 cycles
    ///
    /// This ensures FRAME_READY pacing even in simulation.
    fn is_frame_ready(&self) -> bool {
        match self.last_frame_cycle {
            None => true, // Always ready initially
            Some(last_cycle) => {
                // Calculate cycles per frame based on assumed 100MHz CPU
                // At 100MHz, 60 FPS means 1,666,667 cycles per frame
                const CPU_FREQ_HZ: u64 = 100_000_000;
                let cycles_per_frame = CPU_FREQ_HZ / self.target_fps as u64;
                let elapsed_cycles = self.current_cycle.saturating_sub(last_cycle);
                elapsed_cycles >= cycles_per_frame
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

    /// Read one byte from memory during present operation
    fn present_one_byte(&mut self, ctx: &mut SystemContext) {
        let present = match self.active_present.as_mut() {
            Some(p) => p,
            None => return,
        };

        // Read one byte from memory
        let byte = ctx.read_byte(present.current_addr);
        present.pixel_data.push(byte);

        // Update state
        present.current_addr = present.current_addr.wrapping_add(1);
        present.bytes_remaining -= 1;

        // Check if present is complete
        if present.bytes_remaining == 0 {
            let present_data = self.active_present.take().unwrap();
            self.save_frame_as_png(present_data);
        }
    }

    /// Save the collected frame data as a PNG image
    fn save_frame_as_png(&mut self, present: ActivePresent) {
        let filename = format!("frame_{:04}.png", self.frame_index);

        match self.convert_and_save_image(&present, &filename) {
            Ok(()) => {
                log::info!(
                    "Video: Frame {} saved to {} ({}x{} {:?})",
                    self.frame_index,
                    filename,
                    present.config.width,
                    present.config.height,
                    present.config.format
                );
                self.frame_index += 1;
                self.last_frame_cycle = Some(self.current_cycle);
            }
            Err(e) => {
                log::error!("Video: Failed to save frame {}: {}", self.frame_index, e);
            }
        }
    }

    /// Convert pixel data from source format to RGBA8 and save as PNG
    fn convert_and_save_image(
        &self,
        present: &ActivePresent,
        filename: &str,
    ) -> Result<(), String> {
        use image::{ImageBuffer, Rgba};

        let width = present.config.width;
        let height = present.config.height;
        let pixel_data = &present.pixel_data;

        // Convert pixel data to RGBA8 format
        let rgba_data = match present.config.format {
            VideoFormat::Rgba8 => {
                // Already in RGBA8 format
                pixel_data.clone()
            }
            VideoFormat::Rgb8 => {
                // Convert RGB8 to RGBA8 (add alpha channel)
                let mut rgba = Vec::with_capacity((width * height * 4) as usize);
                for chunk in pixel_data.chunks_exact(3) {
                    rgba.push(chunk[0]); // R
                    rgba.push(chunk[1]); // G
                    rgba.push(chunk[2]); // B
                    rgba.push(255); // A (opaque)
                }
                rgba
            }
            VideoFormat::Rgb565 => {
                // Convert RGB565 to RGBA8
                let mut rgba = Vec::with_capacity((width * height * 4) as usize);
                for chunk in pixel_data.chunks_exact(2) {
                    let rgb565 = u16::from_le_bytes([chunk[0], chunk[1]]);
                    let r = ((rgb565 >> 11) & 0x1F) as u8;
                    let g = ((rgb565 >> 5) & 0x3F) as u8;
                    let b = (rgb565 & 0x1F) as u8;

                    // Scale to 8-bit
                    rgba.push((r << 3) | (r >> 2)); // R: 5-bit to 8-bit
                    rgba.push((g << 2) | (g >> 4)); // G: 6-bit to 8-bit
                    rgba.push((b << 3) | (b >> 2)); // B: 5-bit to 8-bit
                    rgba.push(255); // A: opaque
                }
                rgba
            }
            VideoFormat::R8 => {
                // Convert grayscale to RGBA8
                let mut rgba = Vec::with_capacity((width * height * 4) as usize);
                for &gray in pixel_data {
                    rgba.push(gray); // R
                    rgba.push(gray); // G
                    rgba.push(gray); // B
                    rgba.push(255); // A (opaque)
                }
                rgba
            }
        };

        // Create image buffer from RGBA8 data
        let img_buffer = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(width, height, rgba_data)
            .ok_or_else(|| "Failed to create image buffer from pixel data".to_string())?;

        // Save the image
        img_buffer
            .save(filename)
            .map_err(|e| format!("Failed to save image: {}", e))?;

        Ok(())
    }
}

impl Default for Video {
    fn default() -> Self {
        Self::new()
    }
}

impl BusDevice for Video {
    fn read_word(&mut self, _ctx: &mut SystemContext, offset: u32) -> Result<u32, BusDeviceError> {
        match offset {
            0x00 => Ok(self.video_addr),
            0x04 => Ok(self.video_config),
            0x08 => {
                // VIDEO_STATUS register
                let mut status = 0u32;

                // Bit 0: FRAME_READY
                if self.is_frame_ready() {
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
        self.last_frame_cycle = None;
        self.current_cycle = 0;
        // Note: frame_index is not reset so frames continue numbering across resets
    }

    fn clock_cycle(&mut self, ctx: &mut SystemContext) {
        // Increment cycle counter
        self.current_cycle += 1;

        // Read one byte per clock cycle if a present operation is in progress
        self.present_one_byte(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::Memory;

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
        let config_value = 0x27F | (0x1DF << 12) | (0 << 24);
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
        let config_value = 0xFFF | (0xFFF << 12) | (0 << 24);
        let config = VideoConfig::from_register(config_value).unwrap();
        assert_eq!(config.width, 4096);
        assert_eq!(config.height, 4096);
    }

    #[test]
    fn test_video_config_min_dimensions() {
        // Test: 1x1 RGB565
        let config_value = 0x0 | (0x0 << 12) | (2 << 24);
        let config = VideoConfig::from_register(config_value).unwrap();
        assert_eq!(config.width, 1);
        assert_eq!(config.height, 1);
        assert_eq!(config.format, VideoFormat::Rgb565);
        assert_eq!(config.total_bytes(), 2);
    }

    #[test]
    fn test_video_register_access() {
        let mut video = Video::new();
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
        let mut video = Video::new();
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
        let mut video = Video::new();
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
        let mut video = Video::new();
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
        let config = 1 | (1 << 12) | (0 << 24); // 2x2 RGBA8
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
    }

    #[test]
    fn test_video_invalid_config() {
        let mut video = Video::new();
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
        let mut video = Video::new();
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        // Configure for small image
        video.write_word(&mut ctx, 0x00, 0x8000_1000).unwrap();
        video
            .write_word(&mut ctx, 0x04, 1 | (1 << 12) | (0 << 24))
            .unwrap();

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
        let mut video = Video::new();
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
}
