#![no_std]
#![no_main]

mod common;

#[global_allocator]
static ALLOCATOR: common::SimpleAllocator = common::SimpleAllocator;

use core::panic::PanicInfo;
use riscv_rt::entry;

/// Panic handler
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

/// Entry point
#[entry]
fn main() -> ! {
    // Just write success immediately
    common::write_tohost(common::SUCCESS_CODE);
}
