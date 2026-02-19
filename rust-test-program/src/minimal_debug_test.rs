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
    for &word in &[0x1234_5678, 0xABCD_EF00, 0xAAAA_AAAA] {
        if common::fifo_write_word(word).is_err() {
            common::write_tohost(common::FAILURE_CODE);
        }
    }

    common::write_tohost(common::SUCCESS_CODE);
}
