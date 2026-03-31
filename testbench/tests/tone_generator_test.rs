use riscv_core::{create_tone_generator_runtime, ToneGeneratorTestWrapper};

const TABLE_SIZE: u16 = 1024;
const PHASE_WIDTH: usize = 32;
const TABLE_ADDR_WIDTH: usize = TABLE_SIZE.ilog2() as usize;
const PIPELINE_STAGES: usize = 4;
const HALF_TABLE_SIZE: u16 = TABLE_SIZE / 2;
const ONE_INDEX_STEP_TUNING_WORD: u32 = 1u32 << (PHASE_WIDTH - TABLE_ADDR_WIDTH);

fn expected_sample(index: u16) -> u16 {
    const QADDR_W: usize = TABLE_ADDR_WIDTH - 2;
    const QADDR_MASK: u16 = (1u16 << QADDR_W) - 1;
    const MAX_SIGNED: f64 = 32767.0;
    const MID_TREAD_OFFSET: f64 = 0.5;

    let invert_result = (index >> (TABLE_ADDR_WIDTH - 1)) & 1 != 0;
    let invert_index = (index >> (TABLE_ADDR_WIDTH - 2)) & 1 != 0;
    let qaddr = index & QADDR_MASK;
    let rom_addr = if invert_index {
        (!qaddr) & QADDR_MASK
    } else {
        qaddr
    };

    let normalized_phase = (rom_addr as f64 + MID_TREAD_OFFSET) / (TABLE_SIZE as f64);
    let phase = 2.0 * std::f64::consts::PI * normalized_phase;
    let raw = (MAX_SIGNED * phase.sin()).round() as i16;
    let raw_bits = raw as u16;

    if invert_result {
        raw_bits.wrapping_neg()
    } else {
        raw_bits
    }
}

fn clock_cycle(dut: &mut ToneGeneratorTestWrapper) {
    dut.clk = 0;
    dut.eval();
    dut.clk = 1;
    dut.eval();
}

fn flush_reset_pipeline(dut: &mut ToneGeneratorTestWrapper) {
    dut.rst = 1;
    dut.tuning_word = 0;
    for _ in 0..(PIPELINE_STAGES + 2) {
        clock_cycle(dut);
    }
}

#[test]
fn test_tone_generator_reset_holds_zero_phase_sample() {
    let runtime = create_tone_generator_runtime().expect("Failed to create tone_generator runtime");
    let mut dut = runtime
        .create_model_simple::<ToneGeneratorTestWrapper>()
        .expect("Failed to create tone_generator model");

    dut.rst = 1;
    dut.tuning_word = ONE_INDEX_STEP_TUNING_WORD;
    for _ in 0..(PIPELINE_STAGES + 2) {
        clock_cycle(&mut dut);
    }

    assert_eq!(
        dut.sample,
        expected_sample(0),
        "reset must hold the phase accumulator at index 0"
    );
    assert_eq!(dut.valid, 0, "valid must stay low while reset is asserted");

    for _ in 0..4 {
        clock_cycle(&mut dut);
        assert_eq!(
            dut.sample,
            expected_sample(0),
            "sample must remain at the index-0 sine value while reset is asserted"
        );
        assert_eq!(dut.valid, 0, "valid must stay low while reset is asserted");
    }
}

#[test]
fn test_tone_generator_zero_tuning_word_holds_constant_sample() {
    let runtime = create_tone_generator_runtime().expect("Failed to create tone_generator runtime");
    let mut dut = runtime
        .create_model_simple::<ToneGeneratorTestWrapper>()
        .expect("Failed to create tone_generator model");

    flush_reset_pipeline(&mut dut);
    dut.rst = 0;

    for cycle in 0..(PIPELINE_STAGES + 2) {
        clock_cycle(&mut dut);
        assert_eq!(
            dut.valid != 0,
            cycle >= PIPELINE_STAGES - 1,
            "valid must assert only after the sine-table lookup latency"
        );
        assert_eq!(
            dut.sample,
            expected_sample(0),
            "zero tuning word must keep the output at the index-0 sine sample"
        );
    }
}

#[test]
fn test_tone_generator_advances_one_table_index_per_cycle() {
    let runtime = create_tone_generator_runtime().expect("Failed to create tone_generator runtime");
    let mut dut = runtime
        .create_model_simple::<ToneGeneratorTestWrapper>()
        .expect("Failed to create tone_generator model");

    flush_reset_pipeline(&mut dut);
    dut.rst = 0;
    dut.tuning_word = ONE_INDEX_STEP_TUNING_WORD;

    for cycle in 0..PIPELINE_STAGES {
        clock_cycle(&mut dut);
        assert_eq!(
            dut.valid != 0,
            cycle == PIPELINE_STAGES - 1,
            "valid must assert after exactly {} clocks of lookup latency",
            PIPELINE_STAGES
        );
    }

    for expected_index in 0..8u16 {
        assert_ne!(dut.valid, 0, "sample checks require valid output data");
        assert_eq!(
            dut.sample,
            expected_sample(expected_index),
            "sample must match sine-table index {expected_index} when the tuning word advances one index per cycle"
        );
        clock_cycle(&mut dut);
    }
}

#[test]
fn test_tone_generator_zero_cross_aligns_with_output_sample() {
    let runtime = create_tone_generator_runtime().expect("Failed to create tone_generator runtime");
    let mut dut = runtime
        .create_model_simple::<ToneGeneratorTestWrapper>()
        .expect("Failed to create tone_generator model");

    flush_reset_pipeline(&mut dut);
    dut.rst = 0;
    dut.tuning_word = ONE_INDEX_STEP_TUNING_WORD;

    for cycle in 0..PIPELINE_STAGES {
        clock_cycle(&mut dut);
        assert_eq!(
            dut.valid != 0,
            cycle == PIPELINE_STAGES - 1,
            "valid must assert after exactly {} clocks of lookup latency",
            PIPELINE_STAGES
        );
    }

    for expected_index in 0..TABLE_SIZE {
        assert_ne!(
            dut.valid, 0,
            "zero-cross alignment checks require valid output data"
        );
        assert_eq!(
            dut.sample,
            expected_sample(expected_index),
            "sample must match sine-table index {expected_index}"
        );
        assert_eq!(
            dut.zero_cross != 0,
            expected_index == 0 || expected_index == HALF_TABLE_SIZE,
            "zero_cross must align with the output samples nearest the sine zero crossings at index {expected_index}"
        );
        clock_cycle(&mut dut);
    }
}
