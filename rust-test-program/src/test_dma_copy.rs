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
        let src_base = src.as_mut_ptr() as u32;
        let dst_base = dst.as_mut_ptr() as u32;

        // Step 1: Write test pattern to source array
        // Use a recognizable pattern: byte value = offset & 0xFF
        for i in 0..TEST_SIZE {
            let pattern_byte = i as u8;
            write_volatile((src_base + i as u32) as *mut u8, pattern_byte);
        }

        // Step 2: Clear destination array (write zeros)
        for i in 0..TEST_SIZE {
            write_volatile((dst_base + i as u32) as *mut u8, 0);
        }

        // Step 3: Configure and start DMA transfer
        start_transfer(src_base, dst_base, TEST_SIZE as u32);

        // Step 4: Wait for DMA to complete
        wait_for_completion();

        // Step 5: Verify destination data matches source
        let mut all_match = true;
        for i in 0..TEST_SIZE {
            let expected = i as u8;
            let actual = read_volatile((dst_base + i as u32) as *const u8);
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
