//! Host Bus Handler
//!
//! Burst-capable packet protocol handler mirroring host-bus RTL framing.
//!
//! Packet types:
//! - `0x0`: CPU-initiated request (FPGA -> host)
//! - `0x1`: Host response to CPU request (host -> FPGA)
//! - `0x2`: Host-initiated request (host -> FPGA)
//! - `0x3`: FPGA response to host request (FPGA -> host)

use riscv_shared::bus::{RTL_PERIPH_BASE, RTL_PERIPH_LIMIT};

/// Access size for bus operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessSize {
    /// Byte access (1 byte).
    Byte = 0,
    /// Halfword access (2 bytes).
    Halfword = 1,
    /// Word access (4 bytes).
    Word = 2,
}

impl AccessSize {
    /// Number of bytes transferred per beat for this access size.
    pub fn byte_count(self) -> u8 {
        match self {
            AccessSize::Byte => 1,
            AccessSize::Halfword => 2,
            AccessSize::Word => 4,
        }
    }

    /// Protocol size encoding used in `ctrl0[3:2]`.
    pub fn to_size_code(self) -> u8 {
        self as u8
    }

    /// Convert protocol size encoding (`0`, `1`, `2`) into [`AccessSize`].
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(AccessSize::Byte),
            1 => Some(AccessSize::Halfword),
            2 => Some(AccessSize::Word),
            _ => None,
        }
    }
}

/// Maximum legal burst length in beats (encoded as u16 `len_m1`, so 65536 beats max).
pub const MAX_BURST_BEATS: u32 = 65_536;
const CTRL1_RESERVED_MASK: u8 = 0xFE;

/// Bus request representing a read or write burst transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusRequest {
    /// First-beat address.
    pub addr: u32,
    /// First write beat as a convenience scalar mirror of `data`.
    pub wdata: u32,
    /// `true` for write, `false` for read.
    pub we: bool,
    /// Transfer size per beat.
    pub size: AccessSize,
    /// Number of beats in the burst.
    pub burst_len: u32,
    /// Source address remains fixed across beats.
    pub src_fixed: bool,
    /// Destination address remains fixed across beats.
    pub dst_fixed: bool,
    /// Burst payload bytes (write requests only).
    pub data: Vec<u8>,
}

impl BusRequest {
    /// Create a single-beat read request.
    pub fn read(addr: u32, size: AccessSize) -> Self {
        Self {
            addr,
            wdata: 0,
            we: false,
            size,
            burst_len: 1,
            src_fixed: false,
            dst_fixed: false,
            data: Vec::new(),
        }
    }

    /// Create a single-beat write request.
    pub fn write(addr: u32, data: u32, size: AccessSize) -> Self {
        let mut bytes = data.to_le_bytes().to_vec();
        bytes.truncate(size.byte_count() as usize);
        Self {
            addr,
            wdata: data,
            we: true,
            size,
            burst_len: 1,
            src_fixed: false,
            dst_fixed: false,
            data: bytes,
        }
    }

    /// Create a read request with explicit burst metadata.
    pub fn burst_read(
        addr: u32,
        size: AccessSize,
        burst_len: u32,
        src_fixed: bool,
        dst_fixed: bool,
    ) -> Self {
        Self {
            addr,
            wdata: 0,
            we: false,
            size,
            burst_len,
            src_fixed,
            dst_fixed,
            data: Vec::new(),
        }
    }

    /// Create a write request with explicit burst metadata and payload bytes.
    pub fn burst_write(
        addr: u32,
        size: AccessSize,
        burst_len: u32,
        src_fixed: bool,
        dst_fixed: bool,
        data: Vec<u8>,
    ) -> Self {
        let mut first = [0u8; 4];
        let beat = size.byte_count() as usize;
        let first_slice = data.get(..beat).unwrap_or(&[]);
        first[..first_slice.len()].copy_from_slice(first_slice);
        Self {
            addr,
            wdata: u32::from_le_bytes(first),
            we: true,
            size,
            burst_len,
            src_fixed,
            dst_fixed,
            data,
        }
    }
}

