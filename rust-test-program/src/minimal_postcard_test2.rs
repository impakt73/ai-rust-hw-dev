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
    // Write words directly in packed 4-byte style
    for &word in &[0x7856_3412, 0x00EF_CDAB] {
        if common::fifo_write_word(word).is_err() {
            common::write_tohost(common::FAILURE_CODE);
        }
    }

    // Write a known pattern to mark the end
    if common::fifo_write_word(0xDEAD_BEEF).is_err() {
        common::write_tohost(common::FAILURE_CODE);
    }

    common::write_tohost(common::SUCCESS_CODE);
}
