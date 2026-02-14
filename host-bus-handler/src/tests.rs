//! Unit tests for HostBusHandler

use super::*;

// ============================================================
// Basic Construction and Reset Tests
// ============================================================

#[test]
fn test_new_handler() {
    let handler = HostBusHandler::new();
    assert!(handler.can_accept_rx());
    assert!(!handler.has_tx_data());
    assert!(!handler.has_pending_outgoing_request());
    assert!(!handler.has_incoming_request());
}

#[test]
fn test_default_handler() {
    let handler = HostBusHandler::default();
    assert!(handler.can_accept_rx());
    assert!(!handler.has_tx_data());
}

#[test]
fn test_reset_handler() {
    let mut handler = HostBusHandler::new();

    // Put handler in some state
    let request = BusRequest::write(0x50000000, 0xAB, AccessSize::Byte);
    handler.send_request(request).unwrap();

    // Reset
    handler.reset();

    // Should be back to initial state
    assert!(handler.can_accept_rx());
    assert!(!handler.has_tx_data());
    assert!(!handler.has_pending_outgoing_request());
}

// ============================================================
// AccessSize Tests
// ============================================================

#[test]
fn test_access_size_byte_count() {
    assert_eq!(AccessSize::Byte.byte_count(), 1);
    assert_eq!(AccessSize::Halfword.byte_count(), 2);
    assert_eq!(AccessSize::Word.byte_count(), 4);
}

#[test]
fn test_access_size_from_u8() {
    assert_eq!(AccessSize::from_u8(0), Some(AccessSize::Byte));
    assert_eq!(AccessSize::from_u8(1), Some(AccessSize::Halfword));
    assert_eq!(AccessSize::from_u8(2), Some(AccessSize::Word));
    assert_eq!(AccessSize::from_u8(3), None);
    assert_eq!(AccessSize::from_u8(255), None);
}

// ============================================================
// BusRequest Construction Tests
// ============================================================

#[test]
fn test_bus_request_read() {
    let req = BusRequest::read(0x50000000, AccessSize::Word);
    assert_eq!(req.addr, 0x50000000);
    assert_eq!(req.wdata, 0);
    assert!(!req.we);
    assert_eq!(req.size, AccessSize::Word);
}

#[test]
fn test_bus_request_write() {
    let req = BusRequest::write(0x50000000, 0xDEADBEEF, AccessSize::Word);
    assert_eq!(req.addr, 0x50000000);
    assert_eq!(req.wdata, 0xDEADBEEF);
    assert!(req.we);
    assert_eq!(req.size, AccessSize::Word);
}

// ============================================================
// BusResponse Construction Tests
// ============================================================

#[test]
fn test_bus_response_write_ack() {
    let resp = BusResponse::write_ack(AccessSize::Byte);
    assert_eq!(resp.rdata, 0);
    assert_eq!(resp.size, AccessSize::Byte);
    assert!(resp.we);
}

#[test]
fn test_bus_response_read_data() {
    let resp = BusResponse::read_data(0x12345678, AccessSize::Word);
    assert_eq!(resp.rdata, 0x12345678);
    assert_eq!(resp.size, AccessSize::Word);
    assert!(!resp.we);
}

// ============================================================
// Outgoing Request TX Tests (Host → FPGA)
// ============================================================

#[test]
fn test_send_request_byte_write() {
    let mut handler = HostBusHandler::new();

    // Send a byte write request
    let request = BusRequest::write(0x50000000, 0xAB, AccessSize::Byte);
    handler.send_request(request).unwrap();

    assert!(handler.has_pending_outgoing_request());
    assert!(handler.has_tx_data());

    // Collect TX bytes
    let mut tx_bytes = Vec::new();
    while let Some(byte) = handler.transfer_tx_byte() {
        tx_bytes.push(byte);
    }

    // Expected format: [header][addr0][addr1][addr2][addr3][data0]
    // Header: packet_type=0010, size=00 (byte), reserved=0, we=1 → 0x21
    assert_eq!(tx_bytes.len(), 6);
    assert_eq!(tx_bytes[0], 0x21); // header
    assert_eq!(tx_bytes[1], 0x00); // addr[7:0]
    assert_eq!(tx_bytes[2], 0x00); // addr[15:8]
    assert_eq!(tx_bytes[3], 0x00); // addr[23:16]
    assert_eq!(tx_bytes[4], 0x50); // addr[31:24]
    assert_eq!(tx_bytes[5], 0xAB); // data
}