/// Response to a bus request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusResponse {
    /// First read beat as a convenience scalar mirror of `data`.
    pub rdata: u32,
    /// Transfer size per beat.
    pub size: AccessSize,
    /// Echoed write enable bit from the request.
    pub we: bool,
    /// Echoed first-beat address.
    pub addr: u32,
    /// Echoed burst length.
    pub burst_len: u32,
    /// Echoed source fixed flag.
    pub src_fixed: bool,
    /// Echoed destination fixed flag.
    pub dst_fixed: bool,
    /// Burst payload bytes (read responses only).
    pub data: Vec<u8>,
}

impl BusResponse {
    /// Create a single-beat write acknowledgement response.
    pub fn write_ack(size: AccessSize) -> Self {
        Self {
            rdata: 0,
            size,
            we: true,
            addr: 0,
            burst_len: 1,
            src_fixed: false,
            dst_fixed: false,
            data: Vec::new(),
        }
    }

    /// Create a single-beat read response.
    pub fn read_data(data: u32, size: AccessSize) -> Self {
        let mut bytes = data.to_le_bytes().to_vec();
        bytes.truncate(size.byte_count() as usize);
        Self {
            rdata: data,
            size,
            we: false,
            addr: 0,
            burst_len: 1,
            src_fixed: false,
            dst_fixed: false,
            data: bytes,
        }
    }

    /// Create a burst read response with explicit metadata and payload bytes.
    pub fn burst_read_data(
        addr: u32,
        size: AccessSize,
        burst_len: u32,
        src_fixed: bool,
        dst_fixed: bool,
        data: Vec<u8>,
    ) -> Self {
        let mut first = [0u8; 4];
        let beat = size.byte_count() as usize;
        let first_slice = data.get(..beat).unwrap_or(&[]);
        first[..first_slice.len()].copy_from_slice(first_slice);
        Self {
            rdata: u32::from_le_bytes(first),
            size,
            we: false,
            addr,
            burst_len,
            src_fixed,
            dst_fixed,
            data,
        }
    }
}

/// Error types for handler operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandlerError {
    /// Handler cannot accept more RX packets because both internal buffers are full.
    BufferFull,
    /// A host-initiated request is already pending completion.
    RequestPending,
    /// No TX byte is available to transfer.
    NoDataAvailable,
    /// No CPU-initiated request is available to accept.
    NoRequestAvailable,
    /// `complete_request` was called without a previously accepted request.
    NoOutstandingRequest,
    /// Request touches non-RTL address space or spans RTL boundary.
    InvalidAddressRange,
    /// Burst metadata or payload shape is inconsistent with protocol expectations.
    InvalidBurstConfig,
}

/// Address-region classification for host-initiated requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestAddressRegion {
    /// Entire request span is within RTL peripheral space.
    RtlPeripheral,
    /// Entire request span is outside RTL peripheral space.
    NonRtl,
    /// Request span crosses into/out of RTL peripheral space.
    SpansRtlBoundary,
}

/// Compute the inclusive end address touched by a request burst.
///
/// When the relevant side is fixed (`dst_fixed` for writes, `src_fixed` for reads),
/// only one beat contributes to the address span.
pub fn request_end_addr(request: &BusRequest) -> Option<u32> {
    let stride = u32::from(request.size.byte_count());
    let address_span_beats =
        if (request.we && request.dst_fixed) || (!request.we && request.src_fixed) {
            1
        } else {
            request.burst_len
        };
    let total_bytes = address_span_beats.checked_mul(stride)?;
    let last_byte_offset = total_bytes.checked_sub(1)?;
    request.addr.checked_add(last_byte_offset)
}

