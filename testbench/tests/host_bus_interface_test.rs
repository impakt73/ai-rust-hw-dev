// Host Bus Interface Tests
// Comprehensive testing of the host_bus_interface RTL module
//
// Protocol (Little-Endian, Extended Header Format):
//   CPU-initiated request (type 0000):  [ext_header][addr0-3][data...] (FPGA → Host TX)
//   Host response to CPU (type 0001):   [ext_header][data...]          (Host → FPGA RX)
//
// Extended Header Format:
//   Bits [7:4]: Packet type
//     0000 = CPU-initiated request (FPGA → Host TX)
//     0001 = Host response to CPU request (Host → FPGA RX)
//   Bits [3:2]: size (00=byte, 01=half, 10=word, 11=reserved)
//   Bit  [1]:   Reserved (0)
//   Bit  [0]:   we (1=write, 0=read)

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
    clock_cycle!(dut);
    dut.rst_n = 1;
    clock_cycle!(dut);
}

/// Helper to receive a byte from TX interface
/// Returns (byte_value, success)
fn receive_tx_byte(dut: &mut HostBusInterface, max_cycles: u32) -> Option<u8> {
    for _ in 0..max_cycles {
        if dut.tx_valid != 0 {
            dut.tx_ready = 1;
            dut.eval();
            let byte = dut.tx_data;
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
/// Returns true if handshake completed within max_cycles, false otherwise.
fn send_rx_byte(dut: &mut HostBusInterface, byte: u8, max_cycles: u32) -> bool {
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
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Verify outputs are in expected initial state
    assert_eq!(dut.ready, 0, "ready should be LOW after reset");
    assert_eq!(dut.tx_valid, 0, "tx_valid should be LOW after reset");
    // With the new buffered RX architecture, rx_ready is HIGH when at least one buffer is free.
    // After reset, both buffers are empty, so rx_ready should be HIGH.
    assert_eq!(
        dut.rx_ready, 1,
        "rx_ready should be HIGH after reset (buffers are free)"
    );
    assert_eq!(dut.rdata, 0, "rdata should be 0 after reset");
}

#[test]
fn test_idle_no_transaction() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Run for many cycles without asserting req
    for _ in 0..100 {
        assert_eq!(dut.tx_valid, 0, "tx_valid should stay LOW without request");
        assert_eq!(dut.ready, 0, "ready should stay LOW without request");
        clock_cycle!(dut);
    }
}

// ============================================================
// Basic Write Transaction Tests
// ============================================================

#[test]
fn test_write_word_packet_format() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Start a word write transaction
    dut.addr = 0x12345678;
    dut.wdata = 0xDEADBEEF;
    dut.we = 1;
    dut.size = 0b10; // Word
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;

    // Collect TX packet (should be 9 bytes for word write)
    let mut tx_packet: Vec<u8> = Vec::new();
    for _ in 0..9 {
        if let Some(byte) = receive_tx_byte(&mut dut, 100) {
            tx_packet.push(byte);
        } else {
            panic!("Failed to receive TX byte");
        }
    }

    // Verify packet format
    assert_eq!(tx_packet.len(), 9, "Word write request should be 9 bytes");

    // Byte 0: header = {4'b0, size=10, 1'b0, we=1} = 0x09
    assert_eq!(tx_packet[0], 0x09, "Header byte mismatch");

    // Bytes 1-4: Address (little-endian: LSB first)
    assert_eq!(tx_packet[1], 0x78, "Address[7:0] mismatch");
    assert_eq!(tx_packet[2], 0x56, "Address[15:8] mismatch");
    assert_eq!(tx_packet[3], 0x34, "Address[23:16] mismatch");
    assert_eq!(tx_packet[4], 0x12, "Address[31:24] mismatch");

    // Bytes 5-8: Write data (little-endian: LSB first)
    assert_eq!(tx_packet[5], 0xEF, "WData[7:0] mismatch");
    assert_eq!(tx_packet[6], 0xBE, "WData[15:8] mismatch");
    assert_eq!(tx_packet[7], 0xAD, "WData[23:16] mismatch");
    assert_eq!(tx_packet[8], 0xDE, "WData[31:24] mismatch");
}

#[test]
fn test_write_halfword_packet_format() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Start a halfword write transaction
    dut.addr = 0x80001000;
    dut.wdata = 0x0000CAFE; // Lower 16 bits used
    dut.we = 1;
    dut.size = 0b01; // Halfword
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;

    // Collect TX packet (should be 7 bytes for halfword write)
    let mut tx_packet: Vec<u8> = Vec::new();
    for _ in 0..7 {
        if let Some(byte) = receive_tx_byte(&mut dut, 100) {
            tx_packet.push(byte);
        } else {
            panic!("Failed to receive TX byte");
        }
    }

    assert_eq!(
        tx_packet.len(),
        7,
        "Halfword write request should be 7 bytes"
    );

    // Byte 0: header = {4'b0, size=01, 1'b0, we=1} = 0x05
    assert_eq!(tx_packet[0], 0x05, "Header byte mismatch");

    // Bytes 5-6: Write data (little-endian: LSB first, 2 bytes for halfword)
    assert_eq!(tx_packet[5], 0xFE, "WData[7:0] mismatch");
    assert_eq!(tx_packet[6], 0xCA, "WData[15:8] mismatch");
}

