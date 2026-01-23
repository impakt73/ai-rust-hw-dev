//! Video device register offsets, configuration types, and helpers

use crate::bus::VIDEO_BASE;

/// Video framebuffer address register offset (0x00)
pub const VIDEO_ADDR: u32 = VIDEO_BASE;

/// Video configuration register offset (0x04)
pub const VIDEO_CONFIG: u32 = VIDEO_BASE + 0x04;

/// Video status register offset (0x08)
pub const VIDEO_STATUS: u32 = VIDEO_BASE + 0x08;

/// Video present trigger register offset (0x0C)
pub const VIDEO_PRESENT: u32 = VIDEO_BASE + 0x0C;

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
    pub fn bytes_per_pixel(&self) -> u32 {
        match self {
            VideoFormat::Rgba8 => 4,
            VideoFormat::Rgb8 => 3,
            VideoFormat::Rgb565 => 2,
            VideoFormat::R8 => 1,
        }
    }

    /// Parse format from 8-bit format field
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(VideoFormat::Rgba8),
            1 => Some(VideoFormat::Rgb8),
            2 => Some(VideoFormat::Rgb565),
            3 => Some(VideoFormat::R8),
            _ => None,
        }
    }

    /// Convert format to 8-bit format field
    pub fn to_u8(self) -> u8 {
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
pub struct VideoConfig {
    /// Image width in pixels (1-4096, stored with +1 bias)
    pub width: u32,
    /// Image height in pixels (1-4096, stored with +1 bias)
    pub height: u32,
    /// Pixel format
    pub format: VideoFormat,
}

impl VideoConfig {
    /// Parse configuration from register value
    /// Bits [11:0]   = width - 1  (12 bits, allowing 1-4096)
    /// Bits [23:12]  = height - 1 (12 bits, allowing 1-4096)
    /// Bits [31:24]  = format     (8 bits)
    pub fn from_register(value: u32) -> Option<Self> {
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
    pub fn to_register(self) -> u32 {
        debug_assert!(
            (1..=4096).contains(&self.width),
            "VideoConfig.width out of range: {}",
            self.width
        );
        debug_assert!(
            (1..=4096).contains(&self.height),
            "VideoConfig.height out of range: {}",
            self.height
        );

        let width_minus_1 = self
            .width
            .checked_sub(1)
            .expect("VideoConfig.width must be at least 1")
            & 0x0FFF;
        let height_minus_1 = self
            .height
            .checked_sub(1)
            .expect("VideoConfig.height must be at least 1")
            & 0x0FFF;
        let format_field = self.format.to_u8() as u32;

        width_minus_1 | (height_minus_1 << 12) | (format_field << 24)
    }

    /// Calculate total number of bytes needed for this configuration
    pub fn total_bytes(&self) -> u32 {
        self.width * self.height * self.format.bytes_per_pixel()
    }
}
