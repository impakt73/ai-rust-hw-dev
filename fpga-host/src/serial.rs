//! Serial connection and bus protocol handling
//!
//! This module provides the serial port abstraction and implements the
//! host bus interface protocol for communicating with the FPGA using
//! the host-bus-handler crate.

use crate::memory::SparseMemory;
use host_bus_handler::{AccessSize, BusRequest, BusResponse, HostBusHandler};
use riscv_shared::bus::{DRAM_BASE, DRAM_END};
use serialport::SerialPort;
use std::io::{Read, Write};
use std::time::{Duration, Instant};

/// Size of the intermediate buffers for RX and TX
const BUFFER_SIZE: usize = 64;

/// Timeout for host-initiated requests (1 second)
const HOST_REQUEST_TIMEOUT: Duration = Duration::from_secs(1);

/// Errors that can occur during serial operations
#[derive(Debug)]
pub enum SerialError {
    /// Failed to open the serial port
    OpenFailed(serialport::Error),
    /// I/O error during communication
    IoError(std::io::Error),
    /// Handler error (e.g., buffer full)
    HandlerError(host_bus_handler::HandlerError),
}

impl SerialError {
    /// Check if this is a fatal I/O error that should cause disconnection.
    ///
    /// Returns true for errors like broken pipe (device disconnected),
    /// connection reset, or other unrecoverable I/O failures.
    pub fn is_fatal(&self) -> bool {
        match self {
            SerialError::IoError(e) => {
                use std::io::ErrorKind;
                matches!(
                    e.kind(),
                    ErrorKind::BrokenPipe
                        | ErrorKind::ConnectionReset
                        | ErrorKind::ConnectionAborted
                        | ErrorKind::NotConnected
                        | ErrorKind::PermissionDenied
                )
            }
            // OpenFailed is fatal by nature (we never connected)
            SerialError::OpenFailed(_) => true,
            // Handler errors are typically recoverable
            SerialError::HandlerError(_) => false,
        }
    }
}

impl std::fmt::Display for SerialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SerialError::OpenFailed(e) => write!(f, "Failed to open serial port: {}", e),
            SerialError::IoError(e) => write!(f, "I/O error: {}", e),
            SerialError::HandlerError(e) => write!(f, "Handler error: {:?}", e),
        }
    }
}

impl std::error::Error for SerialError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SerialError::OpenFailed(e) => Some(e),
            SerialError::IoError(e) => Some(e),
            SerialError::HandlerError(_) => None,
        }
    }
}

impl From<std::io::Error> for SerialError {
    fn from(e: std::io::Error) -> Self {
        SerialError::IoError(e)
    }
}

impl From<host_bus_handler::HandlerError> for SerialError {
    fn from(e: host_bus_handler::HandlerError) -> Self {
        SerialError::HandlerError(e)
    }
}

/// Check if an address is within the DRAM range
fn is_dram_address(addr: u32) -> bool {
    (DRAM_BASE..=DRAM_END).contains(&addr)
}

/// Get the size name for logging
pub fn size_name(size: u8) -> &'static str {
    match size {
        0 => "byte",
        1 => "halfword",
        _ => "word",
    }
}

/// Get the size name for an AccessSize
pub fn access_size_name(size: AccessSize) -> &'static str {
    match size {
        AccessSize::Byte => "byte",
        AccessSize::Halfword => "halfword",
        AccessSize::Word => "word",
    }
}

/// Get the number of bytes for a given size code
pub fn bytes_for_size(size: u8) -> u8 {
    match size {
        0 => 1,
        1 => 2,
        _ => 4,
    }
}

/// Event generated when a bus transaction completes
#[derive(Debug)]
pub enum BusEvent {
    /// A read transaction completed (CPU-initiated, handled by host)
    Read {
        addr: u32,
        size: u8,
        data: u32,
        is_dram: bool,
    },
    /// A write transaction completed (CPU-initiated, handled by host)
    Write {
        addr: u32,
        size: u8,
        data: u32,
        is_dram: bool,
    },
    /// A host-initiated read response received
    HostReadResponse { data: u32, size: AccessSize },
    /// A host-initiated write acknowledgment received
    HostWriteResponse { size: AccessSize },
    /// A host-initiated request timed out
    HostRequestTimeout { addr: u32 },
}

/// Pending host-initiated request information for tracking
#[derive(Debug, Clone)]
pub struct PendingHostRequest {
    /// The address being accessed
    pub addr: u32,
    /// Write data (for write requests)
    pub wdata: u32,
    /// Time when the request was sent
    pub sent_at: Instant,
}

