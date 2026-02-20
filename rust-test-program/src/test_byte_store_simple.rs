#![no_std]
#![no_main]

mod common;

#[global_allocator]
static HEAP: common::Heap = common::Heap::empty();

use core::panic::PanicInfo;
use riscv_rt::entry;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

// Static buffer for testing
static mut TEST_BUFFER: [u8; 16] = [0; 16];

#[entry]
fn main() -> ! {
    unsafe {
        let ptr = core::ptr::addr_of_mut!(TEST_BUFFER).cast::<u8>();

        // Write test pattern using ptr::write (which compiles to SB instructions)
        core::ptr::write(ptr.add(0), 0x11u8);
        core::ptr::write(ptr.add(1), 0x22u8);
        core::ptr::write(ptr.add(2), 0x33u8);
        core::ptr::write(ptr.add(3), 0x44u8);
        core::ptr::write(ptr.add(4), 0x55u8);
        core::ptr::write(ptr.add(5), 0x66u8);
        core::ptr::write(ptr.add(6), 0x77u8);
        core::ptr::write(ptr.add(7), 0x88u8);

        // Write marker
        common::fifo_write_word(0xAAAAAAAA).expect("Failed to write to FIFO");

        // Read back and write to FIFO
        for i in 0..8 {
            let byte = core::ptr::read(ptr.add(i));
            common::fifo_write_byte(byte).expect("Failed to write to FIFO");
        }

        // Write marker
        common::fifo_write_word(0xBBBBBBBB).expect("Failed to write to FIFO");
    }

    common::write_tohost(common::SUCCESS_CODE);
}
