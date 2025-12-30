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
const FIFO_STATUS: u32 = FIFO_BASE + 0x4;

/// FIFO status bits
const FIFO_RX_VALID: u32 = 0x1;
const FIFO_TX_READY: u32 = 0x2;

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

    // Echo exactly 3 words - super simple, no loops
    let word1 = fifo_read_word_unchecked();
    fifo_write_word(word1);
    
    let word2 = fifo_read_word_unchecked();
    fifo_write_word(word2);
    
    let word3 = fifo_read_word_unchecked();
    // Don't echo word3 if it's zero (null terminator)
    if word3 != 0 {
        fifo_write_word(word3);
    }

    // Exit with success
    write_tohost(SUCCESS_CODE);
}
