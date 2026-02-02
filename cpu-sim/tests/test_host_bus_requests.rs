// Host Bus Requests Integration Tests
// Tests for host-initiated bus requests via the SimulatorView API
//
// These tests verify the API and types work correctly.
// Complex end-to-end scenarios with CPU programs are tested in test_led_peripheral.rs

use cpu_sim::{HostBusRequest, HostBusResponse};

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
