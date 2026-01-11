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
    // Simpler test: just use inline assembly to test atomic instructions directly
    let mut value: u32 = 0;
    let addr: *mut u32 = &mut value;
    
    unsafe {
        // Test 1: AMOADD.W - add 5 to memory (initially 0)
        let old_value: u32;
        core::arch::asm!(
            "amoadd.w {rd}, {rs2}, ({rs1})",
            rd = out(reg) old_value,
            rs1 = in(reg) addr,
            rs2 = in(reg) 5u32,
        );
        
        if old_value != 0 {
            halt(2); // Error: expected 0
        }
        
        if value != 5 {
            halt(3); // Error: expected 5
        }
        
        // Test 2: AMOSWAP.W - swap with 42
        let old_value2: u32;
        core::arch::asm!(
            "amoswap.w {rd}, {rs2}, ({rs1})",
            rd = out(reg) old_value2,
            rs1 = in(reg) addr,
            rs2 = in(reg) 42u32,
        );
        
        if old_value2 != 5 {
            halt(4); // Error: expected 5
        }
        
        if value != 42 {
            halt(5); // Error: expected 42
        }
    }
    
    // All tests passed!
    halt(common::SUCCESS_CODE); // Success
}

fn halt(code: u32) -> ! {
    common::write_tohost(code)
}
