use riscv_core::AsDynamicVerilatedModel;
use riscv_core::{create_apf_bus_bridge_runtime, ApfBusBridgeTestWrapper};

const MAX_BRIDGE_COMPLETION_CYCLES: usize = 64;

fn clock_cycle(dut: &mut ApfBusBridgeTestWrapper) {
    dut.clk = 0;
    dut.eval();
    dut.clk = 1;
    dut.eval();
    dut.clk = 0;
    dut.eval();
}

fn reset(dut: &mut ApfBusBridgeTestWrapper) {
    dut.rst = 1;
    dut.bridge_addr = 0;
    dut.bridge_rd = 0;
    dut.bridge_wr = 0;
    dut.bridge_wr_data = 0;

    clock_cycle(dut);
    clock_cycle(dut);

    dut.rst = 0;
    dut.eval();
}

fn wait_for_bridge_ready(dut: &mut ApfBusBridgeTestWrapper, max_cycles: usize) {
    for _ in 0..max_cycles {
        if (dut.bridge_rd_ready != 0) && (dut.bridge_wr_ready != 0) {
            return;
        }
        clock_cycle(dut);
    }

    panic!("timed out waiting for APF bridge readiness");
}

fn pulse_write(dut: &mut ApfBusBridgeTestWrapper, addr: u32, data: u32) {
    wait_for_bridge_ready(dut, MAX_BRIDGE_COMPLETION_CYCLES);
    dut.bridge_addr = addr;
    dut.bridge_wr_data = data;
    dut.bridge_wr = 1;
    dut.bridge_rd = 0;
    dut.eval();
    clock_cycle(dut);
    dut.bridge_wr = 0;
    dut.eval();
}

fn pulse_read(dut: &mut ApfBusBridgeTestWrapper, addr: u32) {
    wait_for_bridge_ready(dut, MAX_BRIDGE_COMPLETION_CYCLES);
    dut.bridge_addr = addr;
    dut.bridge_rd = 1;
    dut.bridge_wr = 0;
    dut.eval();
    clock_cycle(dut);
    dut.bridge_rd = 0;
    dut.eval();
}

#[test]
fn test_apf_bus_bridge_writes_and_reads_sram_through_registered_bus() {
    let runtime = create_apf_bus_bridge_runtime().expect("Failed to create APF bus bridge runtime");
    let mut dut = runtime
        .create_model_simple::<ApfBusBridgeTestWrapper>()
        .expect("Failed to create APF bus bridge model");

    reset(&mut dut);

    wait_for_bridge_ready(&mut dut, MAX_BRIDGE_COMPLETION_CYCLES);
    pulse_write(&mut dut, 0x7000_0010, 0xCAFE_BABE);
    assert_eq!(
        dut.bridge_wr_ready, 0,
        "bridge write ready should deassert while a write request is in flight"
    );
    assert_eq!(
        dut.bridge_rd_ready, 0,
        "bridge read ready should also deassert while the bridge is busy"
    );
    wait_for_bridge_ready(&mut dut, MAX_BRIDGE_COMPLETION_CYCLES);

    pulse_read(&mut dut, 0x7000_0010);
    assert_eq!(
        dut.bridge_rd_ready, 0,
        "bridge read ready should deassert while a read request is in flight"
    );
    wait_for_bridge_ready(&mut dut, MAX_BRIDGE_COMPLETION_CYCLES);

    assert_eq!(
        dut.bridge_rd_data, 0xCAFE_BABE,
        "bridge read data should return the SRAM word written through the bus"
    );

    clock_cycle(&mut dut);
    assert_eq!(
        dut.bridge_rd_data, 0xCAFE_BABE,
        "bridge read data should remain stable after the read completes"
    );
}
