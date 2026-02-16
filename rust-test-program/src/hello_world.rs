#![no_std]
#![no_main]

mod common;

#[global_allocator]
static HEAP: common::Heap = common::Heap::empty();

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
    while let Ok(word) = common::fifo_read_word() {
        // If we read a null terminator, we're done
        if word == 0 {
            break;
        }

        // Echo non-zero words - TX should always be ready in simulation
        common::fifo_write_word(word).expect("FIFO write failed");
    }

    // Exit with success
    common::write_tohost(common::SUCCESS_CODE);
}