#[test]
fn test_send_request_halfword_write() {
    let mut handler = HostBusHandler::new();

    let request = BusRequest::write(0x50000008, 0xBEEF, AccessSize::Halfword);
    handler.send_request(request).unwrap();

    let mut tx_bytes = Vec::new();
    while let Some(byte) = handler.transfer_tx_byte() {
        tx_bytes.push(byte);
    }

    // Header: packet_type=0010, size=01 (half), reserved=0, we=1 → 0x25
    assert_eq!(tx_bytes.len(), 7);
    assert_eq!(tx_bytes[0], 0x25); // header
    assert_eq!(tx_bytes[1], 0x08); // addr[7:0]
    assert_eq!(tx_bytes[2], 0x00); // addr[15:8]
    assert_eq!(tx_bytes[3], 0x00); // addr[23:16]
    assert_eq!(tx_bytes[4], 0x50); // addr[31:24]
    assert_eq!(tx_bytes[5], 0xEF); // data[7:0]
    assert_eq!(tx_bytes[6], 0xBE); // data[15:8]
}

#[test]
fn test_send_request_word_write() {
    let mut handler = HostBusHandler::new();

    let request = BusRequest::write(0x5000000C, 0xDEADBEEF, AccessSize::Word);
    handler.send_request(request).unwrap();

    let mut tx_bytes = Vec::new();
    while let Some(byte) = handler.transfer_tx_byte() {
        tx_bytes.push(byte);
    }

    // Header: packet_type=0010, size=10 (word), reserved=0, we=1 → 0x29
    assert_eq!(tx_bytes.len(), 9);
    assert_eq!(tx_bytes[0], 0x29); // header
    assert_eq!(tx_bytes[1], 0x0C); // addr[7:0]
    assert_eq!(tx_bytes[2], 0x00); // addr[15:8]
    assert_eq!(tx_bytes[3], 0x00); // addr[23:16]
    assert_eq!(tx_bytes[4], 0x50); // addr[31:24]
    assert_eq!(tx_bytes[5], 0xEF); // data[7:0]
    assert_eq!(tx_bytes[6], 0xBE); // data[15:8]
    assert_eq!(tx_bytes[7], 0xAD); // data[23:16]
    assert_eq!(tx_bytes[8], 0xDE); // data[31:24]
}

#[test]
fn test_send_request_byte_read() {
    let mut handler = HostBusHandler::new();

    let request = BusRequest::read(0x50000000, AccessSize::Byte);
    handler.send_request(request).unwrap();

    let mut tx_bytes = Vec::new();
    while let Some(byte) = handler.transfer_tx_byte() {
        tx_bytes.push(byte);
    }

    // Header: packet_type=0010, size=00 (byte), reserved=0, we=0 → 0x20
    // Read requests don't have data bytes
    assert_eq!(tx_bytes.len(), 5);
    assert_eq!(tx_bytes[0], 0x20); // header
    assert_eq!(tx_bytes[1], 0x00); // addr[7:0]
    assert_eq!(tx_bytes[2], 0x00); // addr[15:8]
    assert_eq!(tx_bytes[3], 0x00); // addr[23:16]
    assert_eq!(tx_bytes[4], 0x50); // addr[31:24]
}

