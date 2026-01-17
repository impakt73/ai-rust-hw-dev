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
const VIDEO_BASE: u32 = 0x3000_0000;

/// Video register offsets
const VIDEO_ADDR: u32 = VIDEO_BASE + 0x00;
const VIDEO_CONFIG: u32 = VIDEO_BASE + 0x04;
const VIDEO_STATUS: u32 = VIDEO_BASE + 0x08;
const VIDEO_PRESENT: u32 = VIDEO_BASE + 0x0C;

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
        let offset = ((y * WIDTH + x) * 4) as u32;
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

/// Verify a pixel value at (x, y)
fn verify_pixel(x: u32, y: u32, expected_r: u8, expected_g: u8, expected_b: u8, expected_a: u8) -> bool {
    unsafe {
        let offset = ((y * WIDTH + x) * 4) as u32;
        let addr = FRAMEBUFFER_BASE + offset;
        let r = read_volatile(addr as *const u8);
        let g = read_volatile((addr + 1) as *const u8);
        let b = read_volatile((addr + 2) as *const u8);
        let a = read_volatile((addr + 3) as *const u8);
        
        r == expected_r && g == expected_g && b == expected_b && a == expected_a
    }
}

/// Verify test pattern 1 at specific test points
fn verify_pattern_1() -> bool {
    // Check corner pixels
    if !verify_pixel(0, 0, 255, 0, 0, 255) { return false; } // (0,0) even+even=even -> Red
    if !verify_pixel(1, 0, 0, 255, 0, 255) { return false; } // (1,0) odd+even=odd -> Green
    if !verify_pixel(0, 1, 0, 255, 0, 255) { return false; } // (0,1) even+odd=odd -> Green
    if !verify_pixel(1, 1, 255, 0, 0, 255) { return false; } // (1,1) odd+odd=even -> Red
    
    // Check some middle pixels
    if !verify_pixel(32, 32, 255, 0, 0, 255) { return false; } // even+even -> Red
    if !verify_pixel(33, 32, 0, 255, 0, 255) { return false; } // odd+even -> Green
    
    true
}

/// Verify test pattern 2 at specific test points
fn verify_pattern_2() -> bool {
    // Check pattern at (0, 0): (0+0) % 16 = 0 < 8 -> Blue
    if !verify_pixel(0, 0, 0, 0, 255, 255) { return false; }
    
    // Check pattern at (8, 0): (8+0) % 16 = 8 >= 8 -> Yellow
    if !verify_pixel(8, 0, 255, 255, 0, 255) { return false; }
    
    // Check pattern at (0, 8): (0+8) % 16 = 8 >= 8 -> Yellow
    if !verify_pixel(0, 8, 255, 255, 0, 255) { return false; }
    
    // Check pattern at (4, 4): (4+4) % 16 = 8 >= 8 -> Yellow
    if !verify_pixel(4, 4, 255, 255, 0, 255) { return false; }
    
    true
}

/// Verify test pattern 3 at specific test points
fn verify_pattern_3() -> bool {
    // Check gradient at x=0: gray = 0
    if !verify_pixel(0, 0, 0, 0, 0, 255) { return false; }
    
    // Check gradient at x=WIDTH-1: gray = 255
    let gray_max = (((WIDTH - 1) * 255) / WIDTH) as u8;
    if !verify_pixel(WIDTH - 1, 0, gray_max, gray_max, gray_max, 255) { return false; }
    
    // Check gradient at x=32: gray = (32 * 255) / 64 = 127.5 -> 127
    let gray_mid = ((32 * 255) / WIDTH) as u8;
    if !verify_pixel(32, 0, gray_mid, gray_mid, gray_mid, 255) { return false; }
    
    true
}

#[entry]
fn main() -> ! {
    unsafe {
        // Configure Video device
        write_volatile(VIDEO_ADDR as *mut u32, FRAMEBUFFER_BASE);
        write_volatile(VIDEO_CONFIG as *mut u32, make_video_config(WIDTH, HEIGHT, FORMAT_RGBA8));
        
        // Frame 1: Red/Green checkerboard
        wait_for_frame_ready();
        render_pattern_1();
        
        // Verify pattern 1 was written correctly
        if !verify_pattern_1() {
            common::write_tohost(common::FAILURE_CODE);
        }
        
        wait_for_present_ready();
        trigger_present();
        
        // Frame 2: Blue/Yellow diagonal stripes
        wait_for_frame_ready();
        render_pattern_2();
        
        // Verify pattern 2 was written correctly
        if !verify_pattern_2() {
            common::write_tohost(common::FAILURE_CODE);
        }
        
        wait_for_present_ready();
        trigger_present();
        
        // Frame 3: Grayscale gradient
        wait_for_frame_ready();
        render_pattern_3();
        
        // Verify pattern 3 was written correctly
        if !verify_pattern_3() {
            common::write_tohost(common::FAILURE_CODE);
        }
        
        wait_for_present_ready();
        trigger_present();
        
        // Wait for final present to complete
        wait_for_present_ready();
        
        // Success!
        common::write_tohost(common::SUCCESS_CODE);
    }
}
