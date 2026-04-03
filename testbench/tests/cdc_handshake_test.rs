use riscv_core::AsDynamicVerilatedModel;
use riscv_core::{
    create_cdc_handshake_param_runtime, create_cdc_handshake_runtime, CdcHandshakeParamWrapper,
    CdcHandshakeTestWrapper,
};

fn tick(dut: &mut CdcHandshakeTestWrapper, src_rise: bool, dst_rise: bool) {
    dut.src_clk = 0;
    dut.dst_clk = 0;
    dut.eval();

    dut.src_clk = if src_rise { 1 } else { 0 };
    dut.dst_clk = if dst_rise { 1 } else { 0 };
    dut.eval();

    dut.src_clk = 0;
    dut.dst_clk = 0;
    dut.eval();
}

fn tick_param(dut: &mut CdcHandshakeParamWrapper, src_rise: bool, dst_rise: bool) {
    dut.src_clk = 0;
    dut.dst_clk = 0;
    dut.eval();

    dut.src_clk = if src_rise { 1 } else { 0 };
    dut.dst_clk = if dst_rise { 1 } else { 0 };
    dut.eval();

    dut.src_clk = 0;
    dut.dst_clk = 0;
    dut.eval();
}

fn reset_default(dut: &mut CdcHandshakeTestWrapper) {
    dut.rst = 1;
    dut.src_valid = 0;
    dut.dst_ready = 0;
    dut.src_data = 0;
    for _ in 0..3 {
        tick(dut, true, true);
    }
    dut.rst = 0;
    tick(dut, true, true);
}

fn reset_param(dut: &mut CdcHandshakeParamWrapper) {
    dut.rst = 1;
    dut.src_valid = 0;
    dut.dst_ready = 0;
    dut.src_data = 0;
    for _ in 0..3 {
        tick_param(dut, true, true);
    }
    dut.rst = 0;
    tick_param(dut, true, true);
}

#[test]
fn test_cdc_handshake_reset_state_and_single_transfer() {
    let runtime = create_cdc_handshake_runtime().expect("Failed to create cdc_handshake runtime");
    let mut dut = testbench::create_testbench_model::<CdcHandshakeTestWrapper>(&runtime)
        .expect("Failed to create cdc_handshake model");

    reset_default(&mut dut);

    assert_eq!(dut.src_ready, 1, "source should be ready after reset");
    assert_eq!(dut.dst_valid, 0, "destination should be idle after reset");

    dut.src_data = 0xA5;
    dut.src_valid = 1;
    tick(&mut dut, true, false);
    dut.src_valid = 0;

    assert_eq!(
        dut.src_ready, 0,
        "source should stall while transfer is in flight"
    );
    assert_eq!(
        dut.dst_valid, 0,
        "destination should not see data immediately"
    );

    tick(&mut dut, false, true);
    assert_eq!(
        dut.dst_valid, 0,
        "destination should still wait after 1 dst edge"
    );
    tick(&mut dut, false, true);
    assert_eq!(
        dut.dst_valid, 0,
        "destination should still wait after 2 dst edges"
    );
    tick(&mut dut, false, true);
    assert_eq!(
        dut.dst_valid, 1,
        "destination should present the transferred word"
    );
    assert_eq!(
        dut.dst_data, 0xA5,
        "destination data should match the source payload"
    );
}

