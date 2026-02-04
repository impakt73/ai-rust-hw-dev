//! Host Bus Handler
//!
//! This crate provides a Rust implementation that mirrors the hardware behavior
//! of the `host_bus_interface` RTL module for host-side FPGA communication.
//!
//! # Protocol Overview
//!
//! The host bus interface uses a serialized packet protocol with four packet types:
//!
//! - **Type 0000** - CPU-initiated request (FPGA → Host TX)
//! - **Type 0001** - Host response to CPU request (Host → FPGA RX)
//! - **Type 0010** - Host-initiated request (Host → FPGA RX)
//! - **Type 0011** - FPGA response to Host request (FPGA → Host TX)
//!
//! ## Extended Header Format (1 byte):
//! - Bits [7:4]: Packet type
//! - Bits [3:2]: Size (00=byte, 01=half, 10=word, 11=reserved)
//! - Bit [1]: Reserved (0)
//! - Bit [0]: Write enable (1=write, 0=read)
//!
//! # Usage
//!
//! ```rust
//! use host_bus_handler::{HostBusHandler, BusRequest, BusResponse, AccessSize};
//!
//! let mut handler = HostBusHandler::new();
//!
//! // Send an outgoing request
//! let request = BusRequest::read(0x50000000, AccessSize::Word);
//! handler.send_request(request).expect("Should accept request");
//!
//! // Transfer bytes until response is ready
//! while handler.receive_response().is_none() {
//!     // Handle TX bytes (send request to FPGA)
//!     while let Some(byte) = handler.transfer_tx_byte() {
//!         // Send byte over serial/USB to FPGA
//!     }
//!     
//!     // Handle RX bytes (receive response from FPGA)
//!     // if let Some(byte) = get_byte_from_fpga() {
//!     //     handler.transfer_rx_byte(byte);
//!     // }
//!     break; // Exit loop for doc test - in real code, poll for RX bytes
//! }
//! ```

/// Access size for bus operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessSize {
    /// Byte access (1 byte)
    Byte = 0,
    /// Halfword access (2 bytes)
    Halfword = 1,
    /// Word access (4 bytes)
    Word = 2,
}

impl AccessSize {
    /// Get the number of bytes for this access size
    pub fn byte_count(self) -> u8 {
        match self {
            AccessSize::Byte => 1,
            AccessSize::Halfword => 2,
            AccessSize::Word => 4,
        }
    }

    /// Try to convert from a u8 value (0, 1, 2)
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(AccessSize::Byte),
            1 => Some(AccessSize::Halfword),
            2 => Some(AccessSize::Word),
            _ => None,
        }
    }
}

/// Bus request representing a read or write operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusRequest {
    /// Target address
    pub addr: u32,
    /// Write data (ignored for reads)
    pub wdata: u32,
    /// Write enable (true = write, false = read)
    pub we: bool,
    /// Access size
    pub size: AccessSize,
}

impl BusRequest {
    /// Create a read request
    pub fn read(addr: u32, size: AccessSize) -> Self {
        BusRequest {
            addr,
            wdata: 0,
            we: false,
            size,
        }
    }

    /// Create a write request
    pub fn write(addr: u32, data: u32, size: AccessSize) -> Self {
        BusRequest {
            addr,
            wdata: data,
            we: true,
            size,
        }
    }
}

/// Response to a bus request
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusResponse {
    /// Read data (only valid for read requests)
    pub rdata: u32,
    /// Access size (echoed from request)
    pub size: AccessSize,
    /// Write enable (echoed from request)
    pub we: bool,
}

impl BusResponse {
    /// Create a write acknowledgment response
    pub fn write_ack(size: AccessSize) -> Self {
        BusResponse {
            rdata: 0,
            size,
            we: true,
        }
    }

    /// Create a read response with data
    pub fn read_data(data: u32, size: AccessSize) -> Self {
        BusResponse {
            rdata: data,
            size,
            we: false,
        }
    }
}

