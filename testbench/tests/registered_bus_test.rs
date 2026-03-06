use riscv_core::{create_registered_bus_runtime, RegisteredBusWrapper};

fn clock_cycle(dut: &mut RegisteredBusWrapper) {
    dut.clk = 0;
    dut.eval();
    dut.clk = 1;
    dut.eval();
    dut.clk = 0;
    dut.eval();
}

fn configure_ranges(dut: &mut RegisteredBusWrapper) {
    dut.slave0_base_addr = 0x5000_0000;
    dut.slave0_addr_size = 0x0000_1000;
    dut.slave1_base_addr = 0x6000_0000;
    dut.slave1_addr_size = 0x0000_1000;
}

fn set_master_request(
    dut: &mut RegisteredBusWrapper,
    addr: u32,
    wdata: u32,
    we: u8,
    size: u8,
    valid: u8,
) {
    dut.master_mem_a_addr = addr;
    dut.master_mem_a_wdata = wdata;
    dut.master_mem_a_we = we;
    dut.master_mem_a_size = size;
    dut.master_mem_a_valid = valid;
}

fn set_slave0_response(dut: &mut RegisteredBusWrapper, rdata: u32, valid: u8) {
    dut.slave0_mem_d_rdata = rdata;
    dut.slave0_mem_d_valid = valid;
}

fn set_slave1_response(dut: &mut RegisteredBusWrapper, rdata: u32, valid: u8) {
    dut.slave1_mem_d_rdata = rdata;
    dut.slave1_mem_d_valid = valid;
}

fn reset_dut(dut: &mut RegisteredBusWrapper) {
    dut.rst_n = 0;
    dut.master_mem_d_ready = 0;
    dut.slave0_mem_a_ready = 1;
    dut.slave1_mem_a_ready = 1;
    set_master_request(dut, 0, 0, 0, 0, 0);
    set_slave0_response(dut, 0, 0);
    set_slave1_response(dut, 0, 0);
    configure_ranges(dut);
    clock_cycle(dut);

    dut.rst_n = 1;
    clock_cycle(dut);
}

#[test]
fn test_registered_bus_routes_to_two_slaves() {
    let runtime = create_registered_bus_runtime().expect("Failed to create registered_bus runtime");
    let mut dut = runtime
        .create_model_simple::<RegisteredBusWrapper>()
        .expect("Failed to create registered_bus model");

    reset_dut(&mut dut);

    assert_eq!(dut.master_mem_a_ready, 1, "bus should accept first request");

    // Transaction 1: route to slave 0
    set_master_request(&mut dut, 0x5000_0020, 0xABCD_1234, 0, 0b10, 1);
    clock_cycle(&mut dut); // A handshake in IDLE

    set_master_request(&mut dut, 0, 0, 0, 0, 0);
    dut.slave0_mem_a_ready = 0;
    clock_cycle(&mut dut); // decode/selection stage (held)

    assert_eq!(dut.slave0_mem_a_valid, 1, "slave0 should receive A request");
    assert_eq!(
        dut.slave1_mem_a_valid, 0,
        "slave1 should not receive A request"
    );
    assert_eq!(
        dut.slave0_mem_a_addr, 0x5000_0020,
        "slave0 address mismatch"
    );

    dut.slave0_mem_a_ready = 1;
    clock_cycle(&mut dut); // wait for D response
    assert_eq!(
        dut.slave0_mem_d_ready, 1,
        "slave0 D channel should be connected"
    );
    assert_eq!(
        dut.slave1_mem_d_ready, 0,
        "slave1 D channel should be disconnected"
    );
    assert_eq!(
        dut.master_mem_a_ready, 0,
        "new requests must stall while busy"
    );

    set_slave0_response(&mut dut, 0x1111_2222, 1);
    clock_cycle(&mut dut); // capture slave D response
    set_slave0_response(&mut dut, 0, 0);

    assert_eq!(
        dut.master_mem_d_valid, 1,
        "master D response should become valid"
    );
    assert_eq!(
        dut.master_mem_d_rdata, 0x1111_2222,
        "master D response mismatch"
    );

    dut.master_mem_d_ready = 1;
    clock_cycle(&mut dut); // consume master D response
    dut.master_mem_d_ready = 0;

    assert_eq!(dut.master_mem_a_ready, 1, "bus should accept next request");

    // Transaction 2: route to slave 1
    set_master_request(&mut dut, 0x6FFF_F030, 0xDEAD_BEEF, 1, 0b10, 1);
    clock_cycle(&mut dut); // A handshake in IDLE

    set_master_request(&mut dut, 0, 0, 0, 0, 0);
    dut.slave1_mem_a_ready = 0;
    clock_cycle(&mut dut); // decode/selection stage (held)

    assert_eq!(
        dut.slave0_mem_a_valid, 0,
        "slave0 should not receive second request"
    );
    assert_eq!(
        dut.slave1_mem_a_valid, 1,
        "slave1 should receive second request"
    );
    assert_eq!(
        dut.slave1_mem_a_addr, 0x6FFF_F030,
        "slave1 address mismatch"
    );
    assert_eq!(dut.slave1_mem_a_we, 1, "slave1 write-enable mismatch");

    dut.slave1_mem_a_ready = 1;
    clock_cycle(&mut dut); // wait for D response
    set_slave1_response(&mut dut, 0x3333_4444, 1);
    clock_cycle(&mut dut);
    set_slave1_response(&mut dut, 0, 0);

    assert_eq!(
        dut.master_mem_d_valid, 1,
        "second master D response should be valid"
    );
    assert_eq!(
        dut.master_mem_d_rdata, 0x3333_4444,
        "second response data mismatch"
    );
}

#[test]
fn test_registered_bus_unmapped_address_returns_zero() {
    let runtime = create_registered_bus_runtime().expect("Failed to create registered_bus runtime");
    let mut dut = runtime
        .create_model_simple::<RegisteredBusWrapper>()
        .expect("Failed to create registered_bus model");

    reset_dut(&mut dut);

    set_master_request(&mut dut, 0x4000_0000, 0, 0, 0b10, 1);
    clock_cycle(&mut dut); // A handshake in IDLE

    set_master_request(&mut dut, 0, 0, 0, 0, 0);
    clock_cycle(&mut dut); // decode unmapped + queue zero response

    assert_eq!(
        dut.master_mem_a_ready, 0,
        "A channel should stay blocked while response is pending"
    );

    assert_eq!(
        dut.slave0_mem_a_valid, 0,
        "unmapped request must not hit slave0"
    );
    assert_eq!(
        dut.slave1_mem_a_valid, 0,
        "unmapped request must not hit slave1"
    );
    assert_eq!(
        dut.master_mem_d_valid, 1,
        "unmapped request should return a response"
    );
    assert_eq!(
        dut.master_mem_d_rdata, 0,
        "unmapped request should return zero data"
    );

    dut.master_mem_d_ready = 1;
    clock_cycle(&mut dut);

    assert_eq!(
        dut.master_mem_d_valid, 0,
        "response should clear after handshake"
    );
    assert_eq!(
        dut.master_mem_a_ready, 1,
        "bus should become idle after response"
    );
}