/// Serial connection with bus protocol handling
pub struct SerialConnection {
    /// Underlying serial port
    port: Box<dyn SerialPort>,
    /// Device path for status display
    device_path: String,
    /// Baud rate for status display
    baud_rate: u32,
    /// Host bus handler from the crate
    handler: HostBusHandler,
    /// Pending host-initiated request (if any)
    pending_host_request: Option<PendingHostRequest>,
    /// Intermediate RX buffer for bytes read from serial but not yet consumed by handler
    rx_buffer: [u8; BUFFER_SIZE],
    /// Number of valid bytes in rx_buffer (starting from index 0)
    rx_buffer_len: usize,
    /// Intermediate TX buffer for bytes from handler not yet written to serial
    tx_buffer: [u8; BUFFER_SIZE],
    /// Number of valid bytes in tx_buffer (starting from index 0)
    tx_buffer_len: usize,
}

impl SerialConnection {
    /// Create a new serial connection
    pub fn connect(device: &str, baud: u32) -> Result<Self, SerialError> {
        let port = serialport::new(device, baud)
            .timeout(Duration::from_millis(1))
            .open()
            .map_err(SerialError::OpenFailed)?;

        Ok(SerialConnection {
            port,
            device_path: device.to_string(),
            baud_rate: baud,
            handler: HostBusHandler::new(),
            pending_host_request: None,
            rx_buffer: [0u8; BUFFER_SIZE],
            rx_buffer_len: 0,
            tx_buffer: [0u8; BUFFER_SIZE],
            tx_buffer_len: 0,
        })
    }

    /// Get the device path
    pub fn device_path(&self) -> &str {
        &self.device_path
    }

    /// Get the baud rate
    pub fn baud_rate(&self) -> u32 {
        self.baud_rate
    }

    /// Check if there is a pending host-initiated request
    pub fn has_pending_host_request(&self) -> bool {
        self.pending_host_request.is_some()
    }

    /// Get information about the pending host-initiated request
    pub fn pending_host_request(&self) -> Option<&PendingHostRequest> {
        self.pending_host_request.as_ref()
    }

    /// Send a host-initiated bus request
    ///
    /// Returns Ok(()) if the request was accepted, or Err if there's already
    /// a pending request or another error occurred.
    pub fn send_host_request(&mut self, request: BusRequest) -> Result<(), SerialError> {
        if self.pending_host_request.is_some() {
            return Err(SerialError::HandlerError(
                host_bus_handler::HandlerError::RequestPending,
            ));
        }

        // Store info about the request for later logging
        self.pending_host_request = Some(PendingHostRequest {
            addr: request.addr,
            wdata: request.wdata,
            sent_at: Instant::now(),
        });

        self.handler.send_request(request)?;
        Ok(())
    }

    /// Poll for and process bus requests (non-blocking)
    ///
    /// This function uses persistent intermediate buffers to avoid losing data:
    /// - RX: Read from serial port into rx_buffer, then transfer to handler as it accepts bytes
    /// - TX: Transfer from handler into tx_buffer, then write to serial port as it accepts bytes
    ///
    /// Any bytes that couldn't be processed persist in the buffers for the next poll call.
    ///
    /// Returns Ok(Some(event)) if a complete transaction was processed,
    /// Ok(None) if no complete transaction was processed,
    /// or Err if an error occurred.
    pub fn poll(&mut self, memory: &mut SparseMemory) -> Result<Option<BusEvent>, SerialError> {
        // === Check for timeout on pending host requests ===
        if let Some(ref pending) = self.pending_host_request {
            if pending.sent_at.elapsed() > HOST_REQUEST_TIMEOUT {
                let timed_out_addr = pending.addr;
                
                // Drain serial port to remove buffered bytes
                let mut drain_buffer = [0u8; 256];
                while self.port.read(&mut drain_buffer).is_ok() {
                    // Continue draining until no more data
                }
                
                // Clear rx/tx buffers
                self.rx_buffer_len = 0;
                self.tx_buffer_len = 0;
                
                // Clear pending request
                self.pending_host_request = None;
                
                // Reset handler
                self.handler.reset();
                
                // Return timeout event to be handled by main loop
                return Ok(Some(BusEvent::HostRequestTimeout { addr: timed_out_addr }));
            }
        }

        // === RX Path: Serial Port -> rx_buffer -> Handler ===

        // Step 1: Fill rx_buffer from serial port (only into remaining space)
        self.fill_rx_buffer()?;

        // Step 2: Transfer bytes from rx_buffer to handler
        self.drain_rx_buffer_to_handler();

        // === TX Path: Handler -> tx_buffer -> Serial Port ===

        // Step 1: Fill tx_buffer from handler
        self.fill_tx_buffer_from_handler();

        // Step 2: Drain tx_buffer to serial port
        self.drain_tx_buffer()?;

        // === Check for completed transactions ===

        // Check for a response to a host-initiated request
        if self.pending_host_request.is_some() {
            if let Some(response) = self.handler.receive_response() {
                // Clear the pending request - we've received the response
                self.pending_host_request.take();
                let event = if response.we {
                    BusEvent::HostWriteResponse {
                        size: response.size,
                    }
                } else {
                    BusEvent::HostReadResponse {
                        data: response.rdata,
                        size: response.size,
                    }
                };
                return Ok(Some(event));
            }
        }

        // Check for incoming CPU-initiated requests from FPGA
        if self.handler.has_incoming_request() {
            if let Ok(request) = self.handler.accept_request() {
                // Process the request
                let event = self.process_cpu_request(&request, memory);
                return Ok(Some(event));
            }
        }

        Ok(None)
    }

