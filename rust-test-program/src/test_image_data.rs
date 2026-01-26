#![no_std]
#![no_main]

mod common;

#[global_allocator]
static ALLOCATOR: common::SimpleAllocator = common::SimpleAllocator;

use core::panic::PanicInfo;
use riscv_rt::entry;
use riscv_shared::write_pixel_rgba8;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

/// Memory address for test image
const TEST_IMAGE_BASE: u32 = 0x8000_2000;

/// Test image dimensions (small for easy verification)
const IMAGE_WIDTH: u32 = 4;

#[entry]
fn main() -> ! {
    // Create a simple 4x4 RGBA8 test image
    // Each pixel is 4 bytes: R, G, B, A
    // Pattern creates a gradient and test colors

    // Row 0: Red gradient (varying red, full opacity)
    write_pixel(0, 0, 255, 0, 0, 255); // Bright red
    write_pixel(1, 0, 192, 0, 0, 255); // Medium-bright red
    write_pixel(2, 0, 128, 0, 0, 255); // Medium red
    write_pixel(3, 0, 64, 0, 0, 255); // Dark red

    // Row 1: Green gradient
    write_pixel(0, 1, 0, 255, 0, 255); // Bright green
    write_pixel(1, 1, 0, 192, 0, 255); // Medium-bright green
    write_pixel(2, 1, 0, 128, 0, 255); // Medium green
    write_pixel(3, 1, 0, 64, 0, 255); // Dark green

    // Row 2: Blue gradient
    write_pixel(0, 2, 0, 0, 255, 255); // Bright blue
    write_pixel(1, 2, 0, 0, 192, 255); // Medium-bright blue
    write_pixel(2, 2, 0, 0, 128, 255); // Medium blue
    write_pixel(3, 2, 0, 0, 64, 255); // Dark blue

    // Row 3: Grayscale gradient
    write_pixel(0, 3, 255, 255, 255, 255); // White
    write_pixel(1, 3, 170, 170, 170, 255); // Light gray
    write_pixel(2, 3, 85, 85, 85, 255); // Dark gray
    write_pixel(3, 3, 0, 0, 0, 255); // Black

    // Signal successful completion
    common::write_tohost(common::SUCCESS_CODE);
}

/// Helper function to write a single RGBA pixel at (x, y) coordinates
#[inline(never)]
fn write_pixel(x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) {
    write_pixel_rgba8(TEST_IMAGE_BASE, IMAGE_WIDTH, x, y, r, g, b, a);
}
