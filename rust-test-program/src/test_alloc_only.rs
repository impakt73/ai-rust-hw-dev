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
        let layout = Layout::from_size_align(8, 1).unwrap();
        let ptr = ALLOCATOR.alloc(layout);
        
        // Write known pattern
        core::ptr::write(ptr.add(0), 0x12u8);
        core::ptr::write(ptr.add(1), 0x34u8);
        core::ptr::write(ptr.add(2), 0x56u8);
        core::ptr::write(ptr.add(3), 0x78u8);
        core::ptr::write(ptr.add(4), 0x9Au8);
        core::ptr::write(ptr.add(5), 0xBCu8);
        core::ptr::write(ptr.add(6), 0xDEu8);
        core::ptr::write(ptr.add(7), 0xF0u8);
        
        // Write marker
        let _ = common::fifo_write_word(0xAAAAAAAA);
        
        // Read back and write to FIFO
        for i in 0..8 {
            let byte = core::ptr::read(ptr.add(i));
            let _ = common::fifo_write_word(byte as u32);
        }
        
        // Write marker
        let _ = common::fifo_write_word(0xBBBBBBBB);
    }
    
    common::write_tohost(common::SUCCESS_CODE);
}
