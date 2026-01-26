#![no_std]
#![no_main]

#[cfg(feature = "protocol_macros")]
extern crate alloc;

mod common;

use core::panic::PanicInfo;
#[cfg(feature = "protocol_macros")]
use postcard::to_allocvec;
use riscv_rt::entry;
#[cfg(feature = "protocol_macros")]
use serde::Serialize;

#[cfg(feature = "protocol_macros")]
#[global_allocator]
static ALLOCATOR: common::SimpleAllocator = common::SimpleAllocator;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

#[cfg(feature = "protocol_macros")]
#[derive(Serialize)]
struct SimpleStruct {
    a: u32,
    b: u32,
}

#[cfg(feature = "protocol_macros")]
#[entry]
fn main() -> ! {
    let simple = SimpleStruct {
        a: 0x12345678,
        b: 0xABCDEF00,
    };

    if let Ok(bytes) = to_allocvec(&simple) {
        // Write the raw bytes vector length first as a marker
        common::fifo_write_word(bytes.len() as u32).expect("Failed to write length to FIFO");

        // Write each individual byte of the serialized data
        for &byte in bytes.iter() {
            common::fifo_write_word(byte as u32).expect("Failed to write byte to FIFO");
        }

        // Write a marker
        common::fifo_write_word(0xAAAAAAAA).expect("Failed to write marker to FIFO");

        // Now write using the chunking method
        for chunk in bytes.chunks(4) {
            let mut word: u32 = 0;
            for (i, &byte) in chunk.iter().enumerate() {
                word |= (byte as u32) << (i * 8);
            }
            common::fifo_write_word(word).expect("Failed to write word to FIFO");
        }
    }

    common::write_tohost(common::SUCCESS_CODE);
}

#[cfg(not(feature = "protocol_macros"))]
#[entry]
fn main() -> ! {
    // This binary requires the protocol_macros feature to be enabled
    // Skip test when feature is not enabled
    common::write_tohost(common::SUCCESS_CODE);
}