/// Error types for handler operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandlerError {
    /// Handler cannot accept more data (buffers are full)
    BufferFull,
    /// Already have an outstanding request pending response
    RequestPending,
    /// No data available to transfer
    NoDataAvailable,
    /// No request available to accept
    NoRequestAvailable,
    /// No outstanding request to complete
    NoOutstandingRequest,
}

/// RX state machine states (mirroring host_rx_buffer.sv)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RxState {
    /// Idle - waiting for header byte
    Idle,
    /// Receiving response data bytes (for packet type 0001 read responses)
    RespRdata { byte_idx: u8 },
    /// Receiving request address bytes (for packet type 0010)
    ReqAddr { byte_idx: u8 },
    /// Receiving request write data bytes (for packet type 0010 writes)
    ReqWdata { byte_idx: u8 },
    /// Receiving host response data bytes (for packet type 0011 read responses)
    HostRespRdata { byte_idx: u8 },
}

/// TX state machine states for outgoing packets
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TxState {
    /// Idle - no data to transmit
    Idle,
    /// Sending response header (packet type 0001)
    ResponseHeader,
    /// Sending response data bytes
    ResponseData { byte_idx: u8 },
    /// Sending outgoing request header (packet type 0010)
    RequestHeader,
    /// Sending outgoing request address bytes
    RequestAddr { byte_idx: u8 },
    /// Sending outgoing request write data bytes
    RequestWdata { byte_idx: u8 },
}

/// Internal state for a buffered incoming request
#[derive(Debug, Clone, Default)]
struct IncomingRequest {
    /// Whether a complete request is buffered
    valid: bool,
    /// Write enable
    we: bool,
    /// Access size (0, 1, 2)
    size: u8,
    /// Target address
    addr: u32,
    /// Write data (for writes)
    wdata: u32,
}

/// Internal state for a buffered response (to CPU-initiated request)
#[derive(Debug, Clone, Default)]
struct IncomingResponse {
    /// Whether a complete response is buffered
    valid: bool,
    /// Write enable (echoed from request)
    we: bool,
    /// Access size
    size: u8,
    /// Read data
    rdata: u32,
}

/// Host Bus Handler
///
/// This structure abstracts the host-side logic for communicating with an FPGA
/// using the host bus request protocol. It mirrors the behavior of the RTL
/// `host_bus_interface` and `host_rx_buffer` modules.
#[derive(Debug)]
pub struct HostBusHandler {
    // RX state machine
    rx_state: RxState,
    rx_temp_we: bool,
    rx_temp_size: u8,

    // Buffered incoming response (for our outgoing requests - packet type 0011)
    outgoing_response: IncomingResponse,

    // Buffered incoming request (from FPGA - packet type 0010)
    incoming_request: IncomingRequest,

    // Outstanding accepted request (waiting for completion via complete_request)
    accepted_request: Option<(bool, u8)>, // (we, size)

    // TX state machine
    tx_state: TxState,

    // Outgoing request we're transmitting (packet type 0010)
    outgoing_request: Option<BusRequest>,

    // Flag indicating we've started transmitting the outgoing request
    outgoing_request_tx_started: bool,

    // Pending response to send (packet type 0001)
    pending_response: Option<BusResponse>,

    // Temporary data accumulators
    rx_addr_accumulator: u32,
    rx_data_accumulator: u32,
}

impl Default for HostBusHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl HostBusHandler {
    /// Create a new HostBusHandler in the idle state
    pub fn new() -> Self {
        HostBusHandler {
            rx_state: RxState::Idle,
            rx_temp_we: false,
            rx_temp_size: 0,
            outgoing_response: IncomingResponse::default(),
            incoming_request: IncomingRequest::default(),
            accepted_request: None,
            tx_state: TxState::Idle,
            outgoing_request: None,
            outgoing_request_tx_started: false,
            pending_response: None,
            rx_addr_accumulator: 0,
            rx_data_accumulator: 0,
        }
    }

