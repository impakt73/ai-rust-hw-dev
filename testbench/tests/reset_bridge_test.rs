use riscv_core::{
    create_reset_bridge_param_wrapper_runtime, create_reset_bridge_runtime,
    ResetBridgeDefaultWrapper, ResetBridgeParamWrapper,
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
fn test_reset_bridge_powers_up_asserted_and_releases_after_two_cycles() {
    let runtime = create_reset_bridge_runtime().expect("Failed to create reset bridge runtime");
    let mut dut = runtime
        .create_model_simple::<ResetBridgeDefaultWrapper>()
        .unwrap();

    dut.rst_n = 1;
    dut.eval();
    assert_eq!(dut.rst, 1, "reset should power up asserted");

    clock_cycle!(dut);
    assert_eq!(
        dut.rst, 1,
        "reset should remain asserted after 1 release cycle"
    );

    clock_cycle!(dut);
    assert_eq!(dut.rst, 0, "reset should deassert after 2 release cycles");
}

#[test]
fn test_reset_bridge_asserts_immediately_and_releases_synchronously() {
    let runtime = create_reset_bridge_runtime().expect("Failed to create reset bridge runtime");
    let mut dut = runtime
        .create_model_simple::<ResetBridgeDefaultWrapper>()
        .unwrap();

    dut.rst_n = 1;
    dut.eval();
    clock_cycle!(dut);
    clock_cycle!(dut);
    assert_eq!(
        dut.rst, 0,
        "reset should start deasserted after startup release"
    );

    dut.rst_n = 0;
    dut.eval();
    assert_eq!(
        dut.rst, 1,
        "reset must assert immediately on rst_n falling edge"
    );

    dut.rst_n = 1;
    dut.eval();
    assert_eq!(
        dut.rst, 1,
        "reset must stay asserted until release clocks arrive"
    );

    clock_cycle!(dut);
    assert_eq!(
        dut.rst, 1,
        "reset should remain asserted after 1 release cycle"
    );

    clock_cycle!(dut);
    assert_eq!(
        dut.rst, 0,
        "reset should deassert on the synchronized release path"
    );
}

#[test]
fn test_reset_bridge_parameterized_release_delay() {
    let runtime = create_reset_bridge_param_wrapper_runtime()
        .expect("Failed to create parameterized reset bridge runtime");
    let mut dut = runtime
        .create_model_simple::<ResetBridgeParamWrapper>()
        .unwrap();

    dut.rst_n = 1;
    dut.eval();
    assert_eq!(
        dut.rst, 1,
        "parameterized reset bridge should power up asserted"
    );

    for cycle_idx in 0..4 {
        clock_cycle!(dut);
        let expected = if cycle_idx < 3 { 1 } else { 0 };
        assert_eq!(
            dut.rst, expected,
            "4-stage reset bridge should deassert only after 4 release cycles"
        );
    }
}
