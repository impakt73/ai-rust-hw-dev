#![no_std]
#![no_main]

mod common;

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
