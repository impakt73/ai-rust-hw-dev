use riscv_core::{create_square_wave_generator_runtime, SquareWaveGeneratorWrapper};

fn clock_cycle(dut: &mut SquareWaveGeneratorWrapper) {
    dut.clk = 0;
    dut.eval();
    dut.clk = 1;
    dut.eval();
    dut.clk = 0;
    dut.eval();
}

#[test]
fn test_square_wave_stays_low_during_reset() {
    let runtime = create_square_wave_generator_runtime()
        .expect("Failed to create square_wave_generator runtime");
    let mut dut = runtime
        .create_model_simple::<SquareWaveGeneratorWrapper>()
        .expect("Failed to create square_wave_generator model");

    dut.rst_n = 0;
    for _ in 0..4 {
        clock_cycle(&mut dut);
        assert_eq!(
            dut.square_wave, 0,
            "square_wave must stay low while reset is asserted"
        );
    }
}

#[test]
fn test_square_wave_toggles_every_half_period() {
    let runtime = create_square_wave_generator_runtime()
        .expect("Failed to create square_wave_generator runtime");
    let mut dut = runtime
        .create_model_simple::<SquareWaveGeneratorWrapper>()
        .expect("Failed to create square_wave_generator model");

    dut.rst_n = 0;
    clock_cycle(&mut dut);
    dut.rst_n = 1;

    let expected = [0u8, 1, 1, 0, 0, 1, 1, 0];
    for expected_value in expected {
        clock_cycle(&mut dut);
        assert_eq!(
            dut.square_wave, expected_value,
            "square_wave must toggle every two cycles with wrapper parameters"
        );
    }
}
