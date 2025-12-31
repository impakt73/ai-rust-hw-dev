#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr::write_volatile;
use riscv_rt::entry;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

const FIFO_DATA: u32 = 0x4000_0000;
const TOHOST_ADDR: u32 = 0xFFFF_FFF0;

fn write_tohost(value: u32) -> ! {
    unsafe {
        write_volatile(TOHOST_ADDR as *mut u32, value);
    }
    loop {}
}

#[entry]
fn main() -> ! {
    const SUCCESS_CODE: u32 = 42;
    
    // Write the expected magic number and some test data
    unsafe {
        // Write "CPU Started" string as it should appear
        write_volatile(FIFO_DATA as *mut u32, 0x20555043); // "CPU "
        write_volatile(FIFO_DATA as *mut u32, 0x72617453); // "Star"
        write_volatile(FIFO_DATA as *mut u32, 0x00646574); // "ted\0"
        // Write PACKET_MAGIC
        write_volatile(FIFO_DATA as *mut u32, 0x52565043); // "CPVR"
    }
    
    write_tohost(SUCCESS_CODE);
}
