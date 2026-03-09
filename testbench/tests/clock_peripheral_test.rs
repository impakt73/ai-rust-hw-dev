use riscv_core::{create_clock_peripheral_runtime, ClockPeripheral};
use riscv_shared::bus::{CLOCK_ELAPSED_MS_OFFSET, CLOCK_ELAPSED_S_OFFSET, CLOCK_ELAPSED_US_OFFSET};

const ELAPSED_US: u32 = CLOCK_ELAPSED_US_OFFSET;
const ELAPSED_MS: u32 = CLOCK_ELAPSED_MS_OFFSET;
const ELAPSED_S: u32 = CLOCK_ELAPSED_S_OFFSET;

const SIZE_WORD: u8 = 0b10;

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

fn read_register(dut: &mut ClockPeripheral, offset: u32) -> u32 {
    dut.mem_a_addr = offset;
    dut.mem_a_wdata = 0;
    dut.mem_a_we = 0;
    dut.mem_a_size = SIZE_WORD;
    dut.mem_a_valid = 1;
    dut.eval();
    assert_eq!(
        dut.mem_a_ready, 1,
        "clock peripheral should accept read request"
    );

    clock_cycle!(dut);
    dut.mem_a_valid = 0;
    dut.eval();

    for _ in 0..8 {
        if dut.mem_d_valid != 0 {
            let value = dut.mem_d_rdata;
            dut.mem_d_ready = 1;
            dut.eval();
            clock_cycle!(dut);
            dut.mem_d_ready = 0;
            dut.eval();
            return value;
        }

        clock_cycle!(dut);
    }

    panic!("clock peripheral did not return D-channel response");
}

fn write_register(dut: &mut ClockPeripheral, offset: u32, value: u32) {
    dut.mem_a_addr = offset;
    dut.mem_a_wdata = value;
    dut.mem_a_we = 1;
    dut.mem_a_size = SIZE_WORD;
    dut.mem_a_valid = 1;
    dut.eval();
    assert_eq!(
        dut.mem_a_ready, 1,
        "clock peripheral should accept write request"
    );

    clock_cycle!(dut);
    dut.mem_a_valid = 0;
    dut.eval();

    for _ in 0..8 {
        if dut.mem_d_valid != 0 {
            dut.mem_d_ready = 1;
            dut.eval();
            clock_cycle!(dut);
            dut.mem_d_ready = 0;
            dut.mem_a_we = 0;
            dut.eval();
            return;
        }

        clock_cycle!(dut);
    }

    panic!("clock peripheral write did not complete on D channel");
}

fn reset_clock_peripheral(dut: &mut ClockPeripheral) {
    dut.rst_n = 0;
    dut.mem_a_valid = 0;
    dut.mem_a_we = 0;
    dut.mem_a_addr = 0;
    dut.mem_a_wdata = 0;
    dut.mem_a_size = SIZE_WORD;
    dut.mem_d_ready = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    dut.eval();
}

