//! Serial connection and bus protocol handling
//!
//! This module provides the serial port abstraction and implements the
//! host bus interface protocol for communicating with the FPGA.

use crate::memory::SparseMemory;
use riscv_shared::bus::{DRAM_BASE, DRAM_END};
use serialport::SerialPort;
use std::io::{Read, Write};
use std::time::Duration;

/// Errors that can occur during serial operations
#[derive(Debug)]
pub enum SerialError {
    /// Failed to open the serial port
    OpenFailed(serialport::Error),
    /// I/O error during communication
    IoError(std::io::Error),
}

impl std::fmt::Display for SerialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SerialError::OpenFailed(e) => write!(f, "Failed to open serial port: {}", e),
            SerialError::IoError(e) => write!(f, "I/O error: {}", e),
        }
    }
}

impl std::error::Error for SerialError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SerialError::OpenFailed(e) => Some(e),
            SerialError::IoError(e) => Some(e),
        }
    }
}

impl From<std::io::Error> for SerialError {
    fn from(e: std::io::Error) -> Self {
        SerialError::IoError(e)
    }
}

/// Check if an address is within the DRAM range
fn is_dram_address(addr: u32) -> bool {
    (DRAM_BASE..=DRAM_END).contains(&addr)
}

/// Get the size name for logging
///
/// Maps the size encoding used in the bus protocol to a human-readable name:
/// - 0 = byte (1 byte)
/// - 1 = halfword (2 bytes)
/// - 2+ = word (4 bytes)
pub fn size_name(size: u8) -> &'static str {
    match size {
        0 => "byte",
        1 => "halfword",
        _ => "word",
    }
}

/// Get the number of bytes for a given size code
///
/// Maps the size encoding used in the bus protocol to the actual byte count:
/// - 0 = 1 byte
/// - 1 = 2 bytes (halfword)
/// - 2+ = 4 bytes (word)
pub fn bytes_for_size(size: u8) -> u8 {
    match size {
        0 => 1,
        1 => 2,
        _ => 4,
    }
}

/// Host bus interface state machine states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostBusState {
    /// Waiting for header byte from FPGA
    WaitHeader,
    /// Receiving address bytes (4 bytes, little-endian)
    RxAddr { byte_idx: u8 },
    /// Receiving write data bytes (1-4 bytes based on size)
    RxWdata { byte_idx: u8 },
    /// Sending write acknowledgement
    TxAck,
    /// Sending read data bytes (1-4 bytes based on size)
    TxRdata { byte_idx: u8 },
}

/// Captured transaction from host bus interface
#[derive(Debug, Clone, Default)]
struct HostBusTransaction {
    /// Write enable (true = write, false = read)
    we: bool,
    /// Access size (0 = byte, 1 = halfword, 2 = word)
    size: u8,
    /// Address (accumulated little-endian)
    addr: u32,
    /// Write data (accumulated little-endian, only valid for writes)
    wdata: u32,
    /// Read data to send back (only valid for reads)
    rdata: u32,
}

/// Event generated when a bus transaction completes
#[derive(Debug)]
pub enum BusEvent {
    /// A read transaction completed
    Read {
        addr: u32,
        size: u8,
        data: u32,
        is_dram: bool,
    },
    /// A write transaction completed
    Write {
        addr: u32,
        size: u8,
        data: u32,
        is_dram: bool,
    },
}

/// Serial connection with bus protocol handling
pub struct SerialConnection {
    /// Underlying serial port
    port: Box<dyn SerialPort>,
    /// Device path for status display
    device_path: String,
    /// Baud rate for status display
    baud_rate: u32,
    /// Host bus interface state machine
    bus_state: HostBusState,
    /// Current transaction being processed
    current_txn: HostBusTransaction,
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
            bus_state: HostBusState::WaitHeader,
            current_txn: HostBusTransaction::default(),
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

