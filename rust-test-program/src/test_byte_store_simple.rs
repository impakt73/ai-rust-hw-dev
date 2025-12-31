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

// Static buffer for testing
static mut TEST_BUFFER: [u8; 16] = [0; 16];

#[entry]
fn main() -> ! {
    unsafe {
        let ptr = core::ptr::addr_of_mut!(TEST_BUFFER).cast::<u8>();
        
        // Write test pattern using ptr::write (which compiles to SB instructions)
        core::ptr::write(ptr.add(0), 0x11u8);
        core::ptr::write(ptr.add(1), 0x22u8);
        core::ptr::write(ptr.add(2), 0x33u8);
        core::ptr::write(ptr.add(3), 0x44u8);
        core::ptr::write(ptr.add(4), 0x55u8);
        core::ptr::write(ptr.add(5), 0x66u8);
        core::ptr::write(ptr.add(6), 0x77u8);
        core::ptr::write(ptr.add(7), 0x88u8);
        
        // Write marker
        write_volatile(FIFO_DATA as *mut u32, 0xAAAAAAAA);
        
        // Read back and write to FIFO
        for i in 0..8 {
            let byte = core::ptr::read(ptr.add(i));
            write_volatile(FIFO_DATA as *mut u32, byte as u32);
        }
        
        // Write marker
        write_volatile(FIFO_DATA as *mut u32, 0xBBBBBBBB);
    }
    
    write_tohost(42);
}
