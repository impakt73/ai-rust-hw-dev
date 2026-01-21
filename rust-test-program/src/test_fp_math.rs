//! Test floating-point math operations using the F extension
//!
//! This test validates that the RV32F extension works correctly by
//! performing various floating-point calculations and verifying results.

#![no_std]
#![no_main]

mod common;

use core::panic::PanicInfo;
use riscv_rt::entry;

/// Panic handler
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

/// Entry point
#[entry]
fn main() -> ! {
    // Test basic floating-point arithmetic
    let a: f32 = 3.0;
    let b: f32 = 2.0;

    // Addition: 3.0 + 2.0 = 5.0
    let sum = a + b;
    assert_eq_f32(sum, 5.0);

    // Subtraction: 3.0 - 2.0 = 1.0
    let diff = a - b;
    assert_eq_f32(diff, 1.0);

    // Multiplication: 3.0 * 2.0 = 6.0
    let prod = a * b;
    assert_eq_f32(prod, 6.0);

    // Division: 6.0 / 2.0 = 3.0
    let quot = prod / b;
    assert_eq_f32(quot, 3.0);

    // Test more complex expressions
    let x: f32 = 1.5;
    let y: f32 = 2.5;

    // (1.5 * 2.5) + 3.0 = 3.75 + 3.0 = 6.75
    let result = (x * y) + a;
    assert_eq_f32(result, 6.75);

    // Test comparisons
    let c: f32 = 4.0;
    let d: f32 = 4.0;

    if c == d {
        // Equality test passed
    }

    // Test min/max using comparisons
    let min_val = if a < b { a } else { b };
    assert_eq_f32(min_val, 2.0);

    let max_val = if a > b { a } else { b };
    assert_eq_f32(max_val, 3.0);

    // Test conversions
    let int_val: i32 = 42;
    let float_from_int = int_val as f32;
    assert_eq_f32(float_from_int, 42.0);

    let float_val: f32 = 7.8;
    let int_from_float = float_val as i32;
    if int_from_float != 7 {
        common::write_tohost(0xDEAD); // Failure
    }

    // Test negative numbers
    let neg: f32 = -5.5;

    // Multiplication with negative: -5.5 * 2.0 = -11.0
    let neg_prod = neg * b;
    assert_eq_f32(neg_prod, -11.0);

    // Test division with result verification
    let div_test: f32 = 10.0;
    let div_by: f32 = 2.5;
    let div_result = div_test / div_by;
    assert_eq_f32(div_result, 4.0);

    // All tests passed
    common::write_tohost(common::SUCCESS_CODE);
}

/// Helper function to compare f32 values (bitwise comparison)
#[inline(always)]
fn assert_eq_f32(a: f32, b: f32) {
    let a_bits: u32 = a.to_bits();
    let b_bits: u32 = b.to_bits();

    if a_bits != b_bits {
        common::write_tohost(0xDEAD); // Failure marker
    }
}
