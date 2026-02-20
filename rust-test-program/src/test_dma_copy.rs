#![no_std]
#![no_main]

extern crate alloc;

mod common;

#[global_allocator]
static HEAP: common::Heap = common::Heap::empty();

use alloc::vec;
use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};
use riscv_rt::entry;
use riscv_shared::dma::{start_transfer, wait_for_completion};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

/// Test pattern size (64 bytes)
const TEST_SIZE: usize = 64;

#[entry]
fn main() -> ! {
    unsafe {
        common::init_heap(&HEAP);
        let mut src = vec![0u8; TEST_SIZE];
        let mut dst = vec![0u8; TEST_SIZE];
        let src_slice = src.as_mut_slice();
        let dst_slice = dst.as_mut_slice();

        // Step 1: Write test pattern to source array
        // Use a recognizable pattern: byte value = offset & 0xFF
        for i in 0..TEST_SIZE {
            let pattern_byte = i as u8;
            write_volatile(src_slice.as_mut_ptr().add(i), pattern_byte);
        }

        // Step 2: Clear destination array (write zeros)
        for i in 0..TEST_SIZE {
            write_volatile(dst_slice.as_mut_ptr().add(i), 0);
        }

        // Step 3: Configure and start DMA transfer
        start_transfer(
            src_slice.as_mut_ptr() as u32,
            dst_slice.as_mut_ptr() as u32,
            TEST_SIZE as u32,
        );

        // Step 4: Wait for DMA to complete
        wait_for_completion();

        // Step 5: Verify destination data matches source
        let mut all_match = true;
        for i in 0..TEST_SIZE {
            let expected = i as u8;
            let actual = read_volatile(dst_slice.as_ptr().add(i));
            if actual != expected {
                all_match = false;
                break;
            }
        }

        // Step 6: Return result via tohost
        if all_match {
            common::write_tohost(common::SUCCESS_CODE);
        } else {
            common::write_tohost(common::FAILURE_CODE);
        }
    }
}
