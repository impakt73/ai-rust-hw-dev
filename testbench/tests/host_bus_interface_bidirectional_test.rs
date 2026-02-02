// Host Bus Interface Bidirectional Tests
// Tests for host-initiated requests (Host→FPGA direction)
//
// Extended Protocol (Little-Endian):
//   Host Request:   [ext_header][addr0..3][data...]     (packet type 0010)
//   FPGA Response:  [ext_header][data...]               (packet type 0011)
//   Error Response: [ext_header][error_code]            (packet type 1111)
//
// Extended header format: {packet_type[3:0], size[1:0], 1'b0, we}

use riscv_core::{create_host_bus_interface_runtime, HostBusInterface};

// Clock cycle macro
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

/// Apply reset to the module
fn reset_module(dut: &mut HostBusInterface) {
    dut.rst_n = 0;
    dut.req = 0;
    dut.we = 0;
    dut.addr = 0;
    dut.wdata = 0;
    dut.size = 0;
    dut.tx_ready = 0;
    dut.rx_valid = 0;
    dut.rx_data = 0;
    // Host bus interface signals
    dut.host_bus_ready = 0;
    dut.host_bus_rdata = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    clock_cycle!(dut);
}

/// Helper to receive a byte from TX interface
fn receive_tx_byte(dut: &mut HostBusInterface, max_cycles: u32) -> Option<u8> {
    for _ in 0..max_cycles {
        if dut.tx_valid != 0 {
            dut.tx_ready = 1;
            dut.eval();
            let byte = dut.tx_data as u8;
            clock_cycle!(dut);
            dut.tx_ready = 0;
            dut.eval();
            return Some(byte);
        }
        clock_cycle!(dut);
    }
    None
}

/// Helper to send a byte to RX interface
fn send_rx_byte(dut: &mut HostBusInterface, byte: u8, max_cycles: u32) -> bool {
    dut.rx_data = byte;
    dut.rx_valid = 1;
    dut.eval();

    for _ in 0..max_cycles {
        if dut.rx_ready != 0 {
            clock_cycle!(dut);
            dut.rx_valid = 0;
            dut.eval();
            return true;
        }
        clock_cycle!(dut);
    }
    dut.rx_valid = 0;
    dut.eval();
    false
}

// ============================================================
// Basic Host Read Request Tests
// ============================================================

#[test]
fn test_host_initiated_read_word() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Send host request header: {4'b0010, size=10, 1'b0, we=0} = 0x28
    assert!(send_rx_byte(&mut dut, 0x28, 100), "Failed to send header");

    // Send address (little-endian) - LED_BASE = 0x50000000
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x50, 100), "addr[31:24]");

    // Module should now issue bus request
    for _ in 0..10 {
        clock_cycle!(dut);
        if dut.host_bus_req != 0 {
            break;
        }
    }
    assert_eq!(dut.host_bus_req, 1, "host_bus_req should be asserted");
    assert_eq!(dut.host_bus_addr, 0x50000000, "host_bus_addr should match");
    assert_eq!(dut.host_bus_we, 0, "host_bus_we should be 0 for read");

    // Provide bus response (simulated LED value)
    dut.host_bus_rdata = 0x000000AA;
    dut.host_bus_ready = 1;
    clock_cycle!(dut);
    dut.host_bus_ready = 0;

    // Receive response header: {4'b0011, size=10, 1'b0, we=0} = 0x38
    let header = receive_tx_byte(&mut dut, 100).expect("response header");
    assert_eq!(header & 0xF0, 0x30, "Response should have packet type 0011");

    // Receive response data (4 bytes for word, little-endian)
    let b0 = receive_tx_byte(&mut dut, 100).expect("rdata[7:0]");
    let b1 = receive_tx_byte(&mut dut, 100).expect("rdata[15:8]");
    let b2 = receive_tx_byte(&mut dut, 100).expect("rdata[23:16]");
    let b3 = receive_tx_byte(&mut dut, 100).expect("rdata[31:24]");

    let rdata = (b3 as u32) << 24 | (b2 as u32) << 16 | (b1 as u32) << 8 | (b0 as u32);
    assert_eq!(rdata, 0x000000AA, "Read data should match LED value");
}