    /// Poll for and process bus requests (non-blocking)
    ///
    /// Returns Ok(Some(event)) if a complete request was processed,
    /// Ok(None) if no complete request was processed (no data or still in progress),
    /// or Err if an error occurred.
    pub fn poll(&mut self, memory: &mut SparseMemory) -> Result<Option<BusEvent>, SerialError> {
        let mut byte_buf = [0u8; 1];

        loop {
            match self.bus_state {
                HostBusState::WaitHeader => {
                    // Try to read header byte
                    match self.port.read(&mut byte_buf) {
                        Ok(1) => {
                            let header = byte_buf[0];
                            // Parse header: {4'b0000, size[1:0], 1'b0, we}
                            self.current_txn.we = (header & 0x01) != 0;
                            self.current_txn.size = (header >> 2) & 0x03;
                            self.current_txn.addr = 0;
                            self.current_txn.wdata = 0;
                            self.current_txn.rdata = 0;

                            log::debug!(
                                "Received header: 0x{:02x} (we={}, size={})",
                                header,
                                self.current_txn.we,
                                size_name(self.current_txn.size)
                            );
                            self.bus_state = HostBusState::RxAddr { byte_idx: 0 };
                        }
                        Ok(0) => {
                            // No data available
                            return Ok(None);
                        }
                        Err(e)
                            if e.kind() == std::io::ErrorKind::TimedOut
                                || e.kind() == std::io::ErrorKind::WouldBlock =>
                        {
                            // Timeout or would block - no data available
                            return Ok(None);
                        }
                        Err(e) => {
                            // Real I/O error
                            return Err(SerialError::IoError(e));
                        }
                        Ok(_) => unreachable!(),
                    }
                }

                HostBusState::RxAddr { byte_idx } => {
                    match self.port.read(&mut byte_buf) {
                        Ok(1) => {
                            let byte = byte_buf[0] as u32;
                            // Accumulate address (little-endian)
                            self.current_txn.addr |= byte << (byte_idx * 8);

                            if byte_idx == 3 {
                                // Address complete
                                if self.current_txn.we {
                                    // Write: continue receiving write data
                                    self.bus_state = HostBusState::RxWdata { byte_idx: 0 };
                                } else {
                                    // Read: perform read and start sending response
                                    self.perform_read(memory);
                                    self.bus_state = HostBusState::TxRdata { byte_idx: 0 };
                                }
                            } else {
                                self.bus_state = HostBusState::RxAddr {
                                    byte_idx: byte_idx + 1,
                                };
                            }
                        }
                        Ok(0) => {
                            // No data available
                            return Ok(None);
                        }
                        Err(e)
                            if e.kind() == std::io::ErrorKind::TimedOut
                                || e.kind() == std::io::ErrorKind::WouldBlock =>
                        {
                            // Timeout or would block - no data available
                            return Ok(None);
                        }
                        Err(e) => {
                            // Real I/O error
                            return Err(SerialError::IoError(e));
                        }
                        Ok(_) => unreachable!(),
                    }
                }

                HostBusState::RxWdata { byte_idx } => {
                    match self.port.read(&mut byte_buf) {
                        Ok(1) => {
                            let byte = byte_buf[0] as u32;
                            // Accumulate write data (little-endian)
                            self.current_txn.wdata |= byte << (byte_idx * 8);

                            let bytes_needed = bytes_for_size(self.current_txn.size);

                            if byte_idx + 1 >= bytes_needed {
                                // Write data complete - perform write and send ack
                                self.perform_write(memory);
                                self.bus_state = HostBusState::TxAck;
                            } else {
                                self.bus_state = HostBusState::RxWdata {
                                    byte_idx: byte_idx + 1,
                                };
                            }
                        }
                        Ok(0) => {
                            // No data available
                            return Ok(None);
                        }
                        Err(e)
                            if e.kind() == std::io::ErrorKind::TimedOut
                                || e.kind() == std::io::ErrorKind::WouldBlock =>
                        {
                            // Timeout or would block - no data available
                            return Ok(None);
                        }
                        Err(e) => {
                            // Real I/O error
                            return Err(SerialError::IoError(e));
                        }
                        Ok(_) => unreachable!(),
                    }
                }

                HostBusState::TxAck => {
                    // Send acknowledgement byte (0x00)
                    let ack_buf = [0x00u8];
                    match self.port.write(&ack_buf) {
                        Ok(1) => {
                            log::debug!("Sent ACK");
                            let event = BusEvent::Write {
                                addr: self.current_txn.addr,
                                size: self.current_txn.size,
                                data: self.current_txn.wdata,
                                is_dram: is_dram_address(self.current_txn.addr),
                            };
                            self.bus_state = HostBusState::WaitHeader;
                            return Ok(Some(event));
                        }
                        Ok(0) => {
                            // No bytes written, try again later
                            return Ok(None);
                        }
                        Err(e)
                            if e.kind() == std::io::ErrorKind::TimedOut
                                || e.kind() == std::io::ErrorKind::WouldBlock =>
                        {
                            // Timeout or would block - try again later
                            return Ok(None);
                        }
                        Err(e) => {
                            // Real I/O error
                            return Err(SerialError::IoError(e));
                        }
                        Ok(_) => unreachable!(),
                    }
                }

                HostBusState::TxRdata { byte_idx } => {
                    // Send read data byte (little-endian)
                    let byte = ((self.current_txn.rdata >> (byte_idx * 8)) & 0xFF) as u8;
                    let data_buf = [byte];

                    match self.port.write(&data_buf) {
                        Ok(1) => {
                            let bytes_needed = bytes_for_size(self.current_txn.size);

                            if byte_idx + 1 >= bytes_needed {
                                log::debug!("Sent all read data bytes");
                                let event = BusEvent::Read {
                                    addr: self.current_txn.addr,
                                    size: self.current_txn.size,
                                    data: self.current_txn.rdata,
                                    is_dram: is_dram_address(self.current_txn.addr),
                                };
                                self.bus_state = HostBusState::WaitHeader;
                                return Ok(Some(event));
                            } else {
                                self.bus_state = HostBusState::TxRdata {
                                    byte_idx: byte_idx + 1,
                                };
                            }
                        }
                        Ok(0) => {
                            // No bytes written, try again later
                            return Ok(None);
                        }
                        Err(e)
                            if e.kind() == std::io::ErrorKind::TimedOut
                                || e.kind() == std::io::ErrorKind::WouldBlock =>
                        {
                            // Timeout or would block - try again later
                            return Ok(None);
                        }
                        Err(e) => {
                            // Real I/O error
                            return Err(SerialError::IoError(e));
                        }
                        Ok(_) => unreachable!(),
                    }
                }
            }
        }
    }

    /// Perform a read operation
    fn perform_read(&mut self, memory: &SparseMemory) {
        if is_dram_address(self.current_txn.addr) {
            self.current_txn.rdata = match self.current_txn.size {
                0 => memory.read_byte(self.current_txn.addr) as u32,
                1 => memory.read_halfword(self.current_txn.addr) as u32,
                _ => memory.read_word(self.current_txn.addr),
            };
        } else {
            // Non-DRAM reads return 0
            self.current_txn.rdata = 0;
        }
    }

    /// Perform a write operation
    fn perform_write(&self, memory: &mut SparseMemory) {
        if is_dram_address(self.current_txn.addr) {
            match self.current_txn.size {
                0 => memory.write_byte(self.current_txn.addr, self.current_txn.wdata as u8),
                1 => memory.write_halfword(self.current_txn.addr, self.current_txn.wdata as u16),
                _ => memory.write_word(self.current_txn.addr, self.current_txn.wdata),
            }
        }
        // Non-DRAM writes are silently dropped
    }
}
