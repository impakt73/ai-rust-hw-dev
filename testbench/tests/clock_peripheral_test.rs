use riscv_core::{create_clock_peripheral_runtime, ClockPeripheral};
use riscv_shared::bus::{CLOCK_ELAPSED_MS_OFFSET, CLOCK_ELAPSED_S_OFFSET, CLOCK_ELAPSED_US_OFFSET};

// Clock Peripheral Register Offsets (from riscv_shared)
const ELAPSED_US: u32 = CLOCK_ELAPSED_US_OFFSET;
const ELAPSED_MS: u32 = CLOCK_ELAPSED_MS_OFFSET;
const ELAPSED_S: u32 = CLOCK_ELAPSED_S_OFFSET;

// Access size encodings
const SIZE_WORD: u8 = 0b10;

// Clock cycle macro for clock peripheral tests
macro_rules! clock_cycle {
    ($dut:expr) => {
        $dut.clk = 0;
        $dut.eval();
        $dut.clk = 1;
        $dut.eval();
        $dut.clk = 0;
        $dut.eval();
    };
}

/// Helper function to read a register from the clock peripheral
fn read_register(dut: &mut ClockPeripheral, offset: u32) -> u32 {
    dut.addr = offset;
    dut.req = 1;
    dut.we = 0;
    dut.size = SIZE_WORD;
    dut.eval();
    let value = dut.rdata;
    dut.req = 0;
    dut.eval();
    value
}

/// Helper function to apply reset to the clock peripheral
fn reset_clock_peripheral(dut: &mut ClockPeripheral) {
    dut.rst_n = 0;
    dut.we = 0;
    dut.req = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    // Don't do a clock cycle here - let tests control when the first cycle happens
    // This way, immediately after reset, the counter reads 0
}

