use riscv_core::{create_registered_bus_runtime, RegisteredBusWrapper};

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

fn eval_comb(dut: &mut RegisteredBusWrapper) {
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
    master_idx: usize,
    addr: u32,
    wdata: u32,
    we: u8,
    size: u8,
    valid: u8,
) {
    match master_idx {
        0 => {
            dut.master0_mem_a_addr = addr;
            dut.master0_mem_a_wdata = wdata;
            dut.master0_mem_a_we = we;
            dut.master0_mem_a_size = size;
            dut.master0_mem_a_valid = valid;
        }
        1 => {
            dut.master1_mem_a_addr = addr;
            dut.master1_mem_a_wdata = wdata;
            dut.master1_mem_a_we = we;
            dut.master1_mem_a_size = size;
            dut.master1_mem_a_valid = valid;
        }
        _ => panic!("unsupported master index {master_idx}"),
    }
}

fn set_master_d_ready(dut: &mut RegisteredBusWrapper, master_idx: usize, ready: u8) {
    match master_idx {
        0 => dut.master0_mem_d_ready = ready,
        1 => dut.master1_mem_d_ready = ready,
        _ => panic!("unsupported master index {master_idx}"),
    }
}

fn set_slave_a_ready(dut: &mut RegisteredBusWrapper, slave_idx: usize, ready: u8) {
    match slave_idx {
        0 => dut.slave0_mem_a_ready = ready,
        1 => dut.slave1_mem_a_ready = ready,
        _ => panic!("unsupported slave index {slave_idx}"),
    }
}

fn set_slave_response(dut: &mut RegisteredBusWrapper, slave_idx: usize, rdata: u32, valid: u8) {
    match slave_idx {
        0 => {
            dut.slave0_mem_d_rdata = rdata;
            dut.slave0_mem_d_valid = valid;
        }
        1 => {
            dut.slave1_mem_d_rdata = rdata;
            dut.slave1_mem_d_valid = valid;
        }
        _ => panic!("unsupported slave index {slave_idx}"),
    }
}

fn clear_master_requests(dut: &mut RegisteredBusWrapper) {
    set_master_request(dut, 0, 0, 0, 0, 0, 0);
    set_master_request(dut, 1, 0, 0, 0, 0, 0);
}

fn clear_slave_responses(dut: &mut RegisteredBusWrapper) {
    set_slave_response(dut, 0, 0, 0);
    set_slave_response(dut, 1, 0, 0);
}

fn reset_dut(dut: &mut RegisteredBusWrapper) {
    dut.rst = 1;
    set_master_d_ready(dut, 0, 0);
    set_master_d_ready(dut, 1, 0);
    set_slave_a_ready(dut, 0, 1);
    set_slave_a_ready(dut, 1, 1);
    clear_master_requests(dut);
    clear_slave_responses(dut);
    configure_ranges(dut);
    clock_cycle!(dut);

    dut.rst = 0;
    clock_cycle!(dut);
}

