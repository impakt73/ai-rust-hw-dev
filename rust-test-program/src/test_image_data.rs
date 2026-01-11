#![no_std]
#![no_main]

mod common;

use core::panic::PanicInfo;
use core::ptr::write_volatile;
use riscv_rt::entry;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

/// Memory address for test image
const TEST_IMAGE_BASE: u32 = 0x8000_2000;

/// Test image dimensions (small for easy verification)
const IMAGE_WIDTH: usize = 4;
const IMAGE_HEIGHT: usize = 4;

#[entry]
fn main() -> ! {
    unsafe {
        let ptr = TEST_IMAGE_BASE as *mut u8;

        // Create a simple 4x4 RGBA8 test image
        // Each pixel is 4 bytes: R, G, B, A
        // Pattern creates a gradient and test colors
        
        // Row 0: Red gradient (varying red, full opacity)
        write_pixel(ptr, 0, 0, 255, 0, 0, 255);      // Bright red
        write_pixel(ptr, 1, 0, 192, 0, 0, 255);      // Medium-bright red
        write_pixel(ptr, 2, 0, 128, 0, 0, 255);      // Medium red
        write_pixel(ptr, 3, 0, 64, 0, 0, 255);       // Dark red

        // Row 1: Green gradient
        write_pixel(ptr, 0, 1, 0, 255, 0, 255);      // Bright green
        write_pixel(ptr, 1, 1, 0, 192, 0, 255);      // Medium-bright green
        write_pixel(ptr, 2, 1, 0, 128, 0, 255);      // Medium green
        write_pixel(ptr, 3, 1, 0, 64, 0, 255);       // Dark green

        // Row 2: Blue gradient
        write_pixel(ptr, 0, 2, 0, 0, 255, 255);      // Bright blue
        write_pixel(ptr, 1, 2, 0, 0, 192, 255);      // Medium-bright blue
        write_pixel(ptr, 2, 2, 0, 0, 128, 255);      // Medium blue
        write_pixel(ptr, 3, 2, 0, 0, 64, 255);       // Dark blue

        // Row 3: Grayscale gradient
        write_pixel(ptr, 0, 3, 255, 255, 255, 255);  // White
        write_pixel(ptr, 1, 3, 170, 170, 170, 255);  // Light gray
        write_pixel(ptr, 2, 3, 85, 85, 85, 255);     // Dark gray
        write_pixel(ptr, 3, 3, 0, 0, 0, 255);        // Black
    }

    // Signal successful completion
    common::write_tohost(common::SUCCESS_CODE);
}

/// Helper function to write a single RGBA pixel at (x, y) coordinates
#[inline(never)]
unsafe fn write_pixel(base_ptr: *mut u8, x: usize, y: usize, r: u8, g: u8, b: u8, a: u8) {
    // Verify coordinates are within bounds (compiler will optimize this away in release)
    debug_assert!(x < IMAGE_WIDTH && y < IMAGE_HEIGHT);
    
    let offset = (y * IMAGE_WIDTH + x) * 4;
    let pixel_ptr = base_ptr.add(offset);
    
    write_volatile(pixel_ptr, r);
    write_volatile(pixel_ptr.add(1), g);
    write_volatile(pixel_ptr.add(2), b);
    write_volatile(pixel_ptr.add(3), a);
}
