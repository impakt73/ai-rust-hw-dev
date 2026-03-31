use riscv_core::{create_activity_indicator_runtime, ActivityIndicatorWrapper};

use riscv_core::AsDynamicVerilatedModel;
fn clock_cycle(dut: &mut ActivityIndicatorWrapper) {
    dut.clk = 0;
    dut.eval();
    dut.clk = 1;
    dut.eval();
    dut.clk = 0;
    dut.eval();
}

#[test]
fn test_activity_indicator_stays_low_during_reset() {
    let runtime =
        create_activity_indicator_runtime().expect("Failed to create activity_indicator runtime");
    let mut dut = runtime
        .create_model_simple::<ActivityIndicatorWrapper>()
        .expect("Failed to create activity_indicator model");

    dut.rst = 1;
    dut.activity = 0;
    for _ in 0..4 {
        clock_cycle(&mut dut);
        assert_eq!(
            dut.indicator, 0,
            "indicator must stay low while reset is asserted"
        );
    }
}

#[test]
fn test_activity_indicator_emits_single_cycle_per_trigger() {
    let runtime =
        create_activity_indicator_runtime().expect("Failed to create activity_indicator runtime");
    let mut dut = runtime
        .create_model_simple::<ActivityIndicatorWrapper>()
        .expect("Failed to create activity_indicator model");

    dut.rst = 1;
    dut.activity = 0;
    clock_cycle(&mut dut);
    dut.rst = 0;

    dut.activity = 1;
    clock_cycle(&mut dut);
    assert_eq!(dut.indicator, 1, "indicator must go high on trigger");

    dut.activity = 0;
    let expected = [1u8, 0, 0, 0, 0];
    for expected_value in expected {
        clock_cycle(&mut dut);
        assert_eq!(
            dut.indicator, expected_value,
            "indicator must complete one full cycle then remain low"
        );
    }
}

#[test]
fn test_activity_indicator_ignores_retrigger_while_busy() {
    let runtime =
        create_activity_indicator_runtime().expect("Failed to create activity_indicator runtime");
    let mut dut = runtime
        .create_model_simple::<ActivityIndicatorWrapper>()
        .expect("Failed to create activity_indicator model");

    dut.rst = 1;
    dut.activity = 0;
    clock_cycle(&mut dut);
    dut.rst = 0;

    dut.activity = 1;
    clock_cycle(&mut dut);
    assert_eq!(dut.indicator, 1, "first trigger must start pulse");

    dut.activity = 0;
    clock_cycle(&mut dut);
    assert_eq!(
        dut.indicator, 1,
        "indicator must stay high for first half-period"
    );

    dut.activity = 1;
    clock_cycle(&mut dut);
    assert_eq!(
        dut.indicator, 0,
        "retrigger during active pulse must not restart or interrupt the cycle"
    );

    dut.activity = 0;
    for _ in 0..3 {
        clock_cycle(&mut dut);
        assert_eq!(
            dut.indicator, 0,
            "indicator must return to idle low after pulse"
        );
    }
}