/// Classify which memory region a request touches.
///
/// If end-address computation overflows, this is treated as [`RequestAddressRegion::NonRtl`]
/// and rejected by [`HostBusHandler::send_request`].
pub fn classify_request_region(request: &BusRequest) -> RequestAddressRegion {
    let Some(end_addr) = request_end_addr(request) else {
        return RequestAddressRegion::NonRtl;
    };

    let start_in_rtl = (RTL_PERIPH_BASE..RTL_PERIPH_LIMIT).contains(&request.addr);
    let end_in_rtl = (RTL_PERIPH_BASE..RTL_PERIPH_LIMIT).contains(&end_addr);

    match (start_in_rtl, end_in_rtl) {
        (true, true) => RequestAddressRegion::RtlPeripheral,
        (false, false) => RequestAddressRegion::NonRtl,
        _ => RequestAddressRegion::SpansRtlBoundary,
    }
}

/// RX state machine states for burst packet parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RxState {
    /// Waiting for `ctrl0`.
    Idle,
    /// Waiting for `ctrl1`.
    Ctrl1,
    /// Waiting for `len_m1[7:0]`.
    Len0,
    /// Waiting for `len_m1[15:8]`.
    Len1,
    /// Receiving little-endian address bytes.
    Addr { byte_idx: u8 },
    /// Receiving payload bytes (if present for this packet type/direction).
    Payload { remaining: usize },
}

#[derive(Debug, Clone, Default)]
struct IncomingRequest {
    valid: bool,
    request: Option<BusRequest>,
}

#[derive(Debug, Clone, Default)]
struct IncomingResponse {
    valid: bool,
    response: Option<BusResponse>,
}

/// Host-side handler mirroring the RTL host-bus packet interface.
///
/// Interface groups:
/// 1. Byte-stream IO: [`Self::transfer_rx_byte`], [`Self::transfer_tx_byte`]
/// 2. Host-initiated flow: [`Self::send_request`], [`Self::receive_response`]
/// 3. CPU-initiated flow: [`Self::accept_request`], [`Self::complete_request`]
#[derive(Debug)]
pub struct HostBusHandler {
    // RX state machine and temporary packet accumulators.
    rx_state: RxState,
    rx_packet_type: u8,
    rx_temp_we: bool,
    rx_temp_size: u8,
    rx_temp_src_fixed: bool,
    rx_temp_dst_fixed: bool,
    rx_temp_burst_len: u32,
    rx_addr_accumulator: u32,
    rx_payload: Vec<u8>,

    // Buffered packets received from FPGA.
    outgoing_response: IncomingResponse,
    incoming_request: IncomingRequest,
    accepted_request: Option<BusRequest>,

    // TX buffering and pending outbound packets.
    tx_buffer: Vec<u8>,
    tx_index: usize,
    outgoing_request: Option<BusRequest>,
    outgoing_request_tx_started: bool,
    pending_response: Option<BusResponse>,
}

impl Default for HostBusHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl HostBusHandler {
    /// Create a new handler in idle state.
    pub fn new() -> Self {
        HostBusHandler {
            rx_state: RxState::Idle,
            rx_packet_type: 0,
            rx_temp_we: false,
            rx_temp_size: 0,
            rx_temp_src_fixed: false,
            rx_temp_dst_fixed: false,
            rx_temp_burst_len: 1,
            rx_addr_accumulator: 0,
            rx_payload: Vec::new(),
            outgoing_response: IncomingResponse::default(),
            incoming_request: IncomingRequest::default(),
            accepted_request: None,
            tx_buffer: Vec::new(),
            tx_index: 0,
            outgoing_request: None,
            outgoing_request_tx_started: false,
            pending_response: None,
        }
    }

    /// Reset the handler back to its initial idle state.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Returns whether one more RX byte can be accepted right now.
    pub fn can_accept_rx(&self) -> bool {
        if self.rx_state != RxState::Idle {
            return true;
        }
        !self.outgoing_response.valid || !self.incoming_request.valid
    }

