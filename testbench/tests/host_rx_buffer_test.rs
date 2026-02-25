// Host RX Buffer Tests
// Validates unified packet buffering for response/request packet types.

use riscv_core::{create_host_rx_buffer_runtime, HostRxBuffer};

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

fn reset_module(dut: &mut HostRxBuffer) {
    dut.rst_n = 0;
    dut.rx_valid = 0;
    dut.rx_data = 0;
    dut.packet_ready = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    clock_cycle!(dut);
}

fn send_rx_byte(dut: &mut HostRxBuffer, byte: u8, max_cycles: u32) -> bool {
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

#[test]
fn test_reset_state() {
    let runtime = create_host_rx_buffer_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostRxBuffer>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    assert_eq!(
        dut.packet_valid, 0,
        "packet_valid should be LOW after reset"
    );
    assert_eq!(dut.packet_req, 0, "packet_req should reset to 0");
    assert_eq!(dut.packet_size, 0, "packet_size should reset to 0");
    assert_eq!(dut.packet_addr, 0, "packet_addr should reset to 0");
    assert_eq!(dut.packet_data, 0, "packet_data should reset to 0");
    assert_eq!(dut.rx_ready, 1, "rx_ready should be HIGH after reset");
}

#[test]
fn test_receive_response_word_read_packet() {
    let runtime = create_host_rx_buffer_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostRxBuffer>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // type=0001, size=10 (word), we=0
    assert!(send_rx_byte(&mut dut, 0x18, 100), "Failed to send header");
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

    assert_eq!(dut.packet_valid, 1, "packet_valid should be HIGH");
    assert_eq!(dut.packet_req, 0, "packet_req should be 0 for response");
    assert_eq!(dut.packet_we, 0, "packet_we should be 0 for read response");
    assert_eq!(dut.packet_size, 0b10, "packet_size should be word");
    assert_eq!(
        dut.packet_addr, 0,
        "packet_addr should be unused for response"
    );
    assert_eq!(
        dut.packet_data, 0xCAFEBABE,
        "packet_data should contain rdata"
    );
}

#[test]
fn test_receive_request_write_word_packet() {
    let runtime = create_host_rx_buffer_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostRxBuffer>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // type=0010, size=10 (word), we=1
    assert!(send_rx_byte(&mut dut, 0x29, 100), "Failed to send header");
    // address 0x50000000, little-endian
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x50, 100), "addr[31:24]");
    // data 0xDEADBEEF, little-endian
    assert!(send_rx_byte(&mut dut, 0xEF, 100), "data[7:0]");
    assert!(send_rx_byte(&mut dut, 0xBE, 100), "data[15:8]");
    assert!(send_rx_byte(&mut dut, 0xAD, 100), "data[23:16]");
    assert!(send_rx_byte(&mut dut, 0xDE, 100), "data[31:24]");

    assert_eq!(dut.packet_valid, 1, "packet_valid should be HIGH");
    assert_eq!(dut.packet_req, 1, "packet_req should be 1 for request");
    assert_eq!(dut.packet_we, 1, "packet_we should be 1 for write request");
    assert_eq!(dut.packet_size, 0b10, "packet_size should be word");
    assert_eq!(dut.packet_addr, 0x50000000, "packet_addr mismatch");
    assert_eq!(
        dut.packet_data, 0xDEADBEEF,
        "packet_data should contain wdata"
    );
}

#[test]
fn test_packet_ready_clears_valid() {
    let runtime = create_host_rx_buffer_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostRxBuffer>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // type=0001, size=10, we=1 (header-only packet)
    assert!(send_rx_byte(&mut dut, 0x19, 100), "Failed to send header");
    assert_eq!(dut.packet_valid, 1, "packet_valid should be HIGH");

    dut.packet_ready = 1;
    clock_cycle!(dut);
    dut.packet_ready = 0;
    dut.eval();

    assert_eq!(
        dut.packet_valid, 0,
        "packet_valid should clear after packet_ready"
    );
}

