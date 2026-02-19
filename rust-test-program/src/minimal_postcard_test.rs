#![no_std]
#![no_main]

extern crate alloc;

mod common;

use core::panic::PanicInfo;
use riscv_rt::entry;

#[global_allocator]
static HEAP: common::Heap = common::Heap::empty();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

#[entry]
fn main() -> ! {
    common::init_heap(&HEAP);
    // Write a compact deterministic pattern
    for &word in &[0x12, 0x34, 0x56, 0x78] {
        if common::fifo_write_word(word).is_err() {
            common::write_tohost(common::FAILURE_CODE);
        }
    }

    // Test 2: Write a known pattern
    if common::fifo_write_word(0xDEAD_BEEF).is_err() {
        common::write_tohost(common::FAILURE_CODE);
    }

    common::write_tohost(common::SUCCESS_CODE);
}
