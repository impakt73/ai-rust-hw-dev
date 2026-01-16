#![no_std]
#![no_main]

mod common;

use core::panic::PanicInfo;
use riscv_rt::entry;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

#[entry]
fn main() -> ! {
    // Test with stack memory (array)
    let mut stack_array: [u8; 8] = [0; 8];
    
    // Write to stack array using ptr::write
    unsafe {
        let ptr = stack_array.as_mut_ptr();
        core::ptr::write(ptr.add(0), 0x12u8);
        core::ptr::write(ptr.add(1), 0x34u8);
        core::ptr::write(ptr.add(2), 0x56u8);
        core::ptr::write(ptr.add(3), 0x78u8);
        core::ptr::write(ptr.add(4), 0x9Au8);
        core::ptr::write(ptr.add(5), 0xBCu8);
        core::ptr::write(ptr.add(6), 0xDEu8);
        core::ptr::write(ptr.add(7), 0xF0u8);
    }
    
    // Write marker
    common::fifo_write_word(0xAAAAAAAA).expect("Failed to write to FIFO");
    
    // Read and send to FIFO
    for i in 0..8 {
        let byte = stack_array[i];
        common::fifo_write_word(byte as u32).expect("Failed to write to FIFO");
    }
    
    // Write marker
    common::fifo_write_word(0xBBBBBBBB).expect("Failed to write to FIFO");
    
    // Test 2: Direct assignment
    let stack_array2: [u8; 8] = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
    
    // Write marker
    common::fifo_write_word(0xCCCCCCCC).expect("Failed to write to FIFO");
    
    // Read and send to FIFO
    for &byte in &stack_array2 {
        common::fifo_write_word(byte as u32).expect("Failed to write to FIFO");
    }
    
    common::write_tohost(common::SUCCESS_CODE);
}
