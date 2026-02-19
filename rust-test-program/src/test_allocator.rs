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
    let v = [
        0x12u8, 0x34u8, 0x56u8, 0x78u8, 0x9Au8, 0xBCu8, 0xDEu8, 0xF0u8,
    ];

    // Write the vec length
    if common::fifo_write_word(v.len() as u32).is_err() {
        common::write_tohost(common::FAILURE_CODE);
    }

    // Write each byte
    for &byte in v.iter() {
        if common::fifo_write_word(byte as u32).is_err() {
            common::write_tohost(common::FAILURE_CODE);
        }
    }

    common::write_tohost(common::SUCCESS_CODE);
}
