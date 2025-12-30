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

/// Write to tohost
#[inline(never)]
fn write_tohost(value: u32) -> ! {
    const TOHOST_ADDR: u32 = 0xFFFF_FFF0;
    unsafe {
        write_volatile(TOHOST_ADDR as *mut u32, value);
    }
    loop {}
}

/// Entry point
#[entry]
fn main() -> ! {
    // Just write success immediately
    write_tohost(42);
}
