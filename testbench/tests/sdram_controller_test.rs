use riscv_core::AsDynamicVerilatedModel;
use riscv_core::{
    create_sdram_controller_runtime, SdramController100MhzTestWrapper,
    SdramControllerCas2TestWrapper, SdramControllerExtraLatencyTestWrapper,
    SdramControllerTestWrapper,
};

const RESET_CYCLES: usize = 4;
const INIT_TIMEOUT_CYCLES: usize = 4_000;
const OP_TIMEOUT_CYCLES: usize = 128;
const IDLE_REFRESH_OBSERVE_CYCLES: usize = 1_200;

macro_rules! tick {
    ($dut:expr) => {{
        $dut.controller_clk = 0;
        $dut.chip_clk = 0;
        $dut.sample_clk = 0;
        $dut.eval();

        $dut.sample_clk = 1;
        $dut.eval();

        $dut.sample_clk = 0;
        $dut.eval();

        $dut.controller_clk = 1;
        $dut.eval();

        $dut.controller_clk = 0;
        $dut.eval();

        $dut.chip_clk = 1;
        $dut.eval();

        $dut.chip_clk = 0;
        $dut.eval();
    }};
}

macro_rules! init_inputs {
    ($dut:expr) => {{
        $dut.controller_clk = 0;
        $dut.chip_clk = 0;
        $dut.sample_clk = 0;
        $dut.rst = 0;
        $dut.word_rd = 0;
        $dut.word_wr = 0;
        $dut.word_addr = 0;
        $dut.word_data = 0;
        $dut.eval();
    }};
}

macro_rules! reset_and_wait_for_init {
    ($dut:expr) => {{
        init_inputs!($dut);
        $dut.rst = 1;
        for _ in 0..RESET_CYCLES {
            tick!($dut);
        }

        $dut.rst = 0;
        let mut cycles = 0usize;
        while cycles < INIT_TIMEOUT_CYCLES {
            if $dut.word_busy == 0 {
                break;
            }
            tick!($dut);
            cycles += 1;
        }

        assert_eq!(
            $dut.word_busy, 0,
            "controller did not finish initialization"
        );
        cycles
    }};
}

macro_rules! wait_for_idle {
    ($dut:expr, $timeout:expr) => {{
        let mut cycles = 0usize;
        while cycles < $timeout {
            if $dut.word_busy == 0 {
                break;
            }
            tick!($dut);
            cycles += 1;
        }

        assert_eq!($dut.word_busy, 0, "controller did not return idle");
        cycles
    }};
}

macro_rules! wait_for_busy_assert {
    ($dut:expr, $timeout:expr) => {{
        let mut cycles = 0usize;
        while cycles < $timeout {
            if $dut.word_busy != 0 {
                break;
            }
            tick!($dut);
            cycles += 1;
        }

        assert_ne!(
            $dut.word_busy, 0,
            "controller never started the queued request"
        );
        cycles
    }};
}

macro_rules! write_word {
    ($dut:expr, $addr:expr, $data:expr) => {{
        assert_eq!($dut.word_busy, 0, "controller must be idle before a write");
        $dut.word_addr = $addr;
        $dut.word_data = $data;
        $dut.word_wr = 1;
        $dut.eval();
        tick!($dut);
        $dut.word_wr = 0;
        $dut.eval();
        let queued_cycles = wait_for_busy_assert!($dut, OP_TIMEOUT_CYCLES);
        queued_cycles + wait_for_idle!($dut, OP_TIMEOUT_CYCLES)
    }};
}

macro_rules! read_word {
    ($dut:expr, $addr:expr) => {{
        assert_eq!($dut.word_busy, 0, "controller must be idle before a read");
        $dut.word_addr = $addr;
        $dut.word_rd = 1;
        $dut.eval();
        tick!($dut);
        $dut.word_rd = 0;
        $dut.eval();
        let queued_cycles = wait_for_busy_assert!($dut, OP_TIMEOUT_CYCLES);
        let cycles = queued_cycles + wait_for_idle!($dut, OP_TIMEOUT_CYCLES);
        (cycles, $dut.word_q)
    }};
}

#[test]
fn test_sdram_controller_round_trips_data_and_programs_mode_register() {
    let runtime =
        create_sdram_controller_runtime().expect("Failed to create sdram_controller runtime");
    let mut dut = testbench::create_testbench_model::<SdramControllerTestWrapper>(&runtime)
        .expect("Failed to create sdram_controller model");

    let init_cycles = reset_and_wait_for_init!(&mut dut);
    assert!(
        init_cycles > 0,
        "init should take multiple controller cycles"
    );

    let write_cycles = write_word!(&mut dut, 0x000020u32, 0xCAFE_BABEu32);
    let (read_cycles, read_data) = read_word!(&mut dut, 0x000020u32);

    assert_eq!(read_data, 0xCAFE_BABE);
    assert_eq!((dut.loaded_mode_reg >> 4) & 0x7, 3);
    assert_eq!(
        dut.write_cmd_count, 2,
        "32-bit writes should emit two halfword writes"
    );
    assert_eq!(
        dut.read_cmd_count, 2,
        "32-bit reads should emit two halfword reads"
    );
    assert!(write_cycles > 0);
    assert!(read_cycles > 0);
}

