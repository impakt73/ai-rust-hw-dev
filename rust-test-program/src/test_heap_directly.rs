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
    // Test 1: Write directly to allocated memory - ONE BYTE AT A TIME
    unsafe {
        let layout = Layout::from_size_align(8, 1).unwrap();
        let ptr = ALLOCATOR.alloc(layout);
        
        // Send the pointer address to FIFO for debugging
        common::fifo_write_word(0xDEADBEEF);  // Debug marker
        common::fifo_write_word(ptr as u32);  // Pointer address
        
        // Write first byte
        core::ptr::write(ptr.add(0), 0x12u8);
        // Read it back immediately
        let val0 = core::ptr::read(ptr.add(0));
        common::fifo_write_word(val0 as u32);
        
        // Write second byte
        core::ptr::write(ptr.add(1), 0x34u8);
        // Read both bytes back
        let val0_after = core::ptr::read(ptr.add(0));
        let val1 = core::ptr::read(ptr.add(1));
        common::fifo_write_word(val0_after as u32);
        common::fifo_write_word(val1 as u32);
        
        // Write third byte
        core::ptr::write(ptr.add(2), 0x56u8);
        // Read all three bytes back
        let val0_after2 = core::ptr::read(ptr.add(0));
        let val1_after = core::ptr::read(ptr.add(1));
        let val2 = core::ptr::read(ptr.add(2));
        common::fifo_write_word(val0_after2 as u32);
        common::fifo_write_word(val1_after as u32);
        common::fifo_write_word(val2 as u32);
        
        // Now write the remaining bytes
        core::ptr::write(ptr.add(3), 0x78u8);
        core::ptr::write(ptr.add(4), 0x9Au8);
        core::ptr::write(ptr.add(5), 0xBCu8);
        core::ptr::write(ptr.add(6), 0xDEu8);
        core::ptr::write(ptr.add(7), 0xF0u8);
        
        // Write marker
        common::fifo_write_word(0xAAAAAAAA);
        
        // Read back all and write to FIFO
        for i in 0..8 {
            let byte = core::ptr::read(ptr.add(i));
            common::fifo_write_word(byte as u32);
        }
        
        // Write marker
        common::fifo_write_word(0xBBBBBBBB);
    }
    
    common::write_tohost(42);
}
