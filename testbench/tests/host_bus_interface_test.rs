// Host Bus Interface Tests
// Focused burst-native protocol coverage.

use riscv_core::{create_host_bus_interface_runtime, HostBusInterface};

const MAX_RESPONSE_WAIT_CYCLES: usize = 10;
const MAX_COLLECTION_CYCLES: usize = 2000;
const MAX_TARGETED_TEST_CYCLES: usize = 500;

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
    dut.rst = 1;
    dut.mem_a_valid = 0;
    dut.mem_a_we = 0;
    dut.mem_a_addr = 0;
    dut.mem_a_wdata = 0;
    dut.mem_a_size = 0;
    dut.mem_d_ready = 0;
    dut.tx_ready = 0;
    dut.rx_valid = 0;
    dut.rx_data = 0;
    dut.host_mem_a_ready = 0;
    dut.host_mem_d_valid = 0;
    dut.host_mem_d_rdata = 0;
    clock_cycle!(dut);
    dut.rst = 0;
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
    let mut pending_response: Option<u32> = None;

    for _ in 0..MAX_COLLECTION_CYCLES {
        dut.tx_ready = 0;
        dut.host_mem_a_ready = 0;
        dut.host_mem_d_valid = if pending_response.is_some() { 1 } else { 0 };
        dut.host_mem_d_rdata = pending_response.unwrap_or(0);

        dut.eval();

        if dut.host_mem_a_valid != 0 {
            dut.host_mem_a_ready = 1;
            pending_response = Some(if dut.host_mem_a_we == 0 {
                read_model(dut.host_mem_a_addr)
            } else {
                0
            });
        }

        dut.host_mem_d_valid = if pending_response.is_some() { 1 } else { 0 };
        dut.host_mem_d_rdata = pending_response.unwrap_or(0);
        dut.eval();

        if dut.tx_valid != 0 {
            dut.tx_ready = 1;
            dut.eval();
            out.push(dut.tx_data);
        }

        let d_handshake = pending_response.is_some() && dut.host_mem_d_ready != 0;
        clock_cycle!(dut);
        if d_handshake {
            pending_response = None;
        }

        if out.len() == expected_len {
            break;
        }
    }

    out
}

fn collect_host_read_tx(
    dut: &mut HostBusInterface,
    expected_len: usize,
    response_words: &[u32],
) -> (Vec<u32>, Vec<u8>) {
    let mut seen_addrs = Vec::new();
    let mut tx_packet = Vec::new();
    let mut pending_response: Option<u32> = None;
    let mut response_index = 0usize;

    for _ in 0..MAX_COLLECTION_CYCLES {
        dut.tx_ready = 0;
        dut.host_mem_a_ready = 0;
        dut.host_mem_d_valid = if pending_response.is_some() { 1 } else { 0 };
        dut.host_mem_d_rdata = pending_response.unwrap_or(0);

        dut.eval();

        if dut.host_mem_a_valid != 0 {
            let response_word = *response_words
                .get(response_index)
                .expect("unexpected extra host read beat");
            seen_addrs.push(dut.host_mem_a_addr);
            response_index += 1;
            dut.host_mem_a_ready = 1;
            pending_response = Some(response_word);
        }

        dut.host_mem_d_valid = if pending_response.is_some() { 1 } else { 0 };
        dut.host_mem_d_rdata = pending_response.unwrap_or(0);
        dut.eval();

        if dut.tx_valid != 0 {
            dut.tx_ready = 1;
            dut.eval();
            tx_packet.push(dut.tx_data);
        }

        let d_handshake = pending_response.is_some() && dut.host_mem_d_ready != 0;
        clock_cycle!(dut);
        if d_handshake {
            pending_response = None;
        }

        if tx_packet.len() == expected_len {
            break;
        }
    }

    (seen_addrs, tx_packet)
}

#[test]
fn test_reset_state() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    assert_eq!(dut.mem_a_ready, 1);
    assert_eq!(dut.mem_d_valid, 0);
    assert_eq!(dut.tx_valid, 0);
    assert_eq!(dut.rx_ready, 1);
    assert_eq!(dut.host_mem_a_valid, 0);
    assert_eq!(dut.host_mem_d_ready, 0);
}