    /// Reset the handler to initial state
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Check if the handler can accept a new RX byte
    ///
    /// Returns true if at least one buffer is available or if we're in the middle
    /// of receiving a packet.
    pub fn can_accept_rx(&self) -> bool {
        // If actively receiving a packet, must accept
        if self.rx_state != RxState::Idle {
            return true;
        }

        // In idle, can accept if at least one buffer is free
        !self.outgoing_response.valid || !self.incoming_request.valid
    }

    /// Attempts to feed a new received byte into the handler.
    ///
    /// Fails if the handler cannot accept any more data because it has both
    /// a host bus request and a host bus response internally buffered.
    /// Steps the internal RX state machine if the data is accepted.
    ///
    /// # Arguments
    /// * `byte` - The byte received from the FPGA TX interface
    ///
    /// # Returns
    /// * `Ok(())` if the byte was accepted
    /// * `Err(HandlerError::BufferFull)` if both internal buffers are full
    pub fn transfer_rx_byte(&mut self, byte: u8) -> Result<(), HandlerError> {
        if !self.can_accept_rx() {
            return Err(HandlerError::BufferFull);
        }

        match self.rx_state {
            RxState::Idle => {
                // Parse header byte
                let packet_type = (byte >> 4) & 0x0F;
                let we = (byte & 0x01) != 0;
                let size = (byte >> 2) & 0x03;

                match packet_type {
                    0x01 => {
                        // Host response to our CPU-initiated request (we're acting as CPU)
                        // This is a response to a request we sent
                        if self.outgoing_response.valid {
                            // Buffer is full, reject
                            return Err(HandlerError::BufferFull);
                        }

                        self.rx_temp_we = we;
                        self.rx_temp_size = size;
                        self.rx_data_accumulator = 0;

                        if we {
                            // Write acknowledgment - complete immediately
                            self.outgoing_response = IncomingResponse {
                                valid: true,
                                we: true,
                                size,
                                rdata: 0,
                            };
                            // Stay in idle
                        } else {
                            // Read response - need to receive data bytes
                            self.rx_state = RxState::RespRdata { byte_idx: 0 };
                        }
                    }
                    0x00 => {
                        // CPU-initiated request from FPGA (we're acting as Host)
                        // This is a request from the FPGA that we need to handle
                        if self.incoming_request.valid {
                            // Buffer is full, reject
                            return Err(HandlerError::BufferFull);
                        }

                        self.rx_temp_we = we;
                        self.rx_temp_size = size;
                        self.rx_addr_accumulator = 0;
                        self.rx_data_accumulator = 0;
                        self.rx_state = RxState::ReqAddr { byte_idx: 0 };
                    }
                    0x03 => {
                        // FPGA response to our host-initiated request
                        // This is a response to a request we sent (packet type 0010)
                        if self.outgoing_response.valid {
                            return Err(HandlerError::BufferFull);
                        }

                        self.rx_temp_we = we;
                        self.rx_temp_size = size;
                        self.rx_data_accumulator = 0;

                        if we {
                            // Write response - complete immediately
                            self.outgoing_response = IncomingResponse {
                                valid: true,
                                we: true,
                                size,
                                rdata: 0,
                            };
                        } else {
                            // Read response - need to receive data bytes
                            self.rx_state = RxState::HostRespRdata { byte_idx: 0 };
                        }
                    }
                    _ => {
                        // Unknown packet type, ignore (stay in idle)
                    }
                }
            }

            RxState::RespRdata { byte_idx } => {
                // Receiving response data (little-endian)
                self.rx_data_accumulator |= (byte as u32) << (byte_idx * 8);

                let num_bytes = Self::bytes_for_size(self.rx_temp_size);
                if byte_idx + 1 >= num_bytes {
                    // Complete
                    self.outgoing_response = IncomingResponse {
                        valid: true,
                        we: false,
                        size: self.rx_temp_size,
                        rdata: self.rx_data_accumulator,
                    };
                    self.rx_state = RxState::Idle;
                } else {
                    self.rx_state = RxState::RespRdata {
                        byte_idx: byte_idx + 1,
                    };
                }
            }

            RxState::HostRespRdata { byte_idx } => {
                // Receiving host response data (little-endian)
                self.rx_data_accumulator |= (byte as u32) << (byte_idx * 8);

                let num_bytes = Self::bytes_for_size(self.rx_temp_size);
                if byte_idx + 1 >= num_bytes {
                    // Complete
                    self.outgoing_response = IncomingResponse {
                        valid: true,
                        we: false,
                        size: self.rx_temp_size,
                        rdata: self.rx_data_accumulator,
                    };
                    self.rx_state = RxState::Idle;
                } else {
                    self.rx_state = RxState::HostRespRdata {
                        byte_idx: byte_idx + 1,
                    };
                }
            }

            RxState::ReqAddr { byte_idx } => {
                // Receiving request address (little-endian)
                self.rx_addr_accumulator |= (byte as u32) << (byte_idx * 8);

                if byte_idx == 3 {
                    // Address complete
                    if self.rx_temp_we {
                        // Write request - continue receiving data
                        self.rx_state = RxState::ReqWdata { byte_idx: 0 };
                    } else {
                        // Read request - complete
                        self.incoming_request = IncomingRequest {
                            valid: true,
                            we: false,
                            size: self.rx_temp_size,
                            addr: self.rx_addr_accumulator,
                            wdata: 0,
                        };
                        self.rx_state = RxState::Idle;
                    }
                } else {
                    self.rx_state = RxState::ReqAddr {
                        byte_idx: byte_idx + 1,
                    };
                }
            }

            RxState::ReqWdata { byte_idx } => {
                // Receiving request write data (little-endian)
                self.rx_data_accumulator |= (byte as u32) << (byte_idx * 8);

                let num_bytes = Self::bytes_for_size(self.rx_temp_size);
                if byte_idx + 1 >= num_bytes {
                    // Complete
                    self.incoming_request = IncomingRequest {
                        valid: true,
                        we: true,
                        size: self.rx_temp_size,
                        addr: self.rx_addr_accumulator,
                        wdata: self.rx_data_accumulator,
                    };
                    self.rx_state = RxState::Idle;
                } else {
                    self.rx_state = RxState::ReqWdata {
                        byte_idx: byte_idx + 1,
                    };
                }
            }
        }

        Ok(())
    }

