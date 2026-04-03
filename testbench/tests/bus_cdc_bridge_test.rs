use riscv_core::AsDynamicVerilatedModel;
use riscv_core::{create_bus_cdc_bridge_runtime, BusCdcBridgeTestWrapper};

fn tick(dut: &mut BusCdcBridgeTestWrapper, sys_rise: bool, periph_rise: bool) {
    dut.sys_clk = 0;
    dut.periph_clk = 0;
    dut.eval();

    dut.sys_clk = if sys_rise { 1 } else { 0 };
    dut.periph_clk = if periph_rise { 1 } else { 0 };
    dut.eval();

    dut.sys_clk = 0;
    dut.periph_clk = 0;
    dut.eval();
}

fn reset(dut: &mut BusCdcBridgeTestWrapper) {
    dut.rst = 1;
    dut.sys_mem_a_addr = 0;
    dut.sys_mem_a_wdata = 0;
    dut.sys_mem_a_we = 0;
    dut.sys_mem_a_size = 0;
    dut.sys_mem_a_valid = 0;
    dut.sys_mem_d_ready = 0;
    dut.periph_mem_a_ready = 0;
    dut.periph_mem_d_rdata = 0;
    dut.periph_mem_d_valid = 0;

    for _ in 0..4 {
        tick(dut, true, true);
    }

    dut.rst = 0;
    tick(dut, true, true);
}

fn wait_for_periph_request(dut: &mut BusCdcBridgeTestWrapper, max_cycles: usize) {
    for _ in 0..max_cycles {
        if dut.periph_mem_a_valid != 0 {
            return;
        }
        tick(dut, true, true);
    }

    panic!("timed out waiting for peripheral A-channel request");
}

fn wait_for_sys_response(dut: &mut BusCdcBridgeTestWrapper, max_cycles: usize) {
    for _ in 0..max_cycles {
        if dut.sys_mem_d_valid != 0 {
            return;
        }
        tick(dut, true, true);
    }

    panic!("timed out waiting for system D-channel response");
}

#[test]
fn test_bus_cdc_bridge_reset_state() {
    let runtime = create_bus_cdc_bridge_runtime().expect("Failed to create bus_cdc_bridge runtime");
    let mut dut = testbench::create_testbench_model::<BusCdcBridgeTestWrapper>(&runtime)
        .expect("Failed to create bus_cdc_bridge model");

    reset(&mut dut);

    assert_eq!(
        dut.sys_mem_a_ready, 1,
        "system A channel should be ready after reset"
    );
    assert_eq!(
        dut.periph_mem_d_ready, 1,
        "peripheral D channel should be ready after reset"
    );
    assert_eq!(
        dut.periph_mem_a_valid, 0,
        "peripheral A channel should be idle after reset"
    );
    assert_eq!(
        dut.sys_mem_d_valid, 0,
        "system D channel should be idle after reset"
    );
}

#[test]
fn test_bus_cdc_bridge_transfers_address_channel_and_holds_backpressure() {
    let runtime = create_bus_cdc_bridge_runtime().expect("Failed to create bus_cdc_bridge runtime");
    let mut dut = testbench::create_testbench_model::<BusCdcBridgeTestWrapper>(&runtime)
        .expect("Failed to create bus_cdc_bridge model");

    reset(&mut dut);

    dut.sys_mem_a_addr = 0x7000_0014;
    dut.sys_mem_a_wdata = 0xCAFE_BABE;
    dut.sys_mem_a_we = 1;
    dut.sys_mem_a_size = 0b10;
    dut.sys_mem_a_valid = 1;
    dut.eval();

    assert_eq!(
        dut.periph_mem_a_valid, 0,
        "request must not appear in the peripheral domain without clock transfer"
    );

    tick(&mut dut, true, false);
    dut.sys_mem_a_valid = 0;
    dut.eval();

    wait_for_periph_request(&mut dut, 10);

    assert_eq!(dut.periph_mem_a_addr, 0x7000_0014);
    assert_eq!(dut.periph_mem_a_wdata, 0xCAFE_BABE);
    assert_eq!(dut.periph_mem_a_we, 1);
    assert_eq!(dut.periph_mem_a_size, 0b10);

    for _ in 0..2 {
        tick(&mut dut, false, true);
        assert_eq!(
            dut.periph_mem_a_valid, 1,
            "peripheral A valid should remain asserted while ready is low"
        );
        assert_eq!(
            dut.periph_mem_a_wdata, 0xCAFE_BABE,
            "peripheral A payload should remain stable under backpressure"
        );
    }

    dut.periph_mem_a_ready = 1;
    tick(&mut dut, false, true);
    dut.periph_mem_a_ready = 0;
    dut.eval();

    assert_eq!(
        dut.periph_mem_a_valid, 0,
        "peripheral A valid should clear after the ready/valid handshake"
    );
}

