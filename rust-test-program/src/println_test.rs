#![no_std]
#![no_main]

extern crate alloc;

mod common;

use core::panic::PanicInfo;
use riscv_macros::rvprintln;
use riscv_rt::entry;

#[global_allocator]
static ALLOCATOR: common::SimpleAllocator = common::SimpleAllocator;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

#[entry]
fn main() -> ! {
    // Test basic println functionality
    rvprintln!("Hello from RISC-V CPU!");

    // Test formatted output with arguments
    rvprintln!("The answer is {}", 42);

    // Test multiple messages
    rvprintln!("Testing println macro");

    // Signal success
    common::write_tohost(common::SUCCESS_CODE);
}
