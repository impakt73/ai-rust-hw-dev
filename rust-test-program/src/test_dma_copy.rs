#![no_std]
#![no_main]

mod common;

use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};
use riscv_rt::entry;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

/// DMA device base address
const DMA_BASE: u32 = 0x2000_0000;

/// DMA register offsets
const DMA_SRC_ADDR: u32 = DMA_BASE;
const DMA_DST_ADDR: u32 = DMA_BASE + 0x04;
const DMA_SIZE: u32 = DMA_BASE + 0x08;
const DMA_STATUS: u32 = DMA_BASE + 0x0C;
const DMA_DISPATCH: u32 = DMA_BASE + 0x10;

/// DMA status bits
const DMA_STATUS_BUSY: u32 = 1 << 0;

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

        // Step 3: Program DMA registers
        write_volatile(DMA_SRC_ADDR as *mut u32, SRC_BASE);
        write_volatile(DMA_DST_ADDR as *mut u32, DST_BASE);
        write_volatile(DMA_SIZE as *mut u32, TEST_SIZE as u32);

        // Step 4: Dispatch DMA operation (write any value to dispatch register)
        write_volatile(DMA_DISPATCH as *mut u32, 1);

        // Step 5: Poll status register until transfer completes
        loop {
            let status = read_volatile(DMA_STATUS as *const u32);
            if (status & DMA_STATUS_BUSY) == 0 {
                break; // Transfer complete
            }
        }

        // Step 6: Verify destination data matches source
        let mut all_match = true;
        for i in 0..TEST_SIZE {
            let expected = i as u8;
            let actual = read_volatile((DST_BASE + i as u32) as *const u8);
            if actual != expected {
                all_match = false;
                break;
            }
        }

        // Step 7: Return result via tohost
        if all_match {
            common::write_tohost(common::SUCCESS_CODE);
        } else {
            common::write_tohost(common::FAILURE_CODE);
        }
    }
}