#[test]
fn test_bus_cdc_bridge_transfers_response_channel_and_holds_backpressure() {
    let runtime = create_bus_cdc_bridge_runtime().expect("Failed to create bus_cdc_bridge runtime");
    let mut dut = testbench::create_testbench_model::<BusCdcBridgeTestWrapper>(&runtime)
        .expect("Failed to create bus_cdc_bridge model");

    reset(&mut dut);

    dut.periph_mem_d_rdata = 0x1234_5678;
    dut.periph_mem_d_valid = 1;
    dut.eval();

    assert_eq!(
        dut.sys_mem_d_valid, 0,
        "response must not appear in the system domain without clock transfer"
    );

    tick(&mut dut, false, true);
    dut.periph_mem_d_valid = 0;
    dut.eval();

    wait_for_sys_response(&mut dut, 10);

    assert_eq!(dut.sys_mem_d_rdata, 0x1234_5678);

    for _ in 0..2 {
        tick(&mut dut, true, false);
        assert_eq!(
            dut.sys_mem_d_valid, 1,
            "system D valid should remain asserted while ready is low"
        );
        assert_eq!(
            dut.sys_mem_d_rdata, 0x1234_5678,
            "system D payload should remain stable under backpressure"
        );
    }

    dut.sys_mem_d_ready = 1;
    tick(&mut dut, true, false);
    dut.sys_mem_d_ready = 0;
    dut.eval();

    assert_eq!(
        dut.sys_mem_d_valid, 0,
        "system D valid should clear after the ready/valid handshake"
    );
}

#[test]
fn test_bus_cdc_bridge_supports_end_to_end_request_and_response() {
    let runtime = create_bus_cdc_bridge_runtime().expect("Failed to create bus_cdc_bridge runtime");
    let mut dut = testbench::create_testbench_model::<BusCdcBridgeTestWrapper>(&runtime)
        .expect("Failed to create bus_cdc_bridge model");

    reset(&mut dut);

    dut.sys_mem_a_addr = 0x2000_0010;
    dut.sys_mem_a_wdata = 0x0000_00AA;
    dut.sys_mem_a_we = 0;
    dut.sys_mem_a_size = 0b10;
    dut.sys_mem_a_valid = 1;
    tick(&mut dut, true, false);
    dut.sys_mem_a_valid = 0;
    dut.eval();

    wait_for_periph_request(&mut dut, 10);
    assert_eq!(dut.periph_mem_a_addr, 0x2000_0010);
    assert_eq!(dut.periph_mem_a_we, 0);

    dut.periph_mem_a_ready = 1;
    tick(&mut dut, false, true);
    dut.periph_mem_a_ready = 0;
    dut.eval();

    dut.periph_mem_d_rdata = 0x0000_00AA;
    dut.periph_mem_d_valid = 1;
    tick(&mut dut, false, true);
    dut.periph_mem_d_valid = 0;
    dut.eval();

    wait_for_sys_response(&mut dut, 10);
    assert_eq!(dut.sys_mem_d_rdata, 0x0000_00AA);

    dut.sys_mem_d_ready = 1;
    tick(&mut dut, true, false);
    dut.sys_mem_d_ready = 0;
    dut.eval();

    assert_eq!(
        dut.sys_mem_d_valid, 0,
        "system D channel should return to idle after response consumption"
    );
}
