#![no_std]
#![no_main]

extern crate alloc;

mod common;

use alloc::vec::Vec;
use core::panic::PanicInfo;
use riscv_rt::entry;

#[global_allocator]
static ALLOCATOR: common::SimpleAllocator = common::SimpleAllocator;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

#[entry]
fn main() -> ! {
    // Create a Vec with known data to test if the allocator works correctly
    let mut v = Vec::new();
    v.push(0x12u8);
    v.push(0x34u8);
    v.push(0x56u8);
    v.push(0x78u8);
    v.push(0x9Au8);
    v.push(0xBCu8);
    v.push(0xDEu8);
    v.push(0xF0u8);
    
    // Write the vec length
    common::fifo_write_word(v.len() as u32);
    
    // Write each byte
    for &byte in v.iter() {
        common::fifo_write_word(byte as u32);
    }
    
    common::write_tohost(common::SUCCESS_CODE);
}