    /// Feed one received byte into the RX state machine.
    pub fn transfer_rx_byte(&mut self, byte: u8) -> Result<(), HandlerError> {
        if !self.can_accept_rx() {
            return Err(HandlerError::BufferFull);
        }

        match self.rx_state {
            RxState::Idle => {
                let packet_type = (byte >> 4) & 0x0F;
                let size = (byte >> 2) & 0x03;
                // Protocol only defines packet types 0..=3 and size encodings 0..=2.
                if packet_type > 0x03 || size == 0x03 {
                    return Ok(());
                }
                self.rx_packet_type = packet_type;
                self.rx_temp_size = size;
                self.rx_temp_src_fixed = ((byte >> 1) & 0x01) != 0;
                self.rx_temp_dst_fixed = (byte & 0x01) != 0;
                self.rx_state = RxState::Ctrl1;
            }
            RxState::Ctrl1 => {
                // Reserved bits [7:1] must be zero. Drop malformed frames and resync.
                if (byte & CTRL1_RESERVED_MASK) != 0 {
                    self.rx_state = RxState::Idle;
                    return Ok(());
                }
                self.rx_temp_we = (byte & 0x01) != 0;
                self.rx_state = RxState::Len0;
            }
            RxState::Len0 => {
                self.rx_temp_burst_len = u32::from(byte);
                self.rx_state = RxState::Len1;
            }
            RxState::Len1 => {
                self.rx_temp_burst_len |= u32::from(byte) << 8;
                self.rx_temp_burst_len = self.rx_temp_burst_len.saturating_add(1);
                // `len_m1 + 1` should never be zero; if it is, treat as malformed/overflow.
                if self.rx_temp_burst_len == 0 {
                    self.rx_state = RxState::Idle;
                    return Ok(());
                }
                self.rx_addr_accumulator = 0;
                self.rx_state = RxState::Addr { byte_idx: 0 };
            }
            RxState::Addr { byte_idx } => {
                self.rx_addr_accumulator |= u32::from(byte) << (byte_idx * 8);
                if byte_idx == 3 {
                    let payload_len = self.expected_payload_len(
                        self.rx_packet_type,
                        self.rx_temp_we,
                        self.rx_temp_size,
                        self.rx_temp_burst_len,
                    );
                    if payload_len == 0 {
                        self.finalize_rx_packet()?;
                        self.rx_state = RxState::Idle;
                    } else {
                        self.rx_payload.clear();
                        self.rx_payload.reserve(payload_len);
                        self.rx_state = RxState::Payload {
                            remaining: payload_len,
                        };
                    }
                } else {
                    self.rx_state = RxState::Addr {
                        byte_idx: byte_idx + 1,
                    };
                }
            }
            RxState::Payload { remaining } => {
                self.rx_payload.push(byte);
                if remaining == 1 {
                    self.finalize_rx_packet()?;
                    self.rx_state = RxState::Idle;
                } else {
                    self.rx_state = RxState::Payload {
                        remaining: remaining - 1,
                    };
                }
            }
        }

        Ok(())
    }

    /// Pull one byte from the TX state machine if available.
    pub fn transfer_tx_byte(&mut self) -> Option<u8> {
        if self.tx_index >= self.tx_buffer.len() {
            self.tx_buffer.clear();
            self.tx_index = 0;
            self.start_tx_packet()?;
        }

        let byte = *self.tx_buffer.get(self.tx_index)?;
        self.tx_index += 1;

        if self.tx_index >= self.tx_buffer.len() {
            self.tx_buffer.clear();
            self.tx_index = 0;
        }

        Some(byte)
    }

    pub fn has_tx_data(&self) -> bool {
        if self.tx_index < self.tx_buffer.len() {
            return true;
        }

        (self.accepted_request.is_some() && self.pending_response.is_some())
            || (self.outgoing_request.is_some() && !self.outgoing_request_tx_started)
    }

    /// Queue a host-initiated request for transmission.
    pub fn send_request(&mut self, request: BusRequest) -> Result<(), HandlerError> {
        if self.outgoing_request.is_some() {
            return Err(HandlerError::RequestPending);
        }
        if request.burst_len == 0 || request.burst_len > MAX_BURST_BEATS {
            return Err(HandlerError::InvalidBurstConfig);
        }
        if request.we {
            let expected = (request.size.byte_count() as usize) * (request.burst_len as usize);
            if request.data.len() != expected {
                return Err(HandlerError::InvalidBurstConfig);
            }
        } else if !request.data.is_empty() {
            // Read requests are metadata-only; payload bytes would corrupt stream framing.
            return Err(HandlerError::InvalidBurstConfig);
        }
        if classify_request_region(&request) != RequestAddressRegion::RtlPeripheral {
            return Err(HandlerError::InvalidAddressRange);
        }

        self.outgoing_request = Some(request);
        Ok(())
    }