#[test]
fn test_write_byte_packet_format() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Start a byte write transaction
    dut.addr = 0x80002000;
    dut.wdata = 0x000000AB; // Lower 8 bits used
    dut.we = 1;
    dut.size = 0b00; // Byte
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;

    // Collect TX packet (should be 6 bytes for byte write)
    let mut tx_packet: Vec<u8> = Vec::new();
    for _ in 0..6 {
        if let Some(byte) = receive_tx_byte(&mut dut, 100) {
            tx_packet.push(byte);
        } else {
            panic!("Failed to receive TX byte");
        }
    }

    assert_eq!(tx_packet.len(), 6, "Byte write request should be 6 bytes");

    // Byte 0: header = {4'b0, size=00, 1'b0, we=1} = 0x01
    assert_eq!(tx_packet[0], 0x01, "Header byte mismatch");

    // Byte 5: Write data (1 byte)
    assert_eq!(tx_packet[5], 0xAB, "WData[7:0] mismatch");
}

#[test]
fn test_write_transaction_complete() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Start a word write transaction
    dut.addr = 0x80000000;
    dut.wdata = 0x12345678;
    dut.we = 1;
    dut.size = 0b10;
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;

    // Drain TX packet (9 bytes for word write)
    for _ in 0..9 {
        receive_tx_byte(&mut dut, 100).expect("Failed to receive TX byte");
    }

    // Send response packet with extended header (packet type 0001)
    // Header: {packet_type=0001, size=10, 1'b0, we=1} = 0x19
    // With the new buffered architecture, responses go through host_rx_buffer
    assert!(
        send_rx_byte(&mut dut, 0x19, 100),
        "Failed to send write response header"
    );

    // With buffered architecture, need additional clock cycles for:
    // 1. Buffer to latch response and set resp_valid
    // 2. Main state machine to transition from CPU_RESP_WAIT to COMPLETE
    for _ in 0..5 {
        if dut.ready != 0 {
            break;
        }
        clock_cycle!(dut);
    }

    // Verify ready is asserted (should be HIGH in COMPLETE state)
    assert_eq!(dut.ready, 1, "ready should be HIGH after write response");
}

// ============================================================
// Basic Read Transaction Tests
// ============================================================

#[test]
fn test_read_word_returns_data() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Start a word read transaction
    dut.addr = 0xABCD1234;
    dut.we = 0; // Read
    dut.size = 0b10; // Word
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;

    // Drain TX packet (5 bytes for read: header + 4 addr)
    for _ in 0..5 {
        receive_tx_byte(&mut dut, 100).expect("Failed to receive TX byte");
    }

    // Send response with extended header + read data = 0xCAFEBABE
    // Header: {packet_type=0001, size=10, 1'b0, we=0} = 0x18
    assert!(
        send_rx_byte(&mut dut, 0x18, 100),
        "Failed to send read response header"
    );
    // Data (little-endian: LSB first)
    assert!(
        send_rx_byte(&mut dut, 0xBE, 100),
        "Failed to send RData[7:0]"
    );
    assert!(
        send_rx_byte(&mut dut, 0xBA, 100),
        "Failed to send RData[15:8]"
    );
    assert!(
        send_rx_byte(&mut dut, 0xFE, 100),
        "Failed to send RData[23:16]"
    );
    assert!(
        send_rx_byte(&mut dut, 0xCA, 100),
        "Failed to send RData[31:24]"
    );

    // With buffered architecture, need additional clock cycles for state machine
    for _ in 0..5 {
        if dut.ready != 0 {
            break;
        }
        clock_cycle!(dut);
    }

    // Verify read data and ready (ready should be HIGH in COMPLETE state)
    assert_eq!(dut.ready, 1, "ready should be HIGH");
    assert_eq!(dut.rdata, 0xCAFEBABE, "Read data mismatch");
}

