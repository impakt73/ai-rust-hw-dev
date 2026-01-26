#![no_std]
#![no_main]

mod common;

#[global_allocator]
static ALLOCATOR: common::SimpleAllocator = common::SimpleAllocator;

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

/// Source array base address in DRAM
const SRC_BASE: u32 = 0x8000_1000;

/// Destination array base address in DRAM
const DST_BASE: u32 = 0x8000_2000;

#[entry]
fn main() -> ! {
    unsafe {
        // Step 1: Write test pattern to source array
        // Use a recognizable pattern: byte value = offset & 0xFF
        for i in 0..TEST_SIZE {
            let pattern_byte = i as u8;
            write_volatile((SRC_BASE + i as u32) as *mut u8, pattern_byte);
        }

        // Step 2: Clear destination array (write zeros)
        for i in 0..TEST_SIZE {
            write_volatile((DST_BASE + i as u32) as *mut u8, 0);
        }

        // Step 3: Configure and start DMA transfer
        start_transfer(SRC_BASE, DST_BASE, TEST_SIZE as u32);

        // Step 4: Wait for DMA to complete
        wait_for_completion();

        // Step 5: Verify destination data matches source
        let mut all_match = true;
        for i in 0..TEST_SIZE {
            let expected = i as u8;
            let actual = read_volatile((DST_BASE + i as u32) as *const u8);
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