    /// Fetch a decoded response for a previously transmitted host-initiated request.
    pub fn receive_response(&mut self) -> Option<BusResponse> {
        if !self.outgoing_response.valid {
            return None;
        }

        if self.outgoing_request.is_none() && self.accepted_request.is_none() {
            return None;
        }

        self.outgoing_request = None;
        self.outgoing_request_tx_started = false;
        self.outgoing_response.valid = false;
        self.outgoing_response.response.take()
    }

    /// Accept the next buffered CPU-initiated request from FPGA.
    pub fn accept_request(&mut self) -> Result<BusRequest, HandlerError> {
        if !self.incoming_request.valid || self.accepted_request.is_some() {
            return Err(HandlerError::NoRequestAvailable);
        }

        let request = self
            .incoming_request
            .request
            .clone()
            .ok_or(HandlerError::NoRequestAvailable)?;
        self.accepted_request = Some(request.clone());
        self.incoming_request.valid = false;
        self.incoming_request.request = None;
        Ok(request)
    }

    /// Complete an accepted CPU-initiated request and queue a response packet.
    pub fn complete_request(&mut self, mut response: BusResponse) -> Result<(), HandlerError> {
        let Some(accepted) = self.accepted_request.as_ref() else {
            return Err(HandlerError::NoOutstandingRequest);
        };

        if response.addr == 0 {
            response.addr = accepted.addr;
            response.burst_len = accepted.burst_len;
            response.src_fixed = accepted.src_fixed;
            response.dst_fixed = accepted.dst_fixed;
        }

        if response.we != accepted.we
            || response.size != accepted.size
            || response.addr != accepted.addr
            || response.burst_len != accepted.burst_len
            || response.src_fixed != accepted.src_fixed
            || response.dst_fixed != accepted.dst_fixed
        {
            return Err(HandlerError::InvalidBurstConfig);
        }

        let expected_data_len = if accepted.we {
            0
        } else {
            (accepted.size.byte_count() as usize) * (accepted.burst_len as usize)
        };
        if response.data.len() != expected_data_len {
            return Err(HandlerError::InvalidBurstConfig);
        }

        self.pending_response = Some(response);
        Ok(())
    }

    pub fn has_pending_outgoing_request(&self) -> bool {
        self.outgoing_request.is_some()
    }

    pub fn has_incoming_request(&self) -> bool {
        self.incoming_request.valid && self.accepted_request.is_none()
    }

    pub fn is_waiting_for_completion(&self) -> bool {
        self.accepted_request.is_some() && self.pending_response.is_none()
    }

    fn start_tx_packet(&mut self) -> Option<()> {
        if self.accepted_request.is_some() && self.pending_response.is_some() {
            // Priority 1: respond to accepted CPU-initiated request first.
            let response = self.pending_response.take()?;
            self.tx_buffer = Self::encode_packet(
                0x01,
                response.size,
                response.we,
                response.src_fixed,
                response.dst_fixed,
                response.burst_len,
                response.addr,
                &response.data,
            );
            self.accepted_request = None;
            return Some(());
        }

        if self.outgoing_request.is_some() && !self.outgoing_request_tx_started {
            // Priority 2: transmit next host-initiated request.
            let request = self.outgoing_request.as_ref()?;
            self.tx_buffer = Self::encode_packet(
                0x02,
                request.size,
                request.we,
                request.src_fixed,
                request.dst_fixed,
                request.burst_len,
                request.addr,
                &request.data,
            );
            self.outgoing_request_tx_started = true;
            return Some(());
        }

        None
    }

