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
    // Test 1: Write directly to allocated memory
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
        write_volatile(FIFO_DATA as *mut u32, 0xAAAAAAAA);
        
        // Read back and write to FIFO
        for i in 0..8 {
            let byte = core::ptr::read(ptr.add(i));
            write_volatile(FIFO_DATA as *mut u32, byte as u32);
        }
        
        // Write marker
        write_volatile(FIFO_DATA as *mut u32, 0xBBBBBBBB);
    }
    
    // Test 2: Use Vec::with_capacity and set_len
    unsafe {
        let mut v = Vec::with_capacity(8);
        let ptr: *mut u8 = v.as_mut_ptr();
        
        // Write directly to the Vec's buffer
        core::ptr::write(ptr.add(0), 0xAAu8);
        core::ptr::write(ptr.add(1), 0xBBu8);
        core::ptr::write(ptr.add(2), 0xCCu8);
        core::ptr::write(ptr.add(3), 0xDDu8);
        
        v.set_len(4);
        
        // Write marker
        write_volatile(FIFO_DATA as *mut u32, 0xCCCCCCCC);
        
        // Read through Vec API
        for &byte in v.iter() {
            write_volatile(FIFO_DATA as *mut u32, byte as u32);
        }
    }
    
    write_tohost(42);
}
