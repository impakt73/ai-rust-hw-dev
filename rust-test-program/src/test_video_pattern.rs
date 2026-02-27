#![no_std]
#![no_main]

extern crate alloc;

mod common;

#[global_allocator]
static HEAP: common::Heap = common::Heap::empty();

use alloc::vec;
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
const WIDTH: u32 = 16;
const HEIGHT: u32 = 16;

/// Write a pixel to the framebuffer at (x, y)
/// Pixel format is RGBA8 (4 bytes per pixel)
fn write_pixel(framebuffer: &mut [u32], x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) {
    riscv_shared::write_pixel_rgba8(framebuffer.as_mut_ptr() as u32, WIDTH, x, y, r, g, b, a);
}

/// Render test pattern 1: Red/Green checkerboard
fn render_pattern_1(framebuffer: &mut [u32]) {
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            if (x + y) % 2 == 0 {
                write_pixel(framebuffer, x, y, 255, 0, 0, 255); // Red
            } else {
                write_pixel(framebuffer, x, y, 0, 255, 0, 255); // Green
            }
        }
    }
}

/// Render test pattern 2: Blue/Yellow diagonal stripes
fn render_pattern_2(framebuffer: &mut [u32]) {
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            if (x + y) % 16 < 8 {
                write_pixel(framebuffer, x, y, 0, 0, 255, 255); // Blue
            } else {
                write_pixel(framebuffer, x, y, 255, 255, 0, 255); // Yellow
            }
        }
    }
}

/// Render test pattern 3: Grayscale gradient
fn render_pattern_3(framebuffer: &mut [u32]) {
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let gray = ((x * 255) / WIDTH) as u8;
            write_pixel(framebuffer, x, y, gray, gray, gray, 255);
        }
    }
}

fn configure_video_buffer(framebuffer: &mut [u32], config: VideoConfig) {
    unsafe {
        write_volatile(VIDEO_ADDR as *mut u32, framebuffer.as_mut_ptr() as u32);
        write_volatile(VIDEO_CONFIG as *mut u32, config.to_register());
    }
}

#[entry]
fn main() -> ! {
    common::init_heap(&HEAP);
    let mut framebuffer = vec![0u32; (WIDTH * HEIGHT) as usize];

    // Configure Video device
    let config = VideoConfig {
        width: WIDTH,
        height: HEIGHT,
        format: VideoFormat::Rgba8,
    };
    configure_video_buffer(framebuffer.as_mut_slice(), config);

    // Frame 1: Red/Green checkerboard
    wait_for_frame_ready();
    render_pattern_1(framebuffer.as_mut_slice());
    wait_for_present_ready();
    trigger_present();

    // Frame 2: Blue/Yellow diagonal stripes
    wait_for_frame_ready();
    render_pattern_2(framebuffer.as_mut_slice());
    wait_for_present_ready();
    trigger_present();

    // Frame 3: Grayscale gradient
    wait_for_frame_ready();
    render_pattern_3(framebuffer.as_mut_slice());
    wait_for_present_ready();
    trigger_present();

    // Wait for final present to complete
    wait_for_present_ready();

    // Success!
    common::write_tohost(common::SUCCESS_CODE);
}