    /// Attempts to pull a new transmitted byte out of the handler.
    ///
    /// Fails if there are no outgoing bytes ready to be transmitted.
    /// Steps the internal TX state machine if data is returned successfully.
    ///
    /// # Returns
    /// * `Some(byte)` if a byte is ready to transmit
    /// * `None` if no data is ready
    pub fn transfer_tx_byte(&mut self) -> Option<u8> {
        // Check if we need to start a new transmission
        if self.tx_state == TxState::Idle {
            // Priority 1: Send response to an accepted incoming request
            if let Some((we, size)) = self.accepted_request {
                if let Some(ref response) = self.pending_response {
                    // Start sending response
                    self.tx_state = TxState::ResponseHeader;
                    // Store response info for transmission
                    let _ = (we, size, response);
                }
            }

            // Priority 2: Send outgoing request (only if we haven't started transmitting it yet)
            if self.tx_state == TxState::Idle
                && self.outgoing_request.is_some()
                && !self.outgoing_request_tx_started
            {
                self.tx_state = TxState::RequestHeader;
                self.outgoing_request_tx_started = true;
            }
        }

        match self.tx_state {
            TxState::Idle => None,

            TxState::ResponseHeader => {
                let response = self.pending_response.as_ref()?;
                // Format: {packet_type=0001, size[1:0], 0, we}
                let header = 0x10 | ((response.size as u8 & 0x03) << 2) | (response.we as u8);

                if response.we {
                    // Write ack - done after header
                    self.pending_response = None;
                    self.accepted_request = None;
                    self.tx_state = TxState::Idle;
                } else {
                    // Read response - send data bytes next
                    self.tx_state = TxState::ResponseData { byte_idx: 0 };
                }

                Some(header)
            }

            TxState::ResponseData { byte_idx } => {
                let response = self.pending_response.as_ref()?;
                let byte = ((response.rdata >> (byte_idx * 8)) & 0xFF) as u8;
                let num_bytes = response.size.byte_count();

                if byte_idx + 1 >= num_bytes {
                    // Done
                    self.pending_response = None;
                    self.accepted_request = None;
                    self.tx_state = TxState::Idle;
                } else {
                    self.tx_state = TxState::ResponseData {
                        byte_idx: byte_idx + 1,
                    };
                }

                Some(byte)
            }

            TxState::RequestHeader => {
                let request = self.outgoing_request.as_ref()?;
                // Format: {packet_type=0010, size[1:0], 0, we}
                let header = 0x20 | ((request.size as u8 & 0x03) << 2) | (request.we as u8);

                self.tx_state = TxState::RequestAddr { byte_idx: 0 };
                Some(header)
            }

            TxState::RequestAddr { byte_idx } => {
                let request = self.outgoing_request.as_ref()?;
                let byte = ((request.addr >> (byte_idx * 8)) & 0xFF) as u8;

                if byte_idx == 3 {
                    // Address done
                    if request.we {
                        // Write request - send data bytes next
                        self.tx_state = TxState::RequestWdata { byte_idx: 0 };
                    } else {
                        // Read request - done transmitting, wait for response
                        // Keep outgoing_request to match response
                        self.tx_state = TxState::Idle;
                    }
                } else {
                    self.tx_state = TxState::RequestAddr {
                        byte_idx: byte_idx + 1,
                    };
                }

                Some(byte)
            }

            TxState::RequestWdata { byte_idx } => {
                let request = self.outgoing_request.as_ref()?;
                let byte = ((request.wdata >> (byte_idx * 8)) & 0xFF) as u8;
                let num_bytes = request.size.byte_count();

                if byte_idx + 1 >= num_bytes {
                    // Done - wait for response
                    self.tx_state = TxState::Idle;
                } else {
                    self.tx_state = TxState::RequestWdata {
                        byte_idx: byte_idx + 1,
                    };
                }

                Some(byte)
            }
        }
    }

