#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr::write_volatile;
use riscv_rt::entry;

/// Panic handler
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

/// FIFO and special addresses
const FIFO_DATA: u32 = 0x4000_0000;
const TOHOST_ADDR: u32 = 0xFFFF_FFF0;
const MARKER_ADDR: u32 = 0xFFFF_FFF4;

/// Write to tohost address to signal completion
#[inline(never)]
fn write_tohost(value: u32) -> ! {
    unsafe {
        write_volatile(TOHOST_ADDR as *mut u32, value);
    }
    loop {}
}

/// Entry point
#[entry]
fn main() -> ! {
    const SUCCESS_CODE: u32 = 42;

    // Write directly to FIFO - two test words
    unsafe {
        write_volatile(FIFO_DATA as *mut u32, 0xDEADBEEF);
        write_volatile(FIFO_DATA as *mut u32, 0xCAFEBABE);
    }
    
    // Marker: Wrote to FIFO
    unsafe { write_volatile(MARKER_ADDR as *mut u32, 0x1111); }

    // Success
    write_tohost(SUCCESS_CODE);
}
