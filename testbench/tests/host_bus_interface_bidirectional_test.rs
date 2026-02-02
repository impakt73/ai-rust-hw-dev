// Host Bus Interface Bidirectional Tests
// Testing of host-initiated requests through the host_bus_interface RTL module
//
// This file tests the NEW functionality added for bidirectional communication:
//   - Host-initiated read/write requests (packet type 0010)
//   - FPGA responses to host requests (packet type 0011)
//   - Error responses for invalid addresses (packet type 1111)
//   - Address validation (only RTL peripheral range 0x50000000-0x5FFFFFFF allowed)
//
// Extended Header Protocol:
//   Bits [7:4]: Packet type
//     0010 = Host-initiated request (Host → FPGA RX)
//     0011 = FPGA response to Host request (FPGA → Host TX)
//     1111 = Error response (FPGA → Host TX)
//   Bits [3:2]: size (00=byte, 01=half, 10=word)
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
    // Host bus interface signals (for testing host-initiated requests)
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
// Host-Initiated Read Request Tests
// ============================================================

#[test]
fn test_host_initiated_read_word() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // LED base address
    let target_addr: u32 = 0x50000000;

    // Send host request header: {4'b0010, size=10, 1'b0, we=0} = 0x28
    assert!(send_rx_byte(&mut dut, 0x28, 100), "Failed to send header");

    // Send address (little-endian)
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
    assert_eq!(dut.host_bus_addr, target_addr, "host_bus_addr should match");
    assert_eq!(dut.host_bus_we, 0, "host_bus_we should be 0 for read");

    // Provide bus response (simulated LED value)
    dut.host_bus_rdata = 0x000000AA;
    dut.host_bus_ready = 1;
    clock_cycle!(dut);
    dut.host_bus_ready = 0;

    // Receive response header: packet_type=0011, size=10, 0, we=0 -> 0x38
    let header = receive_tx_byte(&mut dut, 100).expect("response header");
    assert_eq!(header & 0xF0, 0x30, "Response packet type should be 0011");

    // Receive response data (4 bytes for word, little-endian)
    let b0 = receive_tx_byte(&mut dut, 100).expect("rdata[7:0]");
    let b1 = receive_tx_byte(&mut dut, 100).expect("rdata[15:8]");
    let b2 = receive_tx_byte(&mut dut, 100).expect("rdata[23:16]");
    let b3 = receive_tx_byte(&mut dut, 100).expect("rdata[31:24]");

    let rdata = (b3 as u32) << 24 | (b2 as u32) << 16 | (b1 as u32) << 8 | (b0 as u32);
    assert_eq!(rdata, 0x000000AA, "Read data should match LED value");
}

#[test]
fn test_host_initiated_read_byte() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // LED base address = 0x50000000

    // Send host request header: {4'b0010, size=00, 1'b0, we=0} = 0x20
    assert!(send_rx_byte(&mut dut, 0x20, 100), "Failed to send header");

    // Send address (little-endian)
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

    // Provide bus response
    dut.host_bus_rdata = 0x55;
    dut.host_bus_ready = 1;
    clock_cycle!(dut);
    dut.host_bus_ready = 0;

    // Receive response header
    let header = receive_tx_byte(&mut dut, 100).expect("response header");
    assert_eq!(header & 0xF0, 0x30, "Response packet type should be 0011");

    // Receive 1 byte data
    let b0 = receive_tx_byte(&mut dut, 100).expect("rdata[7:0]");
    assert_eq!(b0, 0x55, "Read data should match");
}

#[test]
fn test_host_initiated_read_halfword() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    let target_addr: u32 = 0x51000000; // Clock peripheral

    // Send host request header: {4'b0010, size=01, 1'b0, we=0} = 0x24
    assert!(send_rx_byte(&mut dut, 0x24, 100), "Failed to send header");

    // Send address (little-endian)
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x51, 100), "addr[31:24]");

    // Wait for bus request
    for _ in 0..10 {
        clock_cycle!(dut);
        if dut.host_bus_req != 0 {
            break;
        }
    }
    assert_eq!(dut.host_bus_req, 1, "host_bus_req should be asserted");
    assert_eq!(dut.host_bus_addr, target_addr, "address should match");

    // Provide bus response
    dut.host_bus_rdata = 0xCAFE;
    dut.host_bus_ready = 1;
    clock_cycle!(dut);
    dut.host_bus_ready = 0;

    // Receive response header
    let header = receive_tx_byte(&mut dut, 100).expect("response header");
    assert_eq!(header & 0xF0, 0x30, "Response packet type should be 0011");

    // Receive 2 bytes data
    let b0 = receive_tx_byte(&mut dut, 100).expect("rdata[7:0]");
    let b1 = receive_tx_byte(&mut dut, 100).expect("rdata[15:8]");
    let rdata = (b1 as u16) << 8 | (b0 as u16);
    assert_eq!(rdata, 0xCAFE, "Read data should match");
}