#[test]
fn test_sdram_controller_cas_latency_parameter_changes_read_completion_timing() {
    let runtime =
        create_sdram_controller_runtime().expect("Failed to create sdram_controller runtime");

    let mut cas3 = testbench::create_testbench_model::<SdramControllerTestWrapper>(&runtime)
        .expect("Failed to create default sdram_controller model");
    reset_and_wait_for_init!(&mut cas3);
    let _ = write_word!(&mut cas3, 0x000024u32, 0x1234_5678u32);
    let (cas3_read_cycles, cas3_data) = read_word!(&mut cas3, 0x000024u32);

    let mut cas2 = testbench::create_testbench_model::<SdramControllerCas2TestWrapper>(&runtime)
        .expect("Failed to create cas2 sdram_controller model");
    reset_and_wait_for_init!(&mut cas2);
    let _ = write_word!(&mut cas2, 0x000024u32, 0x1234_5678u32);
    let (cas2_read_cycles, cas2_data) = read_word!(&mut cas2, 0x000024u32);

    assert_eq!(cas3_data, 0x1234_5678);
    assert_eq!(cas2_data, 0x1234_5678);
    assert_eq!((cas3.loaded_mode_reg >> 4) & 0x7, 3);
    assert_eq!((cas2.loaded_mode_reg >> 4) & 0x7, 2);
    assert_eq!(
        cas3_read_cycles,
        cas2_read_cycles + 1,
        "CL=3 should take one more controller cycle than CL=2"
    );
}

#[test]
fn test_sdram_controller_extra_read_latency_parameter_adds_completion_cycles() {
    let runtime =
        create_sdram_controller_runtime().expect("Failed to create sdram_controller runtime");

    let mut baseline = testbench::create_testbench_model::<SdramControllerTestWrapper>(&runtime)
        .expect("Failed to create default sdram_controller model");
    reset_and_wait_for_init!(&mut baseline);
    let _ = write_word!(&mut baseline, 0x000028u32, 0x89AB_CDEFu32);
    let (baseline_read_cycles, baseline_data) = read_word!(&mut baseline, 0x000028u32);

    let mut extra =
        testbench::create_testbench_model::<SdramControllerExtraLatencyTestWrapper>(&runtime)
            .expect("Failed to create extra-latency sdram_controller model");
    reset_and_wait_for_init!(&mut extra);
    let _ = write_word!(&mut extra, 0x000028u32, 0x89AB_CDEFu32);
    let (extra_read_cycles, extra_data) = read_word!(&mut extra, 0x000028u32);

    assert_eq!(baseline_data, 0x89AB_CDEF);
    assert_eq!(extra_data, 0x89AB_CDEF);
    assert_eq!(
        extra_read_cycles,
        baseline_read_cycles + 2,
        "extra read latency should add directly to controller-side completion time"
    );
}

#[test]
fn test_sdram_controller_controller_clock_frequency_parameter_scales_timing_counters() {
    let runtime =
        create_sdram_controller_runtime().expect("Failed to create sdram_controller runtime");

    let mut default_freq =
        testbench::create_testbench_model::<SdramControllerTestWrapper>(&runtime)
            .expect("Failed to create default sdram_controller model");
    let default_init_cycles = reset_and_wait_for_init!(&mut default_freq);

    let mut slow_freq =
        testbench::create_testbench_model::<SdramController100MhzTestWrapper>(&runtime)
            .expect("Failed to create 100MHz sdram_controller model");
    let slow_init_cycles = reset_and_wait_for_init!(&mut slow_freq);
    let _ = write_word!(&mut slow_freq, 0x00002Cu32, 0x0BAD_F00Du32);
    let (_, read_data) = read_word!(&mut slow_freq, 0x00002Cu32);

    assert_eq!(read_data, 0x0BAD_F00D);
    assert!(
        default_init_cycles > slow_init_cycles,
        "higher controller frequencies should require more cycles for the same init delay"
    );
}

#[test]
fn test_sdram_controller_issues_refreshes_while_idle() {
    let runtime =
        create_sdram_controller_runtime().expect("Failed to create sdram_controller runtime");
    let mut dut = testbench::create_testbench_model::<SdramControllerTestWrapper>(&runtime)
        .expect("Failed to create sdram_controller model");

    reset_and_wait_for_init!(&mut dut);
    let refreshes_before = dut.refresh_cmd_count;

    // One tREFI window at 133 MHz is about 1_040 controller cycles, so 1_200
    // cycles guarantees the idle controller must emit at least one refresh.
    for _ in 0..IDLE_REFRESH_OBSERVE_CYCLES {
        tick!(&mut dut);
    }

    assert!(
        dut.refresh_cmd_count > refreshes_before,
        "idle controller should periodically issue AUTO REFRESH commands"
    );
}
