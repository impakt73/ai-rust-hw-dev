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
    const SUCCESS_CODE: u32 = 42;
    
    // Write the expected magic number and some test data
    // Write "CPU Started" string as it should appear
    common::fifo_write_word(0x20555043); // "CPU "
    common::fifo_write_word(0x72617453); // "Star"
    common::fifo_write_word(0x00646574); // "ted\0"
    // Write PACKET_MAGIC
    common::fifo_write_word(0x52565043); // "CPVR"
    
    common::write_tohost(SUCCESS_CODE);
}
