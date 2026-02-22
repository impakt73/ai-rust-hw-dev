// Host Bus RX Tests
// Comprehensive testing of the host_bus_rx RTL module
//
// The host_bus_rx module handles:
//   - Response packets (type 0001) for CPU-initiated requests
//   - Request packets (type 0010) for host-initiated requests
//
// Extended Header Format:
//   Bits [7:4]: Packet type
//     0001 = Host response to CPU request (Host → FPGA RX)
//     0010 = Host-initiated request (Host → FPGA RX)
//   Bits [3:2]: size (00=byte, 01=half, 10=word, 11=reserved)
//   Bit  [1]:   Reserved (0)
//   Bit  [0]:   we (1=write, 0=read)

use riscv_core::{create_host_bus_rx_runtime, HostBusRx};

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
fn reset_module(dut: &mut HostBusRx) {
    dut.rst_n = 0;
    dut.rx_valid = 0;
    dut.rx_data = 0;
    dut.resp_consumed = 0;
    dut.req_consumed = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    clock_cycle!(dut);
}

/// Helper to send a byte to RX interface
/// Returns true if handshake completed within max_cycles, false otherwise.
fn send_rx_byte(dut: &mut HostBusRx, byte: u8, max_cycles: u32) -> bool {
    dut.rx_data = byte;
    dut.rx_valid = 1;
    dut.eval();

    // Wait for rx_ready to be asserted (handshake condition)
    for _ in 0..max_cycles {
        if dut.rx_ready != 0 {
            // Handshake complete: advance clock to latch the data, then deassert rx_valid
            clock_cycle!(dut);
            dut.rx_valid = 0;
            dut.eval();
            return true;
        }
        // rx_ready not yet asserted, advance clock and try again
        clock_cycle!(dut);
    }
    // Timeout: deassert rx_valid and return failure
    dut.rx_valid = 0;
    dut.eval();
    false
}

// ============================================================
// Reset State Tests
// ============================================================

#[test]
fn test_reset_state() {
    let runtime = create_host_bus_rx_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusRx>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Verify outputs are in expected initial state
    assert_eq!(dut.resp_valid, 0, "resp_valid should be LOW after reset");
    assert_eq!(dut.req_valid, 0, "req_valid should be LOW after reset");
    assert_eq!(
        dut.rx_ready, 1,
        "rx_ready should be HIGH after reset (both buffers empty)"
    );
    assert_eq!(dut.resp_rdata, 0, "resp_rdata should be 0 after reset");
    assert_eq!(dut.req_addr, 0, "req_addr should be 0 after reset");
    assert_eq!(dut.req_wdata, 0, "req_wdata should be 0 after reset");
}

// ============================================================
// Response Packet Tests (Type 0001)
// ============================================================

#[test]
fn test_receive_write_response() {
    let runtime = create_host_bus_rx_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusRx>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Send write response header: type=0001, size=10 (word), we=1
    // Header: 0001 10 0 1 = 0x19
    assert!(
        send_rx_byte(&mut dut, 0x19, 100),
        "Failed to send write response header"
    );

    // Write response has no data - should be valid immediately
    assert_eq!(
        dut.resp_valid, 1,
        "resp_valid should be HIGH after write response"
    );
    assert_eq!(dut.resp_we, 1, "resp_we should be 1 for write response");
    assert_eq!(dut.resp_size, 0b10, "resp_size should be word (10)");
}

#[test]
fn test_receive_response_byte_read() {
    let runtime = create_host_bus_rx_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusRx>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Send byte read response header: type=0001, size=00 (byte), we=0
    // Header: 0001 00 0 0 = 0x10
    assert!(
        send_rx_byte(&mut dut, 0x10, 100),
        "Failed to send read response header"
    );

    // Not yet valid - need to receive data byte
    assert_eq!(
        dut.resp_valid, 0,
        "resp_valid should be LOW before data byte"
    );

    // Send data byte
    assert!(
        send_rx_byte(&mut dut, 0x42, 100),
        "Failed to send data byte"
    );

    // Now response should be valid
    assert_eq!(
        dut.resp_valid, 1,
        "resp_valid should be HIGH after data byte"
    );
    assert_eq!(dut.resp_we, 0, "resp_we should be 0 for read response");
    assert_eq!(dut.resp_size, 0b00, "resp_size should be byte (00)");
    assert_eq!(dut.resp_rdata, 0x00000042, "resp_rdata should be 0x42");
}

