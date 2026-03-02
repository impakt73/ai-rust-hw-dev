// Host Bus Interface Tests
// Focused burst-native protocol coverage.

use riscv_core::{create_host_bus_interface_runtime, HostBusInterface};

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

fn reset_module(dut: &mut HostBusInterface) {
    dut.rst_n = 0;
    dut.mem_a_valid = 0;
    dut.mem_a_we = 0;
    dut.mem_a_addr = 0;
    dut.mem_a_wdata = 0;
    dut.mem_a_size = 0;
    dut.mem_d_ready = 0;
    dut.tx_ready = 0;
    dut.rx_valid = 0;
    dut.rx_data = 0;
    dut.host_bus_ready = 0;
    dut.host_bus_rdata = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    clock_cycle!(dut);
}

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

fn send_rx_packet(dut: &mut HostBusInterface, bytes: &[u8]) {
    for &byte in bytes {
        assert!(
            send_rx_byte(dut, byte, 200),
            "failed to send byte 0x{byte:02X}"
        );
    }
}

fn collect_tx_bytes_with_bus_model(
    dut: &mut HostBusInterface,
    expected_len: usize,
    mut read_model: impl FnMut(u32) -> u32,
) -> Vec<u8> {
    let mut out = Vec::new();

    for _ in 0..2000 {
        dut.tx_ready = 0;
        dut.host_bus_ready = 0;

        dut.eval();

        if dut.host_bus_req != 0 {
            dut.host_bus_ready = 1;
            if dut.host_bus_we == 0 {
                dut.host_bus_rdata = read_model(dut.host_bus_addr);
            }
        }

        dut.eval();

        if dut.tx_valid != 0 {
            dut.tx_ready = 1;
            dut.eval();
            out.push(dut.tx_data);
        }

        clock_cycle!(dut);

        if out.len() == expected_len {
            break;
        }
    }

    out
}

#[test]
fn test_reset_state() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    assert_eq!(dut.mem_d_valid, 0);
    assert_eq!(dut.tx_valid, 0);
    assert_eq!(dut.rx_ready, 1);
    assert_eq!(dut.host_bus_req, 0);
}

#[test]
fn test_cpu_single_write_request_uses_8byte_metadata_header() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // CPU -> host single-beat write request
    dut.mem_a_addr = 0x1234_5678;
    dut.mem_a_wdata = 0xDEAD_BEEF;
    dut.mem_a_we = 1;
    dut.mem_a_size = 0b10;
    dut.mem_a_valid = 1;
    clock_cycle!(dut);
    dut.mem_a_valid = 0;

    let tx_packet = collect_tx_bytes_with_bus_model(&mut dut, 12, |_| 0);
    assert_eq!(
        tx_packet.len(),
        12,
        "word write should be 8-byte metadata + 4-byte payload"
    );

    // Metadata
    assert_eq!(tx_packet[0], 0x08, "CTRL0 mismatch"); // type=0000 size=10 src/dst fixed cleared
    assert_eq!(tx_packet[1], 0x01, "CTRL1 mismatch"); // we=1
    assert_eq!(tx_packet[2], 0x00, "burst_len_m1[7:0] mismatch");
    assert_eq!(tx_packet[3], 0x00, "burst_len_m1[15:8] mismatch");
    assert_eq!(tx_packet[4], 0x78, "addr[7:0] mismatch");
    assert_eq!(tx_packet[5], 0x56, "addr[15:8] mismatch");
    assert_eq!(tx_packet[6], 0x34, "addr[23:16] mismatch");
    assert_eq!(tx_packet[7], 0x12, "addr[31:24] mismatch");

    // Payload beat
    assert_eq!(tx_packet[8], 0xEF);
    assert_eq!(tx_packet[9], 0xBE);
    assert_eq!(tx_packet[10], 0xAD);
    assert_eq!(tx_packet[11], 0xDE);
}

#[test]
fn test_host_read_burst_streams_two_beats_and_echoes_metadata() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Host -> FPGA read request, burst_len=2, incrementing source
    send_rx_packet(&mut dut, &[0x28, 0x00, 0x01, 0x00, 0x00, 0x10, 0x00, 0x50]);

    let tx_packet = collect_tx_bytes_with_bus_model(&mut dut, 16, |addr| match addr {
        0x5000_1000 => 0xA1A2_A3A4,
        0x5000_1004 => 0xB1B2_B3B4,
        _ => 0,
    });

    assert_eq!(
        tx_packet.len(),
        16,
        "read response should include metadata + two data beats"
    );

    // Response metadata (type 0011)
    assert_eq!(tx_packet[0], 0x38);
    assert_eq!(tx_packet[1], 0x00);
    assert_eq!(tx_packet[2], 0x01);
    assert_eq!(tx_packet[3], 0x00);
    assert_eq!(tx_packet[4], 0x00);
    assert_eq!(tx_packet[5], 0x10);
    assert_eq!(tx_packet[6], 0x00);
    assert_eq!(tx_packet[7], 0x50);

    // Payload beats
    assert_eq!(&tx_packet[8..12], &[0xA4, 0xA3, 0xA2, 0xA1]);
    assert_eq!(&tx_packet[12..16], &[0xB4, 0xB3, 0xB2, 0xB1]);
}

#[test]
fn test_host_write_burst_dst_fixed_keeps_bus_address() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Host -> FPGA write request, burst_len=2, dst_fixed=1
    send_rx_packet(
        &mut dut,
        &[
            0x29, 0x01, 0x01, 0x00, 0x00, 0xF0, 0xFF, 0x52, 0x44, 0x33, 0x22, 0x11, 0x88, 0x77,
            0x66, 0x55,
        ],
    );

    let mut seen_addrs = Vec::new();
    let mut seen_wdata = Vec::new();
    let mut tx_packet = Vec::new();

    for _ in 0..500 {
        dut.host_bus_ready = 0;
        dut.tx_ready = 0;
        dut.eval();

        if dut.host_bus_req != 0 {
            seen_addrs.push(dut.host_bus_addr);
            seen_wdata.push(dut.host_bus_wdata);
            dut.host_bus_ready = 1;
        }

        if dut.tx_valid != 0 {
            dut.tx_ready = 1;
            dut.eval();
            tx_packet.push(dut.tx_data);
        }

        clock_cycle!(dut);

        if tx_packet.len() == 8 && seen_addrs.len() >= 2 {
            break;
        }
    }

    assert!(
        seen_addrs.len() >= 2,
        "expected at least two write beats on bus master port"
    );
    assert_eq!(seen_addrs[0], 0x52FF_F000);
    assert_eq!(
        seen_addrs[1], 0x52FF_F000,
        "dst_fixed write should keep same address"
    );
    assert_eq!(seen_wdata[0], 0x1122_3344);
    assert_eq!(seen_wdata[1], 0x5566_7788);

    // Write response has metadata only (8 bytes)
    assert_eq!(tx_packet.len(), 8);
    assert_eq!(tx_packet[0], 0x39); // type=0011 size=word dst_fixed=1
    assert_eq!(tx_packet[1], 0x01); // we=1
    assert_eq!(tx_packet[2], 0x01);
    assert_eq!(tx_packet[3], 0x00);
    assert_eq!(tx_packet[4], 0x00);
    assert_eq!(tx_packet[5], 0xF0);
    assert_eq!(tx_packet[6], 0xFF);
    assert_eq!(tx_packet[7], 0x52);
}
