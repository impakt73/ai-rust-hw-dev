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
use std::time::Duration;

/// Size of the receive buffer for batch reads
const RX_BUFFER_SIZE: usize = 64;

/// Size of the transmit buffer for batch writes
const TX_BUFFER_SIZE: usize = 64;

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
}

/// Pending host-initiated request information for tracking
#[derive(Debug, Clone)]
pub struct PendingHostRequest {
    /// The address being accessed
    pub addr: u32,
    /// Write data (for write requests)
    pub wdata: u32,
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
        });

        self.handler.send_request(request)?;
        Ok(())
    }

    /// Poll for and process bus requests (non-blocking)
    ///
    /// This function performs batched I/O operations for efficiency.
    /// It reads available bytes in batches and writes pending bytes in batches.
    ///
    /// Returns Ok(Some(event)) if a complete transaction was processed,
    /// Ok(None) if no complete transaction was processed,
    /// or Err if an error occurred.
    pub fn poll(&mut self, memory: &mut SparseMemory) -> Result<Option<BusEvent>, SerialError> {
        // First, try to read available bytes in batch
        let mut rx_buffer = [0u8; RX_BUFFER_SIZE];
        let bytes_read = self.read_available(&mut rx_buffer)?;

        // Feed received bytes to the handler
        for &byte in &rx_buffer[..bytes_read] {
            if self.handler.can_accept_rx() {
                if let Err(e) = self.handler.transfer_rx_byte(byte) {
                    log::warn!("Handler rejected byte: {:?}", e);
                }
            }
        }

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
                // Write any pending TX bytes before returning
                self.write_pending()?;
                return Ok(Some(event));
            }
        }

        // Check for incoming CPU-initiated requests from FPGA
        if self.handler.has_incoming_request() {
            if let Ok(request) = self.handler.accept_request() {
                // Process the request
                let event = self.process_cpu_request(&request, memory);

                // Write any pending TX bytes (including the response)
                self.write_pending()?;

                return Ok(Some(event));
            }
        }

        // Write any pending TX bytes
        self.write_pending()?;

        Ok(None)
    }

    /// Read available bytes from the serial port in batch
    fn read_available(&mut self, buffer: &mut [u8]) -> Result<usize, SerialError> {
        match self.port.read(buffer) {
            Ok(n) => Ok(n),
            Err(e)
                if e.kind() == std::io::ErrorKind::TimedOut
                    || e.kind() == std::io::ErrorKind::WouldBlock =>
            {
                Ok(0)
            }
            Err(e) => Err(SerialError::IoError(e)),
        }
    }

    /// Write pending TX bytes in batch
    fn write_pending(&mut self) -> Result<(), SerialError> {
        // Collect all pending TX bytes
        let mut tx_buffer = [0u8; TX_BUFFER_SIZE];
        let mut tx_count = 0;

        while tx_count < TX_BUFFER_SIZE {
            if let Some(byte) = self.handler.transfer_tx_byte() {
                tx_buffer[tx_count] = byte;
                tx_count += 1;
            } else {
                break;
            }
        }

        // Write all collected bytes
        if tx_count > 0 {
            let mut written = 0;
            while written < tx_count {
                match self.port.write(&tx_buffer[written..tx_count]) {
                    Ok(0) => {
                        // Would block, try again later
                        break;
                    }
                    Ok(n) => {
                        written += n;
                    }
                    Err(e)
                        if e.kind() == std::io::ErrorKind::TimedOut
                            || e.kind() == std::io::ErrorKind::WouldBlock =>
                    {
                        break;
                    }
                    Err(e) => {
                        return Err(SerialError::IoError(e));
                    }
                }
            }
        }

        Ok(())
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
