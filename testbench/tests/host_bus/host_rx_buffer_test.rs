use riscv_core::AsDynamicVerilatedModel;
// Host RX Buffer Tests
// Focused burst-native protocol coverage for 8-byte metadata framing.

use riscv_core::{create_host_bus_rx_runtime, HostBusRx};

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

fn reset_module(dut: &mut HostBusRx) {
    dut.rst = 1;
    dut.rx_valid = 0;
    dut.rx_data = 0;
    dut.packet_ready = 0;
    clock_cycle!(dut);
    dut.rst = 0;
    clock_cycle!(dut);
}

fn send_rx_byte(dut: &mut HostBusRx, byte: u8, max_cycles: u32) -> bool {
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

fn send_packet_bytes(dut: &mut HostBusRx, bytes: &[u8]) {
    for &byte in bytes {
        assert!(
            send_rx_byte(dut, byte, 200),
            "Failed to send byte 0x{byte:02X}"
        );
    }
}

fn consume_packet(dut: &mut HostBusRx) {
    dut.packet_ready = 1;
    clock_cycle!(dut);
    dut.packet_ready = 0;
    dut.eval();
}

#[test]
fn test_reset_state() {
    let runtime = create_host_bus_rx_runtime().expect("Failed to create runtime");
    let mut dut =
        testbench::create_testbench_model::<HostBusRx>(&runtime).expect("Failed to create model");

    reset_module(&mut dut);

    assert_eq!(dut.packet_valid, 0);
    assert_eq!(dut.packet_start, 0);
    assert_eq!(dut.packet_last, 0);
    assert_eq!(dut.rx_ready, 1);
}

#[test]
fn test_decode_metadata_only_read_request_with_flags() {
    let runtime = create_host_bus_rx_runtime().expect("Failed to create runtime");
    let mut dut =
        testbench::create_testbench_model::<HostBusRx>(&runtime).expect("Failed to create model");

    reset_module(&mut dut);

    // Host-initiated read request:
    // CTRL0 = type 0010, size=word(10), src_fixed=1, dst_fixed=0 => 0x2A
    // CTRL1 = we=0
    // burst_len_m1 = 0x0003 (4 beats)
    // base_addr = 0x5000_2000
    send_packet_bytes(&mut dut, &[0x2A, 0x00, 0x03, 0x00, 0x00, 0x20, 0x00, 0x50]);

    assert_eq!(dut.packet_valid, 1);
    assert_eq!(dut.packet_start, 1);
    assert_eq!(dut.packet_last, 1);
    assert_eq!(dut.packet_req, 1);
    assert_eq!(dut.packet_we, 0);
    assert_eq!(dut.packet_size, 0b10);
    assert_eq!(dut.packet_src_fixed, 1);
    assert_eq!(dut.packet_dst_fixed, 0);
    assert_eq!(dut.packet_burst_len_m1, 3);
    assert_eq!(dut.packet_base_addr, 0x5000_2000);
    assert_eq!(dut.packet_data, 0);

    consume_packet(&mut dut);
    assert_eq!(dut.packet_valid, 0);
}

#[test]
fn test_streaming_two_beat_write_request() {
    let runtime = create_host_bus_rx_runtime().expect("Failed to create runtime");
    let mut dut =
        testbench::create_testbench_model::<HostBusRx>(&runtime).expect("Failed to create model");

    reset_module(&mut dut);

    // Host-initiated write request:
    // CTRL0 = type 0010, size=word(10), src_fixed=0, dst_fixed=1 => 0x29
    // CTRL1 = we=1
    // burst_len_m1 = 1 (2 beats)
    // base_addr = 0x5000_1000
    send_packet_bytes(
        &mut dut,
        &[
            0x29, 0x01, 0x01, 0x00, 0x00, 0x10, 0x00, 0x50, 0x44, 0x33, 0x22,
            0x11, // beat0 = 0x11223344
        ],
    );

    assert_eq!(dut.packet_valid, 1);
    assert_eq!(dut.packet_start, 1);
    assert_eq!(dut.packet_last, 0);
    assert_eq!(dut.packet_req, 1);
    assert_eq!(dut.packet_we, 1);
    assert_eq!(dut.packet_dst_fixed, 1);
    assert_eq!(dut.packet_burst_len_m1, 1);
    assert_eq!(dut.packet_base_addr, 0x5000_1000);
    assert_eq!(dut.packet_data, 0x1122_3344);

    // Backpressure while output beat is pending.
    assert_eq!(dut.rx_ready, 0);

    consume_packet(&mut dut);

    send_packet_bytes(
        &mut dut,
        &[
            0x88, 0x77, 0x66, 0x55, // beat1 = 0x55667788
        ],
    );

    assert_eq!(dut.packet_valid, 1);
    assert_eq!(dut.packet_start, 0);
    assert_eq!(dut.packet_last, 1);
    assert_eq!(dut.packet_data, 0x5566_7788);

    consume_packet(&mut dut);
    assert_eq!(dut.packet_valid, 0);
}

#[test]
fn test_single_beat_write_compatibility() {
    let runtime = create_host_bus_rx_runtime().expect("Failed to create runtime");
    let mut dut =
        testbench::create_testbench_model::<HostBusRx>(&runtime).expect("Failed to create model");

    reset_module(&mut dut);

    // Single-beat (legacy compatible) write request: burst_len_m1 = 0
    // [CTRL0=0x29][CTRL1=0x01][len_m1=0x0000][addr=0x12345678][data=0xDEADBEEF]
    send_packet_bytes(
        &mut dut,
        &[
            0x29, 0x01, 0x00, 0x00, 0x78, 0x56, 0x34, 0x12, 0xEF, 0xBE, 0xAD, 0xDE,
        ],
    );

    assert_eq!(dut.packet_valid, 1);
    assert_eq!(dut.packet_start, 1);
    assert_eq!(dut.packet_last, 1);
    assert_eq!(dut.packet_burst_len_m1, 0);
    assert_eq!(dut.packet_base_addr, 0x1234_5678);
    assert_eq!(dut.packet_data, 0xDEAD_BEEF);
}