#[test]
fn test_cpu_single_write_request_uses_8byte_metadata_header() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // CPU -> host single-beat write request
    assert_eq!(dut.mem_a_ready, 1);
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
    let mut pending_write_response = false;

    for _ in 0..MAX_TARGETED_TEST_CYCLES {
        dut.host_mem_a_ready = 0;
        dut.host_mem_d_valid = if pending_write_response { 1 } else { 0 };
        dut.host_mem_d_rdata = 0;
        dut.tx_ready = 0;
        dut.eval();

        if dut.host_mem_a_valid != 0 {
            seen_addrs.push(dut.host_mem_a_addr);
            seen_wdata.push(dut.host_mem_a_wdata);
            dut.host_mem_a_ready = 1;
            pending_write_response = true;
        }

        dut.host_mem_d_valid = if pending_write_response { 1 } else { 0 };
        dut.host_mem_d_rdata = 0;
        dut.eval();

        if dut.tx_valid != 0 {
            dut.tx_ready = 1;
            dut.eval();
            tx_packet.push(dut.tx_data);
        }

        let d_handshake = pending_write_response && dut.host_mem_d_ready != 0;
        clock_cycle!(dut);
        if d_handshake {
            pending_write_response = false;
        }

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

#[test]
fn test_host_read_burst_src_fixed_keeps_bus_address() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Host -> FPGA read request, burst_len=2, src_fixed=1
    send_rx_packet(&mut dut, &[0x2A, 0x00, 0x01, 0x00, 0x00, 0x20, 0x00, 0x60]);

    let (seen_addrs, tx_packet) = collect_host_read_tx(&mut dut, 16, &[0x0102_0304, 0xAABB_CCDD]);

    assert_eq!(seen_addrs, vec![0x6000_2000, 0x6000_2000]);
    assert_eq!(tx_packet.len(), 16);
    assert_eq!(
        &tx_packet[0..8],
        &[0x3A, 0x00, 0x01, 0x00, 0x00, 0x20, 0x00, 0x60]
    );
    assert_eq!(&tx_packet[8..12], &[0x04, 0x03, 0x02, 0x01]);
    assert_eq!(&tx_packet[12..16], &[0xDD, 0xCC, 0xBB, 0xAA]);
}

#[test]
fn test_host_read_burst_byte_and_halfword_stride_increment_addresses() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");

    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");
    reset_module(&mut dut);

    // Host -> FPGA read request, 3 byte-sized beats
    send_rx_packet(&mut dut, &[0x20, 0x00, 0x02, 0x00, 0x00, 0x30, 0x00, 0x60]);
    let (byte_addrs, byte_tx_packet) =
        collect_host_read_tx(&mut dut, 11, &[0x0000_0011, 0x0000_0022, 0x0000_0033]);

    assert_eq!(byte_addrs, vec![0x6000_3000, 0x6000_3001, 0x6000_3002]);
    assert_eq!(
        byte_tx_packet,
        vec![0x30, 0x00, 0x02, 0x00, 0x00, 0x30, 0x00, 0x60, 0x11, 0x22, 0x33]
    );

    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");
    reset_module(&mut dut);

    // Host -> FPGA read request, 2 halfword-sized beats
    send_rx_packet(&mut dut, &[0x24, 0x00, 0x01, 0x00, 0x40, 0x30, 0x00, 0x60]);
    let (halfword_addrs, halfword_tx_packet) =
        collect_host_read_tx(&mut dut, 12, &[0x0000_1234, 0x0000_ABCD]);

    assert_eq!(halfword_addrs, vec![0x6000_3040, 0x6000_3042]);
    assert_eq!(
        halfword_tx_packet,
        vec![0x34, 0x00, 0x01, 0x00, 0x40, 0x30, 0x00, 0x60, 0x34, 0x12, 0xCD, 0xAB,]
    );
}

