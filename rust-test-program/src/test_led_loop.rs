#![no_std]
#![no_main]

mod common;

#[global_allocator]
static ALLOCATOR: common::SimpleAllocator = common::SimpleAllocator;

use core::panic::PanicInfo;
use core::ptr::write_volatile;
use riscv_rt::entry;
use riscv_shared::bus::LED_BASE;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

/// CPU clock frequency in Hz
const CPU_CLOCK_HZ: u32 = 25_000_000;

/// Target time for a full sweep (up and down) in seconds
const SWEEP_TIME_SECONDS: u32 = 1;

/// Number of LED positions in a full sweep cycle
/// Up: 0→1→2→3→4→5→6→7 (8 positions)
/// Down: 7→6→5→4→3→2→1→0 (8 positions)
/// Total: 16 position changes per full sweep
const POSITIONS_PER_SWEEP: u32 = 16;

/// Calculate delay cycles per LED position
/// For 1 second sweep with 16 positions = 62.5ms per position
/// At 25MHz: 62.5ms * 25,000,000 cycles/sec = 1,562,500 cycles
const DELAY_CYCLES: u32 = (CPU_CLOCK_HZ * SWEEP_TIME_SECONDS) / POSITIONS_PER_SWEEP;

/// Busy-wait delay loop
/// Each iteration takes approximately 4-5 cycles:
///   - Loop counter increment (addi): ~1 cycle
///   - Compare (blt/bne): ~1 cycle
///   - Branch taken: ~2 cycles (fetch + decode)
///   - Conservative estimate: 5 cycles per iteration
#[inline(never)]
fn delay_cycles(cycles: u32) {
    let iterations = cycles / 5;
    for _ in 0..iterations {
        // Empty loop body - the loop overhead provides the delay
        // Using core::hint::black_box to prevent optimization
        core::hint::black_box(());
    }
}

/// Write a value to the LED output register
#[inline(never)]
fn write_led(value: u8) {
    unsafe {
        write_volatile(LED_BASE as *mut u32, value as u32);
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
            delay_cycles(DELAY_CYCLES);
        }

        // Sweep down: positions 7 down to 0
        // Start at 6 to avoid duplicating position 7 and 0
        for pos in (0..7u8).rev() {
            write_led(led_pattern(pos));
            delay_cycles(DELAY_CYCLES);
        }
    }
}