#[test]
fn test_read_halfword_returns_data() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Start a halfword read transaction
    dut.addr = 0x80001000;
    dut.we = 0;
    dut.size = 0b01; // Halfword
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;

    // Drain TX packet (5 bytes for read)
    for _ in 0..5 {
        receive_tx_byte(&mut dut, 100).expect("Failed to receive TX byte");
    }

    // Send response with extended header + halfword data
    // Header: {packet_type=0001, size=01, 1'b0, we=0} = 0x14
    assert!(
        send_rx_byte(&mut dut, 0x14, 100),
        "Failed to send read response header"
    );
    // Data (little-endian: LSB first)
    assert!(
        send_rx_byte(&mut dut, 0xCD, 100),
        "Failed to send RData[7:0]"
    );
    assert!(
        send_rx_byte(&mut dut, 0xAB, 100),
        "Failed to send RData[15:8]"
    );

    // With buffered architecture, need additional clock cycles for state machine
    for _ in 0..5 {
        if dut.ready != 0 {
            break;
        }
        clock_cycle!(dut);
    }

    assert_eq!(dut.ready, 1, "ready should be HIGH");
    // Upper bits should be zeroed for sub-word reads
    assert_eq!(
        dut.rdata, 0x0000ABCD,
        "read halfword should be 0xABCD with upper bits zeroed"
    );
}

#[test]
fn test_read_byte_returns_data() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Start a byte read transaction
    dut.addr = 0x80002000;
    dut.we = 0;
    dut.size = 0b00; // Byte
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;

    // Drain TX packet (5 bytes for read)
    for _ in 0..5 {
        receive_tx_byte(&mut dut, 100).expect("Failed to receive TX byte");
    }

    // Send response with extended header + byte data
    // Header: {packet_type=0001, size=00, 1'b0, we=0} = 0x10
    assert!(
        send_rx_byte(&mut dut, 0x10, 100),
        "Failed to send read response header"
    );
    assert!(
        send_rx_byte(&mut dut, 0x42, 100),
        "Failed to send RData[7:0]"
    );

    // With buffered architecture, need additional clock cycles for state machine
    for _ in 0..5 {
        if dut.ready != 0 {
            break;
        }
        clock_cycle!(dut);
    }

    assert_eq!(dut.ready, 1, "ready should be HIGH");
    // Upper bits should be zeroed for sub-word reads
    assert_eq!(
        dut.rdata, 0x00000042,
        "read byte should be 0x42 with upper bits zeroed"
    );
}

#[test]
fn test_read_request_packet_format() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Start a read transaction (we=0)
    dut.addr = 0x12345678;
    dut.we = 0;
    dut.size = 0b10; // Word
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;

    // Collect all TX bytes (should be exactly 5 for read)
    let mut tx_packet: Vec<u8> = Vec::new();
    for _ in 0..5 {
        if let Some(byte) = receive_tx_byte(&mut dut, 100) {
            tx_packet.push(byte);
        } else {
            panic!("Failed to receive TX byte");
        }
    }

    assert_eq!(tx_packet.len(), 5, "Read request should be 5 bytes");

    // Header byte: {4'b0, size=10, 1'b0, we=0} = 0x08
    assert_eq!(tx_packet[0], 0x08, "Header byte for read mismatch");

    // Address bytes (little-endian: LSB first)
    assert_eq!(tx_packet[1], 0x78, "Address[7:0] mismatch");
    assert_eq!(tx_packet[2], 0x56, "Address[15:8] mismatch");
    assert_eq!(tx_packet[3], 0x34, "Address[23:16] mismatch");
    assert_eq!(tx_packet[4], 0x12, "Address[31:24] mismatch");

    // Verify tx_valid goes low after 5 bytes (no more data)
    clock_cycle!(dut);
    assert_eq!(
        dut.tx_valid, 0,
        "tx_valid should be LOW after read request complete"
    );
}

