#![no_std]
#![no_main]

mod common;

use core::panic::PanicInfo;
use core::sync::atomic::{AtomicU32, Ordering};
use riscv_rt::entry;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

#[entry]
fn main() -> ! {
    // Test atomic operations using Rust's atomic types
    // This will compile to RISC-V atomic instructions

    // Create atomic counter in static memory
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    // Test 1: Atomic fetch_add
    let old_value = COUNTER.fetch_add(5, Ordering::SeqCst);
    if old_value != 0 {
        halt(2); // Error: expected 0
    }

    let current = COUNTER.load(Ordering::SeqCst);
    if current != 5 {
        halt(3); // Error: expected 5
    }

    // Test 2: Atomic swap
    let old_value = COUNTER.swap(42, Ordering::SeqCst);
    if old_value != 5 {
        halt(4); // Error: expected 5
    }

    let current = COUNTER.load(Ordering::SeqCst);
    if current != 42 {
        halt(5); // Error: expected 42
    }

    // Test 3: Atomic compare_exchange (should succeed)
    let result = COUNTER.compare_exchange(42, 100, Ordering::SeqCst, Ordering::SeqCst);
    if result != Ok(42) {
        halt(6); // Error: compare_exchange should succeed
    }

    let current = COUNTER.load(Ordering::SeqCst);
    if current != 100 {
        halt(7); // Error: expected 100
    }

    // Test 4: Atomic compare_exchange (should fail)
    let result = COUNTER.compare_exchange(42, 200, Ordering::SeqCst, Ordering::SeqCst);
    if result != Err(100) {
        halt(8); // Error: compare_exchange should fail
    }

    let current = COUNTER.load(Ordering::SeqCst);
    if current != 100 {
        halt(9); // Error: value should remain 100
    }

    // Test 5: Atomic fetch_and
    COUNTER.store(0xFF, Ordering::SeqCst);
    let old_value = COUNTER.fetch_and(0x0F, Ordering::SeqCst);
    if old_value != 0xFF {
        halt(10); // Error: expected 0xFF
    }

    let current = COUNTER.load(Ordering::SeqCst);
    if current != 0x0F {
        halt(11); // Error: expected 0x0F
    }

    // Test 6: Atomic fetch_or
    let old_value = COUNTER.fetch_or(0xF0, Ordering::SeqCst);
    if old_value != 0x0F {
        halt(12); // Error: expected 0x0F
    }

    let current = COUNTER.load(Ordering::SeqCst);
    if current != 0xFF {
        halt(13); // Error: expected 0xFF
    }

    // Test 7: Atomic fetch_xor
    let old_value = COUNTER.fetch_xor(0xAA, Ordering::SeqCst);
    if old_value != 0xFF {
        halt(14); // Error: expected 0xFF
    }

    let current = COUNTER.load(Ordering::SeqCst);
    if current != 0x55 {
        halt(15); // Error: expected 0x55
    }

    // Test 8: Atomic fetch_max
    COUNTER.store(10, Ordering::SeqCst);
    let old_value = COUNTER.fetch_max(20, Ordering::SeqCst);
    if old_value != 10 {
        halt(16); // Error: expected 10
    }

    let current = COUNTER.load(Ordering::SeqCst);
    if current != 20 {
        halt(17); // Error: expected 20
    }

    // Test 9: Atomic fetch_min
    let old_value = COUNTER.fetch_min(15, Ordering::SeqCst);
    if old_value != 20 {
        halt(18); // Error: expected 20
    }

    let current = COUNTER.load(Ordering::SeqCst);
    if current != 15 {
        halt(19); // Error: expected 15
    }

    // All tests passed!
    halt(common::SUCCESS_CODE); // Success
}

fn halt(code: u32) -> ! {
    common::write_tohost(code)
}