    /// Check if there are any bytes ready to transmit
    pub fn has_tx_data(&self) -> bool {
        match self.tx_state {
            TxState::Idle => {
                // Check if we would start transmitting
                (self.accepted_request.is_some() && self.pending_response.is_some())
                    || (self.outgoing_request.is_some() && !self.outgoing_request_tx_started)
            }
            _ => true,
        }
    }

    // ========================================================================
    // High-level outgoing request interface (send_request, receive_response)
    // ========================================================================

    /// Attempts to push a new BusRequest structure into the handler.
    ///
    /// If the handler already has an outstanding bus request, it rejects the new
    /// request to follow the rule of only having one request outstanding at a time.
    /// Otherwise, the handler stores the request and begins the transmission process.
    ///
    /// # Arguments
    /// * `request` - The bus request to send
    ///
    /// # Returns
    /// * `Ok(())` if the request was accepted
    /// * `Err(HandlerError::RequestPending)` if there's already an outstanding request
    pub fn send_request(&mut self, request: BusRequest) -> Result<(), HandlerError> {
        if self.outgoing_request.is_some() {
            return Err(HandlerError::RequestPending);
        }

        self.outgoing_request = Some(request);
        // TX state will be started by transfer_tx_byte when called
        Ok(())
    }

    /// Attempts to pull a new BusResponse structure out of the handler.
    ///
    /// If the handler has internally already received a response for a prior
    /// request it sent, then it can be consumed via this function.
    ///
    /// # Returns
    /// * `Some(response)` if a response is ready
    /// * `None` if no response is available
    pub fn receive_response(&mut self) -> Option<BusResponse> {
        if !self.outgoing_response.valid {
            return None;
        }

        // Only return response if we had sent a request
        // (this filters out responses meant for incoming request handling)
        if self.outgoing_request.is_none() && self.accepted_request.is_none() {
            return None;
        }

        // Clear the outgoing request since we're receiving its response
        self.outgoing_request = None;
        self.outgoing_request_tx_started = false;

        let response = BusResponse {
            rdata: self.outgoing_response.rdata,
            size: AccessSize::from_u8(self.outgoing_response.size).unwrap_or(AccessSize::Byte),
            we: self.outgoing_response.we,
        };
        self.outgoing_response.valid = false;
        Some(response)
    }

