#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};
use riscv_rt::entry;

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
const FIFO_BASE: u32 = 0x4000_0000;
const FIFO_DATA: u32 = FIFO_BASE + 0x0;

/// Read a word from the FIFO (without status check - just read and return)
#[inline(never)]
fn fifo_read_word_unchecked() -> u32 {
    unsafe { read_volatile(FIFO_DATA as *const u32) }
}

/// Write a word to the FIFO
#[inline(never)]
fn fifo_write_word(word: u32) {
    unsafe {
        // TX is always ready in simulation, so just write
        write_volatile(FIFO_DATA as *mut u32, word);
    }
}

/// Entry point for the bare metal Rust program
/// Uses riscv_rt which properly initializes stack pointer
#[entry]
fn main() -> ! {
    const SUCCESS_CODE: u32 = 42;

    // Echo functionality: Read from RX FIFO and write to TX FIFO
    // Continue until we receive a null terminator word (0x00000000)
    // Maximum of 50 iterations to prevent infinite loops
    for _ in 0..50 {
        let word = fifo_read_word_unchecked();
        
        // If we read a null terminator, we're done
        if word == 0 {
            break;
        }
        
        // Echo non-zero words
        fifo_write_word(word);
    }

    // Exit with success
    write_tohost(SUCCESS_CODE);
}
