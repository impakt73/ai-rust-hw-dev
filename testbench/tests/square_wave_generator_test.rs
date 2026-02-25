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
    for _ in 0..8 {
        clock_cycle(&mut dut);
        assert_eq!(
            dut.square_wave, 0,
            "square_wave must stay low while reset is asserted"
        );
    }
}

#[test]
fn test_square_wave_toggles_at_configured_rate() {
    let runtime = create_square_wave_generator_runtime()
        .expect("Failed to create square_wave_generator runtime");
    let mut dut = runtime
        .create_model_simple::<SquareWaveGeneratorWrapper>()
        .expect("Failed to create square_wave_generator model");

    // Wrapper parameters:
    //   CLK_FREQ_HZ = 100
    //   SQUARE_WAVE_FREQ_HZ = 5
    // => HALF_PERIOD_CYCLES = 100 / (2 * 5) = 10
    const HALF_PERIOD_CYCLES: usize = 10;

    dut.rst_n = 0;
    clock_cycle(&mut dut);
    dut.rst_n = 1;

    for cycle in 1..=40 {
        clock_cycle(&mut dut);
        let expected = ((cycle / HALF_PERIOD_CYCLES) % 2) as u8;
        assert_eq!(
            dut.square_wave, expected,
            "unexpected square_wave value at cycle {cycle}"
        );
    }
}
