#![no_std]
#![no_main]

extern crate alloc;

mod common;

use core::panic::PanicInfo;
use riscv_rt::entry;
use riscv_shared::rvprintln;

#[global_allocator]
static HEAP: common::Heap = common::Heap::empty();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

#[entry]
fn main() -> ! {
    common::init_heap(&HEAP);
    // Test basic println functionality
    rvprintln!("Hello from RISC-V CPU!");

    // Test formatted output with arguments
    rvprintln!("The answer is {}", 42);

    // Test multiple messages
    rvprintln!("Testing println macro");

    // Signal success
    common::write_tohost(common::SUCCESS_CODE);
}
