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
        // Pack RGBA into a single u32 for better performance
        let packed = r as u32 | (g as u32) << 8 | (b as u32) << 16 | (a as u32) << 24;
        write_volatile(addr as *mut u32, packed);
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
        // Pack r and g into a single u16 for better performance
        let rg_packed = r as u16 | (g as u16) << 8;
        write_volatile(addr as *mut u16, rg_packed);
        write_volatile((addr + 2) as *mut u8, b);
    }
}

/// Write a pixel to a framebuffer in RGB565 format (2 bytes per pixel)
///
/// RGB565 format packs colors as: 5 bits red, 6 bits green, 5 bits blue
///
/// # Arguments
/// * `framebuffer_base` - Base address of the framebuffer in memory
/// * `width` - Width of the framebuffer in pixels
/// * `x` - X coordinate of the pixel (0-based)
/// * `y` - Y coordinate of the pixel (0-based)
/// * `r` - Red component (0-255, scaled to 5 bits)
/// * `g` - Green component (0-255, scaled to 6 bits)
/// * `b` - Blue component (0-255, scaled to 5 bits)
///
/// # Safety
/// This function performs volatile memory writes. The caller must ensure:
/// - `framebuffer_base` points to valid, writable memory
/// - The coordinates (x, y) are within the framebuffer dimensions
/// - The framebuffer has sufficient space for (y * width + x) * 2 bytes
pub fn write_pixel_rgb565(framebuffer_base: u32, width: u32, x: u32, y: u32, r: u8, g: u8, b: u8) {
    unsafe {
        let offset = (y * width + x) * 2;
        let addr = framebuffer_base + offset;
        // Convert 8-bit RGB to RGB565 format and pack into single u16
        let r5 = (r >> 3) as u16; // 5 bits red
        let g6 = (g >> 2) as u16; // 6 bits green
        let b5 = (b >> 3) as u16; // 5 bits blue
        let packed = (r5 << 11) | (g6 << 5) | b5;
        write_volatile(addr as *mut u16, packed);
    }
}

/// Write a pixel to a framebuffer in R8 (grayscale) format (1 byte per pixel)
///
/// # Arguments
/// * `framebuffer_base` - Base address of the framebuffer in memory
/// * `width` - Width of the framebuffer in pixels
/// * `x` - X coordinate of the pixel (0-based)
/// * `y` - Y coordinate of the pixel (0-based)
/// * `gray` - Grayscale value (0-255)
///
/// # Safety
/// This function performs volatile memory writes. The caller must ensure:
/// - `framebuffer_base` points to valid, writable memory
/// - The coordinates (x, y) are within the framebuffer dimensions
/// - The framebuffer has sufficient space for (y * width + x) bytes
pub fn write_pixel_r8(framebuffer_base: u32, width: u32, x: u32, y: u32, gray: u8) {
    unsafe {
        let offset = y * width + x;
        let addr = framebuffer_base + offset;
        write_volatile(addr as *mut u8, gray);
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
/// * `format` - Pixel format
/// * `r` - Red component (0-255)
/// * `g` - Green component (0-255)
/// * `b` - Blue component (0-255)
/// * `a` - Alpha component (0-255, ignored for formats without alpha)
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
        VideoFormat::Rgb565 => write_pixel_rgb565(framebuffer_base, width, x, y, r, g, b),
        VideoFormat::R8 => {
            // Pass through the r value unchanged
            write_pixel_r8(framebuffer_base, width, x, y, r);
        }
    }
}