#[test]
fn test_cpu_read_response_is_buffered_on_d_channel_until_ready() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    dut.mem_a_addr = 0x9000_0040;
    dut.mem_a_wdata = 0;
    dut.mem_a_we = 0;
    dut.mem_a_size = 0b10;
    dut.mem_a_valid = 1;
    clock_cycle!(dut);
    dut.mem_a_valid = 0;

    let tx_packet = collect_tx_bytes_with_bus_model(&mut dut, 8, |_| 0);
    assert_eq!(tx_packet.len(), 8, "read request should emit metadata only");

    send_rx_packet(
        &mut dut,
        &[
            0x18, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x90, 0x78, 0x56, 0x34, 0x12,
        ],
    );

    for _ in 0..MAX_RESPONSE_WAIT_CYCLES {
        dut.eval();
        if dut.mem_d_valid != 0 {
            break;
        }
        clock_cycle!(dut);
    }

    assert_eq!(
        dut.mem_d_valid, 1,
        "CPU response should appear on D channel"
    );
    assert_eq!(dut.mem_d_rdata, 0x1234_5678);
    assert_eq!(
        dut.mem_a_ready, 0,
        "single outstanding request should block new A traffic"
    );

    clock_cycle!(dut);
    dut.eval();
    assert_eq!(
        dut.mem_d_valid, 1,
        "response should remain buffered until ready"
    );
    assert_eq!(dut.mem_d_rdata, 0x1234_5678);

    dut.mem_d_ready = 1;
    clock_cycle!(dut);
    dut.mem_d_ready = 0;
    dut.eval();

    assert_eq!(
        dut.mem_d_valid, 0,
        "response should clear after D handshake"
    );
    assert_eq!(
        dut.mem_a_ready, 1,
        "A channel should reopen after response is consumed"
    );
}

#[test]
fn test_host_write_response_keeps_tx_priority_over_pending_cpu_request() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Host -> FPGA single-beat write request
    send_rx_packet(
        &mut dut,
        &[
            0x28, 0x01, 0x00, 0x00, 0x00, 0x40, 0x00, 0x60, 0x44, 0x33, 0x22, 0x11,
        ],
    );

    let mut tx_packet = Vec::new();
    let mut host_a_seen = false;
    let mut cpu_request_issued = false;
    let mut clear_cpu_request = false;

    for _ in 0..MAX_TARGETED_TEST_CYCLES {
        dut.host_mem_a_ready = 0;
        dut.host_mem_d_valid = 0;
        dut.host_mem_d_rdata = 0;
        dut.tx_ready = 0;
        if clear_cpu_request {
            dut.mem_a_valid = 0;
        }
        dut.eval();

        if dut.host_mem_a_valid != 0 && !host_a_seen {
            dut.host_mem_a_ready = 1;
            host_a_seen = true;
        }

        if host_a_seen && !cpu_request_issued && dut.host_mem_d_ready != 0 {
            dut.host_mem_d_valid = 1;
            dut.mem_a_addr = 0x1234_5678;
            dut.mem_a_wdata = 0xDEAD_BEEF;
            dut.mem_a_we = 1;
            dut.mem_a_size = 0b10;
            dut.mem_a_valid = 1;
            cpu_request_issued = true;
            clear_cpu_request = true;
        }

        dut.eval();

        if dut.tx_valid != 0 {
            dut.tx_ready = 1;
            dut.eval();
            tx_packet.push(dut.tx_data);
        }

        clock_cycle!(dut);

        if clear_cpu_request {
            dut.mem_a_valid = 0;
            clear_cpu_request = false;
        }

        if tx_packet.len() == 20 {
            break;
        }
    }

    assert_eq!(
        tx_packet,
        vec![
            0x38, 0x01, 0x00, 0x00, 0x00, 0x40, 0x00, 0x60, 0x08, 0x01, 0x00, 0x00, 0x78, 0x56,
            0x34, 0x12, 0xEF, 0xBE, 0xAD, 0xDE,
        ]
    );
}

