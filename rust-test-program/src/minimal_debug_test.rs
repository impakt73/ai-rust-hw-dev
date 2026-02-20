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
    let simple = SimpleStruct {
        a: 0x12345678,
        b: 0xABCDEF00,
    };

    if let Ok(bytes) = to_allocvec(&simple) {
        // Write the raw bytes vector length first as a marker
        common::fifo_write_byte(bytes.len() as u8).expect("Failed to write length to FIFO");

        // Write each individual byte of the serialized data
        for &byte in bytes.iter() {
            common::fifo_write_byte(byte).expect("Failed to write byte to FIFO");
        }

        // Write a marker
        for byte in 0xAAAA_AAAAu32.to_le_bytes() {
            common::fifo_write_byte(byte).expect("Failed to write marker to FIFO");
        }

        // Write the same data again for comparison
        for &byte in bytes.iter() {
            common::fifo_write_byte(byte).expect("Failed to write byte to FIFO");
        }
    }

    common::write_tohost(common::SUCCESS_CODE);
}
