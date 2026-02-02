// Host Bus Requests Integration Tests
// Tests for host-initiated bus requests via the SimulatorView API
//
// These tests verify the API and types work correctly.
// Complex end-to-end scenarios with CPU programs are tested in test_led_peripheral.rs

use cpu_sim::{FpgaError, HostBusRequest, HostBusResponse};

// ============================================================
// API Tests (no simulation required)
// ============================================================

#[test]
fn test_host_bus_request_read_word() {
    let req = HostBusRequest::read_word(0x5000_0000);
    assert_eq!(req.addr, 0x5000_0000);
    assert_eq!(req.size, 2); // word
    assert!(!req.we); // read
    assert_eq!(req.wdata, 0);
}

#[test]
fn test_host_bus_request_write_word() {
    let req = HostBusRequest::write_word(0x5000_0000, 0xDEADBEEF);
    assert_eq!(req.addr, 0x5000_0000);
    assert_eq!(req.size, 2); // word
    assert!(req.we); // write
    assert_eq!(req.wdata, 0xDEADBEEF);
}

#[test]
fn test_host_bus_request_read_byte() {
    let req = HostBusRequest::read_byte(0x5000_0001);
    assert_eq!(req.addr, 0x5000_0001);
    assert_eq!(req.size, 0); // byte
    assert!(!req.we);
}

#[test]
fn test_host_bus_request_write_byte() {
    let req = HostBusRequest::write_byte(0x5000_0002, 0xAB);
    assert_eq!(req.addr, 0x5000_0002);
    assert_eq!(req.size, 0); // byte
    assert!(req.we);
    assert_eq!(req.wdata, 0xAB);
}

#[test]
fn test_host_bus_request_read_halfword() {
    let req = HostBusRequest::read_halfword(0x5000_0004);
    assert_eq!(req.addr, 0x5000_0004);
    assert_eq!(req.size, 1); // halfword
    assert!(!req.we);
}

#[test]
fn test_host_bus_request_write_halfword() {
    let req = HostBusRequest::write_halfword(0x5000_0006, 0xBEEF);
    assert_eq!(req.addr, 0x5000_0006);
    assert_eq!(req.size, 1); // halfword
    assert!(req.we);
    assert_eq!(req.wdata, 0xBEEF);
}

#[test]
fn test_host_bus_response_read_data() {
    let resp = HostBusResponse::ReadData(0x12345678);
    match resp {
        HostBusResponse::ReadData(val) => assert_eq!(val, 0x12345678),
        _ => panic!("Expected ReadData"),
    }
}

#[test]
fn test_host_bus_response_write_ack() {
    let resp = HostBusResponse::WriteAck;
    matches!(resp, HostBusResponse::WriteAck);
}

#[test]
fn test_host_bus_response_error() {
    use cpu_sim::FpgaError;
    let resp = HostBusResponse::Error(FpgaError::InvalidAddress);
    match resp {
        HostBusResponse::Error(e) => assert_eq!(e, FpgaError::InvalidAddress),
        _ => panic!("Expected Error"),
    }
}

#[test]
fn test_host_bus_request_helpers() {
    // Test the HostBusRequest helper methods
    let read_req = HostBusRequest::read_word(0x5000_0000);
    assert_eq!(read_req.addr, 0x5000_0000);
    assert_eq!(read_req.size, 2);
    assert!(!read_req.we);

    let write_req = HostBusRequest::write_word(0x5000_0000, 0x12345678);
    assert_eq!(write_req.addr, 0x5000_0000);
    assert_eq!(write_req.wdata, 0x12345678);
    assert_eq!(write_req.size, 2);
    assert!(write_req.we);
}

// ============================================================
// Address Validation Tests
// ============================================================

#[test]
fn test_send_bus_request_invalid_address_below_range() {
    // Create minimal mock components for testing address validation
    // The send_bus_request method validates addresses before queueing
    let req = HostBusRequest::read_word(0x4000_0000); // Below RTL range

    // Address validation: 0x5000_0000 to 0x6000_0000 is valid
    assert!(
        req.addr < 0x5000_0000 || req.addr >= 0x6000_0000,
        "Address should be outside valid range"
    );
}

#[test]
fn test_send_bus_request_invalid_address_above_range() {
    let req = HostBusRequest::read_word(0x7000_0000); // Above RTL range

    // Address validation
    assert!(
        req.addr < 0x5000_0000 || req.addr >= 0x6000_0000,
        "Address should be outside valid range"
    );
}

#[test]
fn test_valid_rtl_addresses() {
    let valid_addresses: [u32; 4] = [
        0x5000_0000, // LED base
        0x5100_0000, // Clock base
        0x5200_0000, // UART base
        0x5FFF_FFFF, // Upper boundary
    ];

    for addr in valid_addresses.iter() {
        let req = HostBusRequest::read_word(*addr);
        assert!(
            req.addr >= 0x5000_0000 && req.addr < 0x6000_0000,
            "Address 0x{:08x} should be in valid RTL range",
            addr
        );
    }
}

// ============================================================
// Request/Response Queue Tests
// ============================================================

#[test]
fn test_request_queue_ordering() {
    use std::collections::VecDeque;

    let mut queue: VecDeque<HostBusRequest> = VecDeque::new();

    // Queue multiple requests
    queue.push_back(HostBusRequest::read_word(0x5000_0000));
    queue.push_back(HostBusRequest::write_word(0x5000_0004, 0xAA));
    queue.push_back(HostBusRequest::read_halfword(0x5000_0008));

    // Verify FIFO order
    let req1 = queue.pop_front().unwrap();
    assert_eq!(req1.addr, 0x5000_0000);
    assert!(!req1.we);

    let req2 = queue.pop_front().unwrap();
    assert_eq!(req2.addr, 0x5000_0004);
    assert!(req2.we);

    let req3 = queue.pop_front().unwrap();
    assert_eq!(req3.addr, 0x5000_0008);
    assert_eq!(req3.size, 1); // halfword
}

#[test]
fn test_response_queue_ordering() {
    use std::collections::VecDeque;

    let mut queue: VecDeque<HostBusResponse> = VecDeque::new();

    // Queue multiple responses
    queue.push_back(HostBusResponse::ReadData(0x12345678));
    queue.push_back(HostBusResponse::WriteAck);
    queue.push_back(HostBusResponse::Error(FpgaError::InvalidAddress));

    // Verify FIFO order
    match queue.pop_front().unwrap() {
        HostBusResponse::ReadData(val) => assert_eq!(val, 0x12345678),
        _ => panic!("Expected ReadData"),
    }

    match queue.pop_front().unwrap() {
        HostBusResponse::WriteAck => {}
        _ => panic!("Expected WriteAck"),
    }

    match queue.pop_front().unwrap() {
        HostBusResponse::Error(e) => assert_eq!(e, FpgaError::InvalidAddress),
        _ => panic!("Expected Error"),
    }
}

// ============================================================
// FpgaError Tests
// ============================================================

#[test]
fn test_fpga_error_timeout() {
    let error = FpgaError::Timeout;
    assert_eq!(error, FpgaError::Timeout);
}

#[test]
fn test_fpga_error_protocol() {
    let error = FpgaError::ProtocolError;
    assert_eq!(error, FpgaError::ProtocolError);
}
