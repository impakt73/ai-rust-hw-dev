use riscv_core::{
    create_debouncer_runtime, create_debouncer_single_cycle_runtime, DebouncerSingleCycleWrapper,
    DebouncerWrapper,
};
use riscv_core::AsDynamicVerilatedModel;

const WRAPPER_CLK_FREQ_HZ: u64 = 1_000_000;
const WRAPPER_STABLE_TIME_US: u64 = 3;
const WRAPPER_STABLE_CYCLES: usize = ((WRAPPER_CLK_FREQ_HZ as u128)
    * (WRAPPER_STABLE_TIME_US as u128))
    .div_ceil(1_000_000u128) as usize;
const EXPECTED_TRANSITION_DELAY_CYCLES: usize = WRAPPER_STABLE_CYCLES - 1;

fn clock_cycle(dut: &mut DebouncerWrapper) {
    dut.clk = 0;
    dut.eval();
    dut.clk = 1;
    dut.eval();
    dut.clk = 0;
    dut.eval();
}

fn clock_cycle_single(dut: &mut DebouncerSingleCycleWrapper) {
    dut.clk = 0;
    dut.eval();
    dut.clk = 1;
    dut.eval();
    dut.clk = 0;
    dut.eval();
}

#[test]
fn test_debouncer_stays_low_during_reset() {
    let runtime = create_debouncer_runtime().expect("Failed to create debouncer runtime");
    let mut dut = runtime
        .create_model_simple::<DebouncerWrapper>()
        .expect("Failed to create debouncer model");

    dut.rst = 1;
    dut.noisy_in = 1;

    for _ in 0..4 {
        clock_cycle(&mut dut);
        assert_eq!(
            dut.debounced_out, 0,
            "debounced_out must stay low while reset is asserted"
        );
    }
}

#[test]
fn test_debouncer_rejects_short_glitches() {
    let runtime = create_debouncer_runtime().expect("Failed to create debouncer runtime");
    let mut dut = runtime
        .create_model_simple::<DebouncerWrapper>()
        .expect("Failed to create debouncer model");

    dut.rst = 1;
    dut.noisy_in = 0;
    clock_cycle(&mut dut);
    dut.rst = 0;

    dut.noisy_in = 1;
    clock_cycle(&mut dut);
    clock_cycle(&mut dut);

    dut.noisy_in = 0;
    for _ in 0..4 {
        clock_cycle(&mut dut);
        assert_eq!(
            dut.debounced_out, 0,
            "short glitches must not change the debounced output"
        );
    }
}

#[test]
fn test_debouncer_accepts_only_stable_level_changes() {
    let runtime = create_debouncer_runtime().expect("Failed to create debouncer runtime");
    let mut dut = runtime
        .create_model_simple::<DebouncerWrapper>()
        .expect("Failed to create debouncer model");

    dut.rst = 1;
    dut.noisy_in = 0;
    clock_cycle(&mut dut);
    dut.rst = 0;

    dut.noisy_in = 1;
    for expected in (0..=EXPECTED_TRANSITION_DELAY_CYCLES).map(|cycle_idx| {
        if cycle_idx < EXPECTED_TRANSITION_DELAY_CYCLES {
            0
        } else {
            1
        }
    }) {
        clock_cycle(&mut dut);
        assert_eq!(
            dut.debounced_out, expected,
            "output should only rise after the input is stable for the debounce interval"
        );
    }

    dut.noisy_in = 0;
    for expected in (0..=EXPECTED_TRANSITION_DELAY_CYCLES).map(|cycle_idx| {
        if cycle_idx < EXPECTED_TRANSITION_DELAY_CYCLES {
            1
        } else {
            0
        }
    }) {
        clock_cycle(&mut dut);
        assert_eq!(
            dut.debounced_out, expected,
            "output should only fall after the input is stable for the debounce interval"
        );
    }
}

#[test]
fn test_debouncer_single_cycle_updates_on_first_stable_sample() {
    let runtime =
        create_debouncer_single_cycle_runtime().expect("Failed to create single-cycle runtime");
    let mut dut = runtime
        .create_model_simple::<DebouncerSingleCycleWrapper>()
        .expect("Failed to create single-cycle debouncer model");

    dut.rst = 1;
    dut.noisy_in = 0;
    clock_cycle_single(&mut dut);
    dut.rst = 0;

    dut.noisy_in = 1;
    clock_cycle_single(&mut dut);
    assert_eq!(
        dut.debounced_out, 1,
        "single-cycle debounce should accept the first stable high sample"
    );

    dut.noisy_in = 0;
    clock_cycle_single(&mut dut);
    assert_eq!(
        dut.debounced_out, 0,
        "single-cycle debounce should accept the first stable low sample"
    );
}
