//! Helper functions for interacting with Video device via MMIO
//!
//! This module provides high-level helper functions for common Video device
//! operations like waiting for status flags, triggering presents, and writing pixels.

use core::ptr::{read_volatile, write_volatile};

use crate::video::{VideoFormat, VIDEO_PRESENT, VIDEO_STATUS};

/// Video status bit flags
pub const FRAME_READY: u32 = 1 << 0;
pub const PRESENT_READY: u32 = 1 << 1;

/// Wait for FRAME_READY bit to be set in VIDEO_STATUS register
///
/// This function polls the VIDEO_STATUS register until the FRAME_READY bit is set,
/// indicating that the video device is ready to accept a new frame.
///
/// # Safety
/// This function performs volatile MMIO reads and should only be called in a
/// bare-metal environment where the VIDEO_STATUS register is properly mapped.
pub fn wait_for_frame_ready() {
    unsafe {
        loop {
            let status = read_volatile(VIDEO_STATUS as *const u32);
            if (status & FRAME_READY) != 0 {
                break;
            }
        }
    }
}

/// Wait for PRESENT_READY bit to be set in VIDEO_STATUS register
///
/// This function polls the VIDEO_STATUS register until the PRESENT_READY bit is set,
/// indicating that the video device is ready to accept a present operation.
///
/// # Safety
/// This function performs volatile MMIO reads and should only be called in a
/// bare-metal environment where the VIDEO_STATUS register is properly mapped.
pub fn wait_for_present_ready() {
    unsafe {
        loop {
            let status = read_volatile(VIDEO_STATUS as *const u32);
            if (status & PRESENT_READY) != 0 {
                break;
            }
        }
    }
}

/// Trigger a present operation by writing to VIDEO_PRESENT register
///
/// This function triggers the video device to present the current framebuffer.
/// The caller should ensure that PRESENT_READY is set before calling this function.
///
/// # Safety
/// This function performs volatile MMIO writes and should only be called in a
/// bare-metal environment where the VIDEO_PRESENT register is properly mapped.
pub fn trigger_present() {
    unsafe {
        write_volatile(VIDEO_PRESENT as *mut u32, 0);
    }
}

/// Write a pixel to a framebuffer in RGBA8 format (4 bytes per pixel)
///
/// # Arguments
/// * `framebuffer_base` - Base address of the framebuffer in memory
/// * `width` - Width of the framebuffer in pixels
/// * `x` - X coordinate of the pixel (0-based)
/// * `y` - Y coordinate of the pixel (0-based)
/// * `r` - Red component (0-255)
/// * `g` - Green component (0-255)
/// * `b` - Blue component (0-255)
/// * `a` - Alpha component (0-255)
///
/// # Safety
/// This function performs volatile memory writes. The caller must ensure:
/// - `framebuffer_base` points to valid, writable memory
/// - The coordinates (x, y) are within the framebuffer dimensions
/// - The framebuffer has sufficient space for (y * width + x) * 4 bytes
#[allow(clippy::too_many_arguments)]
pub fn write_pixel_rgba8(
    framebuffer_base: u32,
    width: u32,
    x: u32,
    y: u32,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
) {
    unsafe {
        let offset = (y * width + x) * 4;
        let addr = framebuffer_base + offset;
        write_volatile(addr as *mut u8, r);
        write_volatile((addr + 1) as *mut u8, g);
        write_volatile((addr + 2) as *mut u8, b);
        write_volatile((addr + 3) as *mut u8, a);
    }
}

/// Write a pixel to a framebuffer in RGB8 format (3 bytes per pixel)
///
/// # Arguments
/// * `framebuffer_base` - Base address of the framebuffer in memory
/// * `width` - Width of the framebuffer in pixels
/// * `x` - X coordinate of the pixel (0-based)
/// * `y` - Y coordinate of the pixel (0-based)
/// * `r` - Red component (0-255)
/// * `g` - Green component (0-255)
/// * `b` - Blue component (0-255)
///
/// # Safety
/// This function performs volatile memory writes. The caller must ensure:
/// - `framebuffer_base` points to valid, writable memory
/// - The coordinates (x, y) are within the framebuffer dimensions
/// - The framebuffer has sufficient space for (y * width + x) * 3 bytes
pub fn write_pixel_rgb8(framebuffer_base: u32, width: u32, x: u32, y: u32, r: u8, g: u8, b: u8) {
    unsafe {
        let offset = (y * width + x) * 3;
        let addr = framebuffer_base + offset;
        write_volatile(addr as *mut u8, r);
        write_volatile((addr + 1) as *mut u8, g);
        write_volatile((addr + 2) as *mut u8, b);
    }
}

/// Write a pixel to a framebuffer with automatic format detection
///
/// This is a convenience function that automatically selects the correct pixel
/// writing function based on the format.
///
/// # Arguments
/// * `framebuffer_base` - Base address of the framebuffer in memory
/// * `width` - Width of the framebuffer in pixels
/// * `x` - X coordinate of the pixel (0-based)
/// * `y` - Y coordinate of the pixel (0-based)
/// * `format` - Pixel format (only RGBA8 and RGB8 are supported)
/// * `r` - Red component (0-255)
/// * `g` - Green component (0-255)
/// * `b` - Blue component (0-255)
/// * `a` - Alpha component (0-255, ignored for RGB8)
///
/// # Safety
/// This function performs volatile memory writes. The caller must ensure:
/// - `framebuffer_base` points to valid, writable memory
/// - The coordinates (x, y) are within the framebuffer dimensions
/// - The framebuffer has sufficient space based on the format
#[allow(clippy::too_many_arguments)]
pub fn write_pixel(
    framebuffer_base: u32,
    width: u32,
    x: u32,
    y: u32,
    format: VideoFormat,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
) {
    match format {
        VideoFormat::Rgba8 => write_pixel_rgba8(framebuffer_base, width, x, y, r, g, b, a),
        VideoFormat::Rgb8 => write_pixel_rgb8(framebuffer_base, width, x, y, r, g, b),
        _ => {
            // For other formats, we don't have a simple implementation
            // This could be extended in the future
        }
    }
}
