#![no_std]
#![no_main]

extern crate alloc;

mod common;

use core::panic::PanicInfo;
use riscv_rt::entry;
use riscv_shared::fifo::FifoUwrite;
use ufmt::uwriteln;

#[global_allocator]
static HEAP: common::Heap = common::Heap::empty();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

#[entry]
fn main() -> ! {
    common::init_heap(&HEAP);
    let mut fifo = FifoUwrite::new();
    let _ = uwriteln!(&mut fifo, "Hello from RISC-V CPU!");

    let _ = uwriteln!(&mut fifo, "The answer is {}", 42);

    let _ = uwriteln!(&mut fifo, "Testing println macro");

    // Signal success
    common::write_tohost(common::SUCCESS_CODE);
}