// ============================================================
// Host-Initiated Write Request Tests
// ============================================================

#[test]
fn test_host_initiated_write_word() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    let target_addr: u32 = 0x50000000; // LED
    let write_data: u32 = 0xDEADBEEF;

    // Send host request header: {4'b0010, size=10, 1'b0, we=1} = 0x29
    assert!(send_rx_byte(&mut dut, 0x29, 100), "Failed to send header");

    // Send address (little-endian)
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x50, 100), "addr[31:24]");

    // Send write data (little-endian)
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
    assert_eq!(dut.host_bus_addr, target_addr, "host_bus_addr should match");
    assert_eq!(dut.host_bus_we, 1, "host_bus_we should be 1 for write");
    assert_eq!(
        dut.host_bus_wdata, write_data,
        "host_bus_wdata should match"
    );

    // Provide bus response
    dut.host_bus_ready = 1;
    clock_cycle!(dut);
    dut.host_bus_ready = 0;

    // Receive response header (write ack): packet_type=0011, size=10, 0, we=1 -> 0x39
    let header = receive_tx_byte(&mut dut, 100).expect("response header");
    assert_eq!(header & 0xF0, 0x30, "Response packet type should be 0011");
    assert_eq!(header & 0x01, 0x01, "Response we bit should be 1");
}

#[test]
fn test_host_initiated_write_byte() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // LED base address = 0x50000000
    let write_data: u8 = 0xAB;

    // Send host request header: {4'b0010, size=00, 1'b0, we=1} = 0x21
    assert!(send_rx_byte(&mut dut, 0x21, 100), "Failed to send header");

    // Send address (little-endian)
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x50, 100), "addr[31:24]");

    // Send write data (1 byte)
    assert!(send_rx_byte(&mut dut, write_data, 100), "wdata[7:0]");

    // Wait for bus request
    for _ in 0..10 {
        clock_cycle!(dut);
        if dut.host_bus_req != 0 {
            break;
        }
    }
    assert_eq!(dut.host_bus_req, 1, "host_bus_req should be asserted");
    assert_eq!(dut.host_bus_we, 1, "host_bus_we should be 1 for write");
    assert_eq!(
        dut.host_bus_wdata & 0xFF,
        write_data as u32,
        "wdata byte should match"
    );

    // Provide bus response
    dut.host_bus_ready = 1;
    clock_cycle!(dut);
    dut.host_bus_ready = 0;

    // Receive response header
    let header = receive_tx_byte(&mut dut, 100).expect("response header");
    assert_eq!(header & 0xF0, 0x30, "Response packet type should be 0011");
}

// ============================================================
// Address Validation Tests
// ============================================================

#[test]
fn test_host_request_invalid_address_dram() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Send host request to DRAM address (invalid - would loop back to host)
    // DRAM_BASE = 0x80000000

    // Send request header: {4'b0010, size=10, 1'b0, we=0} = 0x28
    assert!(send_rx_byte(&mut dut, 0x28, 100), "Failed to send header");

    // Send invalid address (little-endian) - 0x80000000
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x80, 100), "addr[31:24]");

    // Module should send error response without issuing bus request
    // Wait for response
    for _ in 0..20 {
        clock_cycle!(dut);
    }

    // Receive error header (packet type 1111)
    let header = receive_tx_byte(&mut dut, 100).expect("Error response header");
    assert_eq!(header & 0xF0, 0xF0, "Should receive error packet type 1111");

    // Receive error code
    let error_code = receive_tx_byte(&mut dut, 100).expect("Error code");
    assert_eq!(
        error_code, 0xFF,
        "Should receive error code 0xFF (invalid address)"
    );

    // Verify bus was NOT accessed
    assert_eq!(
        dut.host_bus_req, 0,
        "host_bus_req should NOT be asserted for invalid address"
    );
}

#[test]
fn test_host_request_invalid_address_fifo() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Send host request to FIFO address (invalid) - 0x40000000

    // Send request header
    assert!(send_rx_byte(&mut dut, 0x28, 100), "Failed to send header");

    // Send invalid address (little-endian) - 0x40000000
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[7:0]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[15:8]");
    assert!(send_rx_byte(&mut dut, 0x00, 100), "addr[23:16]");
    assert!(send_rx_byte(&mut dut, 0x40, 100), "addr[31:24]");

    // Wait for response
    for _ in 0..20 {
        clock_cycle!(dut);
    }

    // Receive error header
    let header = receive_tx_byte(&mut dut, 100).expect("Error response header");
    assert_eq!(header & 0xF0, 0xF0, "Should receive error packet type 1111");
}

