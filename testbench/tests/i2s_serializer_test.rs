use riscv_core::{
    create_i2s_serializer_runtime, I2sSerializerEqualWidthWrapper, I2sSerializerExpandWrapper,
    I2sSerializerTruncateWrapper,
};

macro_rules! clock_cycle {
    ($dut:expr) => {
        // End each helper cycle with clk low to leave outputs in a stable
        // post-edge state for assertions after the active rising edge.
        $dut.clk = 0;
        $dut.eval();
        $dut.clk = 1;
        $dut.eval();
        $dut.clk = 0;
        $dut.eval();
    };
}

macro_rules! reset_dut {
    ($dut:expr) => {
        $dut.rst_n = 0;
        $dut.sample_valid = 0;
        $dut.sample_data = 0;
        clock_cycle!($dut);
        $dut.rst_n = 1;
        $dut.eval();
    };
}

macro_rules! capture_bits {
    ($dut:expr, $cycles:expr) => {{
        // Sample i2s_sd after each full low-high-low helper cycle so each entry
        // reflects the stable serial-data value driven for that transmitted bit.
        let mut bits = Vec::with_capacity($cycles);
        for _ in 0..$cycles {
            clock_cycle!($dut);
            bits.push($dut.i2s_sd);
        }
        bits
    }};
}

#[test]
fn test_i2s_serializer_zero_fills_when_no_sample_is_available() {
    let runtime = create_i2s_serializer_runtime().expect("Failed to create I2S serializer runtime");
    let mut dut = runtime
        .create_model_simple::<I2sSerializerEqualWidthWrapper>()
        .expect("Failed to create equal-width I2S serializer model");

    reset_dut!(&mut dut);

    assert_eq!(
        dut.sample_ready, 1,
        "serializer should request a sample after reset"
    );
    assert_eq!(
        dut.i2s_lrclk, 0,
        "first word should start on the left channel"
    );
    assert_eq!(
        dut.i2s_sd, 0,
        "serial data should idle low before the first word"
    );

    dut.clk = 0;
    dut.eval();
    assert_eq!(
        dut.i2s_bclk, 0,
        "bit clock output must mirror clk low level"
    );
    dut.clk = 1;
    dut.eval();
    assert_eq!(
        dut.i2s_bclk, 1,
        "bit clock output must mirror clk high level"
    );
    dut.clk = 0;
    dut.eval();

    assert_eq!(
        dut.sample_ready, 0,
        "reload slot should complete during the first active bit-clock edge"
    );
    assert_eq!(
        dut.i2s_lrclk, 0,
        "left channel select should remain low for first word"
    );

    let bits = capture_bits!(&mut dut, 8);
    assert_eq!(bits, vec![0; 8], "missing input sample must transmit zeros");
    assert_eq!(
        dut.sample_ready, 1,
        "serializer should request the next sample after finishing a word"
    );
}

#[test]
fn test_i2s_serializer_serializes_back_to_back_samples_and_toggles_lrclk() {
    let runtime = create_i2s_serializer_runtime().expect("Failed to create I2S serializer runtime");
    let mut dut = runtime
        .create_model_simple::<I2sSerializerEqualWidthWrapper>()
        .expect("Failed to create equal-width I2S serializer model");

    reset_dut!(&mut dut);

    dut.sample_data = 0xA5;
    dut.sample_valid = 1;
    clock_cycle!(&mut dut);
    dut.sample_valid = 0;
    dut.eval();

    assert_eq!(
        dut.sample_ready, 0,
        "serializer must hold off producer while shifting"
    );
    assert_eq!(
        dut.i2s_lrclk, 0,
        "first accepted sample should use left channel"
    );

    let left_bits = capture_bits!(&mut dut, 8);
    assert_eq!(
        left_bits,
        vec![1, 0, 1, 0, 0, 1, 0, 1],
        "equal-width serializer must shift MSB-first data"
    );
    assert_eq!(
        dut.sample_ready, 1,
        "serializer must request a new sample per word"
    );

    dut.sample_data = 0x3C;
    dut.sample_valid = 1;
    clock_cycle!(&mut dut);
    dut.sample_valid = 0;
    dut.eval();

    assert_eq!(
        dut.i2s_lrclk, 1,
        "second word should switch to the right channel"
    );
    assert_eq!(
        dut.i2s_sd, 0,
        "serializer should insert the I2S alignment cycle before the next MSB"
    );

    let right_bits = capture_bits!(&mut dut, 8);
    assert_eq!(
        right_bits,
        vec![0, 0, 1, 1, 1, 1, 0, 0],
        "second sample must also shift MSB-first"
    );
}

#[test]
fn test_i2s_serializer_pads_narrow_samples_with_trailing_zeros() {
    let runtime = create_i2s_serializer_runtime().expect("Failed to create I2S serializer runtime");
    let mut dut = runtime
        .create_model_simple::<I2sSerializerExpandWrapper>()
        .expect("Failed to create expanding I2S serializer model");

    reset_dut!(&mut dut);

    dut.sample_data = 0xA5;
    dut.sample_valid = 1;
    clock_cycle!(&mut dut);
    dut.sample_valid = 0;
    dut.eval();

    let bits = capture_bits!(&mut dut, 12);
    assert_eq!(
        bits,
        vec![1, 0, 1, 0, 0, 1, 0, 1, 0, 0, 0, 0],
        "narrower input samples must be left-justified and zero-padded in the output word"
    );
}

#[test]
fn test_i2s_serializer_truncates_wider_samples_to_most_significant_bits() {
    let runtime = create_i2s_serializer_runtime().expect("Failed to create I2S serializer runtime");
    let mut dut = runtime
        .create_model_simple::<I2sSerializerTruncateWrapper>()
        .expect("Failed to create truncating I2S serializer model");

    reset_dut!(&mut dut);

    dut.sample_data = 0xABC;
    dut.sample_valid = 1;
    clock_cycle!(&mut dut);
    dut.sample_valid = 0;
    dut.eval();

    let bits = capture_bits!(&mut dut, 8);
    assert_eq!(
        bits,
        vec![1, 0, 1, 0, 1, 0, 1, 1],
        "wider input samples must drop extra least-significant bits"
    );
}