#[test]
fn test_registered_bus_arbitrates_requests_and_routes_responses() {
    let runtime = create_registered_bus_runtime().expect("Failed to create registered_bus runtime");
    let mut dut = runtime
        .create_model_simple::<RegisteredBusWrapper>()
        .expect("Failed to create registered_bus model");

    reset_dut(&mut dut);

    set_master_request(&mut dut, 0, 0x5000_0020, 0xAAAA_0001, 0, 0b10, 1);
    set_master_request(&mut dut, 1, 0x6000_0030, 0xBBBB_0002, 1, 0b10, 1);
    eval_comb(&mut dut);

    assert_eq!(
        dut.master0_mem_a_ready, 1,
        "master0 should win address arbitration"
    );
    assert_eq!(
        dut.master1_mem_a_ready, 0,
        "master1 should wait behind master0"
    );

    clock_cycle!(dut);
    set_master_request(&mut dut, 0, 0, 0, 0, 0, 0);
    set_slave_a_ready(&mut dut, 0, 0);
    eval_comb(&mut dut);
    clock_cycle!(dut);

    assert_eq!(
        dut.slave0_mem_a_valid, 1,
        "slave0 should see master0 request first"
    );
    assert_eq!(
        dut.slave0_mem_a_addr, 0x5000_0020,
        "slave0 address mismatch"
    );
    assert_eq!(
        dut.slave1_mem_a_valid, 0,
        "slave1 must remain idle while master0 is buffered"
    );
    assert_eq!(
        dut.master0_mem_a_ready, 0,
        "address side should stall while request is buffered"
    );
    assert_eq!(
        dut.master1_mem_a_ready, 0,
        "master1 must wait while request is buffered"
    );

    set_slave_a_ready(&mut dut, 0, 1);
    eval_comb(&mut dut);
    clock_cycle!(dut);
    eval_comb(&mut dut);

    assert_eq!(
        dut.master1_mem_a_ready, 1,
        "master1 should become eligible once slave0 accepts"
    );

    clock_cycle!(dut);
    set_master_request(&mut dut, 1, 0, 0, 0, 0, 0);
    set_slave_a_ready(&mut dut, 1, 0);
    eval_comb(&mut dut);
    clock_cycle!(dut);

    assert_eq!(
        dut.slave1_mem_a_valid, 1,
        "slave1 should receive the buffered master1 request"
    );
    assert_eq!(
        dut.slave1_mem_a_addr, 0x6000_0030,
        "slave1 address mismatch"
    );
    assert_eq!(dut.slave1_mem_a_we, 1, "slave1 write-enable mismatch");

    set_slave_a_ready(&mut dut, 1, 1);
    eval_comb(&mut dut);
    clock_cycle!(dut);

    set_slave_response(&mut dut, 0, 0x1111_2222, 1);
    set_slave_response(&mut dut, 1, 0x3333_4444, 1);
    eval_comb(&mut dut);
    clock_cycle!(dut);
    eval_comb(&mut dut);

    assert_eq!(
        dut.master0_mem_d_valid, 1,
        "slave0 response should be routed first"
    );
    assert_eq!(
        dut.master0_mem_d_rdata, 0x1111_2222,
        "master0 response data mismatch"
    );
    assert_eq!(
        dut.master1_mem_d_valid, 0,
        "slave1 response must wait behind slave0"
    );

    set_master_d_ready(&mut dut, 0, 1);
    eval_comb(&mut dut);
    clock_cycle!(dut);
    set_master_d_ready(&mut dut, 0, 0);
    eval_comb(&mut dut);

    assert_eq!(
        dut.master0_mem_d_valid, 0,
        "master0 response should clear after handshake"
    );
    assert_eq!(
        dut.master1_mem_d_valid, 0,
        "slave1 response should not transfer in the same cycle"
    );

    clock_cycle!(dut);
    eval_comb(&mut dut);

    assert_eq!(
        dut.master1_mem_d_valid, 1,
        "slave1 response should route after slave0 completes"
    );
    assert_eq!(
        dut.master1_mem_d_rdata, 0x3333_4444,
        "master1 response data mismatch"
    );
}

#[test]
fn test_registered_bus_holds_same_slave_request_until_prior_response_is_captured() {
    let runtime = create_registered_bus_runtime().expect("Failed to create registered_bus runtime");
    let mut dut = runtime
        .create_model_simple::<RegisteredBusWrapper>()
        .expect("Failed to create registered_bus model");

    reset_dut(&mut dut);

    set_master_request(&mut dut, 0, 0x5000_0010, 0xAAAA_5555, 0, 0b10, 1);
    eval_comb(&mut dut);
    clock_cycle!(dut);
    set_master_request(&mut dut, 0, 0, 0, 0, 0, 0);
    clock_cycle!(dut);

    set_master_request(&mut dut, 1, 0x5000_0040, 0xBBBB_6666, 1, 0b10, 1);
    eval_comb(&mut dut);
    assert_eq!(
        dut.master1_mem_a_ready, 1,
        "master1 request should be accepted while the bus is idle"
    );
    clock_cycle!(dut);
    set_master_request(&mut dut, 1, 0, 0, 0, 0, 0);
    eval_comb(&mut dut);
    clock_cycle!(dut);

    assert_eq!(
        dut.slave0_mem_a_valid, 0,
        "slave0 must stay busy while its prior response is pending"
    );
    assert_eq!(
        dut.master0_mem_d_valid, 0,
        "no response should be visible before slave0 responds"
    );

    set_master_d_ready(&mut dut, 0, 0);
    set_slave_response(&mut dut, 0, 0x1234_5678, 1);
    eval_comb(&mut dut);
    clock_cycle!(dut);
    clear_slave_responses(&mut dut);
    set_slave_a_ready(&mut dut, 0, 0);
    eval_comb(&mut dut);
    clock_cycle!(dut);

    assert_eq!(
        dut.master0_mem_d_valid, 1,
        "master0 should receive the first response"
    );
    assert_eq!(
        dut.master0_mem_d_rdata, 0x1234_5678,
        "master0 response data mismatch"
    );
    assert_eq!(
        dut.slave0_mem_a_valid, 1,
        "slave0 should see the held master1 request once the response is captured"
    );
    assert_eq!(
        dut.slave0_mem_a_addr, 0x5000_0040,
        "held request address mismatch"
    );
    assert_eq!(dut.slave0_mem_a_we, 1, "held request write-enable mismatch");

    set_slave_a_ready(&mut dut, 0, 1);
    eval_comb(&mut dut);
    clock_cycle!(dut);
    eval_comb(&mut dut);

    assert_eq!(
        dut.master1_mem_a_ready, 0,
        "no new address request should be accepted until the held request is dispatched"
    );
}