#[test]
fn test_send_request_word_read() {
    let mut handler = HostBusHandler::new();

    let request = BusRequest::read(0x51000004, AccessSize::Word);
    handler.send_request(request).unwrap();

    let mut tx_bytes = Vec::new();
    while let Some(byte) = handler.transfer_tx_byte() {
        tx_bytes.push(byte);
    }

    // Header: packet_type=0010, size=10 (word), reserved=0, we=0 → 0x28
    assert_eq!(tx_bytes.len(), 5);
    assert_eq!(tx_bytes[0], 0x28); // header
    assert_eq!(tx_bytes[1], 0x04); // addr[7:0]
    assert_eq!(tx_bytes[2], 0x00); // addr[15:8]
    assert_eq!(tx_bytes[3], 0x00); // addr[23:16]
    assert_eq!(tx_bytes[4], 0x51); // addr[31:24]
}

#[test]
fn test_send_request_rejects_when_pending() {
    let mut handler = HostBusHandler::new();

    let request1 = BusRequest::read(0x50000000, AccessSize::Byte);
    handler.send_request(request1).unwrap();

    // Try to send another request - should fail
    let request2 = BusRequest::read(0x50000004, AccessSize::Byte);
    let result = handler.send_request(request2);
    assert_eq!(result, Err(HandlerError::RequestPending));
}

#[test]
fn test_send_request_rejects_non_rtl_address() {
    let mut handler = HostBusHandler::new();

    let request = BusRequest::read(0x8000_0000, AccessSize::Word);
    assert_eq!(
        handler.send_request(request),
        Err(HandlerError::InvalidAddressRange)
    );
}

#[test]
fn test_send_request_rejects_spanning_rtl_boundary() {
    let mut handler = HostBusHandler::new();

    let request = BusRequest::read(0x4FFF_FFFF, AccessSize::Word);
    assert_eq!(
        handler.send_request(request),
        Err(HandlerError::InvalidAddressRange)
    );
}

#[test]
fn test_send_request_rejects_spanning_rtl_upper_boundary() {
    let mut handler = HostBusHandler::new();

    let request = BusRequest::read(0x5FFF_FFFF, AccessSize::Word);
    assert_eq!(
        handler.send_request(request),
        Err(HandlerError::InvalidAddressRange)
    );
}

// ============================================================
// Outgoing Response RX Tests (FPGA → Host for our requests)
// ============================================================

#[test]
fn test_receive_write_response() {
    let mut handler = HostBusHandler::new();

    // Send a write request
    let request = BusRequest::write(0x50000000, 0xAB, AccessSize::Byte);
    handler.send_request(request).unwrap();

    // Drain TX bytes
    while handler.transfer_tx_byte().is_some() {}

    // No response yet
    assert!(handler.receive_response().is_none());

    // Receive write acknowledgment (packet type 0011)
    // Header: packet_type=0011, size=00 (byte), reserved=0, we=1 → 0x31
    handler.transfer_rx_byte(0x31).unwrap();

    // Now we should have a response
    let response = handler.receive_response().unwrap();
    assert!(response.we);
    assert_eq!(response.size, AccessSize::Byte);
}

#[test]
fn test_receive_byte_read_response() {
    let mut handler = HostBusHandler::new();

    // Send a read request
    let request = BusRequest::read(0x50000000, AccessSize::Byte);
    handler.send_request(request).unwrap();

    // Drain TX bytes
    while handler.transfer_tx_byte().is_some() {}

    // Receive read response (packet type 0011)
    // Header: packet_type=0011, size=00 (byte), reserved=0, we=0 → 0x30
    handler.transfer_rx_byte(0x30).unwrap();
    // Data byte
    handler.transfer_rx_byte(0xAB).unwrap();

    let response = handler.receive_response().unwrap();
    assert!(!response.we);
    assert_eq!(response.size, AccessSize::Byte);
    assert_eq!(response.rdata, 0xAB);
}

#[test]
fn test_receive_word_read_response() {
    let mut handler = HostBusHandler::new();

    // Send a word read request
    let request = BusRequest::read(0x50000000, AccessSize::Word);
    handler.send_request(request).unwrap();

    // Drain TX bytes
    while handler.transfer_tx_byte().is_some() {}

    // Receive word read response (packet type 0011)
    // Header: packet_type=0011, size=10 (word), reserved=0, we=0 → 0x38
    handler.transfer_rx_byte(0x38).unwrap();
    // Data bytes (little-endian: 0xDEADBEEF)
    handler.transfer_rx_byte(0xEF).unwrap();
    handler.transfer_rx_byte(0xBE).unwrap();
    handler.transfer_rx_byte(0xAD).unwrap();
    handler.transfer_rx_byte(0xDE).unwrap();

    let response = handler.receive_response().unwrap();
    assert!(!response.we);
    assert_eq!(response.size, AccessSize::Word);
    assert_eq!(response.rdata, 0xDEADBEEF);
}

