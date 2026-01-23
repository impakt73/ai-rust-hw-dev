#![no_std]
#![no_main]

mod common;

use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};
use riscv_rt::entry;
use riscv_shared::{VIDEO_ADDR, VIDEO_CONFIG, VIDEO_PRESENT, VIDEO_STATUS};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

/// Video status bits
const FRAME_READY: u32 = 1 << 0;
const PRESENT_READY: u32 = 1 << 1;

/// Video formats
const FORMAT_RGBA8: u32 = 0;

/// Test image dimensions
const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;

/// Framebuffer base address in DRAM
const FRAMEBUFFER_BASE: u32 = 0x8000_1000;

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
/// Pixel format is RGBA8 (4 bytes per pixel)
fn write_pixel(x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) {
    unsafe {
        let offset = (y * WIDTH + x) * 4;
        let addr = FRAMEBUFFER_BASE + offset;
        write_volatile(addr as *mut u8, r);
        write_volatile((addr + 1) as *mut u8, g);
        write_volatile((addr + 2) as *mut u8, b);
        write_volatile((addr + 3) as *mut u8, a);
    }
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
        write_volatile(VIDEO_ADDR as *mut u32, FRAMEBUFFER_BASE);
        write_volatile(
            VIDEO_CONFIG as *mut u32,
            make_video_config(WIDTH, HEIGHT, FORMAT_RGBA8),
        );

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
