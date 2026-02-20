#![no_std]
#![no_main]

extern crate alloc;

mod common;

use core::panic::PanicInfo;
use postcard::to_allocvec;
use riscv_rt::entry;
use serde::Serialize;

#[global_allocator]
static HEAP: common::Heap = common::Heap::empty();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

#[derive(Serialize)]
struct SimpleStruct {
    a: u32,
    b: u32,
}

#[entry]
fn main() -> ! {
    common::init_heap(&HEAP);
    // Test 1: Serialize a simple struct and write the raw serialized bytes
    let simple = SimpleStruct {
        a: 0x12345678,
        b: 0xABCDEF00,
    };

    if let Ok(bytes) = to_allocvec(&simple) {
        // Write bytes directly
        for &byte in &bytes {
            common::fifo_write_byte(byte).expect("Failed to write to FIFO");
        }
    }

    // Write a known pattern to mark the end
    for byte in 0xDEAD_BEEFu32.to_le_bytes() {
        common::fifo_write_byte(byte).expect("Failed to write to FIFO");
    }

    common::write_tohost(common::SUCCESS_CODE);
}