// ============================================================
// Incoming Request RX Tests (FPGA → Host, CPU-initiated)
// ============================================================

#[test]
fn test_accept_byte_write_request() {
    let mut handler = HostBusHandler::new();

    // Receive a byte write request (packet type 0000)
    // Header: packet_type=0000, size=00 (byte), reserved=0, we=1 → 0x01
    handler.transfer_rx_byte(0x01).unwrap();
    // Address (little-endian: 0x50000000)
    handler.transfer_rx_byte(0x00).unwrap();
    handler.transfer_rx_byte(0x00).unwrap();
    handler.transfer_rx_byte(0x00).unwrap();
    handler.transfer_rx_byte(0x50).unwrap();
    // Data
    handler.transfer_rx_byte(0xCD).unwrap();

    // Request should be available
    assert!(handler.has_incoming_request());

    let request = handler.accept_request().unwrap();
    assert_eq!(request.addr, 0x50000000);
    assert_eq!(request.wdata, 0xCD);
    assert!(request.we);
    assert_eq!(request.size, AccessSize::Byte);
}

#[test]
fn test_accept_word_read_request() {
    let mut handler = HostBusHandler::new();

    // Receive a word read request (packet type 0000)
    // Header: packet_type=0000, size=10 (word), reserved=0, we=0 → 0x08
    handler.transfer_rx_byte(0x08).unwrap();
    // Address (little-endian: 0x80000100)
    handler.transfer_rx_byte(0x00).unwrap();
    handler.transfer_rx_byte(0x01).unwrap();
    handler.transfer_rx_byte(0x00).unwrap();
    handler.transfer_rx_byte(0x80).unwrap();

    let request = handler.accept_request().unwrap();
    assert_eq!(request.addr, 0x80000100);
    assert_eq!(request.wdata, 0);
    assert!(!request.we);
    assert_eq!(request.size, AccessSize::Word);
}

#[test]
fn test_accept_word_write_request() {
    let mut handler = HostBusHandler::new();

    // Receive a word write request
    // Header: packet_type=0000, size=10 (word), reserved=0, we=1 → 0x09
    handler.transfer_rx_byte(0x09).unwrap();
    // Address (little-endian: 0x80000200)
    handler.transfer_rx_byte(0x00).unwrap();
    handler.transfer_rx_byte(0x02).unwrap();
    handler.transfer_rx_byte(0x00).unwrap();
    handler.transfer_rx_byte(0x80).unwrap();
    // Data (little-endian: 0x12345678)
    handler.transfer_rx_byte(0x78).unwrap();
    handler.transfer_rx_byte(0x56).unwrap();
    handler.transfer_rx_byte(0x34).unwrap();
    handler.transfer_rx_byte(0x12).unwrap();

    let request = handler.accept_request().unwrap();
    assert_eq!(request.addr, 0x80000200);
    assert_eq!(request.wdata, 0x12345678);
    assert!(request.we);
    assert_eq!(request.size, AccessSize::Word);
}

#[test]
fn test_accept_request_fails_when_none() {
    let handler = HostBusHandler::new();
    // Cannot accept yet - no request buffered
    assert!(!handler.has_incoming_request());
}

// ============================================================
// Complete Request and Response TX Tests (Host → FPGA response)
// ============================================================

