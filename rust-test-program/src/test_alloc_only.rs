#![no_std]
#![no_main]

extern crate alloc;

mod common;

use core::panic::PanicInfo;
use riscv_rt::entry;

// Use the shared embedded allocator for allocation-only validation.
use core::alloc::{GlobalAlloc, Layout};

#[global_allocator]
static HEAP: common::Heap = common::Heap::empty();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

#[entry]
fn main() -> ! {
    common::init_heap(&HEAP);
    unsafe {
        let layout = Layout::from_size_align(8, 1).unwrap();
        let ptr = HEAP.alloc(layout);

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
        common::fifo_write_word(0xAAAAAAAA).expect("Failed to write to FIFO");

        // Read back and write to FIFO
        for i in 0..8 {
            let byte = core::ptr::read(ptr.add(i));
            common::fifo_write_word(byte as u32).expect("Failed to write to FIFO");
        }

        // Write marker
        common::fifo_write_word(0xBBBBBBBB).expect("Failed to write to FIFO");
    }

    common::write_tohost(common::SUCCESS_CODE);
}