    /// Fill rx_buffer from serial port (only reads into remaining available space)
    fn fill_rx_buffer(&mut self) -> Result<(), SerialError> {
        let available_space = BUFFER_SIZE - self.rx_buffer_len;
        if available_space == 0 {
            return Ok(()); // Buffer is full, cannot read more
        }

        // Read into the available space at the end of the buffer
        match self.port.read(&mut self.rx_buffer[self.rx_buffer_len..]) {
            Ok(n) => {
                self.rx_buffer_len += n;
                Ok(())
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::TimedOut
                    || e.kind() == std::io::ErrorKind::WouldBlock =>
            {
                Ok(()) // No data available, that's fine
            }
            Err(e) => Err(SerialError::IoError(e)),
        }
    }

    /// Transfer bytes from rx_buffer to handler until handler can't accept more
    fn drain_rx_buffer_to_handler(&mut self) {
        let mut consumed = 0;
        for i in 0..self.rx_buffer_len {
            if !self.handler.can_accept_rx() {
                break; // Handler is full, stop
            }
            if let Err(e) = self.handler.transfer_rx_byte(self.rx_buffer[i]) {
                log::warn!("Handler rejected byte: {:?}", e);
                break;
            }
            consumed += 1;
        }

        // Shift remaining bytes to the front of the buffer
        if consumed > 0 && consumed < self.rx_buffer_len {
            self.rx_buffer.copy_within(consumed..self.rx_buffer_len, 0);
        }
        self.rx_buffer_len -= consumed;
    }

    /// Fill tx_buffer from handler until buffer is full or handler has no more data
    fn fill_tx_buffer_from_handler(&mut self) {
        while self.tx_buffer_len < BUFFER_SIZE {
            if let Some(byte) = self.handler.transfer_tx_byte() {
                self.tx_buffer[self.tx_buffer_len] = byte;
                self.tx_buffer_len += 1;
            } else {
                break; // No more data from handler
            }
        }
    }

    /// Drain tx_buffer to serial port, keeping any bytes that couldn't be written
    fn drain_tx_buffer(&mut self) -> Result<(), SerialError> {
        if self.tx_buffer_len == 0 {
            return Ok(()); // Nothing to write
        }

        match self.port.write(&self.tx_buffer[..self.tx_buffer_len]) {
            Ok(n) => {
                // Shift remaining bytes to the front of the buffer
                if n > 0 && n < self.tx_buffer_len {
                    self.tx_buffer.copy_within(n..self.tx_buffer_len, 0);
                }
                self.tx_buffer_len -= n;
                Ok(())
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::TimedOut
                    || e.kind() == std::io::ErrorKind::WouldBlock =>
            {
                Ok(()) // Cannot write now, keep data for next poll
            }
            Err(e) => Err(SerialError::IoError(e)),
        }
    }

    /// Process a CPU-initiated request and generate response
    fn process_cpu_request(
        &mut self,
        request: &host_bus_handler::BusRequest,
        memory: &mut SparseMemory,
    ) -> BusEvent {
        let size = request.size.to_size_code();
        let is_dram = is_dram_address(request.addr);

        if request.we {
            // Write request
            if is_dram {
                match request.size {
                    AccessSize::Byte => memory.write_byte(request.addr, request.wdata as u8),
                    AccessSize::Halfword => {
                        memory.write_halfword(request.addr, request.wdata as u16)
                    }
                    AccessSize::Word => memory.write_word(request.addr, request.wdata),
                }
            }
            // Send write acknowledgment
            let response = BusResponse::write_ack(request.size);
            if let Err(e) = self.handler.complete_request(response) {
                log::error!("Failed to complete write request: {:?}", e);
            }

            BusEvent::Write {
                addr: request.addr,
                size,
                data: request.wdata,
                is_dram,
            }
        } else {
            // Read request
            let rdata = if is_dram {
                match request.size {
                    AccessSize::Byte => memory.read_byte(request.addr) as u32,
                    AccessSize::Halfword => memory.read_halfword(request.addr) as u32,
                    AccessSize::Word => memory.read_word(request.addr),
                }
            } else {
                0 // Non-DRAM reads return 0
            };

            // Send read response
            let response = BusResponse::read_data(rdata, request.size);
            if let Err(e) = self.handler.complete_request(response) {
                log::error!("Failed to complete read request: {:?}", e);
            }

            BusEvent::Read {
                addr: request.addr,
                size,
                data: rdata,
                is_dram,
            }
        }
    }
}