// ============================================================
// Flow Control Tests
// ============================================================

#[test]
fn test_tx_backpressure() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Start transaction
    dut.addr = 0x11111111;
    dut.we = 1;
    dut.size = 0b10;
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;

    // Wait until tx_valid is asserted
    for _ in 0..10 {
        if dut.tx_valid != 0 {
            break;
        }
        clock_cycle!(dut);
    }
    assert_eq!(dut.tx_valid, 1, "tx_valid should be asserted");

    // Keep tx_ready LOW for several cycles (backpressure)
    let first_byte = dut.tx_data;
    dut.tx_ready = 0;
    for _ in 0..10 {
        clock_cycle!(dut);
        // tx_valid should remain asserted
        assert_eq!(
            dut.tx_valid, 1,
            "tx_valid should stay HIGH during backpressure"
        );
        // tx_data should not change
        assert_eq!(
            dut.tx_data, first_byte,
            "tx_data should not change during backpressure"
        );
    }

    // Now accept the byte
    dut.tx_ready = 1;
    dut.eval();
    clock_cycle!(dut);

    // Verify tx_data has advanced to the next byte
    let second_byte = dut.tx_data;
    assert_eq!(
        dut.tx_valid, 1,
        "tx_valid should remain HIGH after accepting the first byte"
    );
    assert_ne!(
        second_byte, first_byte,
        "tx_data should advance to the next byte after backpressure is released"
    );
}

#[test]
fn test_rx_delayed_valid() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Start word write transaction and drain TX (9 bytes)
    dut.addr = 0x00000000;
    dut.we = 1;
    dut.size = 0b10;
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;

    for _ in 0..9 {
        receive_tx_byte(&mut dut, 100).expect("TX byte");
    }

    // Module should now be waiting for RX
    // rx_ready should be asserted
    for _ in 0..5 {
        clock_cycle!(dut);
    }
    assert_eq!(
        dut.rx_ready, 1,
        "rx_ready should be HIGH waiting for response"
    );

    // Delay sending response for many cycles
    for _ in 0..50 {
        clock_cycle!(dut);
        assert_eq!(dut.ready, 0, "ready should stay LOW waiting for response");
    }

    // Now send response with extended header
    // Header for word write response: {packet_type=0001, size=10, 1'b0, we=1} = 0x19
    send_rx_byte(&mut dut, 0x19, 100);

    // With buffered architecture, need additional clock cycles for state machine
    for _ in 0..5 {
        if dut.ready != 0 {
            break;
        }
        clock_cycle!(dut);
    }

    assert_eq!(dut.ready, 1, "ready should be HIGH after delayed response");
}

#[test]
fn test_rx_ready_buffered_architecture() {
    // With the new buffered RX architecture, rx_ready is driven by the host_rx_buffer module.
    // It is HIGH when at least one buffer is free (response or request buffer).
    // It is LOW only when BOTH buffers are full.
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // rx_ready should be HIGH in IDLE (both buffers are empty after reset)
    assert_eq!(
        dut.rx_ready, 1,
        "rx_ready should be HIGH in IDLE (buffers free)"
    );

    // Start read transaction
    dut.addr = 0x80000000;
    dut.we = 0;
    dut.size = 0b10;
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;

    // During TX phase, rx_ready should still be HIGH because the buffer is free
    for _ in 0..3 {
        assert_eq!(
            dut.rx_ready, 1,
            "rx_ready should be HIGH during TX phase (buffer free)"
        );
        receive_tx_byte(&mut dut, 100);
    }

    // Finish TX (5 bytes total for read)
    for _ in 0..2 {
        receive_tx_byte(&mut dut, 100);
    }

    // After TX phase, rx_ready should still be HIGH (buffer still free)
    clock_cycle!(dut);
    assert_eq!(dut.rx_ready, 1, "rx_ready should be HIGH (buffer free)");
}

// ============================================================
// Size Variation Tests
// ============================================================

