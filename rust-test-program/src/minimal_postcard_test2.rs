#![no_std]
#![no_main]

extern crate alloc;

mod common;

use core::panic::PanicInfo;
use riscv_rt::entry;
use postcard::to_allocvec;
use serde::Serialize;

#[global_allocator]
static ALLOCATOR: common::SimpleAllocator = common::SimpleAllocator;

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
    // Test 1: Serialize a simple struct and write it like packet_test.rs does
    let simple = SimpleStruct { a: 0x12345678, b: 0xABCDEF00 };
    
    if let Ok(bytes) = to_allocvec(&simple) {
        // Write bytes in chunks of 4, just like packet_test.rs
        for chunk in bytes.chunks(4) {
            let mut word: u32 = 0;
            for (i, &byte) in chunk.iter().enumerate() {
                word |= (byte as u32) << (i * 8);
            }
            let _ = common::fifo_write_word(word);
        }
    }
    
    // Write a known pattern to mark the end
    let _ = common::fifo_write_word(0xDEADBEEF);
    
    common::write_tohost(common::SUCCESS_CODE);
}