#[test]
fn test_cdc_handshake_holds_data_under_backpressure() {
    let runtime = create_cdc_handshake_runtime().expect("Failed to create cdc_handshake runtime");
    let mut dut = testbench::create_testbench_model::<CdcHandshakeTestWrapper>(&runtime)
        .expect("Failed to create cdc_handshake model");

    reset_default(&mut dut);

    dut.src_data = 0x3C;
    dut.src_valid = 1;
    tick(&mut dut, true, false);
    dut.src_valid = 0;

    for _ in 0..3 {
        tick(&mut dut, false, true);
    }
    assert_eq!(
        dut.dst_valid, 1,
        "destination should latch the transferred word"
    );
    assert_eq!(
        dut.dst_data, 0x3C,
        "destination should hold the latched data"
    );
    assert_eq!(
        dut.src_ready, 0,
        "source must remain stalled until destination consumes data"
    );

    for _ in 0..2 {
        tick(&mut dut, true, true);
        assert_eq!(
            dut.dst_valid, 1,
            "dst_valid should stay asserted while dst_ready is low"
        );
        assert_eq!(
            dut.dst_data, 0x3C,
            "dst_data should remain stable under backpressure"
        );
        assert_eq!(
            dut.src_ready, 0,
            "source should stay stalled during backpressure"
        );
    }

    dut.dst_ready = 1;
    tick(&mut dut, false, true);
    dut.dst_ready = 0;
    assert_eq!(
        dut.dst_valid, 0,
        "destination should clear valid after consumption"
    );

    tick(&mut dut, true, false);
    assert_eq!(
        dut.src_ready, 0,
        "acknowledge must still synchronize back to the source"
    );
    tick(&mut dut, true, false);
    assert_eq!(
        dut.src_ready, 0,
        "source should still wait after 2 src edges"
    );
    tick(&mut dut, true, false);
    assert_eq!(
        dut.src_ready, 1,
        "source should become ready once acknowledge returns"
    );
}

#[test]
fn test_cdc_handshake_sync_stage_and_width_parameterization() {
    let runtime = create_cdc_handshake_param_runtime()
        .expect("Failed to create parameterized cdc_handshake runtime");
    let mut dut = testbench::create_testbench_model::<CdcHandshakeParamWrapper>(&runtime)
        .expect("Failed to create parameterized cdc_handshake model");

    reset_param(&mut dut);

    dut.src_data = 0xBEEF;
    dut.src_valid = 1;
    tick_param(&mut dut, true, false);
    dut.src_valid = 0;

    tick_param(&mut dut, false, true);
    assert_eq!(
        dut.dst_valid, 0,
        "destination should still wait after 1 dst edge"
    );
    tick_param(&mut dut, false, true);
    assert_eq!(
        dut.dst_valid, 0,
        "destination should still wait after 2 dst edges"
    );
    tick_param(&mut dut, false, true);
    assert_eq!(
        dut.dst_valid, 0,
        "destination should still wait after 3 dst edges"
    );
    tick_param(&mut dut, false, true);
    assert_eq!(
        dut.dst_valid, 1,
        "destination should assert after 4 dst edges (3 sync stages + edge-detect stage)"
    );
    assert_eq!(
        dut.dst_data, 0xBEEF,
        "16-bit payload should transfer intact"
    );
}

#[test]
fn test_cdc_handshake_reset_clears_inflight_transfer() {
    let runtime = create_cdc_handshake_runtime().expect("Failed to create cdc_handshake runtime");
    let mut dut = testbench::create_testbench_model::<CdcHandshakeTestWrapper>(&runtime)
        .expect("Failed to create cdc_handshake model");

    reset_default(&mut dut);

    dut.src_data = 0x55;
    dut.src_valid = 1;
    tick(&mut dut, true, false);
    dut.src_valid = 0;
    assert_eq!(
        dut.src_ready, 0,
        "source should track the in-flight transfer"
    );

    dut.rst = 1;
    for _ in 0..2 {
        tick(&mut dut, true, true);
    }
    dut.rst = 0;
    tick(&mut dut, true, true);

    assert_eq!(
        dut.src_ready, 1,
        "reset should return the source to the ready state"
    );
    assert_eq!(
        dut.dst_valid, 0,
        "reset should drop any in-flight destination valid"
    );

    dut.src_data = 0x66;
    dut.src_valid = 1;
    tick(&mut dut, true, false);
    dut.src_valid = 0;
    for _ in 0..3 {
        tick(&mut dut, false, true);
    }
    assert_eq!(dut.dst_valid, 1, "new transfers should work after reset");
    assert_eq!(
        dut.dst_data, 0x66,
        "post-reset payload should transfer correctly"
    );
}