#[test]
fn test_byte_access_size() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Byte access (size = 00)
    dut.addr = 0x00000000;
    dut.we = 0;
    dut.size = 0b00; // Byte
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;

    let cmd = receive_tx_byte(&mut dut, 100).expect("Command byte");
    // Command: {4'b0, size=00, 1'b0, we=0} = 0x00
    assert_eq!(cmd, 0x00, "Byte access command byte mismatch");
}

#[test]
fn test_halfword_access_size() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Halfword access (size = 01)
    dut.addr = 0x00000000;
    dut.we = 1;
    dut.size = 0b01; // Halfword
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;

    let cmd = receive_tx_byte(&mut dut, 100).expect("Command byte");
    // Command: {4'b0, size=01, 1'b0, we=1} = 0x05
    assert_eq!(cmd, 0x05, "Halfword write command byte mismatch");
}

// ============================================================
// Additional Test Cases
// ============================================================

#[test]
fn test_write_blocking() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Start a word write transaction
    dut.addr = 0x80000000;
    dut.wdata = 0x12345678;
    dut.we = 1;
    dut.size = 0b10;
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;

    // Verify ready stays LOW during TX phase (9 bytes for word write)
    for _ in 0..9 {
        assert_eq!(dut.ready, 0, "ready should be LOW during TX");
        receive_tx_byte(&mut dut, 100).expect("TX byte");
    }

    // Verify ready stays LOW during RX wait phase
    for _ in 0..10 {
        clock_cycle!(dut);
        assert_eq!(dut.ready, 0, "ready should be LOW waiting for response");
    }
}

#[test]
fn test_consecutive_transactions() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Perform two back-to-back transactions
    for iteration in 0u32..2 {
        let test_addr = 0x80000000_u32.wrapping_add(iteration * 4);
        let test_data = 0xDEAD0000_u32.wrapping_add(iteration);

        // Start word write transaction
        dut.addr = test_addr;
        dut.wdata = test_data;
        dut.we = 1;
        dut.size = 0b10;
        dut.req = 1;
        clock_cycle!(dut);
        dut.req = 0;

        // Drain TX (9 bytes for word write)
        for _ in 0..9 {
            receive_tx_byte(&mut dut, 100).expect("TX byte");
        }

        // Send response with extended header
        // Header for word write response: {packet_type=0001, size=10, 1'b0, we=1} = 0x19
        send_rx_byte(&mut dut, 0x19, 100);

        // With buffered architecture, need additional clock cycles for state machine
        for _ in 0..5 {
            if dut.ready != 0 {
                break;
            }
            clock_cycle!(dut);
        }

        // Verify completion (ready should be HIGH in COMPLETE state)
        assert_eq!(dut.ready, 1, "Transaction {} should complete", iteration);

        // Wait a cycle for state to return to IDLE before next transaction
        clock_cycle!(dut);
    }
}

#[test]
fn test_all_ones_address() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Test with all-ones address (word write = 9 bytes)
    dut.addr = 0xFFFFFFFF;
    dut.wdata = 0xFFFFFFFF;
    dut.we = 1;
    dut.size = 0b10;
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;

    // Collect TX packet (9 bytes for word write)
    let mut tx_packet: Vec<u8> = Vec::new();
    for _ in 0..9 {
        if let Some(byte) = receive_tx_byte(&mut dut, 100) {
            tx_packet.push(byte);
        }
    }

    assert_eq!(tx_packet.len(), 9, "Word write should be 9 bytes");

    // Verify address bytes are all 0xFF (little-endian)
    assert_eq!(tx_packet[1], 0xFF, "Address[7:0] should be 0xFF");
    assert_eq!(tx_packet[2], 0xFF, "Address[15:8] should be 0xFF");
    assert_eq!(tx_packet[3], 0xFF, "Address[23:16] should be 0xFF");
    assert_eq!(tx_packet[4], 0xFF, "Address[31:24] should be 0xFF");

    // Verify write data bytes are all 0xFF (little-endian)
    assert_eq!(tx_packet[5], 0xFF, "WData[7:0] should be 0xFF");
    assert_eq!(tx_packet[6], 0xFF, "WData[15:8] should be 0xFF");
    assert_eq!(tx_packet[7], 0xFF, "WData[23:16] should be 0xFF");
    assert_eq!(tx_packet[8], 0xFF, "WData[31:24] should be 0xFF");
}

