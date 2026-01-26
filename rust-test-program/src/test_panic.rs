#![no_std]
#![no_main]

mod common;

#[global_allocator]
static ALLOCATOR: common::SimpleAllocator = common::SimpleAllocator;

use core::panic::PanicInfo;
use riscv_rt::entry;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

#[entry]
fn main() -> ! {
    // Deliberately cause a panic
    panic!("Test panic");
}
