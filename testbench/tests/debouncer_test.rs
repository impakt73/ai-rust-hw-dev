use riscv_core::{create_debouncer_runtime, DebouncerWrapper};

fn clock_cycle(dut: &mut DebouncerWrapper) {
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

    dut.rst_n = 0;
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

    dut.rst_n = 0;
    dut.noisy_in = 0;
    clock_cycle(&mut dut);
    dut.rst_n = 1;

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

    dut.rst_n = 0;
    dut.noisy_in = 0;
    clock_cycle(&mut dut);
    dut.rst_n = 1;

    dut.noisy_in = 1;
    for expected in [0u8, 0, 0, 0, 1] {
        clock_cycle(&mut dut);
        assert_eq!(
            dut.debounced_out, expected,
            "output should only rise after the input is stable for the debounce interval"
        );
    }

    dut.noisy_in = 0;
    for expected in [1u8, 1, 1, 1, 0] {
        clock_cycle(&mut dut);
        assert_eq!(
            dut.debounced_out, expected,
            "output should only fall after the input is stable for the debounce interval"
        );
    }
}