// ============================================================
// Host-Initiated Request Tests
// These tests verify that the host_bus_interface correctly processes
// host-initiated requests (type 0010) and sends back FPGA responses (type 0011)
// ============================================================

/// Helper to send a host-initiated read request and receive the response
fn send_host_read_request(dut: &mut HostBusInterface, addr: u32, size: u8) -> Option<Vec<u8>> {
    // Send request header: type=0010, size, we=0
    // Header: 0010 ss 0 0 where ss is size
    let header = 0x20 | ((size & 0x03) << 2);
    if !send_rx_byte(dut, header, 100) {
        return None;
    }

    // Send 4 address bytes (little-endian)
    if !send_rx_byte(dut, (addr & 0xFF) as u8, 100) {
        return None;
    }
    if !send_rx_byte(dut, ((addr >> 8) & 0xFF) as u8, 100) {
        return None;
    }
    if !send_rx_byte(dut, ((addr >> 16) & 0xFF) as u8, 100) {
        return None;
    }
    if !send_rx_byte(dut, ((addr >> 24) & 0xFF) as u8, 100) {
        return None;
    }

    // Wait for response by running clock cycles until host_bus_req is asserted
    // Then provide a response
    for _ in 0..100 {
        if dut.host_bus_req != 0 {
            // Request is pending - provide a ready response
            dut.host_bus_ready = 1;
            dut.host_bus_rdata = 0xCAFEBABE; // Test read data
            clock_cycle!(dut);
            dut.host_bus_ready = 0;
            break;
        }
        clock_cycle!(dut);
    }

    // Wait for and collect TX response packet
    let mut response = Vec::new();
    let bytes_needed = match size {
        0 => 2, // Header + 1 data byte
        1 => 3, // Header + 2 data bytes
        _ => 5, // Header + 4 data bytes
    };

    for _ in 0..bytes_needed {
        if let Some(byte) = receive_tx_byte(dut, 100) {
            response.push(byte);
        } else {
            return None;
        }
    }

    Some(response)
}

/// Helper to send a host-initiated write request and receive the acknowledgement
fn send_host_write_request(
    dut: &mut HostBusInterface,
    addr: u32,
    wdata: u32,
    size: u8,
) -> Option<u8> {
    // Send request header: type=0010, size, we=1
    // Header: 0010 ss 0 1 where ss is size
    let header = 0x21 | ((size & 0x03) << 2);
    if !send_rx_byte(dut, header, 100) {
        return None;
    }

    // Send 4 address bytes (little-endian)
    if !send_rx_byte(dut, (addr & 0xFF) as u8, 100) {
        return None;
    }
    if !send_rx_byte(dut, ((addr >> 8) & 0xFF) as u8, 100) {
        return None;
    }
    if !send_rx_byte(dut, ((addr >> 16) & 0xFF) as u8, 100) {
        return None;
    }
    if !send_rx_byte(dut, ((addr >> 24) & 0xFF) as u8, 100) {
        return None;
    }

    // Send data bytes based on size (little-endian)
    let data_bytes = match size {
        0 => 1,
        1 => 2,
        _ => 4,
    };
    for i in 0..data_bytes {
        let byte = ((wdata >> (i * 8)) & 0xFF) as u8;
        if !send_rx_byte(dut, byte, 100) {
            return None;
        }
    }

    // Wait for response by running clock cycles until host_bus_req is asserted
    for _ in 0..100 {
        if dut.host_bus_req != 0 {
            // Request is pending - provide a ready response
            dut.host_bus_ready = 1;
            clock_cycle!(dut);
            dut.host_bus_ready = 0;
            break;
        }
        clock_cycle!(dut);
    }

    // Receive TX response header (just 1 byte for write ack)
    receive_tx_byte(dut, 100)
}

#[test]
fn test_host_initiated_read_request() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Send host-initiated word read request to address 0x50000000
    let response = send_host_read_request(&mut dut, 0x50000000, 0b10);

    assert!(response.is_some(), "Should receive response packet");
    let resp = response.unwrap();

    // Verify response header: type=0011, size=10, we=0 = 0x38
    assert_eq!(
        resp[0], 0x38,
        "Response header should be 0x38 (type=0011, size=10, we=0)"
    );

    // Verify data bytes (little-endian: 0xCAFEBABE)
    assert_eq!(resp[1], 0xBE, "RData[7:0] mismatch");
    assert_eq!(resp[2], 0xBA, "RData[15:8] mismatch");
    assert_eq!(resp[3], 0xFE, "RData[23:16] mismatch");
    assert_eq!(resp[4], 0xCA, "RData[31:24] mismatch");
}