    fn finalize_rx_packet(&mut self) -> Result<(), HandlerError> {
        let size = AccessSize::from_u8(self.rx_temp_size).unwrap_or(AccessSize::Byte);
        let request = BusRequest {
            addr: self.rx_addr_accumulator,
            wdata: Self::extract_first_word(&self.rx_payload),
            we: self.rx_temp_we,
            size,
            burst_len: self.rx_temp_burst_len,
            src_fixed: self.rx_temp_src_fixed,
            dst_fixed: self.rx_temp_dst_fixed,
            data: self.rx_payload.clone(),
        };

        match self.rx_packet_type {
            0x00 | 0x02 => {
                // Request packet: buffer for host-side consumer.
                if self.incoming_request.valid {
                    return Err(HandlerError::BufferFull);
                }
                self.incoming_request.valid = true;
                self.incoming_request.request = Some(request);
            }
            0x01 | 0x03 => {
                // Response packet: buffer for host-initiated request path.
                if self.outgoing_response.valid {
                    return Err(HandlerError::BufferFull);
                }

                if self.rx_packet_type == 0x03 {
                    if let Some(out_req) = self.outgoing_request.as_ref() {
                        let metadata_matches = out_req.we == self.rx_temp_we
                            && out_req.size == size
                            && out_req.addr == self.rx_addr_accumulator
                            && out_req.burst_len == self.rx_temp_burst_len
                            && out_req.src_fixed == self.rx_temp_src_fixed
                            && out_req.dst_fixed == self.rx_temp_dst_fixed;
                        if !metadata_matches {
                            // Drop mismatched response silently to keep stream parsing forward
                            // progress without tearing down the host link.
                            self.rx_payload.clear();
                            return Ok(());
                        }
                    }
                }

                let response = if self.rx_temp_we {
                    BusResponse {
                        rdata: 0,
                        size,
                        we: true,
                        addr: self.rx_addr_accumulator,
                        burst_len: self.rx_temp_burst_len,
                        src_fixed: self.rx_temp_src_fixed,
                        dst_fixed: self.rx_temp_dst_fixed,
                        data: Vec::new(),
                    }
                } else {
                    BusResponse {
                        rdata: Self::extract_first_word(&self.rx_payload),
                        size,
                        we: false,
                        addr: self.rx_addr_accumulator,
                        burst_len: self.rx_temp_burst_len,
                        src_fixed: self.rx_temp_src_fixed,
                        dst_fixed: self.rx_temp_dst_fixed,
                        data: self.rx_payload.clone(),
                    }
                };

                self.outgoing_response.valid = true;
                self.outgoing_response.response = Some(response);
            }
            _ => {}
        }

        self.rx_payload.clear();
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn encode_packet(
        packet_type: u8,
        size: AccessSize,
        we: bool,
        src_fixed: bool,
        dst_fixed: bool,
        burst_len: u32,
        addr: u32,
        payload: &[u8],
    ) -> Vec<u8> {
        let mut out = Vec::new();
        let ctrl0 = ((packet_type & 0x0F) << 4)
            | ((size.to_size_code() & 0x03) << 2)
            | ((src_fixed as u8) << 1)
            | (dst_fixed as u8);
        out.push(ctrl0);
        out.push(we as u8);
        let len_m1 = (burst_len.saturating_sub(1)) as u16;
        out.extend_from_slice(&len_m1.to_le_bytes());
        out.extend_from_slice(&addr.to_le_bytes());
        out.extend_from_slice(payload);
        out
    }

    fn extract_first_word(payload: &[u8]) -> u32 {
        let mut bytes = [0u8; 4];
        let n = payload.len().min(4);
        bytes[..n].copy_from_slice(&payload[..n]);
        u32::from_le_bytes(bytes)
    }

    fn expected_payload_len(&self, packet_type: u8, we: bool, size: u8, burst_len: u32) -> usize {
        let beat = usize::from(Self::bytes_for_size(size));
        let beats = burst_len as usize;
        match packet_type {
            0x00 | 0x02 if we => beat * beats,
            0x01 | 0x03 if !we => beat * beats,
            _ => 0,
        }
    }

    fn bytes_for_size(size: u8) -> u8 {
        match size {
            0 => 1,
            1 => 2,
            _ => 4,
        }
    }
}

#[cfg(test)]
#[path = "host_bus_handler_tests.rs"]
mod tests;