#[test]
fn test_receive_response_halfword_read() {
    let runtime = create_host_bus_rx_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusRx>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Send halfword read response header: type=0001, size=01 (halfword), we=0
    // Header: 0001 01 0 0 = 0x14
    assert!(send_rx_byte(&mut dut, 0x14, 100), "Failed to send header");

    // Send 2 data bytes (little-endian: LSB first)
    assert!(
        send_rx_byte(&mut dut, 0xCD, 100),
        "Failed to send data[7:0]"
    );
    assert!(
        send_rx_byte(&mut dut, 0xAB, 100),
        "Failed to send data[15:8]"
    );

    // Response should be valid
    assert_eq!(dut.resp_valid, 1, "resp_valid should be HIGH");
    assert_eq!(dut.resp_size, 0b01, "resp_size should be halfword (01)");
    assert_eq!(dut.resp_rdata, 0x0000ABCD, "resp_rdata should be 0xABCD");
}

#[test]
fn test_receive_response_word_read() {
    let runtime = create_host_bus_rx_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusRx>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Send word read response header: type=0001, size=10 (word), we=0
    // Header: 0001 10 0 0 = 0x18
    assert!(send_rx_byte(&mut dut, 0x18, 100), "Failed to send header");

    // Send 4 data bytes (little-endian: LSB first)
    assert!(
        send_rx_byte(&mut dut, 0xBE, 100),
        "Failed to send data[7:0]"
    );
    assert!(
        send_rx_byte(&mut dut, 0xBA, 100),
        "Failed to send data[15:8]"
    );
    assert!(
        send_rx_byte(&mut dut, 0xFE, 100),
        "Failed to send data[23:16]"
    );
    assert!(
        send_rx_byte(&mut dut, 0xCA, 100),
        "Failed to send data[31:24]"
    );

    // Response should be valid
    assert_eq!(dut.resp_valid, 1, "resp_valid should be HIGH");
    assert_eq!(dut.resp_size, 0b10, "resp_size should be word (10)");
    assert_eq!(
        dut.resp_rdata, 0xCAFEBABE,
        "resp_rdata should be 0xCAFEBABE"
    );
}

// ============================================================
// Request Packet Tests (Type 0010)
// ============================================================

#[test]
fn test_receive_request_read() {
    let runtime = create_host_bus_rx_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusRx>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Send read request header: type=0010, size=10 (word), we=0
    // Header: 0010 10 0 0 = 0x28
    assert!(send_rx_byte(&mut dut, 0x28, 100), "Failed to send header");

    // Request not yet valid - need address bytes
    assert_eq!(dut.req_valid, 0, "req_valid should be LOW before address");

    // Send 4 address bytes (little-endian: 0x50000000)
    assert!(
        send_rx_byte(&mut dut, 0x00, 100),
        "Failed to send addr[7:0]"
    );
    assert!(
        send_rx_byte(&mut dut, 0x00, 100),
        "Failed to send addr[15:8]"
    );
    assert!(
        send_rx_byte(&mut dut, 0x00, 100),
        "Failed to send addr[23:16]"
    );
    assert!(
        send_rx_byte(&mut dut, 0x50, 100),
        "Failed to send addr[31:24]"
    );

    // Read request complete - no data bytes needed
    assert_eq!(dut.req_valid, 1, "req_valid should be HIGH");
    assert_eq!(dut.req_we, 0, "req_we should be 0 for read request");
    assert_eq!(dut.req_size, 0b10, "req_size should be word (10)");
    assert_eq!(dut.req_addr, 0x50000000, "req_addr should be 0x50000000");
}

#[test]
fn test_receive_request_write_byte() {
    let runtime = create_host_bus_rx_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusRx>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Send byte write request header: type=0010, size=00 (byte), we=1
    // Header: 0010 00 0 1 = 0x21
    assert!(send_rx_byte(&mut dut, 0x21, 100), "Failed to send header");

    // Send 4 address bytes (0x50000000)
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x50, 100), "addr[31:24]");

    // Request not yet valid - need 1 data byte
    assert_eq!(dut.req_valid, 0, "req_valid should be LOW before data");

    // Send 1 data byte
    assert!(send_rx_byte(&mut dut, 0xAB, 100), "wdata[7:0]");

    // Request complete
    assert_eq!(dut.req_valid, 1, "req_valid should be HIGH");
    assert_eq!(dut.req_we, 1, "req_we should be 1 for write request");
    assert_eq!(dut.req_size, 0b00, "req_size should be byte (00)");
    assert_eq!(dut.req_addr, 0x50000000, "req_addr mismatch");
    assert_eq!(dut.req_wdata, 0x000000AB, "req_wdata mismatch");
}

#[test]
fn test_receive_request_write_word() {
    let runtime = create_host_bus_rx_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusRx>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Send word write request header: type=0010, size=10 (word), we=1
    // Header: 0010 10 0 1 = 0x29
    assert!(send_rx_byte(&mut dut, 0x29, 100), "Failed to send header");

    // Send 4 address bytes (0x50000000)
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x50, 100), "addr[31:24]");

    // Send 4 data bytes (0xDEADBEEF little-endian)
    assert!(send_rx_byte(&mut dut, 0xEF, 100), "wdata[7:0]");
    assert!(send_rx_byte(&mut dut, 0xBE, 100), "wdata[15:8]");
    assert!(send_rx_byte(&mut dut, 0xAD, 100), "wdata[23:16]");
    assert!(send_rx_byte(&mut dut, 0xDE, 100), "wdata[31:24]");

    // Request complete
    assert_eq!(dut.req_valid, 1, "req_valid should be HIGH");
    assert_eq!(dut.req_we, 1, "req_we should be 1");
    assert_eq!(dut.req_size, 0b10, "req_size should be word (10)");
    assert_eq!(dut.req_addr, 0x50000000, "req_addr mismatch");
    assert_eq!(dut.req_wdata, 0xDEADBEEF, "req_wdata mismatch");
}