#[test]
fn test_host_request_valid_rtl_peripheral_range() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Test multiple valid RTL peripheral addresses
    let valid_addrs = [
        0x50000000u32, // LED base
        0x50000004,    // LED offset
        0x51000000,    // Clock peripheral base
        0x51000004,    // Clock offset
        0x52000000,    // UART base
        0x5F000000,    // End of RTL range
    ];

    for &addr in &valid_addrs {
        reset_module(&mut dut);

        // Send host read request
        assert!(
            send_rx_byte(&mut dut, 0x28, 100),
            "Failed to send header for {:08X}",
            addr
        );

        // Send address
        assert!(
            send_rx_byte(&mut dut, (addr & 0xFF) as u8, 100),
            "addr[7:0]"
        );
        assert!(
            send_rx_byte(&mut dut, ((addr >> 8) & 0xFF) as u8, 100),
            "addr[15:8]"
        );
        assert!(
            send_rx_byte(&mut dut, ((addr >> 16) & 0xFF) as u8, 100),
            "addr[23:16]"
        );
        assert!(
            send_rx_byte(&mut dut, ((addr >> 24) & 0xFF) as u8, 100),
            "addr[31:24]"
        );

        // Wait for bus request
        for _ in 0..10 {
            clock_cycle!(dut);
            if dut.host_bus_req != 0 {
                break;
            }
        }

        // Verify bus request was issued (valid address)
        assert_eq!(
            dut.host_bus_req, 1,
            "host_bus_req should be asserted for valid address {:08X}",
            addr
        );
        assert_eq!(
            dut.host_bus_addr, addr,
            "host_bus_addr should match {:08X}",
            addr
        );
    }
}

// ============================================================
// Return to idle Tests
// ============================================================

#[test]
fn test_host_request_returns_to_idle() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    // Complete a host read request
    assert!(send_rx_byte(&mut dut, 0x28, 100), "header");
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

    // Complete bus transaction
    dut.host_bus_rdata = 0x12345678;
    dut.host_bus_ready = 1;
    clock_cycle!(dut);
    dut.host_bus_ready = 0;

    // Drain response
    receive_tx_byte(&mut dut, 100); // header
    receive_tx_byte(&mut dut, 100); // d0
    receive_tx_byte(&mut dut, 100); // d1
    receive_tx_byte(&mut dut, 100); // d2
    receive_tx_byte(&mut dut, 100); // d3

    // Module should return to idle
    for _ in 0..10 {
        clock_cycle!(dut);
    }

    // Verify ready for new CPU transaction
    dut.addr = 0x80000000;
    dut.we = 0;
    dut.size = 0b10;
    dut.req = 1;
    clock_cycle!(dut);

    // Should see tx_valid for CPU request
    for _ in 0..10 {
        if dut.tx_valid != 0 {
            break;
        }
        clock_cycle!(dut);
    }
    assert_eq!(
        dut.tx_valid, 1,
        "Module should accept new CPU request after host request completes"
    );
}

// ============================================================
// Multiple Host Requests Tests
// ============================================================

#[test]
fn test_multiple_sequential_host_requests() {
    let runtime = create_host_bus_interface_runtime().expect("Failed to create runtime");
    let mut dut = runtime
        .create_model_simple::<HostBusInterface>()
        .expect("Failed to create model");

    reset_module(&mut dut);

    for i in 0..3 {
        let offset = i as u32 * 4;
        let addr = 0x50000000 + offset;
        let expected_data = 0x100 + i as u32;

        // Send host read request
        assert!(send_rx_byte(&mut dut, 0x28, 100), "header {}", i);
        assert!(
            send_rx_byte(&mut dut, (addr & 0xFF) as u8, 100),
            "addr[7:0]"
        );
        assert!(
            send_rx_byte(&mut dut, ((addr >> 8) & 0xFF) as u8, 100),
            "addr[15:8]"
        );
        assert!(
            send_rx_byte(&mut dut, ((addr >> 16) & 0xFF) as u8, 100),
            "addr[23:16]"
        );
        assert!(
            send_rx_byte(&mut dut, ((addr >> 24) & 0xFF) as u8, 100),
            "addr[31:24]"
        );

        // Wait for bus request
        for _ in 0..10 {
            clock_cycle!(dut);
            if dut.host_bus_req != 0 {
                break;
            }
        }
        assert_eq!(dut.host_bus_req, 1, "bus_req {}", i);
        assert_eq!(dut.host_bus_addr, addr, "addr {}", i);

        // Complete bus transaction
        dut.host_bus_rdata = expected_data;
        dut.host_bus_ready = 1;
        clock_cycle!(dut);
        dut.host_bus_ready = 0;

        // Drain response
        let header = receive_tx_byte(&mut dut, 100).expect("header");
        assert_eq!(header & 0xF0, 0x30, "response type {}", i);

        let b0 = receive_tx_byte(&mut dut, 100).expect("d0");
        let b1 = receive_tx_byte(&mut dut, 100).expect("d1");
        let b2 = receive_tx_byte(&mut dut, 100).expect("d2");
        let b3 = receive_tx_byte(&mut dut, 100).expect("d3");

        let rdata = (b3 as u32) << 24 | (b2 as u32) << 16 | (b1 as u32) << 8 | (b0 as u32);
        assert_eq!(rdata, expected_data, "rdata {}", i);
    }
}
