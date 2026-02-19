#![no_std]
#![no_main]

extern crate alloc;

mod common;

use core::panic::PanicInfo;
use riscv_rt::entry;

#[global_allocator]
static HEAP: common::Heap = common::Heap::empty();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

#[entry]
fn main() -> ! {
    common::init_heap(&HEAP);
    const DEBUG_1: [u32; 9] = [
        0x92d9a0c3, 0x000e0005, 0x00000002, 0x6c654817, 0x66206f6c, 0x206d6f72, 0x43534952,
        0x4320562d, 0x0a215550,
    ];
    const DEBUG_2: [u32; 8] = [
        0x92d9a0c3, 0x000e0005, 0x00000002, 0x65685411, 0x736e6120, 0x20726577, 0x34207369,
        0x00000a32,
    ];
    const DEBUG_3: [u32; 9] = [
        0x92d9a0c3, 0x000e0005, 0x00000002, 0x73655416, 0x676e6974, 0x69727020, 0x6e6c746e,
        0x63616d20, 0x000a6f72,
    ];

    for &word in DEBUG_1.iter().chain(DEBUG_2.iter()).chain(DEBUG_3.iter()) {
        if common::fifo_write_word(word).is_err() {
            common::write_tohost(common::FAILURE_CODE);
        }
    }

    // Signal success
    common::write_tohost(common::SUCCESS_CODE);
}