// ============================================================
// Buffer Management Tests
// ============================================================

#[test]
fn test_consume_response() {
    let runtime = create_host_bus_rx_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusRx>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Fill response buffer with write response
    assert!(send_rx_byte(&mut dut, 0x19, 100), "Failed to send header");
    assert_eq!(dut.resp_valid, 1, "resp_valid should be HIGH");

    // Consume the response
    dut.resp_consumed = 1;
    clock_cycle!(dut);
    dut.resp_consumed = 0;

    // Response should now be invalid
    assert_eq!(dut.resp_valid, 0, "resp_valid should be LOW after consume");
}

#[test]
fn test_consume_request() {
    let runtime = create_host_bus_rx_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusRx>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Fill request buffer with read request
    assert!(send_rx_byte(&mut dut, 0x28, 100), "header"); // type=0010, size=10, we=0
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x50, 100), "addr[31:24]");

    assert_eq!(dut.req_valid, 1, "req_valid should be HIGH");

    // Consume the request
    dut.req_consumed = 1;
    clock_cycle!(dut);
    dut.req_consumed = 0;

    // Request should now be invalid
    assert_eq!(dut.req_valid, 0, "req_valid should be LOW after consume");
}

#[test]
#[ignore = "Test requires dual-buffer behavior (response + request simultaneously). New design buffers only one packet type at a time per architectural simplification."]
fn test_both_buffers_can_be_filled() {
    let runtime = create_host_bus_rx_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusRx>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Fill response buffer with write response
    assert!(send_rx_byte(&mut dut, 0x19, 100), "response header");
    assert_eq!(dut.resp_valid, 1, "resp_valid should be HIGH");

    // Fill request buffer with read request
    assert!(send_rx_byte(&mut dut, 0x28, 100), "request header");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x50, 100), "addr[31:24]");
    assert_eq!(dut.req_valid, 1, "req_valid should be HIGH");

    // Both buffers full - rx_ready should be LOW
    assert_eq!(
        dut.rx_ready, 0,
        "rx_ready should be LOW when both buffers are full"
    );
}

#[test]
#[ignore = "Test requires dual-buffer behavior (response + request simultaneously). New design buffers only one packet type at a time per architectural simplification."]
fn test_backpressure_recovery() {
    let runtime = create_host_bus_rx_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusRx>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Fill response buffer
    assert!(send_rx_byte(&mut dut, 0x19, 100), "response header");
    assert_eq!(dut.resp_valid, 1, "resp_valid should be HIGH");

    // Fill request buffer
    assert!(send_rx_byte(&mut dut, 0x28, 100), "request header");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x50, 100), "addr[31:24]");
    assert_eq!(dut.req_valid, 1, "req_valid should be HIGH");

    // Both buffers full - rx_ready should be LOW
    assert_eq!(dut.rx_ready, 0, "rx_ready should be LOW");

    // Consume response buffer
    dut.resp_consumed = 1;
    clock_cycle!(dut);
    dut.resp_consumed = 0;

    // rx_ready should recover
    assert_eq!(
        dut.rx_ready, 1,
        "rx_ready should be HIGH after consuming response"
    );
}

#[test]
fn test_interleaved_packets() {
    let runtime = create_host_bus_rx_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusRx>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Send first response
    assert!(send_rx_byte(&mut dut, 0x19, 100), "response 1 header");
    assert_eq!(dut.resp_valid, 1, "resp_valid should be HIGH");

    // Consume it
    dut.resp_consumed = 1;
    clock_cycle!(dut);
    dut.resp_consumed = 0;
    assert_eq!(dut.resp_valid, 0, "resp_valid should be LOW");

    // Send first request
    assert!(send_rx_byte(&mut dut, 0x28, 100), "request 1 header");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x50, 100), "addr[31:24]");
    assert_eq!(dut.req_valid, 1, "req_valid should be HIGH");

    // Consume it
    dut.req_consumed = 1;
    clock_cycle!(dut);
    dut.req_consumed = 0;
    assert_eq!(dut.req_valid, 0, "req_valid should be LOW");

    // Send second response
    assert!(send_rx_byte(&mut dut, 0x19, 100), "response 2 header");
    assert_eq!(dut.resp_valid, 1, "resp_valid should be HIGH again");
}
