#![no_std]
#![no_main]

mod common;

#[global_allocator]
static ALLOCATOR: common::SimpleAllocator = common::SimpleAllocator;

use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};
use riscv_rt::entry;

/// Panic handler for bare metal - infinite loop on panic
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

/// Entry point for the bare metal Rust program
/// This function implements test logic using regular Rust code
/// Uses riscv_rt which properly initializes stack pointer
#[entry]
fn main() -> ! {
    // ====== Test 1: Arithmetic Operations ======
    let x1: u32 = 10;
    let x2: u32 = 20;
    let x3 = x1.wrapping_add(x2); // 30
    let x4 = x2.wrapping_sub(x1); // 10
    let x5 = x1.wrapping_add(5); // 15

    if x1 != 10 || x2 != 20 || x3 != 30 || x4 != 10 || x5 != 15 {
        common::write_tohost(common::FAILURE_CODE);
    }

    // ====== Test 2: Logical Operations ======
    let x6 = x1 & x2; // 0
    let x7 = x1 | x2; // 30
    let x8 = x1 ^ x2; // 30
    let x9 = x1 & 15; // 10
    let x10 = x1 | 5; // 15
    let x11 = x1 ^ 7; // 13

    if x6 != 0 || x7 != 30 || x8 != 30 || x9 != 10 || x10 != 15 || x11 != 13 {
        common::write_tohost(common::FAILURE_CODE);
    }

    // ====== Test 3: Shift Operations ======
    let x12: u32 = 8;
    let x13 = x12 << 2; // 32
    let x14 = x13 >> 1; // 16
    let x15: i32 = -8;
    let x16 = x15 >> 1; // -4 (arithmetic shift)

    if x12 != 8 || x13 != 32 || x14 != 16 || x15 != -8 || x16 != -4 {
        common::write_tohost(common::FAILURE_CODE);
    }

    // ====== Test 4: Comparison Operations ======
    let a: u32 = 5;
    let b: u32 = 10;
    let cmp1 = if (a as i32) < (b as i32) { 1 } else { 0 }; // signed comparison
    let cmp2 = if (a as i32) < 3 { 1 } else { 0 }; // should be 0
    let cmp3 = if a < b { 1 } else { 0 }; // unsigned comparison

    if cmp1 != 1 || cmp2 != 0 || cmp3 != 1 {
        common::write_tohost(common::FAILURE_CODE);
    }

    // ====== Test 5: Memory Store and Load Verification ======
    const BASE_ADDR: u32 = 0x8000_1000;
    let val1: u32 = 100;
    let val2: u32 = 200;
    let val3: u32 = 300;

    unsafe {
        // Store values to memory
        write_volatile(BASE_ADDR as *mut u32, val1);
        write_volatile((BASE_ADDR + 4) as *mut u32, val2);
        write_volatile((BASE_ADDR + 8) as *mut u32, val3);

        // Load values from memory
        let loaded1 = read_volatile(BASE_ADDR as *const u32);
        let loaded2 = read_volatile((BASE_ADDR + 4) as *const u32);
        let loaded3 = read_volatile((BASE_ADDR + 8) as *const u32);

        if loaded1 != val1 || loaded2 != val2 || loaded3 != val3 {
            common::write_tohost(common::FAILURE_CODE);
        }
    }

    // ====== Test 6: Loop with Counter ======
    let mut accumulator: u32 = 0;
    let mut counter: u32 = 5;

    while counter > 0 {
        accumulator = accumulator.wrapping_add(1);
        counter = counter.wrapping_sub(1);
    }

    if accumulator != 5 {
        common::write_tohost(common::FAILURE_CODE);
    }

    // ====== Test 7: Array Operations ======
    const ARRAY_SIZE: usize = 5;
    let mut array: [u32; ARRAY_SIZE] = [0; ARRAY_SIZE];

    // Initialize array
    for (i, item) in array.iter_mut().enumerate() {
        *item = (i as u32) * 2;
    }

    // Verify array contents
    for (i, &item) in array.iter().enumerate() {
        if item != (i as u32) * 2 {
            common::write_tohost(common::FAILURE_CODE);
        }
    }

    // ====== Test 8: Function Calls ======
    let result = add_numbers(10, 20);
    if result != 30 {
        common::write_tohost(common::FAILURE_CODE);
    }

    let result2 = multiply_by_shift(7, 3); // 7 * 8 = 56
    if result2 != 56 {
        common::write_tohost(common::FAILURE_CODE);
    }

    // ====== All Tests Passed ======
    common::write_tohost(common::SUCCESS_CODE);
}

/// Simple addition function to test function calls
#[inline(never)]
fn add_numbers(a: u32, b: u32) -> u32 {
    a.wrapping_add(b)
}

/// Multiply by shifting (multiply by 2^shift_amount)
#[inline(never)]
fn multiply_by_shift(value: u32, shift_amount: u32) -> u32 {
    value << shift_amount
}
