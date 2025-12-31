#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr::{write_volatile, addr_of_mut};
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

// Test direct access to static mut array
static mut HEAP: [u8; 8192] = [0; 8192];

#[entry]
fn main() -> ! {
    // Test 1: Write directly to static mut HEAP using ptr::write
    unsafe {
        let ptr = addr_of_mut!(HEAP).cast::<u8>();
        
        core::ptr::write(ptr.add(0), 0x12u8);
        core::ptr::write(ptr.add(1), 0x34u8);
        core::ptr::write(ptr.add(2), 0x56u8);
        core::ptr::write(ptr.add(3), 0x78u8);
        core::ptr::write(ptr.add(4), 0x9Au8);
        core::ptr::write(ptr.add(5), 0xBCu8);
        core::ptr::write(ptr.add(6), 0xDEu8);
        core::ptr::write(ptr.add(7), 0xF0u8);
        
        // Write marker
        write_volatile(FIFO_DATA as *mut u32, 0xAAAAAAAA);
        
        // Read back
        for i in 0..8 {
            let byte = core::ptr::read(ptr.add(i));
            write_volatile(FIFO_DATA as *mut u32, byte as u32);
        }
    }
    
    // Test 2: Write to HEAP using index notation
    unsafe {
        HEAP[10] = 0xAAu8;
        HEAP[11] = 0xBBu8;
        HEAP[12] = 0xCCu8;
        HEAP[13] = 0xDDu8;
        
        // Write marker
        write_volatile(FIFO_DATA as *mut u32, 0xBBBBBBBB);
        
        // Read back
        for i in 10..14 {
            write_volatile(FIFO_DATA as *mut u32, HEAP[i] as u32);
        }
    }
    
    write_tohost(42);
}
