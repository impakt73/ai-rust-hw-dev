#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr::write_volatile;
use riscv_rt::entry;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

const FIFO_DATA: u32 = 0x4000_0000;
const TOHOST_ADDR: u32 = 0xFFFF_FFF0;

fn write_tohost(value: u32) -> ! {
    unsafe {
        write_volatile(TOHOST_ADDR as *mut u32, value);
    }
    loop {}
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
    unsafe {
        write_volatile(FIFO_DATA as *mut u32, 0xAAAAAAAA);
    }
    
    // Read and send to FIFO
    for i in 0..8 {
        let byte = stack_array[i];
        unsafe {
            write_volatile(FIFO_DATA as *mut u32, byte as u32);
        }
    }
    
    // Write marker
    unsafe {
        write_volatile(FIFO_DATA as *mut u32, 0xBBBBBBBB);
    }
    
    // Test 2: Direct assignment
    let stack_array2: [u8; 8] = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
    
    // Write marker
    unsafe {
        write_volatile(FIFO_DATA as *mut u32, 0xCCCCCCCC);
    }
    
    // Read and send to FIFO
    for &byte in &stack_array2 {
        unsafe {
            write_volatile(FIFO_DATA as *mut u32, byte as u32);
        }
    }
    
    write_tohost(42);
}