    // ========================================================================
    // High-level incoming request interface (accept_request, complete_request)
    // ========================================================================

    /// If the handler has received a complete request and now has it buffered
    /// internally, this function allows the caller to consume it.
    ///
    /// It is then the caller's responsibility to act on the data in the request
    /// and then call `complete_request()` with any associated response data so
    /// the handler can transmit the response back to the other side.
    ///
    /// # Returns
    /// * `Ok(request)` if a request is available
    /// * `Err(HandlerError::NoRequestAvailable)` if no request is buffered
    pub fn accept_request(&mut self) -> Result<BusRequest, HandlerError> {
        if !self.incoming_request.valid {
            return Err(HandlerError::NoRequestAvailable);
        }

        if self.accepted_request.is_some() {
            // Already have an accepted request that hasn't been completed
            return Err(HandlerError::NoRequestAvailable);
        }

        let request = BusRequest {
            addr: self.incoming_request.addr,
            wdata: self.incoming_request.wdata,
            we: self.incoming_request.we,
            size: AccessSize::from_u8(self.incoming_request.size).unwrap_or(AccessSize::Byte),
        };

        // Mark as accepted
        self.accepted_request = Some((self.incoming_request.we, self.incoming_request.size));
        self.incoming_request.valid = false;

        Ok(request)
    }

    /// Complete an outstanding bus request that was acquired via a prior call
    /// to `accept_request()`.
    ///
    /// It rejects the call if there isn't currently an outstanding request that
    /// has been accepted but not completed yet.
    ///
    /// # Arguments
    /// * `response` - The response to send back
    ///
    /// # Returns
    /// * `Ok(())` if the response was accepted
    /// * `Err(HandlerError::NoOutstandingRequest)` if no request is pending completion
    pub fn complete_request(&mut self, response: BusResponse) -> Result<(), HandlerError> {
        if self.accepted_request.is_none() {
            return Err(HandlerError::NoOutstandingRequest);
        }

        self.pending_response = Some(response);
        // TX state will be started by transfer_tx_byte when called
        Ok(())
    }

    // ========================================================================
    // Helper functions
    // ========================================================================

    /// Get the number of data bytes for a given size value
    fn bytes_for_size(size: u8) -> u8 {
        match size {
            0 => 1, // byte
            1 => 2, // halfword
            _ => 4, // word
        }
    }

    /// Check if the handler has an outstanding outgoing request
    pub fn has_pending_outgoing_request(&self) -> bool {
        self.outgoing_request.is_some()
    }

    /// Check if the handler has a buffered incoming request ready to accept
    pub fn has_incoming_request(&self) -> bool {
        self.incoming_request.valid && self.accepted_request.is_none()
    }

    /// Check if the handler is waiting for a response to complete
    pub fn is_waiting_for_completion(&self) -> bool {
        self.accepted_request.is_some() && self.pending_response.is_none()
    }
}

#[cfg(test)]
mod tests;
