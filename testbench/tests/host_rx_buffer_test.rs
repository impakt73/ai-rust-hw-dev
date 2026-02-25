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
    assert_eq!(dut.packet_type, 0, "packet_type should reset to 0");
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
    assert_eq!(dut.packet_type, 0b0001, "packet_type should be response");
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
    assert_eq!(dut.packet_type, 0b0010, "packet_type should be request");
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
