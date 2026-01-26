#![no_std]
#![no_main]

#[cfg(feature = "protocol_macros")]
extern crate alloc;

mod common;

use core::panic::PanicInfo;
use riscv_rt::entry;
#[cfg(feature = "protocol_macros")]
use riscv_shared::rvprintln;

#[cfg(feature = "protocol_macros")]
#[global_allocator]
static ALLOCATOR: common::SimpleAllocator = common::SimpleAllocator;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

#[cfg(feature = "protocol_macros")]
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

#[cfg(not(feature = "protocol_macros"))]
#[entry]
fn main() -> ! {
    // This binary requires the protocol_macros feature to be enabled
    // Skip test when feature is not enabled
    common::write_tohost(common::SUCCESS_CODE);
}