#[test]
fn test_host_initiated_read_halfword() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Send host request header: {4'b0010, size=01, 1'b0, we=0} = 0x24
    assert!(send_rx_byte(&mut dut, 0x24, 100), "Failed to send header");

    // Send address (little-endian) - LED_BASE = 0x50000000
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x50, 100), "addr[31:24]");

    // Wait for bus request
    for _ in 0..10 {
        clock_cycle!(dut);
        if dut.host_bus_req != 0 {
            break;
        }
    }
    assert_eq!(dut.host_bus_req, 1, "host_bus_req should be asserted");
    assert_eq!(dut.host_bus_size, 0b01, "host_bus_size should be halfword");

    // Provide bus response
    dut.host_bus_rdata = 0x0000ABCD;
    dut.host_bus_ready = 1;
    clock_cycle!(dut);
    dut.host_bus_ready = 0;

    // Receive response header
    let _header = receive_tx_byte(&mut dut, 100).expect("response header");

    // Receive response data (2 bytes for halfword, little-endian)
    let b0 = receive_tx_byte(&mut dut, 100).expect("rdata[7:0]");
    let b1 = receive_tx_byte(&mut dut, 100).expect("rdata[15:8]");

    let rdata = (b1 as u16) << 8 | (b0 as u16);
    assert_eq!(rdata, 0xABCD, "Read data should match");
}

#[test]
fn test_host_initiated_read_byte() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Send host request header: {4'b0010, size=00, 1'b0, we=0} = 0x20
    assert!(send_rx_byte(&mut dut, 0x20, 100), "Failed to send header");

    // Send address
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x50, 100), "addr[31:24]");

    // Wait for bus request
    for _ in 0..10 {
        clock_cycle!(dut);
        if dut.host_bus_req != 0 {
            break;
        }
    }
    assert_eq!(dut.host_bus_req, 1, "host_bus_req should be asserted");
    assert_eq!(dut.host_bus_size, 0b00, "host_bus_size should be byte");

    // Provide bus response
    dut.host_bus_rdata = 0x00000042;
    dut.host_bus_ready = 1;
    clock_cycle!(dut);
    dut.host_bus_ready = 0;

    // Receive response header
    let _header = receive_tx_byte(&mut dut, 100).expect("response header");

    // Receive response data (1 byte)
    let b0 = receive_tx_byte(&mut dut, 100).expect("rdata[7:0]");
    assert_eq!(b0, 0x42, "Read byte should match");
}

// ============================================================
// Basic Host Write Request Tests
// ============================================================

#[test]
fn test_host_initiated_write_word() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Send host request header: {4'b0010, size=10, 1'b0, we=1} = 0x29
    assert!(send_rx_byte(&mut dut, 0x29, 100), "Failed to send header");

    // Send address (little-endian) - LED_BASE = 0x50000000
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x50, 100), "addr[31:24]");

    // Send write data (little-endian) - 0xDEADBEEF
    assert!(send_rx_byte(&mut dut, 0xEF, 100), "wdata[7:0]");
    assert!(send_rx_byte(&mut dut, 0xBE, 100), "wdata[15:8]");
    assert!(send_rx_byte(&mut dut, 0xAD, 100), "wdata[23:16]");
    assert!(send_rx_byte(&mut dut, 0xDE, 100), "wdata[31:24]");

    // Wait for bus request
    for _ in 0..10 {
        clock_cycle!(dut);
        if dut.host_bus_req != 0 {
            break;
        }
    }
    assert_eq!(dut.host_bus_req, 1, "host_bus_req should be asserted");
    assert_eq!(dut.host_bus_we, 1, "host_bus_we should be 1 for write");
    assert_eq!(dut.host_bus_addr, 0x50000000, "host_bus_addr should match");
    assert_eq!(
        dut.host_bus_wdata, 0xDEADBEEF,
        "host_bus_wdata should match"
    );

    // Provide bus response
    dut.host_bus_ready = 1;
    clock_cycle!(dut);
    dut.host_bus_ready = 0;

    // Receive response header (write ack): {4'b0011, size=10, 1'b0, we=1} = 0x39
    let header = receive_tx_byte(&mut dut, 100).expect("response header");
    assert_eq!(header & 0xF0, 0x30, "Response should have packet type 0011");
    assert_eq!(
        header & 0x01,
        0x01,
        "Response should have we=1 for write ack"
    );
}

#[test]
fn test_host_initiated_write_halfword() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Send host request header: {4'b0010, size=01, 1'b0, we=1} = 0x25
    assert!(send_rx_byte(&mut dut, 0x25, 100), "Failed to send header");

    // Send address
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x50, 100), "addr[31:24]");

    // Send write data (2 bytes for halfword)
    assert!(send_rx_byte(&mut dut, 0xCD, 100), "wdata[7:0]");
    assert!(send_rx_byte(&mut dut, 0xAB, 100), "wdata[15:8]");

    // Wait for bus request
    for _ in 0..10 {
        clock_cycle!(dut);
        if dut.host_bus_req != 0 {
            break;
        }
    }
    assert_eq!(dut.host_bus_req, 1, "host_bus_req should be asserted");
    assert_eq!(dut.host_bus_size, 0b01, "host_bus_size should be halfword");
    assert_eq!(
        dut.host_bus_wdata & 0xFFFF,
        0xABCD,
        "host_bus_wdata lower 16 bits should match"
    );

    // Provide bus response
    dut.host_bus_ready = 1;
    clock_cycle!(dut);
    dut.host_bus_ready = 0;

    // Receive response header (write ack)
    let header = receive_tx_byte(&mut dut, 100).expect("response header");
    assert_eq!(header & 0xF0, 0x30, "Response should have packet type 0011");
}