#[test]
fn test_backpressure_with_single_packet_storage() {
    let runtime = create_host_rx_buffer_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostRxBuffer>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Fill the unified buffer with a complete packet.
    assert!(send_rx_byte(&mut dut, 0x19, 100), "Failed to send header");
    assert_eq!(dut.packet_valid, 1, "packet_valid should be HIGH");
    assert_eq!(
        dut.rx_ready, 0,
        "rx_ready should be LOW when buffer is full"
    );

    // Consume packet and verify flow-control recovery.
    dut.packet_ready = 1;
    clock_cycle!(dut);
    dut.packet_ready = 0;
    dut.eval();

    assert_eq!(
        dut.packet_valid, 0,
        "packet_valid should be LOW after ready"
    );
    assert_eq!(
        dut.rx_ready, 1,
        "rx_ready should recover after packet is consumed"
    );
}

#[test]
fn test_response_byte_and_halfword_reads() {
    let runtime = create_host_rx_buffer_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostRxBuffer>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Byte response: type=0001, size=00, we=0
    assert!(send_rx_byte(&mut dut, 0x10, 100), "header byte response");
    assert!(send_rx_byte(&mut dut, 0x42, 100), "data[7:0] byte response");
    assert_eq!(dut.packet_valid, 1, "byte response should be valid");
    assert_eq!(dut.packet_req, 0, "byte response should not be request");
    assert_eq!(dut.packet_size, 0b00, "byte response size mismatch");
    assert_eq!(dut.packet_data, 0x0000_0042, "byte response data mismatch");

    dut.packet_ready = 1;
    clock_cycle!(dut);
    dut.packet_ready = 0;
    dut.eval();

    // Halfword response: type=0001, size=01, we=0
    assert!(send_rx_byte(&mut dut, 0x14, 100), "header half response");
    assert!(send_rx_byte(&mut dut, 0xCD, 100), "data[7:0] half response");
    assert!(
        send_rx_byte(&mut dut, 0xAB, 100),
        "data[15:8] half response"
    );
    assert_eq!(dut.packet_valid, 1, "halfword response should be valid");
    assert_eq!(dut.packet_size, 0b01, "halfword response size mismatch");
    assert_eq!(
        dut.packet_data, 0x0000_ABCD,
        "halfword response data mismatch"
    );
}

#[test]
fn test_write_response_header_only_semantics() {
    let runtime = create_host_rx_buffer_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostRxBuffer>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // type=0001, size=10, we=1 (no payload bytes)
    assert!(send_rx_byte(&mut dut, 0x19, 100), "write response header");
    assert_eq!(
        dut.packet_valid, 1,
        "write response should complete on header"
    );
    assert_eq!(dut.packet_req, 0, "write response should not be request");
    assert_eq!(dut.packet_we, 1, "write response should preserve we");
    assert_eq!(dut.packet_size, 0b10, "write response size mismatch");
    assert_eq!(
        dut.packet_data, 0,
        "write response should have empty data payload"
    );
}

#[test]
fn test_request_read_and_subword_write() {
    let runtime = create_host_rx_buffer_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostRxBuffer>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Request read: type=0010, size=10, we=0
    assert!(send_rx_byte(&mut dut, 0x28, 100), "read request header");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "read addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "read addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "read addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x50, 100), "read addr[31:24]");
    assert_eq!(
        dut.packet_valid, 1,
        "read request should be valid after address"
    );
    assert_eq!(dut.packet_req, 1, "read request should set packet_req");
    assert_eq!(dut.packet_we, 0, "read request should clear we");
    assert_eq!(
        dut.packet_addr, 0x5000_0000,
        "read request address mismatch"
    );

    dut.packet_ready = 1;
    clock_cycle!(dut);
    dut.packet_ready = 0;
    dut.eval();

    // Request write byte: type=0010, size=00, we=1
    assert!(send_rx_byte(&mut dut, 0x21, 100), "byte write header");
    assert!(send_rx_byte(&mut dut, 0x04, 100), "byte addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x03, 100), "byte addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x02, 100), "byte addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x01, 100), "byte addr[31:24]");
    assert!(send_rx_byte(&mut dut, 0xA5, 100), "byte write data");
    assert_eq!(dut.packet_valid, 1, "byte write request should be valid");
    assert_eq!(
        dut.packet_req, 1,
        "byte write request should set packet_req"
    );
    assert_eq!(dut.packet_size, 0b00, "byte write request size mismatch");
    assert_eq!(
        dut.packet_addr, 0x0102_0304,
        "byte write request address mismatch"
    );
    assert_eq!(
        dut.packet_data, 0x0000_00A5,
        "byte write request data mismatch"
    );
}
