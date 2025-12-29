#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr::write_volatile;

/// Panic handler for bare metal - infinite loop on panic
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

/// Write to tohost address to signal completion
#[inline(never)]
fn write_tohost(value: u32) -> ! {
    const TOHOST_ADDR: u32 = 0xFFFF_FFF0;
    unsafe {
        write_volatile(TOHOST_ADDR as *mut u32, value);
    }
    loop {}
}

/// FIFO memory-mapped I/O addresses
const FIFO_BASE: u32 = 0xF0000000;
const FIFO_DATA: u32 = FIFO_BASE + 0x0;
const FIFO_STATUS: u32 = FIFO_BASE + 0x4;

/// FIFO status bits
#[allow(dead_code)]
const FIFO_EMPTY: u32 = 0x1;
const FIFO_FULL: u32 = 0x2;

/// Write a byte to the FIFO
#[inline(never)]
fn fifo_write_byte(byte: u8) {
    unsafe {
        // Wait for FIFO to not be full
        loop {
            let status = core::ptr::read_volatile(FIFO_STATUS as *const u32);
            if (status & FIFO_FULL) == 0 {
                break;
            }
        }
        
        // Write byte to FIFO
        write_volatile(FIFO_DATA as *mut u32, byte as u32);
    }
}

/// Write a string to the FIFO
#[inline(never)]
fn fifo_write_string(s: &str) {
    for byte in s.bytes() {
        fifo_write_byte(byte);
    }
}

/// Entry point for the bare metal Rust program
#[no_mangle]
#[link_section = ".text"]
pub extern "C" fn _start() -> ! {
    const SUCCESS_CODE: u32 = 42;

    // Write "Hello World!" to the FIFO
    fifo_write_string("Hello World!");

    // Exit with success
    write_tohost(SUCCESS_CODE);
}
