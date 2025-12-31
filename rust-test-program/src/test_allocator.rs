#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;
use core::panic::PanicInfo;
use core::ptr::write_volatile;
use riscv_rt::entry;

// Simple bump allocator
use core::alloc::{GlobalAlloc, Layout};
use core::ptr::addr_of_mut;
use core::sync::atomic::{AtomicUsize, Ordering};

#[global_allocator]
static ALLOCATOR: SimpleAllocator = SimpleAllocator;

struct SimpleAllocator;

unsafe impl GlobalAlloc for SimpleAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        static mut HEAP: [u8; 8192] = [0; 8192];
        static OFFSET: AtomicUsize = AtomicUsize::new(0);

        let size = layout.size();
        let align = layout.align();
        let current_offset = OFFSET.load(Ordering::Relaxed);
        let aligned_offset = (current_offset + align - 1) & !(align - 1);

        if aligned_offset + size > 8192 {
            core::ptr::null_mut()
        } else {
            let ptr = addr_of_mut!(HEAP).cast::<u8>().add(aligned_offset);
            OFFSET.store(aligned_offset + size, Ordering::Relaxed);
            ptr
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

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
    unsafe {
        write_volatile(FIFO_DATA as *mut u32, v.len() as u32);
    }
    
    // Write each byte
    for &byte in v.iter() {
        unsafe {
            write_volatile(FIFO_DATA as *mut u32, byte as u32);
        }
    }
    
    write_tohost(42);
}
