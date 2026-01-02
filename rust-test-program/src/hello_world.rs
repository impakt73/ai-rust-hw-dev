#![no_std]
#![no_main]

mod common;

use core::panic::PanicInfo;
use riscv_rt::entry;

/// Panic handler for bare metal - infinite loop on panic
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
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
        let word = common::fifo_read_word_unchecked();

        // If we read a null terminator, we're done
        if word == 0 {
            break;
        }

        // Echo non-zero words
        common::fifo_write_word(word);
    }

    // Exit with success
    common::write_tohost(SUCCESS_CODE);
}