#[test]
fn test_clock_peripheral_reset_state() {
    let runtime =
        create_clock_peripheral_runtime().expect("Failed to create clock peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<ClockPeripheral>()
        .expect("Failed to create clock peripheral model");

    // Apply reset
    reset_clock_peripheral(&mut dut);

    // Verify ready signal is asserted (single-cycle peripheral)
    assert_eq!(dut.ready, 1, "Clock peripheral should be ready");

    // All time counters should be 0 after reset
    let elapsed_us = read_register(&mut dut, ELAPSED_US);
    let elapsed_ms = read_register(&mut dut, ELAPSED_MS);
    let elapsed_s = read_register(&mut dut, ELAPSED_S);

    assert_eq!(elapsed_us, 0, "ELAPSED_US should be 0 after reset");
    assert_eq!(elapsed_ms, 0, "ELAPSED_MS should be 0 after reset");
    assert_eq!(elapsed_s, 0, "ELAPSED_S should be 0 after reset");
}

#[test]
fn test_clock_peripheral_always_ready() {
    let runtime =
        create_clock_peripheral_runtime().expect("Failed to create clock peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<ClockPeripheral>()
        .expect("Failed to create clock peripheral model");

    // Apply reset
    reset_clock_peripheral(&mut dut);

    // Verify ready stays high for multiple cycles
    for i in 0..100 {
        assert_eq!(dut.ready, 1, "Ready should stay high at cycle {}", i);
        clock_cycle!(dut);
    }
}

#[test]
fn test_clock_peripheral_microseconds() {
    let runtime =
        create_clock_peripheral_runtime().expect("Failed to create clock peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<ClockPeripheral>()
        .expect("Failed to create clock peripheral model");

    // Apply reset
    reset_clock_peripheral(&mut dut);

    // At 1 MHz (default CLK_FREQ_HZ), 1 cycle = 1 microsecond
    // Run for a few cycles and check that microseconds increment correctly

    // Initial value should be 0
    let us_0 = read_register(&mut dut, ELAPSED_US);
    assert_eq!(us_0, 0, "Initial ELAPSED_US should be 0");

    // Run 1 cycle (should be 1 microsecond)
    clock_cycle!(dut);
    let us_1 = read_register(&mut dut, ELAPSED_US);
    assert_eq!(us_1, 1, "After 1 cycle at 1MHz, should be 1 us");

    // Run 1 more cycle (should be 2 microseconds)
    clock_cycle!(dut);
    let us_2 = read_register(&mut dut, ELAPSED_US);
    assert_eq!(us_2, 2, "After 2 cycles at 1MHz, should be 2 us");

    // Run 8 more cycles (total 10 cycles = 10 microseconds)
    for _ in 0..8 {
        clock_cycle!(dut);
    }
    let us_10 = read_register(&mut dut, ELAPSED_US);
    assert_eq!(us_10, 10, "After 10 cycles at 1MHz, should be 10 us");
}

#[test]
fn test_clock_peripheral_milliseconds() {
    let runtime =
        create_clock_peripheral_runtime().expect("Failed to create clock peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<ClockPeripheral>()
        .expect("Failed to create clock peripheral model");

    // Apply reset
    reset_clock_peripheral(&mut dut);

    // At 1 MHz, 1000 cycles = 1 millisecond

    // Initial value should be 0
    let ms_0 = read_register(&mut dut, ELAPSED_MS);
    assert_eq!(ms_0, 0, "Initial ELAPSED_MS should be 0");

    // Run 1000 cycles (should be 1 millisecond)
    for _ in 0..1000 {
        clock_cycle!(dut);
    }
    let ms_1 = read_register(&mut dut, ELAPSED_MS);
    assert_eq!(ms_1, 1, "After 1000 cycles at 1MHz, should be 1 ms");

    // Run 9000 more cycles (total 10000 cycles = 10 milliseconds)
    for _ in 0..9000 {
        clock_cycle!(dut);
    }
    let ms_10 = read_register(&mut dut, ELAPSED_MS);
    assert_eq!(ms_10, 10, "After 10000 cycles at 1MHz, should be 10 ms");
}

#[test]
fn test_clock_peripheral_seconds() {
    let runtime =
        create_clock_peripheral_runtime().expect("Failed to create clock peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<ClockPeripheral>()
        .expect("Failed to create clock peripheral model");

    // Apply reset
    reset_clock_peripheral(&mut dut);

    // At 1 MHz, 1,000,000 cycles = 1 second

    // Initial value should be 0
    let s_0 = read_register(&mut dut, ELAPSED_S);
    assert_eq!(s_0, 0, "Initial ELAPSED_S should be 0");

    // Run 500,000 cycles (should be 0 seconds still)
    for _ in 0..500_000 {
        clock_cycle!(dut);
    }
    let s_500k = read_register(&mut dut, ELAPSED_S);
    assert_eq!(
        s_500k, 0,
        "After 500,000 cycles at 1MHz, should still be 0 s"
    );

    // Run 500,000 more cycles (total 1,000,000 cycles = 1 second)
    for _ in 0..500_000 {
        clock_cycle!(dut);
    }
    let s_1m = read_register(&mut dut, ELAPSED_S);
    assert_eq!(s_1m, 1, "After 1,000,000 cycles at 1MHz, should be 1 s");

    // Run 1,000,000 more cycles (total 2,000,000 cycles = 2 seconds)
    for _ in 0..1_000_000 {
        clock_cycle!(dut);
    }
    let s_2m = read_register(&mut dut, ELAPSED_S);
    assert_eq!(s_2m, 2, "After 2,000,000 cycles at 1MHz, should be 2 s");
}

#[test]
fn test_clock_peripheral_all_counters_increment() {
    let runtime =
        create_clock_peripheral_runtime().expect("Failed to create clock peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<ClockPeripheral>()
        .expect("Failed to create clock peripheral model");

    // Apply reset
    reset_clock_peripheral(&mut dut);

    // Run 2,000,000 cycles (2 seconds at 1 MHz)
    for _ in 0..2_000_000 {
        clock_cycle!(dut);
    }

    // Read all counters
    let elapsed_us = read_register(&mut dut, ELAPSED_US);
    let elapsed_ms = read_register(&mut dut, ELAPSED_MS);
    let elapsed_s = read_register(&mut dut, ELAPSED_S);

    // Verify all counters have incremented correctly
    assert_eq!(
        elapsed_us, 2_000_000,
        "After 2,000,000 cycles at 1MHz, should be 2,000,000 us"
    );
    assert_eq!(
        elapsed_ms, 2000,
        "After 2,000,000 cycles at 1MHz, should be 2000 ms"
    );
    assert_eq!(
        elapsed_s, 2,
        "After 2,000,000 cycles at 1MHz, should be 2 s"
    );
}

#[test]
fn test_clock_peripheral_read_only() {
    let runtime =
        create_clock_peripheral_runtime().expect("Failed to create clock peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<ClockPeripheral>()
        .expect("Failed to create clock peripheral model");

    // Apply reset
    reset_clock_peripheral(&mut dut);

    // Run a few cycles to get non-zero values
    for _ in 0..10 {
        clock_cycle!(dut);
    }

    // Read current values
    let us_before = read_register(&mut dut, ELAPSED_US);
    let ms_before = read_register(&mut dut, ELAPSED_MS);
    let s_before = read_register(&mut dut, ELAPSED_S);

    // Attempt to write to each register (should be ignored)
    dut.addr = ELAPSED_US;
    dut.wdata = 0xDEADBEEF;
    dut.we = 1;
    dut.req = 1;
    dut.size = SIZE_WORD;
    dut.eval();
    clock_cycle!(dut);
    dut.we = 0;
    dut.req = 0;

    dut.addr = ELAPSED_MS;
    dut.wdata = 0xCAFEBABE;
    dut.we = 1;
    dut.req = 1;
    dut.eval();
    clock_cycle!(dut);
    dut.we = 0;
    dut.req = 0;

    dut.addr = ELAPSED_S;
    dut.wdata = 0x12345678;
    dut.we = 1;
    dut.req = 1;
    dut.eval();
    clock_cycle!(dut);
    dut.we = 0;
    dut.req = 0;

    // Read values after write attempts
    // They should have incremented by the clock cycles, not been overwritten
    let us_after = read_register(&mut dut, ELAPSED_US);
    let ms_after = read_register(&mut dut, ELAPSED_MS);
    let s_after = read_register(&mut dut, ELAPSED_S);

    // Values should have incremented (3 extra cycles from write attempts)
    // At 1 MHz: 3 cycles = 3 us, not enough for 1 ms
    assert_eq!(
        us_after,
        us_before + 3,
        "ELAPSED_US should increment, not be overwritten"
    );
    assert_eq!(
        ms_after, ms_before,
        "ELAPSED_MS should be same (not enough cycles for 1ms)"
    );
    assert_eq!(
        s_after, s_before,
        "ELAPSED_S should be same (not enough cycles for 1s)"
    );

    // Specifically check they were NOT set to the attempted write values
    assert_ne!(us_after, 0xDEADBEEF, "ELAPSED_US should not be writable");
    assert_ne!(ms_after, 0xCAFEBABE, "ELAPSED_MS should not be writable");
    assert_ne!(s_after, 0x12345678, "ELAPSED_S should not be writable");
}

#[test]
fn test_clock_peripheral_unmapped_register() {
    let runtime =
        create_clock_peripheral_runtime().expect("Failed to create clock peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<ClockPeripheral>()
        .expect("Failed to create clock peripheral model");

    // Apply reset
    reset_clock_peripheral(&mut dut);

    // Try to read from an unmapped register offset (e.g., 0x0C)
    let unmapped = read_register(&mut dut, 0x0C);
    assert_eq!(unmapped, 0, "Unmapped register should return 0");

    // Try another unmapped offset
    let unmapped2 = read_register(&mut dut, 0x10);
    assert_eq!(unmapped2, 0, "Unmapped register should return 0");
}
