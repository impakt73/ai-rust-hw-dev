#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr::write_volatile;

/// Panic handler for bare metal - infinite loop on panic
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

/// Write to tohost address to signal completion
#[inline(never)]
fn write_tohost(value: u32) -> ! {
    const TOHOST_ADDR: u32 = 0xFFFF_FFF0;
    unsafe {
        write_volatile(TOHOST_ADDR as *mut u32, value);
    }
    loop {}
}

/// FIFO memory-mapped I/O addresses
const FIFO_BASE: u32 = 0x4000_0000;
const FIFO_DATA: u32 = FIFO_BASE + 0x0;
const FIFO_STATUS: u32 = FIFO_BASE + 0x4;

/// FIFO status bits
const _FIFO_RX_VALID: u32 = 0x1;
const FIFO_TX_READY: u32 = 0x2;

/// Write a byte to the FIFO
/// Note: This is a legacy function - the FIFO actually works with u32 words
#[inline(never)]
fn fifo_write_word(word: u32) {
    unsafe {
        // Wait for FIFO to be ready
        loop {
            let status = core::ptr::read_volatile(FIFO_STATUS as *const u32);
            if (status & FIFO_TX_READY) != 0 {
                break;
            }
        }

        // Write word to FIFO
        write_volatile(FIFO_DATA as *mut u32, word);
    }
}

/// Write a string to the FIFO
/// Chunks the string into u32 words (4 bytes each) with zero padding at the end
#[inline(never)]
fn fifo_write_string(s: &str) {
    let bytes = s.as_bytes();
    let mut i = 0;
    
    while i < bytes.len() {
        let mut word: u32 = 0;
        
        // Pack up to 4 bytes into a u32 word (little-endian)
        for j in 0..4 {
            if i + j < bytes.len() {
                word |= (bytes[i + j] as u32) << (j * 8);
            }
            // Remaining bytes are implicitly 0 (zero-padding)
        }
        
        fifo_write_word(word);
        i += 4;
    }
}

/// Entry point for the bare metal Rust program
#[no_mangle]
#[link_section = ".text"]
pub extern "C" fn _start() -> ! {
    const SUCCESS_CODE: u32 = 42;

    // Write "Hello World!" to the FIFO
    fifo_write_string("Hello World!");

    // Exit with success
    write_tohost(SUCCESS_CODE);
}
