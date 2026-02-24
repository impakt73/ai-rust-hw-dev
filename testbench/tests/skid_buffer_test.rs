use riscv_core::{
    create_skid_buffer_bypass_runtime, create_skid_buffer_runtime, SkidBufferBypassTestWrapper,
    SkidBufferTestWrapper,
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

fn reset_default(dut: &mut SkidBufferTestWrapper) {
    dut.rst_n = 0;
    dut.s_valid = 0;
    dut.s_data = 0;
    dut.m_ready = 0;
    clock_cycle!(dut);
    clock_cycle!(dut);
    dut.rst_n = 1;
    clock_cycle!(dut);
}

fn reset_bypass(dut: &mut SkidBufferBypassTestWrapper) {
    dut.rst_n = 0;
    dut.s_valid = 0;
    dut.s_data = 0;
    dut.m_ready = 0;
    clock_cycle!(dut);
    clock_cycle!(dut);
    dut.rst_n = 1;
    clock_cycle!(dut);
}

#[test]
fn test_skid_buffer_two_entry_backpressure() {
    let runtime = create_skid_buffer_runtime().expect("Failed to create skid_buffer runtime");
    let mut dut = runtime
        .create_model_simple::<SkidBufferTestWrapper>()
        .expect("Failed to create skid_buffer model");

    reset_default(&mut dut);
    assert_eq!(dut.m_valid, 0, "output must be invalid after reset");
    assert_eq!(dut.s_ready, 1, "input must be ready after reset");

    dut.s_valid = 1;
    dut.s_data = 0x11;
    dut.m_ready = 0;
    clock_cycle!(dut);
    assert_eq!(dut.m_valid, 1, "first write should fill output entry");
    assert_eq!(dut.m_data, 0x11);
    assert_eq!(dut.s_ready, 1, "second entry should still be available");

    dut.s_data = 0x22;
    clock_cycle!(dut);
    assert_eq!(dut.m_valid, 1);
    assert_eq!(dut.m_data, 0x11);
    assert_eq!(
        dut.s_ready, 0,
        "buffer must backpressure when both entries are full"
    );

    dut.s_data = 0x33;
    clock_cycle!(dut);
    assert_eq!(dut.m_data, 0x11, "extra write while full must be ignored");

    dut.s_valid = 0;
    dut.m_ready = 1;
    clock_cycle!(dut);
    assert_eq!(dut.m_valid, 1, "second entry should advance to output");
    assert_eq!(dut.m_data, 0x22, "buffer must preserve ordering");
}

#[test]
fn test_skid_buffer_non_bypass_inserts_bubble() {
    let runtime = create_skid_buffer_runtime().expect("Failed to create skid_buffer runtime");
    let mut dut = runtime
        .create_model_simple::<SkidBufferTestWrapper>()
        .expect("Failed to create skid_buffer model");

    reset_default(&mut dut);

    dut.s_valid = 1;
    dut.s_data = 0x41;
    dut.m_ready = 0;
    clock_cycle!(dut);
    assert_eq!(dut.m_valid, 1);
    assert_eq!(dut.m_data, 0x41);

    dut.s_data = 0x42;
    dut.m_ready = 1;
    clock_cycle!(dut);
    assert_eq!(dut.m_valid, 0, "non-bypass mode should insert a bubble");

    dut.s_valid = 0;
    dut.m_ready = 0;
    clock_cycle!(dut);
    assert_eq!(dut.m_valid, 1);
    assert_eq!(
        dut.m_data, 0x42,
        "queued value should appear after bubble cycle"
    );
}

#[test]
fn test_skid_buffer_bypass_refills_without_bubble() {
    let runtime =
        create_skid_buffer_bypass_runtime().expect("Failed to create skid_buffer bypass runtime");
    let mut dut = runtime
        .create_model_simple::<SkidBufferBypassTestWrapper>()
        .expect("Failed to create skid_buffer bypass model");

    reset_bypass(&mut dut);

    dut.s_valid = 1;
    dut.s_data = 0x51;
    dut.m_ready = 0;
    clock_cycle!(dut);
    assert_eq!(dut.m_valid, 1);
    assert_eq!(dut.m_data, 0x51);

    dut.s_data = 0x52;
    dut.m_ready = 1;
    clock_cycle!(dut);
    assert_eq!(dut.m_valid, 1, "bypass mode should keep output valid");
    assert_eq!(
        dut.m_data, 0x52,
        "new input should refill output without a bubble"
    );
}