#[test]
fn test_complete_write_request() {
    let mut handler = HostBusHandler::new();

    // Receive a byte write request
    handler.transfer_rx_byte(0x01).unwrap(); // header
    handler.transfer_rx_byte(0x00).unwrap(); // addr
    handler.transfer_rx_byte(0x00).unwrap();
    handler.transfer_rx_byte(0x00).unwrap();
    handler.transfer_rx_byte(0x50).unwrap();
    handler.transfer_rx_byte(0xAB).unwrap(); // data

    // Accept it
    let _request = handler.accept_request().unwrap();
    assert!(handler.is_waiting_for_completion());

    // Complete with write ack
    let response = BusResponse::write_ack(AccessSize::Byte);
    handler.complete_request(response).unwrap();
    assert!(!handler.is_waiting_for_completion());

    // Collect TX bytes
    let mut tx_bytes = Vec::new();
    while let Some(byte) = handler.transfer_tx_byte() {
        tx_bytes.push(byte);
    }

    // Response: header only for write ack
    // Header: packet_type=0001, size=00 (byte), reserved=0, we=1 → 0x11
    assert_eq!(tx_bytes.len(), 1);
    assert_eq!(tx_bytes[0], 0x11);
}

#[test]
fn test_complete_read_request() {
    let mut handler = HostBusHandler::new();

    // Receive a word read request
    handler.transfer_rx_byte(0x08).unwrap(); // header (word read)
    handler.transfer_rx_byte(0x00).unwrap(); // addr
    handler.transfer_rx_byte(0x00).unwrap();
    handler.transfer_rx_byte(0x00).unwrap();
    handler.transfer_rx_byte(0x50).unwrap();

    // Accept it
    let _request = handler.accept_request().unwrap();

    // Complete with read data
    let response = BusResponse::read_data(0xCAFEBABE, AccessSize::Word);
    handler.complete_request(response).unwrap();

    // Collect TX bytes
    let mut tx_bytes = Vec::new();
    while let Some(byte) = handler.transfer_tx_byte() {
        tx_bytes.push(byte);
    }

    // Response: header + 4 data bytes
    // Header: packet_type=0001, size=10 (word), reserved=0, we=0 → 0x18
    assert_eq!(tx_bytes.len(), 5);
    assert_eq!(tx_bytes[0], 0x18); // header
    assert_eq!(tx_bytes[1], 0xBE); // data[7:0]
    assert_eq!(tx_bytes[2], 0xBA); // data[15:8]
    assert_eq!(tx_bytes[3], 0xFE); // data[23:16]
    assert_eq!(tx_bytes[4], 0xCA); // data[31:24]
}

#[test]
fn test_complete_request_fails_when_none() {
    let mut handler = HostBusHandler::new();

    let response = BusResponse::write_ack(AccessSize::Byte);
    let result = handler.complete_request(response);
    assert_eq!(result, Err(HandlerError::NoOutstandingRequest));
}

// ============================================================
// Edge Cases and Complex Scenarios
// ============================================================

#[test]
fn test_incoming_request_while_outgoing_pending() {
    let mut handler = HostBusHandler::new();

    // Send an outgoing request (but don't transmit all bytes yet)
    let request = BusRequest::read(0x50000000, AccessSize::Word);
    handler.send_request(request).unwrap();

    // Transmit only part of the request
    let _ = handler.transfer_tx_byte(); // header
    let _ = handler.transfer_tx_byte(); // addr0

    // Now receive an incoming request while still pending
    // This simulates full-duplex operation
    handler.transfer_rx_byte(0x01).unwrap(); // header (byte write)
    handler.transfer_rx_byte(0x04).unwrap(); // addr
    handler.transfer_rx_byte(0x00).unwrap();
    handler.transfer_rx_byte(0x00).unwrap();
    handler.transfer_rx_byte(0x50).unwrap();
    handler.transfer_rx_byte(0xEE).unwrap(); // data

    // Should have the incoming request buffered
    assert!(handler.has_incoming_request());
    assert!(handler.has_pending_outgoing_request());

    // Accept and complete the incoming request
    let incoming = handler.accept_request().unwrap();
    assert_eq!(incoming.addr, 0x50000004);
    assert_eq!(incoming.wdata, 0xEE);

    handler
        .complete_request(BusResponse::write_ack(AccessSize::Byte))
        .unwrap();

    // Continue transmitting outgoing request
    while handler.transfer_tx_byte().is_some() {}

    // Now receive response for our outgoing request
    handler.transfer_rx_byte(0x38).unwrap(); // header (word read response)
    handler.transfer_rx_byte(0x78).unwrap();
    handler.transfer_rx_byte(0x56).unwrap();
    handler.transfer_rx_byte(0x34).unwrap();
    handler.transfer_rx_byte(0x12).unwrap();

    let response = handler.receive_response().unwrap();
    assert_eq!(response.rdata, 0x12345678);
}

