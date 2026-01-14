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
    // Echo functionality: Read from RX FIFO and write to TX FIFO
    // Continue until we receive a null terminator word (0x00000000) or the FIFO is empty
    // Maximum of 50 iterations to prevent infinite loops
    for _ in 0..50 {
        // Try to read from FIFO - if empty, we're done
        let word = match common::fifo_read_word() {
            Ok(w) => w,
            Err(_) => break, // Both EmptyRead and FullWrite (though FullWrite shouldn't happen on read)
        };

        // If we read a null terminator, we're done
        if word == 0 {
            break;
        }

        // Echo non-zero words - TX should always be ready in simulation
        // but we handle the error case for completeness
        if common::fifo_write_word(word).is_err() {
            break;
        }
    }

    // Exit with success
    common::write_tohost(common::SUCCESS_CODE);
}
