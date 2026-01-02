#![no_std]
#![no_main]

extern crate alloc;

mod common;

use core::panic::PanicInfo;
use riscv_rt::entry;

// Simple bump allocator
use core::alloc::{GlobalAlloc, Layout};

#[global_allocator]
static ALLOCATOR: common::SimpleAllocator = common::SimpleAllocator;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

#[entry]
fn main() -> ! {
    unsafe {
        // Allocate 4 bytes
        let layout = Layout::from_size_align(4, 1).unwrap();
        let ptr = ALLOCATOR.alloc(layout);
        
        // Write a single byte at offset 0
        core::ptr::write(ptr.add(0), 0xAAu8);
        
        // Read back all 4 bytes
        common::fifo_write_word(0x11111111);  // Marker
        for i in 0..4 {
            let byte = core::ptr::read(ptr.add(i));
            common::fifo_write_word(byte as u32);
        }
        common::fifo_write_word(0x22222222);  // Marker
        
        // Write a single byte at offset 1
        core::ptr::write(ptr.add(1), 0xBBu8);
        
        // Read back all 4 bytes
        common::fifo_write_word(0x33333333);  // Marker
        for i in 0..4 {
            let byte = core::ptr::read(ptr.add(i));
            common::fifo_write_word(byte as u32);
        }
        common::fifo_write_word(0x44444444);  // Marker
    }
    
    common::write_tohost(42);
}
