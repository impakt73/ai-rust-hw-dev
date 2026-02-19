#![no_std]
#![no_main]

extern crate alloc;

mod common;

use core::panic::PanicInfo;
use riscv_rt::entry;
use rkyv::{rancor::Error, to_bytes, Archive, Serialize};

#[global_allocator]
static HEAP: common::Heap = common::Heap::empty();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

#[derive(Archive, Serialize)]
struct SimpleStruct {
    a: u32,
    b: u32,
}

#[entry]
fn main() -> ! {
    common::init_heap(&HEAP);
    // Test 1: Serialize a simple struct
    let simple = SimpleStruct {
        a: 0x12345678,
        b: 0xABCDEF00,
    };

    if let Ok(bytes) = to_bytes::<Error>(&simple) {
        // Write each byte individually to FIFO to see if duplication happens
        for &byte in bytes.iter() {
            if common::fifo_write_word(byte as u32).is_err() {
                break;
            }
        }
    }

    // Test 2: Write a known pattern
    let _ = common::fifo_write_word(0xDEADBEEF);

    common::write_tohost(common::SUCCESS_CODE);
}
