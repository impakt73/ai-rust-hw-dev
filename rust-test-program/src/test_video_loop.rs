#![no_std]
#![no_main]

mod common;

use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};
use riscv_rt::entry;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

/// Video device base address
const VIDEO_BASE: u32 = 0x2000_0000;

/// Video register offsets
const VIDEO_ADDR: u32 = VIDEO_BASE;
const VIDEO_CONFIG: u32 = VIDEO_BASE + 0x04;
const VIDEO_STATUS: u32 = VIDEO_BASE + 0x08;
const VIDEO_PRESENT: u32 = VIDEO_BASE + 0x0C;

/// Video status bits
const FRAME_READY: u32 = 1 << 0;
const PRESENT_READY: u32 = 1 << 1;

/// Video formats
const FORMAT_RGB8: u32 = 1;

/// Test image dimensions
const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;

/// Framebuffer base address in DRAM
const FRAMEBUFFER_BASE: u32 = 0x8000_1000;

/// Checkerboard pattern size (in pixels)
const CHECKER_SIZE: u32 = 8;

/// Helper to create VIDEO_CONFIG register value
/// Bits [11:0]   = width - 1
/// Bits [23:12]  = height - 1
/// Bits [31:24]  = format
const fn make_video_config(width: u32, height: u32, format: u32) -> u32 {
    ((width - 1) & 0xFFF) | (((height - 1) & 0xFFF) << 12) | ((format & 0xFF) << 24)
}

/// Wait for FRAME_READY bit to be set
fn wait_for_frame_ready() {
    unsafe {
        loop {
            let status = read_volatile(VIDEO_STATUS as *const u32);
            if (status & FRAME_READY) != 0 {
                break;
            }
        }
    }
}

/// Wait for PRESENT_READY bit to be set
fn wait_for_present_ready() {
    unsafe {
        loop {
            let status = read_volatile(VIDEO_STATUS as *const u32);
            if (status & PRESENT_READY) != 0 {
                break;
            }
        }
    }
}

/// Trigger a present operation
fn trigger_present() {
    unsafe {
        write_volatile(VIDEO_PRESENT as *mut u32, 0);
    }
}

/// Write a pixel to the framebuffer at (x, y)
/// Pixel format is RGB8 (3 bytes per pixel)
fn write_pixel(x: u32, y: u32, r: u8, g: u8, b: u8) {
    unsafe {
        let offset = (y * WIDTH + x) * 3;
        let addr = FRAMEBUFFER_BASE + offset;
        write_volatile(addr as *mut u8, r);
        write_volatile((addr + 1) as *mut u8, g);
        write_volatile((addr + 2) as *mut u8, b);
    }
}

/// Render a black and white scrolling checkerboard pattern
/// The pattern scrolls by 1 pixel per frame in both x and y dimensions
fn render_scrolling_checkerboard(offset_x: u32, offset_y: u32) {
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            // Apply scrolling offset (modulo to wrap around)
            let scrolled_x = (x + offset_x) % (CHECKER_SIZE * 2);
            let scrolled_y = (y + offset_y) % (CHECKER_SIZE * 2);

            // Determine which checker square we're in
            let checker_x = scrolled_x / CHECKER_SIZE;
            let checker_y = scrolled_y / CHECKER_SIZE;

            // Checkerboard pattern: alternate black and white
            let is_white = (checker_x + checker_y).is_multiple_of(2);

            if is_white {
                // White pixel
                write_pixel(x, y, 255, 255, 255);
            } else {
                // Black pixel
                write_pixel(x, y, 0, 0, 0);
            }
        }
    }
}

#[entry]
fn main() -> ! {
    unsafe {
        // Configure Video device
        write_volatile(VIDEO_ADDR as *mut u32, FRAMEBUFFER_BASE);
        write_volatile(
            VIDEO_CONFIG as *mut u32,
            make_video_config(WIDTH, HEIGHT, FORMAT_RGB8),
        );

        // Initialize scroll offset
        let mut scroll_offset: u32 = 0;

        // Main infinite loop
        loop {
            // Wait for video to be ready for a new frame
            wait_for_frame_ready();

            // Render the scrolling checkerboard pattern
            // Scroll by 1 pixel per frame in both dimensions
            render_scrolling_checkerboard(scroll_offset, scroll_offset);

            // Wait until we can present
            wait_for_present_ready();

            // Trigger the present operation
            trigger_present();

            // Increment scroll offset for next frame (wraps around at 16 pixels)
            scroll_offset = (scroll_offset + 1) % (CHECKER_SIZE * 2);
        }
    }
}
