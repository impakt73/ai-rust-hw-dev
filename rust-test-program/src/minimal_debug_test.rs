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
    let simple = SimpleStruct {
        a: 0x12345678,
        b: 0xABCDEF00,
    };

    if let Ok(bytes) = to_bytes::<Error>(&simple) {
        // Write the raw bytes vector length first as a marker
        let _ = common::fifo_write_word(bytes.len() as u32);

        // Write each individual byte of the serialized data
        for &byte in bytes.iter() {
            if common::fifo_write_word(byte as u32).is_err() {
                break;
            }
        }

        // Write a marker
        let _ = common::fifo_write_word(0xAAAAAAAA);

        // Now write using the chunking method
        for chunk in bytes.chunks(4) {
            let mut word: u32 = 0;
            for (i, &byte) in chunk.iter().enumerate() {
                word |= (byte as u32) << (i * 8);
            }
            if common::fifo_write_word(word).is_err() {
                break;
            }
        }
    }

    common::write_tohost(common::SUCCESS_CODE);
}
