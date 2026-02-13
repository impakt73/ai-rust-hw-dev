use riscv_core::{
    create_ff_sync_param_wrapper_runtime, create_ff_sync_runtime, FfSyncDefaultWrapper,
    FfSyncParamWrapper,
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
fn test_ff_sync_default_reset_state() {
    let runtime = create_ff_sync_runtime().expect("Failed to create ff_sync runtime");
    let mut dut = runtime
        .create_model_simple::<FfSyncDefaultWrapper>()
        .unwrap();

    dut.clk = 0;
    dut.rst_n = 0;
    dut.din = 1;
    clock_cycle!(dut);

    assert_eq!(dut.dout, 0, "dout should clear to 0 during reset");
}

#[test]
fn test_ff_sync_default_three_stage_delay() {
    let runtime = create_ff_sync_runtime().expect("Failed to create ff_sync runtime");
    let mut dut = runtime
        .create_model_simple::<FfSyncDefaultWrapper>()
        .unwrap();

    dut.clk = 0;
    dut.rst_n = 0;
    dut.din = 0;
    clock_cycle!(dut);

    dut.rst_n = 1;
    dut.din = 1;

    clock_cycle!(dut);
    assert_eq!(dut.dout, 0, "dout should remain low after 1 cycle");

    clock_cycle!(dut);
    assert_eq!(dut.dout, 0, "dout should remain low after 2 cycles");

    clock_cycle!(dut);
    assert_eq!(dut.dout, 1, "dout should update after 3 cycles");
}

#[test]
fn test_ff_sync_parameterized_two_stage_and_width() {
    let runtime = create_ff_sync_param_wrapper_runtime()
        .expect("Failed to create parameterized ff_sync runtime");
    let mut dut = runtime.create_model_simple::<FfSyncParamWrapper>().unwrap();

    dut.clk = 0;
    dut.rst_n = 0;
    dut.din = 0;
    clock_cycle!(dut);

    dut.rst_n = 1;
    dut.din = 0b1010;

    clock_cycle!(dut);
    assert_eq!(dut.dout, 0, "dout should remain low after 1 cycle");

    clock_cycle!(dut);
    assert_eq!(dut.dout, 0b1010, "dout should update after 2 cycles");
}
