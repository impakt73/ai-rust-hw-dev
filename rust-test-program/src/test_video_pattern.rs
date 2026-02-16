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

/// Write a pixel to the framebuffer at (x, y)
/// Pixel format is RGBA8 (4 bytes per pixel)
fn write_pixel(x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) {
    riscv_shared::write_pixel_rgba8(FRAMEBUFFER_BASE, WIDTH, x, y, r, g, b, a);
}

/// Render test pattern 1: Red/Green checkerboard
fn render_pattern_1() {
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            if (x + y) % 2 == 0 {
                write_pixel(x, y, 255, 0, 0, 255); // Red
            } else {
                write_pixel(x, y, 0, 255, 0, 255); // Green
            }
        }
    }
}

/// Render test pattern 2: Blue/Yellow diagonal stripes
fn render_pattern_2() {
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            if (x + y) % 16 < 8 {
                write_pixel(x, y, 0, 0, 255, 255); // Blue
            } else {
                write_pixel(x, y, 255, 255, 0, 255); // Yellow
            }
        }
    }
}

/// Render test pattern 3: Grayscale gradient
fn render_pattern_3() {
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let gray = ((x * 255) / WIDTH) as u8;
            write_pixel(x, y, gray, gray, gray, 255);
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
            format: VideoFormat::Rgba8,
        };
        write_volatile(VIDEO_ADDR as *mut u32, FRAMEBUFFER_BASE);
        write_volatile(VIDEO_CONFIG as *mut u32, config.to_register());

        // Frame 1: Red/Green checkerboard
        wait_for_frame_ready();
        render_pattern_1();
        wait_for_present_ready();
        trigger_present();

        // Frame 2: Blue/Yellow diagonal stripes
        wait_for_frame_ready();
        render_pattern_2();
        wait_for_present_ready();
        trigger_present();

        // Frame 3: Grayscale gradient
        wait_for_frame_ready();
        render_pattern_3();
        wait_for_present_ready();
        trigger_present();

        // Wait for final present to complete
        wait_for_present_ready();

        // Success!
        common::write_tohost(common::SUCCESS_CODE);
    }
}
