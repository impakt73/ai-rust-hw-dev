use super::*;
use riscv_shared::bus::RTL_PERIPH_LIMIT;

fn drain_tx(handler: &mut HostBusHandler) -> Vec<u8> {
    let mut out = Vec::new();
    while let Some(byte) = handler.transfer_tx_byte() {
        out.push(byte);
    }
    out
}

fn feed_rx_bytes(handler: &mut HostBusHandler, bytes: &[u8]) {
    for b in bytes {
        handler
            .transfer_rx_byte(*b)
            .expect("rx byte should be accepted");
    }
}

#[test]
fn test_single_request_is_encoded_as_burst_len_one() {
    let mut handler = HostBusHandler::new();
    handler
        .send_request(BusRequest::write(
            0x5000_1000,
            0x1122_3344,
            AccessSize::Word,
        ))
        .expect("request should be accepted");

    let bytes = drain_tx(&mut handler);
    assert_eq!(bytes[0], 0x28, "ctrl0 type=0010,size=word,fixed=0");
    assert_eq!(bytes[1], 0x01, "ctrl1 we=1");
    assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]), 0, "len_m1=0");
    assert_eq!(&bytes[4..8], &0x5000_1000u32.to_le_bytes());
    assert_eq!(&bytes[8..12], &0x1122_3344u32.to_le_bytes());
}

#[test]
fn test_burst_write_packet_encoding() {
    let mut handler = HostBusHandler::new();
    let data = vec![
        0x44, 0x33, 0x22, 0x11, 0x88, 0x77, 0x66, 0x55, 0xCC, 0xBB, 0xAA, 0x99,
    ];
    handler
        .send_request(BusRequest::burst_write(
            0x5000_2000,
            AccessSize::Word,
            3,
            false,
            false,
            data.clone(),
        ))
        .expect("burst write should be accepted");

    let bytes = drain_tx(&mut handler);
    assert_eq!(bytes[0], 0x28);
    assert_eq!(bytes[1], 0x01);
    assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]), 2);
    assert_eq!(&bytes[4..8], &0x5000_2000u32.to_le_bytes());
    assert_eq!(&bytes[8..], data.as_slice());
}

#[test]
fn test_decode_burst_read_response() {
    let mut handler = HostBusHandler::new();
    handler
        .send_request(BusRequest::burst_read(
            0x5000_3000,
            AccessSize::Word,
            2,
            false,
            false,
        ))
        .expect("request should be accepted");
    let _ = drain_tx(&mut handler);

    let mut response = vec![0x38, 0x00, 0x01, 0x00];
    response.extend_from_slice(&0x5000_3000u32.to_le_bytes());
    response.extend_from_slice(&0xA1A2_A3A4u32.to_le_bytes());
    response.extend_from_slice(&0xB1B2_B3B4u32.to_le_bytes());
    feed_rx_bytes(&mut handler, &response);

    let decoded = handler
        .receive_response()
        .expect("response should be decoded");
    assert!(!decoded.we);
    assert_eq!(decoded.size, AccessSize::Word);
    assert_eq!(decoded.burst_len, 2);
    assert_eq!(decoded.addr, 0x5000_3000);
    assert_eq!(decoded.data.len(), 8);
    assert_eq!(decoded.rdata, 0xA1A2_A3A4);
}

#[test]
fn test_accept_and_complete_cpu_side_request_with_metadata_echo() {
    let mut handler = HostBusHandler::new();

    let mut request = vec![0x00, 0x01, 0x00, 0x00];
    request.extend_from_slice(&0x5000_0010u32.to_le_bytes());
    request.extend_from_slice(&[0xAB]);
    feed_rx_bytes(&mut handler, &request);

    let req = handler
        .accept_request()
        .expect("request should be available");
    assert!(req.we);
    assert_eq!(req.size, AccessSize::Byte);
    assert_eq!(req.addr, 0x5000_0010);
    assert_eq!(req.burst_len, 1);

    handler
        .complete_request(BusResponse::write_ack(AccessSize::Byte))
        .expect("response should be accepted");

    let tx = drain_tx(&mut handler);
    assert_eq!(tx[0], 0x10, "packet type 0001, byte, no fixed flags");
    assert_eq!(tx[1], 0x01);
    assert_eq!(&tx[4..8], &0x5000_0010u32.to_le_bytes());
}

#[test]
fn test_reject_invalid_burst_config() {
    let mut handler = HostBusHandler::new();
    let bad = BusRequest::burst_write(
        0x5000_0000,
        AccessSize::Word,
        2,
        false,
        false,
        vec![1, 2, 3, 4],
    );
    assert!(matches!(
        handler.send_request(bad),
        Err(HandlerError::InvalidBurstConfig)
    ));
}

#[test]
fn test_reject_read_request_with_payload_bytes() {
    let mut handler = HostBusHandler::new();
    let mut bad = BusRequest::burst_read(0x5000_0000, AccessSize::Word, 1, false, false);
    bad.data = vec![0xAA, 0xBB, 0xCC, 0xDD];
    assert!(matches!(
        handler.send_request(bad),
        Err(HandlerError::InvalidBurstConfig)
    ));
}

#[test]
fn test_request_end_addr_uses_burst_span_for_incrementing_requests() {
    let req = BusRequest::burst_read(0x5000_0000, AccessSize::Word, 4, false, false);
    assert_eq!(request_end_addr(&req), Some(0x5000_000F));
    assert_eq!(
        classify_request_region(&req),
        RequestAddressRegion::RtlPeripheral
    );
}

#[test]
fn test_request_end_addr_uses_single_beat_for_fixed_requests() {
    let req = BusRequest::burst_read(0x5000_0000, AccessSize::Word, 4, true, false);
    assert_eq!(request_end_addr(&req), Some(0x5000_0003));
    assert_eq!(
        classify_request_region(&req),
        RequestAddressRegion::RtlPeripheral
    );
}

#[test]
fn test_classify_request_region_detects_burst_boundary_crossing() {
    let req = BusRequest::burst_read(RTL_PERIPH_LIMIT - 4, AccessSize::Word, 2, false, false);
    assert_eq!(
        classify_request_region(&req),
        RequestAddressRegion::SpansRtlBoundary
    );

    let fixed_req = BusRequest::burst_read(RTL_PERIPH_LIMIT - 4, AccessSize::Word, 2, true, false);
    assert_eq!(
        classify_request_region(&fixed_req),
        RequestAddressRegion::RtlPeripheral
    );
}

#[test]
fn test_complete_request_rejects_mismatched_burst_read_payload() {
    let mut handler = HostBusHandler::new();

    let mut request = vec![0x08, 0x00, 0x01, 0x00];
    request.extend_from_slice(&0x5000_1000u32.to_le_bytes());
    feed_rx_bytes(&mut handler, &request);

    let accepted = handler
        .accept_request()
        .expect("request should be available");
    assert!(!accepted.we);
    assert_eq!(accepted.burst_len, 2);

    let bad_response = BusResponse::read_data(0x1122_3344, AccessSize::Word);
    assert!(matches!(
        handler.complete_request(bad_response),
        Err(HandlerError::InvalidBurstConfig)
    ));
}
