use riscv_core::{create_host_bus_mux_runtime, HostBusMux};

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

fn reset_module(dut: &mut HostBusMux) {
    dut.rst_n = 0;
    dut.cpu_mem_a_addr = 0;
    dut.cpu_mem_a_wdata = 0;
    dut.cpu_mem_a_we = 0;
    dut.cpu_mem_a_size = 0;
    dut.cpu_mem_a_valid = 0;
    dut.cpu_mem_d_ready = 0;
    dut.sys_mem_a_ready = 0;
    dut.sys_mem_d_rdata = 0;
    dut.sys_mem_d_valid = 0;
    dut.host_mem_a_ready = 0;
    dut.host_mem_d_rdata = 0;
    dut.host_mem_d_valid = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    clock_cycle!(dut);
}

#[test]
fn test_low_address_routes_to_system_path_and_holds_response() {
    let runtime = create_host_bus_mux_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusMux>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Initial post-reset state: mux should accept the first CPU A-channel request.
    assert_eq!(dut.cpu_mem_a_ready, 1);
    dut.cpu_mem_a_addr = 0x5000_0010;
    dut.cpu_mem_a_wdata = 0xCAFE_BABE;
    dut.cpu_mem_a_we = 0;
    dut.cpu_mem_a_size = 0b10;
    dut.cpu_mem_a_valid = 1;
    clock_cycle!(dut);
    dut.cpu_mem_a_valid = 0;

    dut.eval();
    assert_eq!(
        dut.sys_mem_a_valid, 1,
        "low address should route to system path"
    );
    assert_eq!(dut.host_mem_a_valid, 0, "host path must remain idle");
    assert_eq!(dut.sys_mem_a_addr, 0x5000_0010);
    assert_eq!(
        dut.cpu_mem_a_ready, 0,
        "registered outputs should block new CPU A traffic once the request is buffered"
    );

    clock_cycle!(dut);
    dut.eval();
    assert_eq!(
        dut.sys_mem_a_valid, 1,
        "system request should remain registered until the downstream handshake"
    );

    dut.sys_mem_a_ready = 1;
    clock_cycle!(dut);
    dut.sys_mem_a_ready = 0;

    dut.sys_mem_d_rdata = 0x1122_3344;
    dut.sys_mem_d_valid = 1;
    dut.eval();
    assert_eq!(dut.sys_mem_d_ready, 1, "mux should accept system response");
    clock_cycle!(dut);
    dut.sys_mem_d_valid = 0;

    dut.eval();
    assert_eq!(dut.cpu_mem_d_valid, 1);
    assert_eq!(dut.cpu_mem_d_rdata, 0x1122_3344);
    assert_eq!(
        dut.cpu_mem_a_ready, 0,
        "response buffering should block new A traffic"
    );

    clock_cycle!(dut);
    dut.eval();
    assert_eq!(
        dut.cpu_mem_d_valid, 1,
        "response must remain valid until consumed"
    );

    dut.cpu_mem_d_ready = 1;
    clock_cycle!(dut);
    dut.cpu_mem_d_ready = 0;
    dut.eval();

    assert_eq!(dut.cpu_mem_d_valid, 0);
    assert_eq!(dut.cpu_mem_a_ready, 1);
}

#[test]
fn test_high_address_routes_to_host_path() {
    let runtime = create_host_bus_mux_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusMux>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    dut.cpu_mem_a_addr = 0x9000_0020;
    dut.cpu_mem_a_wdata = 0x5566_7788;
    dut.cpu_mem_a_we = 1;
    dut.cpu_mem_a_size = 0b10;
    dut.cpu_mem_a_valid = 1;
    clock_cycle!(dut);
    dut.cpu_mem_a_valid = 0;

    dut.eval();
    assert_eq!(dut.sys_mem_a_valid, 0, "system path must remain idle");
    assert_eq!(
        dut.host_mem_a_valid, 1,
        "high address should route to host path"
    );
    assert_eq!(dut.host_mem_a_addr, 0x9000_0020);
    assert_eq!(dut.host_mem_a_wdata, 0x5566_7788);
    assert_eq!(dut.host_mem_a_we, 1);

    clock_cycle!(dut);
    dut.eval();
    assert_eq!(
        dut.host_mem_a_valid, 1,
        "host request should remain registered until the downstream handshake"
    );
    assert_eq!(dut.host_mem_a_addr, 0x9000_0020);
    assert_eq!(dut.host_mem_a_wdata, 0x5566_7788);
    assert_eq!(dut.host_mem_a_we, 1);

    dut.host_mem_a_ready = 1;
    clock_cycle!(dut);
    dut.host_mem_a_ready = 0;

    dut.host_mem_d_rdata = 0xAABB_CCDD;
    dut.host_mem_d_valid = 1;
    dut.eval();
    assert_eq!(dut.host_mem_d_ready, 1, "mux should accept host response");
    clock_cycle!(dut);
    dut.host_mem_d_valid = 0;

    dut.eval();
    assert_eq!(dut.cpu_mem_d_valid, 1);
    assert_eq!(dut.cpu_mem_d_rdata, 0xAABB_CCDD);
}