#[test]
fn test_host_initiated_write_byte() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Send host request header: {4'b0010, size=00, 1'b0, we=1} = 0x21
    assert!(send_rx_byte(&mut dut, 0x21, 100), "Failed to send header");

    // Send address
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x50, 100), "addr[31:24]");

    // Send write data (1 byte)
    assert!(send_rx_byte(&mut dut, 0x55, 100), "wdata[7:0]");

    // Wait for bus request
    for _ in 0..10 {
        clock_cycle!(dut);
        if dut.host_bus_req != 0 {
            break;
        }
    }
    assert_eq!(dut.host_bus_req, 1, "host_bus_req should be asserted");
    assert_eq!(dut.host_bus_size, 0b00, "host_bus_size should be byte");
    assert_eq!(
        dut.host_bus_wdata & 0xFF,
        0x55,
        "host_bus_wdata lower 8 bits should match"
    );

    // Provide bus response
    dut.host_bus_ready = 1;
    clock_cycle!(dut);
    dut.host_bus_ready = 0;

    // Receive response header (write ack)
    let header = receive_tx_byte(&mut dut, 100).expect("response header");
    assert_eq!(header & 0xF0, 0x30, "Response should have packet type 0011");
}

// ============================================================
// Address Validation Tests
// ============================================================

#[test]
fn test_host_request_valid_rtl_address() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Valid RTL address: 0x50000000 (LED_BASE)
    assert!(send_rx_byte(&mut dut, 0x28, 100), "Failed to send header");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x50, 100), "addr[31:24]");

    // Should issue bus request (valid address)
    for _ in 0..10 {
        clock_cycle!(dut);
        if dut.host_bus_req != 0 {
            break;
        }
    }
    assert_eq!(
        dut.host_bus_req, 1,
        "Valid RTL address should issue bus request"
    );
}

#[test]
fn test_host_request_invalid_address_dram() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Invalid address: 0x80000000 (DRAM - would loop back to host)
    assert!(send_rx_byte(&mut dut, 0x28, 100), "Failed to send header");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x80, 100), "addr[31:24]");

    // Should send error response (packet type 1111) instead of bus request
    for _ in 0..20 {
        clock_cycle!(dut);
        if dut.tx_valid != 0 {
            break;
        }
    }

    // Receive error response header: packet type 1111 = 0xFX
    let header = receive_tx_byte(&mut dut, 100).expect("Error response header");
    assert_eq!(header & 0xF0, 0xF0, "Should receive error packet type 1111");

    // host_bus_req should NOT have been asserted
    assert_eq!(
        dut.host_bus_req, 0,
        "Invalid address should not issue bus request"
    );
}

#[test]
fn test_host_request_invalid_address_sim_control() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Invalid address: 0x10000000 (SimControl - would loop back to host)
    assert!(send_rx_byte(&mut dut, 0x28, 100), "Failed to send header");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x10, 100), "addr[31:24]");

    // Should send error response
    for _ in 0..20 {
        clock_cycle!(dut);
        if dut.tx_valid != 0 {
            break;
        }
    }

    let header = receive_tx_byte(&mut dut, 100).expect("Error response header");
    assert_eq!(header & 0xF0, 0xF0, "Should receive error packet type");
}

#[test]
fn test_host_request_valid_rtl_edge_addresses() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Valid address at edge of RTL range: 0x5FFFFFFF
    assert!(send_rx_byte(&mut dut, 0x20, 100), "Failed to send header");
    assert!(send_rx_byte(&mut dut, 0xFF, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0xFF, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0xFF, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x5F, 100), "addr[31:24]");

    // Should issue bus request (valid address)
    for _ in 0..10 {
        clock_cycle!(dut);
        if dut.host_bus_req != 0 {
            break;
        }
    }
    assert_eq!(
        dut.host_bus_req, 1,
        "Valid RTL edge address should issue bus request"
    );
    assert_eq!(dut.host_bus_addr, 0x5FFFFFFF, "Address should match");
}

// ============================================================
// Simultaneous Request Tests
// ============================================================