#[test]
fn test_host_initiated_write_request() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Send host-initiated word write request to address 0x50000000 with data 0xDEADBEEF
    let response = send_host_write_request(&mut dut, 0x50000000, 0xDEADBEEF, 0b10);

    assert!(response.is_some(), "Should receive response header");

    // Verify response header: type=0011, size=10, we=1 = 0x39
    assert_eq!(
        response.unwrap(),
        0x39,
        "Response header should be 0x39 (type=0011, size=10, we=1)"
    );
}

#[test]
fn test_host_request_master_interface_signals() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Master interface should be idle initially
    assert_eq!(dut.host_bus_req, 0, "host_bus_req should be LOW initially");

    // Send host-initiated read request header and address
    let addr: u32 = 0x50001234;
    let size: u8 = 0b10; // word

    // Header: type=0010, size=10, we=0 = 0x28
    assert!(send_rx_byte(&mut dut, 0x28, 100), "header");
    assert!(send_rx_byte(&mut dut, 0x34, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x12, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x50, 100), "addr[31:24]");

    // Wait for master interface request to be asserted
    for _ in 0..100 {
        if dut.host_bus_req != 0 {
            break;
        }
        clock_cycle!(dut);
    }

    // Verify master interface signals
    assert_eq!(dut.host_bus_req, 1, "host_bus_req should be HIGH");
    assert_eq!(
        dut.host_bus_addr, addr,
        "host_bus_addr should match request address"
    );
    assert_eq!(dut.host_bus_we, 0, "host_bus_we should be 0 for read");
    assert_eq!(dut.host_bus_size, size, "host_bus_size should be word (10)");
}

#[test]
fn test_host_write_request_wdata_propagation() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Send host-initiated word write request
    let addr: u32 = 0x50000000;
    let wdata: u32 = 0xDEADBEEF;

    // Header: type=0010, size=10, we=1 = 0x29
    assert!(send_rx_byte(&mut dut, 0x29, 100), "header");

    // Address bytes
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x50, 100), "addr[31:24]");

    // Write data bytes (little-endian)
    assert!(send_rx_byte(&mut dut, 0xEF, 100), "wdata[7:0]");
    assert!(send_rx_byte(&mut dut, 0xBE, 100), "wdata[15:8]");
    assert!(send_rx_byte(&mut dut, 0xAD, 100), "wdata[23:16]");
    assert!(send_rx_byte(&mut dut, 0xDE, 100), "wdata[31:24]");

    // Wait for master interface request
    for _ in 0..100 {
        if dut.host_bus_req != 0 {
            break;
        }
        clock_cycle!(dut);
    }

    // Verify master interface signals
    assert_eq!(dut.host_bus_req, 1, "host_bus_req should be HIGH");
    assert_eq!(dut.host_bus_addr, addr, "host_bus_addr mismatch");
    assert_eq!(dut.host_bus_wdata, wdata, "host_bus_wdata mismatch");
    assert_eq!(dut.host_bus_we, 1, "host_bus_we should be 1 for write");
}

#[test]
fn test_cpu_request_priority_over_host_request() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Start CPU-initiated request
    dut.addr = 0x80000000;
    dut.wdata = 0x12345678;
    dut.we = 1;
    dut.size = 0b10;
    dut.req = 1;
    clock_cycle!(dut);
    dut.req = 0;

    // Simultaneously try to send host-initiated request header
    // It should be buffered but not processed until CPU request completes
    let header = 0x28; // type=0010, size=10, we=0
    send_rx_byte(&mut dut, header, 10);

    // CPU request should continue being transmitted (TX phase)
    let mut tx_bytes_received = 0;
    for _ in 0..20 {
        if receive_tx_byte(&mut dut, 5).is_some() {
            tx_bytes_received += 1;
        }
    }

    // Should have received at least some TX bytes for CPU request
    assert!(
        tx_bytes_received > 0,
        "CPU request TX should have priority over buffered host request"
    );
}
