//! Test floating-point math operations using the F extension
//!
//! This test validates that the RV32F extension works correctly by
//! performing various floating-point calculations and verifying results.

#![no_std]
#![no_main]

mod common;

#[global_allocator]
static HEAP: common::Heap = common::Heap::empty();

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
    // Use black_box to prevent the compiler from optimizing out floating-point operations
    // Test basic floating-point arithmetic
    let a: f32 = core::hint::black_box(3.0_f32);
    let b: f32 = core::hint::black_box(2.0_f32);

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
    let x: f32 = core::hint::black_box(1.5_f32);
    let y: f32 = core::hint::black_box(2.5_f32);

    // (1.5 * 2.5) + 3.0 = 3.75 + 3.0 = 6.75
    let result = (x * y) + a;
    assert_eq_f32(result, 6.75);

    // Test comparisons
    let c: f32 = core::hint::black_box(4.0_f32);
    let d: f32 = core::hint::black_box(4.0_f32);

    if c == d {
        // Equality test passed
    }

    // Test min/max using comparisons
    let min_val = if a < b { a } else { b };
    assert_eq_f32(min_val, 2.0);

    let max_val = if a > b { a } else { b };
    assert_eq_f32(max_val, 3.0);

    // Test conversions
    let int_val: i32 = core::hint::black_box(42_i32);
    let float_from_int = int_val as f32;
    assert_eq_f32(float_from_int, 42.0);

    let float_val: f32 = core::hint::black_box(7.8_f32);
    let int_from_float = float_val as i32;
    if int_from_float != 7 {
        common::write_tohost(0xDEAD); // Failure
    }

    // Test negative numbers
    let neg: f32 = core::hint::black_box(-5.5_f32);

    // Multiplication with negative: -5.5 * 2.0 = -11.0
    let neg_prod = neg * b;
    assert_eq_f32(neg_prod, -11.0);

    // Test division with result verification
    let div_test: f32 = core::hint::black_box(10.0_f32);
    let div_by: f32 = core::hint::black_box(2.5_f32);
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