#[test]
fn test_registered_bus_unmapped_address_returns_zero_to_requesting_master() {
    let runtime = create_registered_bus_runtime().expect("Failed to create registered_bus runtime");
    let mut dut = runtime
        .create_model_simple::<RegisteredBusWrapper>()
        .expect("Failed to create registered_bus model");

    reset_dut(&mut dut);

    set_master_request(&mut dut, 1, 0x4000_0000, 0xDEAD_BEEF, 1, 0b10, 1);
    eval_comb(&mut dut);

    assert_eq!(
        dut.master0_mem_a_ready, 0,
        "inactive master0 should not be selected"
    );
    assert_eq!(
        dut.master1_mem_a_ready, 1,
        "master1 should be able to issue the unmapped request"
    );

    clock_cycle!(dut);
    set_master_request(&mut dut, 1, 0, 0, 0, 0, 0);
    eval_comb(&mut dut);
    clock_cycle!(dut);
    eval_comb(&mut dut);

    assert_eq!(
        dut.slave0_mem_a_valid, 0,
        "unmapped request must not hit slave0"
    );
    assert_eq!(
        dut.slave1_mem_a_valid, 0,
        "unmapped request must not hit slave1"
    );
    assert_eq!(
        dut.master0_mem_d_valid, 0,
        "master0 must not see master1's response"
    );
    assert_eq!(
        dut.master1_mem_d_valid, 1,
        "master1 should receive the synthesized zero response"
    );
    assert_eq!(
        dut.master1_mem_d_rdata, 0,
        "unmapped access should return zero data"
    );

    set_master_d_ready(&mut dut, 1, 1);
    eval_comb(&mut dut);
    clock_cycle!(dut);
    eval_comb(&mut dut);

    assert_eq!(
        dut.master1_mem_d_valid, 0,
        "master1 response should clear after handshake"
    );
}

#[test]
fn test_registered_bus_unmapped_and_slave_response_same_cycle_do_not_interfere() {
    let runtime = create_registered_bus_runtime().expect("Failed to create registered_bus runtime");
    let mut dut = runtime
        .create_model_simple::<RegisteredBusWrapper>()
        .expect("Failed to create registered_bus model");

    reset_dut(&mut dut);

    set_master_request(&mut dut, 0, 0x5000_0010, 0xAAAA_1111, 0, 0b10, 1);
    eval_comb(&mut dut);
    assert_eq!(
        dut.master0_mem_a_ready, 1,
        "master0 mapped request should be accepted"
    );
    clock_cycle!(dut);

    set_master_request(&mut dut, 0, 0, 0, 0, 0, 0);
    eval_comb(&mut dut);
    clock_cycle!(dut);

    set_master_request(&mut dut, 1, 0x4000_0000, 0xBBBB_2222, 1, 0b10, 1);
    eval_comb(&mut dut);
    assert_eq!(
        dut.master1_mem_a_ready, 1,
        "master1 unmapped request should be accepted while slave0 response is pending"
    );
    clock_cycle!(dut);

    set_master_request(&mut dut, 1, 0, 0, 0, 0, 0);
    set_slave_response(&mut dut, 0, 0x1234_5678, 1);
    eval_comb(&mut dut);
    clock_cycle!(dut);
    eval_comb(&mut dut);

    assert_eq!(
        dut.master1_mem_d_valid, 1,
        "master1 should receive the synthesized unmapped response first"
    );
    assert_eq!(
        dut.master1_mem_d_rdata, 0,
        "concurrent unmapped request must still return zero data"
    );
    assert_eq!(
        dut.master0_mem_d_valid, 0,
        "slave0 response must not overwrite the unmapped response"
    );

    set_master_d_ready(&mut dut, 1, 1);
    eval_comb(&mut dut);
    clock_cycle!(dut);
    set_master_d_ready(&mut dut, 1, 0);
    eval_comb(&mut dut);

    assert_eq!(
        dut.master1_mem_d_valid, 0,
        "master1 synthesized response should clear after handshake"
    );
    assert_eq!(
        dut.slave0_mem_d_ready, 0,
        "slave0 ready pop should still be low in the cycle where the unmapped response is consumed"
    );

    clock_cycle!(dut);
    clear_slave_responses(&mut dut);
    eval_comb(&mut dut);

    assert_eq!(
        dut.master0_mem_d_valid, 1,
        "slave0 response should be delivered after the unmapped response completes"
    );
    assert_eq!(
        dut.slave0_mem_d_ready, 1,
        "slave0 should receive a one-cycle ready pop when the bus captures its response"
    );
    assert_eq!(
        dut.master0_mem_d_rdata, 0x1234_5678,
        "master0 should still receive the original slave response data"
    );
}