#[test]
fn test_buffer_full_rejection() {
    let mut handler = HostBusHandler::new();

    // Fill incoming request buffer
    handler.transfer_rx_byte(0x01).unwrap(); // byte write
    handler.transfer_rx_byte(0x00).unwrap();
    handler.transfer_rx_byte(0x00).unwrap();
    handler.transfer_rx_byte(0x00).unwrap();
    handler.transfer_rx_byte(0x50).unwrap();
    handler.transfer_rx_byte(0xAA).unwrap();

    // Don't accept it - buffer is now full

    // Send an outgoing request to fill that buffer too
    handler
        .send_request(BusRequest::read(0x51000000, AccessSize::Word))
        .unwrap();

    // Drain TX
    while handler.transfer_tx_byte().is_some() {}

    // Receive response to fill outgoing_response buffer
    handler.transfer_rx_byte(0x38).unwrap();
    handler.transfer_rx_byte(0x11).unwrap();
    handler.transfer_rx_byte(0x22).unwrap();
    handler.transfer_rx_byte(0x33).unwrap();
    handler.transfer_rx_byte(0x44).unwrap();

    // Both buffers are full, should reject
    assert!(!handler.can_accept_rx());

    let result = handler.transfer_rx_byte(0x00);
    assert_eq!(result, Err(HandlerError::BufferFull));
}

#[test]
fn test_unknown_packet_type_ignored() {
    let mut handler = HostBusHandler::new();

    // Send an unknown packet type (0x50 = type 5)
    handler.transfer_rx_byte(0x50).unwrap();

    // Should be ignored, handler still in idle accepting
    assert!(handler.can_accept_rx());
    assert!(!handler.has_incoming_request());
}

#[test]
fn test_halfword_read_request_and_response() {
    let mut handler = HostBusHandler::new();

    // Receive a halfword read request
    // Header: packet_type=0000, size=01 (half), reserved=0, we=0 → 0x04
    handler.transfer_rx_byte(0x04).unwrap();
    handler.transfer_rx_byte(0x10).unwrap(); // addr
    handler.transfer_rx_byte(0x00).unwrap();
    handler.transfer_rx_byte(0x00).unwrap();
    handler.transfer_rx_byte(0x50).unwrap();

    let request = handler.accept_request().unwrap();
    assert_eq!(request.addr, 0x50000010);
    assert_eq!(request.size, AccessSize::Halfword);
    assert!(!request.we);

    // Complete with halfword read data
    handler
        .complete_request(BusResponse::read_data(0xABCD, AccessSize::Halfword))
        .unwrap();

    let mut tx_bytes = Vec::new();
    while let Some(byte) = handler.transfer_tx_byte() {
        tx_bytes.push(byte);
    }

    // Header: packet_type=0001, size=01 (half), reserved=0, we=0 → 0x14
    assert_eq!(tx_bytes.len(), 3);
    assert_eq!(tx_bytes[0], 0x14);
    assert_eq!(tx_bytes[1], 0xCD); // data[7:0]
    assert_eq!(tx_bytes[2], 0xAB); // data[15:8]
}