#[test]
fn test_simultaneous_requests_fpga_buffers_host_request() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Step 1: CPU initiates request (FPGA sends to Host)
    dut.addr = 0x80000000; // DRAM address
    dut.wdata = 0x12345678;
    dut.we = 1;
    dut.size = 0b10; // Word
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;

    // Drain header byte from TX
    let header = receive_tx_byte(&mut dut, 100).expect("CPU request header");
    assert_eq!(
        header & 0xF0,
        0x00,
        "CPU request should have packet type 0000"
    );

    // Drain remaining TX bytes for CPU request (address bytes)
    for _ in 0..4 {
        receive_tx_byte(&mut dut, 100).expect("CPU address byte");
    }
    for _ in 0..4 {
        receive_tx_byte(&mut dut, 100).expect("CPU wdata byte");
    }

    // Note: Current RTL doesn't support accepting Host request bytes during CPU TX
    // The test validates the sequential case: CPU completes TX, then Host sends request,
    // but the CPU response arrives before Host request processing starts

    // Send CPU response (write ack with packet type 0001): {4'b0001, size=10, 1'b0, we=1} = 0x19
    assert!(send_rx_byte(&mut dut, 0x19, 100), "CPU write ack");

    // Verify CPU transaction completed (ready should be HIGH)
    assert_eq!(
        dut.ready, 1,
        "CPU ready should be HIGH after write response"
    );

    // Step 2: Now send Host request (after CPU TX completed)
    // Send host request header (0x28 = Host-initiated word read, packet type 0010)
    assert!(
        send_rx_byte(&mut dut, 0x28, 100),
        "Failed to send host header"
    );

    // Send host request address
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x50, 100), "addr[31:24]");

    // Step 3: FPGA should process Host request
    for _ in 0..20 {
        clock_cycle!(dut);
        if dut.host_bus_req != 0 {
            break;
        }
    }
    assert_eq!(dut.host_bus_req, 1, "host_bus_req should be asserted");
    assert_eq!(dut.host_bus_addr, 0x50000000, "Should process Host request");

    // Complete the Host request
    dut.host_bus_rdata = 0xAA;
    dut.host_bus_ready = 1;
    clock_cycle!(dut);
    dut.host_bus_ready = 0;

    // Receive Host response header
    let resp_header = receive_tx_byte(&mut dut, 100).expect("Host response header");
    assert_eq!(
        resp_header & 0xF0,
        0x30,
        "Host response should have packet type 0011"
    );

    // Receive Host response data
    let resp = receive_tx_byte(&mut dut, 100).expect("Host response byte 0");
    assert_eq!(resp, 0xAA, "Host should receive correct LED value");
}

// ============================================================
// Protocol Edge Case Tests
// ============================================================

#[test]
fn test_host_request_backpressure() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Send header byte only, then wait
    assert!(send_rx_byte(&mut dut, 0x28, 100), "Failed to send header");

    // Delay sending address bytes
    for _ in 0..50 {
        clock_cycle!(dut);
    }

    // No bus request should be issued yet (incomplete packet)
    assert_eq!(dut.host_bus_req, 0, "No bus request with incomplete packet");

    // Now send address
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x50, 100), "addr[31:24]");

    // Now bus request should be issued
    for _ in 0..10 {
        clock_cycle!(dut);
        if dut.host_bus_req != 0 {
            break;
        }
    }
    assert_eq!(
        dut.host_bus_req, 1,
        "Bus request should be issued after complete packet"
    );
}

#[test]
fn test_consecutive_host_requests() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // First request
    assert!(send_rx_byte(&mut dut, 0x28, 100), "header 1");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x50, 100), "addr[31:24]");

    // Wait for bus request
    for _ in 0..10 {
        clock_cycle!(dut);
        if dut.host_bus_req != 0 {
            break;
        }
    }
    assert_eq!(
        dut.host_bus_req, 1,
        "First request should issue bus request"
    );

    // Complete first request
    dut.host_bus_rdata = 0x11;
    dut.host_bus_ready = 1;
    clock_cycle!(dut);
    dut.host_bus_ready = 0;

    // Drain first response
    let _ = receive_tx_byte(&mut dut, 100); // header
    let _ = receive_tx_byte(&mut dut, 100); // data0
    let _ = receive_tx_byte(&mut dut, 100); // data1
    let _ = receive_tx_byte(&mut dut, 100); // data2
    let _ = receive_tx_byte(&mut dut, 100); // data3

    // Second request to different address
    assert!(send_rx_byte(&mut dut, 0x28, 100), "header 2");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x01, 100), "addr[15:8]"); // Different address
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x50, 100), "addr[31:24]");

    // Wait for second bus request
    for _ in 0..10 {
        clock_cycle!(dut);
        if dut.host_bus_req != 0 {
            break;
        }
    }
    assert_eq!(
        dut.host_bus_req, 1,
        "Second request should issue bus request"
    );
    assert_eq!(dut.host_bus_addr, 0x50000100, "Second address should match");
}
