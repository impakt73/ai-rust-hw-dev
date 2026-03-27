#![no_std]
#![no_main]

mod common;

#[global_allocator]
static HEAP: common::Heap = common::Heap::empty();

use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};
use riscv_rt::entry;
use riscv_shared::bus::{sysctrl_elapsed_ms_addr, sysctrl_led_out_addr};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

/// Target delay between LED position changes in milliseconds
/// For a full sweep (16 positions) with ~62ms per position ≈ 1 second total
const DELAY_MS: u32 = 62;

/// Read the elapsed milliseconds from the system controller
#[inline(never)]
fn read_elapsed_ms() -> u32 {
    unsafe { read_volatile(sysctrl_elapsed_ms_addr() as *const u32) }
}

/// Delay for the specified number of milliseconds using the system controller
/// This provides accurate timing independent of CPU implementation details
#[inline(never)]
fn delay_ms(ms: u32) {
    let start_ms = read_elapsed_ms();

    // Wait until the target time is reached
    // Handle wraparound correctly
    loop {
        let current_ms = read_elapsed_ms();

        // Check if we've reached the target
        // This works correctly even if the counter wraps around
        let elapsed = current_ms.wrapping_sub(start_ms);
        if elapsed >= ms {
            break;
        }

        // Prevent the compiler from optimizing away the loop
        core::hint::black_box(current_ms);
    }
}

/// Write a value to the LED output register
#[inline(never)]
fn write_led(value: u8) {
    unsafe {
        write_volatile(sysctrl_led_out_addr() as *mut u32, value as u32);
    }
}

/// Generate LED pattern for a single lit LED at the given position (0-7)
fn led_pattern(position: u8) -> u8 {
    1u8 << position
}

#[entry]
fn main() -> ! {
    // Main infinite loop - sweep LED pattern forever
    loop {
        // Sweep up: positions 0 through 7
        for pos in 0..8u8 {
            write_led(led_pattern(pos));
            delay_ms(DELAY_MS);
        }

        // Sweep down: positions 7 down to 0
        // Start at 6 to avoid duplicating position 7 and 0
        for pos in (0..7u8).rev() {
            write_led(led_pattern(pos));
            delay_ms(DELAY_MS);
        }
    }
}
