#![no_std]
#![no_main]

mod common;

#[global_allocator]
static HEAP: common::Heap = common::Heap::empty();

use core::panic::PanicInfo;
use core::ptr::addr_of_mut;
use riscv_rt::entry;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

// Test direct access to static mut array using .uninit section to avoid BSS zero-initialization
#[link_section = ".uninit"]
static mut STATIC_HEAP: [u8; 8192] = [0; 8192];

#[entry]
fn main() -> ! {
    // Test 1: Write directly to static mut HEAP using ptr::write
    unsafe {
        let ptr = addr_of_mut!(STATIC_HEAP).cast::<u8>();

        core::ptr::write(ptr.add(0), 0x12u8);
        core::ptr::write(ptr.add(1), 0x34u8);
        core::ptr::write(ptr.add(2), 0x56u8);
        core::ptr::write(ptr.add(3), 0x78u8);
        core::ptr::write(ptr.add(4), 0x9Au8);
        core::ptr::write(ptr.add(5), 0xBCu8);
        core::ptr::write(ptr.add(6), 0xDEu8);
        core::ptr::write(ptr.add(7), 0xF0u8);

        // Write marker
        common::fifo_write_word(0xAAAAAAAA).expect("Failed to write to FIFO");

        // Read back
        for i in 0..8 {
            let byte = core::ptr::read(ptr.add(i));
            common::fifo_write_word(byte as u32).expect("Failed to write to FIFO");
        }
    }

    // Test 2: Write to HEAP using pointer arithmetic
    unsafe {
        let ptr = addr_of_mut!(STATIC_HEAP).cast::<u8>();
        core::ptr::write(ptr.add(10), 0xAAu8);
        core::ptr::write(ptr.add(11), 0xBBu8);
        core::ptr::write(ptr.add(12), 0xCCu8);
        core::ptr::write(ptr.add(13), 0xDDu8);

        // Write marker
        common::fifo_write_word(0xBBBBBBBB).expect("Failed to write to FIFO");

        // Read back using pointer arithmetic
        for i in 10..14 {
            let byte = core::ptr::read(ptr.add(i));
            common::fifo_write_word(byte as u32).expect("Failed to write to FIFO");
        }
    }

    common::write_tohost(common::SUCCESS_CODE);
}