#[test]
fn test_halfword_write_request() {
    let mut handler = HostBusHandler::new();

    // Receive a halfword write request
    // Header: packet_type=0000, size=01 (half), reserved=0, we=1 → 0x05
    handler.transfer_rx_byte(0x05).unwrap();
    handler.transfer_rx_byte(0x20).unwrap(); // addr
    handler.transfer_rx_byte(0x00).unwrap();
    handler.transfer_rx_byte(0x00).unwrap();
    handler.transfer_rx_byte(0x50).unwrap();
    handler.transfer_rx_byte(0xEF).unwrap(); // data[7:0]
    handler.transfer_rx_byte(0xBE).unwrap(); // data[15:8]

    let request = handler.accept_request().unwrap();
    assert_eq!(request.addr, 0x50000020);
    assert_eq!(request.wdata, 0xBEEF);
    assert_eq!(request.size, AccessSize::Halfword);
    assert!(request.we);
}

#[test]
fn test_multiple_request_response_cycles() {
    let mut handler = HostBusHandler::new();

    for i in 0..3 {
        // Send request
        let request = BusRequest::write(0x50000000 + i * 4, 0x11 * (i + 1), AccessSize::Byte);
        handler.send_request(request).unwrap();

        // Drain TX
        while handler.transfer_tx_byte().is_some() {}

        // Receive write ack (packet type 0011)
        handler.transfer_rx_byte(0x31).unwrap();

        let response = handler.receive_response().unwrap();
        assert!(response.we);

        // Should be ready for next request
        assert!(!handler.has_pending_outgoing_request());
    }
}

#[test]
fn test_receive_response_returns_none_without_request() {
    let mut handler = HostBusHandler::new();

    // No request sent, no response should be available
    assert!(handler.receive_response().is_none());

    // Even if we receive bytes that look like a response
    handler.transfer_rx_byte(0x30).unwrap(); // read response header
    handler.transfer_rx_byte(0xAB).unwrap(); // data

    // Still should return None - no outstanding request
    assert!(handler.receive_response().is_none());
}

#[test]
fn test_cannot_accept_same_request_twice() {
    let mut handler = HostBusHandler::new();

    // Receive a request
    handler.transfer_rx_byte(0x01).unwrap();
    handler.transfer_rx_byte(0x00).unwrap();
    handler.transfer_rx_byte(0x00).unwrap();
    handler.transfer_rx_byte(0x00).unwrap();
    handler.transfer_rx_byte(0x50).unwrap();
    handler.transfer_rx_byte(0xAA).unwrap();

    // Accept it
    let _request = handler.accept_request().unwrap();

    // Try to accept again - should fail (request is now in-progress)
    let result = handler.accept_request();
    assert_eq!(result, Err(HandlerError::NoRequestAvailable));
}

// ============================================================
// Response to CPU-initiated request (packet type 0001) tests
// ============================================================

#[test]
fn test_receive_cpu_initiated_write_response() {
    let mut handler = HostBusHandler::new();

    // Receive a write ack for a CPU-initiated request (packet type 0001)
    // This happens when FPGA CPU sends a request and host responds
    // We're simulating receiving the response as if we sent a request via packet type 0010

    // First send a request
    handler
        .send_request(BusRequest::write(0x50000000, 0x55, AccessSize::Byte))
        .unwrap();

    // Drain TX
    while handler.transfer_tx_byte().is_some() {}

    // Receive response (packet type 0011 for host-initiated response)
    handler.transfer_rx_byte(0x31).unwrap();

    let response = handler.receive_response().unwrap();
    assert!(response.we);
    assert_eq!(response.size, AccessSize::Byte);
}

#[test]
fn test_receive_halfword_read_response() {
    let mut handler = HostBusHandler::new();

    // Send halfword read
    handler
        .send_request(BusRequest::read(0x51000000, AccessSize::Halfword))
        .unwrap();

    // Drain TX
    while handler.transfer_tx_byte().is_some() {}

    // Receive response (packet type 0011, size=01, we=0 → 0x34)
    handler.transfer_rx_byte(0x34).unwrap();
    handler.transfer_rx_byte(0xCD).unwrap();
    handler.transfer_rx_byte(0xAB).unwrap();

    let response = handler.receive_response().unwrap();
    assert!(!response.we);
    assert_eq!(response.size, AccessSize::Halfword);
    assert_eq!(response.rdata, 0xABCD);
}