#[test]
fn test_host_halfword_write_response_preserves_metadata() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Host -> FPGA single-beat halfword write request.
    send_rx_packet(
        &mut dut,
        &[0x24, 0x01, 0x00, 0x00, 0x04, 0x00, 0x00, 0x70, 0x34, 0x12],
    );

    let mut tx_packet = Vec::new();
    let mut pending_write_response = false;

    for _ in 0..MAX_TARGETED_TEST_CYCLES {
        dut.host_mem_a_ready = 0;
        dut.host_mem_d_valid = if pending_write_response { 1 } else { 0 };
        dut.host_mem_d_rdata = 0;
        dut.tx_ready = 0;
        dut.eval();

        if dut.host_mem_a_valid != 0 {
            assert_eq!(dut.host_mem_a_addr, 0x7000_0004);
            assert_eq!(dut.host_mem_a_we, 1);
            assert_eq!(dut.host_mem_a_size, 0b01);
            assert_eq!(dut.host_mem_a_wdata, 0x0000_1234);
            dut.host_mem_a_ready = 1;
            pending_write_response = true;
        }

        dut.host_mem_d_valid = if pending_write_response { 1 } else { 0 };
        dut.host_mem_d_rdata = 0;
        dut.eval();

        if dut.tx_valid != 0 {
            dut.tx_ready = 1;
            dut.eval();
            tx_packet.push(dut.tx_data);
        }

        let d_handshake = pending_write_response && dut.host_mem_d_ready != 0;
        clock_cycle!(dut);
        if d_handshake {
            pending_write_response = false;
        }

        if tx_packet.len() == 8 {
            break;
        }
    }

    assert_eq!(
        tx_packet,
        vec![0x34, 0x01, 0x00, 0x00, 0x04, 0x00, 0x00, 0x70]
    );
}

#[test]
fn test_stalled_tx_keeps_multi_beat_host_response_ahead_of_cpu_request() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Host -> FPGA read request, burst_len=2, incrementing source.
    send_rx_packet(&mut dut, &[0x28, 0x00, 0x01, 0x00, 0x00, 0x10, 0x00, 0x50]);

    let read_responses = [0xA1A2_A3A4, 0xB1B2_B3B4];
    let mut read_response_index = 0usize;
    let mut pending_response: Option<u32> = None;
    let mut tx_packet = Vec::new();
    let mut issue_cpu_request = false;
    let mut clear_cpu_request = false;
    let mut release_tx_stall = false;
    let mut data_handshakes = 0usize;

    for _ in 0..MAX_COLLECTION_CYCLES {
        dut.host_mem_a_ready = 0;
        dut.host_mem_d_valid = if pending_response.is_some() { 1 } else { 0 };
        dut.host_mem_d_rdata = pending_response.unwrap_or(0);
        dut.tx_ready = if release_tx_stall { 1 } else { 0 };
        if clear_cpu_request {
            dut.mem_a_valid = 0;
        }
        dut.eval();

        if dut.host_mem_a_valid != 0 {
            let response_word = *read_responses
                .get(read_response_index)
                .expect("unexpected extra host read beat");
            read_response_index += 1;
            dut.host_mem_a_ready = 1;
            pending_response = Some(response_word);
        }

        if !issue_cpu_request && read_response_index >= 1 && dut.tx_valid != 0 {
            dut.mem_a_addr = 0x1234_5678;
            dut.mem_a_wdata = 0;
            dut.mem_a_we = 0;
            dut.mem_a_size = 0b10;
            dut.mem_a_valid = 1;
            issue_cpu_request = true;
            clear_cpu_request = true;
        }

        dut.host_mem_d_valid = if pending_response.is_some() { 1 } else { 0 };
        dut.host_mem_d_rdata = pending_response.unwrap_or(0);
        dut.eval();

        if release_tx_stall && dut.tx_valid != 0 {
            dut.tx_ready = 1;
            dut.eval();
            tx_packet.push(dut.tx_data);
        }

        let d_handshake = pending_response.is_some() && dut.host_mem_d_ready != 0;
        clock_cycle!(dut);
        if d_handshake {
            pending_response = None;
            data_handshakes += 1;
            if data_handshakes == 2 {
                release_tx_stall = true;
            }
        }

        if clear_cpu_request {
            dut.mem_a_valid = 0;
            clear_cpu_request = false;
        }

        if tx_packet.len() == 24 {
            break;
        }
    }

    assert_eq!(
        tx_packet,
        vec![
            0x38, 0x00, 0x01, 0x00, 0x00, 0x10, 0x00, 0x50, 0xA4, 0xA3, 0xA2, 0xA1, 0xB4, 0xB3,
            0xB2, 0xB1, 0x08, 0x00, 0x00, 0x00, 0x78, 0x56, 0x34, 0x12,
        ]
    );
}
