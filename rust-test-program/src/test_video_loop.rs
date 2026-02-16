#![no_std]
#![no_main]

mod common;

#[global_allocator]
static HEAP: common::Heap = common::Heap::empty();

use common::{trigger_present, wait_for_frame_ready, wait_for_present_ready};
use core::panic::PanicInfo;
use core::ptr::write_volatile;
use riscv_rt::entry;
use riscv_shared::{VideoConfig, VideoFormat, VIDEO_ADDR, VIDEO_CONFIG};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

/// Test image dimensions
const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;

/// Framebuffer base address in DRAM
const FRAMEBUFFER_BASE: u32 = 0x8000_1000;

/// Checkerboard pattern size (in pixels)
const CHECKER_SIZE: u32 = 8;

/// Write a pixel to the framebuffer at (x, y)
/// Pixel format is RGB8 (3 bytes per pixel)
fn write_pixel(x: u32, y: u32, r: u8, g: u8, b: u8) {
    riscv_shared::write_pixel_rgb8(FRAMEBUFFER_BASE, WIDTH, x, y, r, g, b);
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
        let config = VideoConfig {
            width: WIDTH,
            height: HEIGHT,
            format: VideoFormat::Rgb8,
        };
        write_volatile(VIDEO_ADDR as *mut u32, FRAMEBUFFER_BASE);
        write_volatile(VIDEO_CONFIG as *mut u32, config.to_register());

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
