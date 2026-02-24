use riscv_core::{
    create_skid_buffer_bypass_runtime, create_skid_buffer_default_runtime, SkidBufferBypassWrapper,
    SkidBufferDefaultWrapper,
};

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

#[test]
fn test_skid_buffer_default_is_registered() {
    let runtime =
        create_skid_buffer_default_runtime().expect("Failed to create skid_buffer default runtime");
    let mut dut = runtime
        .create_model_simple::<SkidBufferDefaultWrapper>()
        .expect("Failed to create skid_buffer default model");

    dut.rst_n = 0;
    dut.in_valid = 0;
    dut.in_data = 0;
    dut.out_ready = 1;
    clock_cycle!(dut);

    dut.rst_n = 1;
    dut.in_valid = 1;
    dut.in_data = 0x3C;
    dut.out_ready = 1;
    dut.eval();
    assert_eq!(
        dut.out_valid, 0,
        "default configuration should not bypass combinationally"
    );

    clock_cycle!(dut);
    assert_eq!(
        dut.out_valid, 1,
        "output should become valid after one clock"
    );
    assert_eq!(dut.out_data, 0x3C, "registered data should match input");
}

#[test]
fn test_skid_buffer_bypass_passes_through_when_empty() {
    let runtime =
        create_skid_buffer_bypass_runtime().expect("Failed to create skid_buffer bypass runtime");
    let mut dut = runtime
        .create_model_simple::<SkidBufferBypassWrapper>()
        .expect("Failed to create skid_buffer bypass model");

    dut.rst_n = 0;
    dut.in_valid = 0;
    dut.in_data = 0;
    dut.out_ready = 1;
    clock_cycle!(dut);

    dut.rst_n = 1;
    dut.in_valid = 1;
    dut.in_data = 0xA5;
    dut.out_ready = 1;
    dut.eval();
    assert_eq!(
        dut.out_valid, 1,
        "bypass mode should assert out_valid same cycle"
    );
    assert_eq!(
        dut.out_data, 0xA5,
        "bypass mode should forward data same cycle"
    );
}

#[test]
fn test_skid_buffer_bypass_captures_when_stalled() {
    let runtime =
        create_skid_buffer_bypass_runtime().expect("Failed to create skid_buffer bypass runtime");
    let mut dut = runtime
        .create_model_simple::<SkidBufferBypassWrapper>()
        .expect("Failed to create skid_buffer bypass model");

    dut.rst_n = 0;
    dut.in_valid = 0;
    dut.in_data = 0;
    dut.out_ready = 0;
    clock_cycle!(dut);

    dut.rst_n = 1;
    dut.in_valid = 1;
    dut.in_data = 0x5A;
    dut.out_ready = 0;
    dut.eval();
    assert_eq!(dut.out_valid, 1, "bypass should present data while empty");
    assert_eq!(
        dut.out_data, 0x5A,
        "bypass data should be visible immediately"
    );

    clock_cycle!(dut);

    dut.in_valid = 0;
    dut.out_ready = 1;
    dut.eval();
    assert_eq!(dut.out_valid, 1, "stalled data should be buffered");
    assert_eq!(dut.out_data, 0x5A, "buffered data should be retained");
}
