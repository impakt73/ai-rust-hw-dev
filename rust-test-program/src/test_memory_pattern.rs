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

/// Memory address for test pattern
const TEST_MEMORY_BASE: u32 = 0x8000_1000;

/// Test pattern size (256 bytes)
const TEST_PATTERN_SIZE: usize = 256;

#[entry]
fn main() -> ! {
    unsafe {
        let ptr = TEST_MEMORY_BASE as *mut u8;

        // Write a recognizable test pattern:
        // Pattern: byte value equals its offset modulo 256
        // This creates a repeating pattern: 0x00, 0x01, 0x02, ..., 0xFF
        for i in 0..TEST_PATTERN_SIZE {
            write_volatile(ptr.add(i), i as u8);
        }

        // Write a marker pattern at the start to verify correct address
        // Magic bytes: 0xDE, 0xAD, 0xBE, 0xEF
        write_volatile(TEST_MEMORY_BASE as *mut u8, 0xDE);
        write_volatile((TEST_MEMORY_BASE + 1) as *mut u8, 0xAD);
        write_volatile((TEST_MEMORY_BASE + 2) as *mut u8, 0xBE);
        write_volatile((TEST_MEMORY_BASE + 3) as *mut u8, 0xEF);
    }

    // Signal successful completion
    common::write_tohost(42);
}
