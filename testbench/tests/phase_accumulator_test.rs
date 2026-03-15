use riscv_core::{create_phase_accumulator_runtime, PhaseAccumulatorWrapper};

fn clock_cycle(dut: &mut PhaseAccumulatorWrapper) {
    dut.clk = 0;
    dut.eval();
    dut.clk = 1;
    dut.eval();
    dut.clk = 0;
    dut.eval();
}

#[test]
fn test_phase_accumulator_tick_stays_low_during_reset() {
    let runtime =
        create_phase_accumulator_runtime().expect("Failed to create phase_accumulator runtime");
    let mut dut = runtime
        .create_model_simple::<PhaseAccumulatorWrapper>()
        .expect("Failed to create phase_accumulator model");

    dut.rst = 1;
    for _ in 0..4 {
        clock_cycle(&mut dut);
        assert_eq!(dut.tick, 0, "tick must stay low while reset is asserted");
    }
}

#[test]
fn test_phase_accumulator_tick_count() {
    let runtime =
        create_phase_accumulator_runtime().expect("Failed to create phase_accumulator runtime");
    let mut dut = runtime
        .create_model_simple::<PhaseAccumulatorWrapper>()
        .expect("Failed to create phase_accumulator model");

    const PHASE_WIDTH: u32 = 16;
    const CLK_FREQ_HZ: u64 = 100;
    const TICK_FREQ_HZ: u64 = 33;
    const TEST_CYCLES: u64 = 600;

    let phase_modulus = 1u64 << PHASE_WIDTH;
    let phase_increment = ((TICK_FREQ_HZ * phase_modulus) + (CLK_FREQ_HZ / 2)) / CLK_FREQ_HZ; // Round to nearest
    let expected_ticks = (TEST_CYCLES * phase_increment) >> PHASE_WIDTH;

    dut.rst = 1;
    clock_cycle(&mut dut);
    dut.rst = 0;

    let mut tick_count = 0u64;
    for _ in 0..TEST_CYCLES {
        clock_cycle(&mut dut);
        tick_count += u64::from(dut.tick);
    }

    assert_eq!(
        tick_count, expected_ticks,
        "tick count must match phase accumulator expectation"
    );
}