#[test]
fn test_clock_peripheral_reset_state() {
    let runtime =
        create_clock_peripheral_runtime().expect("Failed to create clock peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<ClockPeripheral>()
        .expect("Failed to create clock peripheral model");

    reset_clock_peripheral(&mut dut);

    assert_eq!(
        dut.mem_a_ready, 1,
        "Clock peripheral should be idle after reset"
    );
    assert_eq!(
        dut.mem_d_valid, 0,
        "No response should be pending after reset"
    );

    let elapsed_us = read_register(&mut dut, ELAPSED_US);
    let elapsed_ms = read_register(&mut dut, ELAPSED_MS);
    let elapsed_s = read_register(&mut dut, ELAPSED_S);

    assert_eq!(elapsed_us, 0, "ELAPSED_US should be 0 after reset");
    assert_eq!(elapsed_ms, 0, "ELAPSED_MS should be 0 after reset");
    assert_eq!(elapsed_s, 0, "ELAPSED_S should be 0 after reset");
}

#[test]
fn test_clock_peripheral_idle_a_channel_ready() {
    let runtime =
        create_clock_peripheral_runtime().expect("Failed to create clock peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<ClockPeripheral>()
        .expect("Failed to create clock peripheral model");

    reset_clock_peripheral(&mut dut);

    for i in 0..32 {
        assert_eq!(
            dut.mem_a_ready, 1,
            "A channel should stay ready while idle at cycle {}",
            i
        );
        assert_eq!(
            dut.mem_d_valid, 0,
            "D channel should stay idle while no request is pending"
        );
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

    reset_clock_peripheral(&mut dut);

    let us_0 = read_register(&mut dut, ELAPSED_US);
    assert_eq!(us_0, 0, "Initial ELAPSED_US should be 0");

    // Each A/D transaction itself advances the free-running counter, so validate
    // that the counter moved forward by at least the explicit delay cycles here.
    clock_cycle!(dut);
    let us_1 = read_register(&mut dut, ELAPSED_US);
    assert!(
        us_1 > us_0,
        "ELAPSED_US should advance after one clock cycle"
    );

    clock_cycle!(dut);
    let us_2 = read_register(&mut dut, ELAPSED_US);
    assert!(us_2 > us_1, "ELAPSED_US should keep advancing");

    for _ in 0..8 {
        clock_cycle!(dut);
    }
    let us_10 = read_register(&mut dut, ELAPSED_US);
    assert!(
        us_10 >= us_2 + 8,
        "ELAPSED_US should advance by at least the additional 8 cycles"
    );
}

#[test]
fn test_clock_peripheral_milliseconds() {
    let runtime =
        create_clock_peripheral_runtime().expect("Failed to create clock peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<ClockPeripheral>()
        .expect("Failed to create clock peripheral model");

    reset_clock_peripheral(&mut dut);

    let ms_0 = read_register(&mut dut, ELAPSED_MS);
    assert_eq!(ms_0, 0, "Initial ELAPSED_MS should be 0");

    for _ in 0..1000 {
        clock_cycle!(dut);
    }
    let ms_1 = read_register(&mut dut, ELAPSED_MS);
    assert_eq!(ms_1, 1, "After 1000 cycles at 1MHz, should be 1 ms");

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

    reset_clock_peripheral(&mut dut);

    let s_0 = read_register(&mut dut, ELAPSED_S);
    assert_eq!(s_0, 0, "Initial ELAPSED_S should be 0");

    for _ in 0..500_000 {
        clock_cycle!(dut);
    }
    let s_500k = read_register(&mut dut, ELAPSED_S);
    assert_eq!(
        s_500k, 0,
        "After 500,000 cycles at 1MHz, should still be 0 s"
    );

    for _ in 0..500_000 {
        clock_cycle!(dut);
    }
    let s_1m = read_register(&mut dut, ELAPSED_S);
    assert_eq!(s_1m, 1, "After 1,000,000 cycles at 1MHz, should be 1 s");

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

    reset_clock_peripheral(&mut dut);

    for _ in 0..2_000_000 {
        clock_cycle!(dut);
    }

    let elapsed_us = read_register(&mut dut, ELAPSED_US);
    let elapsed_ms = read_register(&mut dut, ELAPSED_MS);
    let elapsed_s = read_register(&mut dut, ELAPSED_S);

    assert_eq!(elapsed_us, 2_000_000);
    assert_eq!(elapsed_ms, 2000);
    assert_eq!(elapsed_s, 2);
}

#[test]
fn test_clock_peripheral_read_only() {
    let runtime =
        create_clock_peripheral_runtime().expect("Failed to create clock peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<ClockPeripheral>()
        .expect("Failed to create clock peripheral model");

    reset_clock_peripheral(&mut dut);

    for _ in 0..10 {
        clock_cycle!(dut);
    }

    let us_before = read_register(&mut dut, ELAPSED_US);
    let ms_before = read_register(&mut dut, ELAPSED_MS);
    let s_before = read_register(&mut dut, ELAPSED_S);

    write_register(&mut dut, ELAPSED_US, 0xDEAD_BEEF);
    write_register(&mut dut, ELAPSED_MS, 0xCAFE_BABE);
    write_register(&mut dut, ELAPSED_S, 0x1234_5678);

    let us_after = read_register(&mut dut, ELAPSED_US);
    let ms_after = read_register(&mut dut, ELAPSED_MS);
    let s_after = read_register(&mut dut, ELAPSED_S);

    assert!(
        us_after > us_before,
        "ELAPSED_US should continue incrementing"
    );
    assert!(
        ms_after >= ms_before,
        "ELAPSED_MS should never move backwards"
    );
    assert!(s_after >= s_before, "ELAPSED_S should never move backwards");
    assert_ne!(us_after, 0xDEAD_BEEF, "ELAPSED_US should not be writable");
    assert_ne!(ms_after, 0xCAFE_BABE, "ELAPSED_MS should not be writable");
    assert_ne!(s_after, 0x1234_5678, "ELAPSED_S should not be writable");
}

#[test]
fn test_clock_peripheral_unmapped_register() {
    let runtime =
        create_clock_peripheral_runtime().expect("Failed to create clock peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<ClockPeripheral>()
        .expect("Failed to create clock peripheral model");

    reset_clock_peripheral(&mut dut);

    let unmapped = read_register(&mut dut, 0x0C);
    assert_eq!(unmapped, 0, "Unmapped register should return 0");

    let unmapped2 = read_register(&mut dut, 0x1C);
    assert_eq!(unmapped2, 0, "Unmapped register should return 0");
}
