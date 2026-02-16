//! Device runtime trait and shared types for host-device communication
//!
//! This crate defines the [`DeviceRuntime`] trait for communicating with a
//! RISC-V CPU device, along with shared types like [`BusEvent`] and
//! [`DeviceError`]. Use [`create_device_runtime`] to create a runtime
//! instance for the desired backend (e.g., FPGA over serial).

mod fpga;
mod sim;

use host_bus_handler::AccessSize;
pub use host_bus_handler::BusRequest;
use host_bus_handler::RequestAddressRegion;
use riscv_shared::bus::{sysctrl_boot_addr, sysctrl_status_addr, SYSCTRL_STATUS_CPU_BOOTING};
use std::path::Path;

/// Errors that can occur during device operations
#[derive(Debug)]
pub enum DeviceError {
    /// Failed to open the device connection
    OpenFailed(Box<dyn std::error::Error + Send + Sync>),
    /// I/O error during communication
    IoError(std::io::Error),
    /// Handler error (e.g., buffer full)
    HandlerError(host_bus_handler::HandlerError),
}

impl DeviceError {
    /// Check if this is a fatal error that should cause disconnection.
    ///
    /// Returns true for errors like broken pipe (device disconnected),
    /// connection reset, or other unrecoverable I/O failures.
    pub fn is_fatal(&self) -> bool {
        match self {
            DeviceError::IoError(e) => {
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
            DeviceError::OpenFailed(_) => true,
            // Handler errors are typically recoverable
            DeviceError::HandlerError(_) => false,
        }
    }
}

impl std::fmt::Display for DeviceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceError::OpenFailed(e) => write!(f, "Failed to open device: {}", e),
            DeviceError::IoError(e) => write!(f, "I/O error: {}", e),
            DeviceError::HandlerError(e) => write!(f, "Handler error: {:?}", e),
        }
    }
}

impl std::error::Error for DeviceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DeviceError::OpenFailed(e) => Some(e.as_ref()),
            DeviceError::IoError(e) => Some(e),
            DeviceError::HandlerError(_) => None,
        }
    }
}

impl From<std::io::Error> for DeviceError {
    fn from(e: std::io::Error) -> Self {
        DeviceError::IoError(e)
    }
}

impl From<host_bus_handler::HandlerError> for DeviceError {
    fn from(e: host_bus_handler::HandlerError) -> Self {
        DeviceError::HandlerError(e)
    }
}

/// Available device runtime backends
#[derive(Debug, Clone)]
pub enum DeviceRuntimeType {
    /// FPGA device connected over a serial port
    Fpga {
        /// Serial device path (e.g., /dev/ttyUSB0)
        device: String,
        /// Baud rate for serial communication
        baud: u32,
        /// Startup reset mode (default: None)
        startup_reset: StartupReset,
    },
    /// Software simulator
    Sim,
}

/// Reset mode for [`DeviceRuntime::reset`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetKind {
    /// Reset only the CPU core.
    Cpu,
    /// Reset the full system (including the system controller).
    System,
}

/// Startup reset mode for FPGA device runtime initialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StartupReset {
    /// No reset performed at startup.
    #[default]
    None,
    /// Reset only the CPU core at startup.
    Cpu,
    /// Reset the full system (including the system controller) at startup.
    System,
}

/// Create a device runtime for the specified backend.
pub fn create_device_runtime(
    runtime_type: DeviceRuntimeType,
) -> Result<Box<dyn DeviceRuntime>, DeviceError> {
    match runtime_type {
        DeviceRuntimeType::Fpga {
            device,
            baud,
            startup_reset,
        } => {
            let runtime = fpga::FpgaDeviceRuntime::connect(&device, baud, startup_reset)?;
            Ok(Box::new(runtime))
        }
        DeviceRuntimeType::Sim => {
            let runtime = sim::SimDeviceRuntime::new()
                .map_err(|e| DeviceError::OpenFailed(Box::new(std::io::Error::other(e))))?;
            Ok(Box::new(runtime))
        }
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
    HostReadResponse {
        addr: u32,
        data: u32,
        size: AccessSize,
    },
    /// A host-initiated write acknowledgment received
    HostWriteResponse {
        addr: u32,
        wdata: u32,
        size: AccessSize,
    },
    /// A host-initiated request timed out
    HostRequestTimeout { addr: u32 },
    /// A tohost-based termination was detected
    TohostTermination { value: u32 },
}

/// Pending host-initiated request information for tracking
#[derive(Debug, Clone)]
pub struct PendingHostRequest {
    /// The address being accessed
    pub addr: u32,
    /// Write data (for write requests)
    pub wdata: u32,
    /// Time when the request was sent
    pub sent_at: std::time::Instant,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HostRequestRoute {
    HostBusHandler,
    SystemBus,
    InvalidSpanningRegion,
}

pub(crate) fn classify_host_request_route(request: &BusRequest) -> HostRequestRoute {
    if host_bus_handler::request_end_addr(request).is_none() {
        return HostRequestRoute::InvalidSpanningRegion;
    }

    match host_bus_handler::classify_request_region(request) {
        RequestAddressRegion::RtlPeripheral => HostRequestRoute::HostBusHandler,
        RequestAddressRegion::NonRtl => HostRequestRoute::SystemBus,
        RequestAddressRegion::SpansRtlBoundary => HostRequestRoute::InvalidSpanningRegion,
    }
}

/// Trait defining the interface for communicating with a RISC-V CPU device.
///
/// Implementations handle the details of the communication channel (e.g.,
/// serial port for FPGA, simulation bus for software models). The runtime
/// manages background I/O and provides a non-blocking polling interface.
pub trait DeviceRuntime: std::fmt::Display {
    /// Returns whether this runtime supports the RISC-V F extension.
    fn supports_riscv_f_extension(&self) -> bool;

    /// Send a host-initiated bus request.
    ///
    /// Returns Ok(()) if the request was accepted, or Err if there's already
    /// a pending request or another error occurred.
    fn send_host_request(&mut self, request: BusRequest) -> Result<(), DeviceError>;

    /// Poll for bus events (non-blocking).
    ///
    /// Returns the next available event, or None if no events are ready.
    fn poll(&mut self) -> Result<Option<BusEvent>, DeviceError>;

    /// Check if there is a pending host-initiated request.
    fn has_pending_host_request(&self) -> bool;

    /// Trigger a reset on the target device.
    ///
    /// Implementations must provide backend-specific reset handling.
    fn reset(&mut self, kind: ResetKind) -> Result<(), DeviceError>;

    /// Load an ELF file into the device's memory.
    ///
    /// For FPGA runtimes, this populates the internal memory model.
    /// For simulator runtimes, this loads directly into the simulator's memory.
    ///
    /// Returns the ELF entry point address on success.
    fn load_elf(&mut self, path: &Path) -> Result<u32, DeviceError>;

    /// Load raw program bytes into device memory at the specified address.
    ///
    /// The provided bytes are written into memory starting at `boot_pc`
    /// so they can be executed later via [`boot_cpu`] with the same address.
    ///
    /// # Arguments
    /// * `boot_pc` - Address at which to load the program. Must be within the
    ///   valid DRAM range; an error is returned if the address or the resulting
    ///   range falls outside DRAM.
    /// * `data` - Byte slice containing the program data (typically encoded
    ///   RISC-V instructions in little-endian format)
    ///
    /// # Errors
    /// Returns `Err(DeviceError)` if:
    /// - The address range `[boot_pc, boot_pc + data.len())` is outside the
    ///   valid DRAM range
    /// - The background thread is disconnected or times out
    fn load_program(&mut self, boot_pc: u32, data: &[u8]) -> Result<(), DeviceError>;

    /// Write a memory region using host-initiated bus requests.
    ///
    /// This default implementation issues the largest request size possible at
    /// each step (word, then halfword, then byte), without touching addresses
    /// outside the requested region.
    fn write_memory_region(&mut self, start_addr: u32, data: &[u8]) -> Result<(), DeviceError> {
        let mut offset = 0usize;
        while offset < data.len() {
            let addr = start_addr.wrapping_add(offset as u32);
            let remaining = data.len() - offset;
            let (size, step) = if remaining >= 4 && (addr & 0x3) == 0 {
                (AccessSize::Word, 4usize)
            } else if remaining >= 2 && (addr & 0x1) == 0 {
                (AccessSize::Halfword, 2usize)
            } else {
                (AccessSize::Byte, 1usize)
            };
            let wdata = match step {
                4 => u32::from_le_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]),
                2 => u16::from_le_bytes([data[offset], data[offset + 1]]) as u32,
                1 => data[offset] as u32,
                _ => unreachable!(),
            };
            self.send_host_request(BusRequest::write(addr, wdata, size))?;

            loop {
                match self.poll()? {
                    Some(BusEvent::HostWriteResponse {
                        addr: resp_addr,
                        size: resp_size,
                        ..
                    }) if resp_addr == addr
                        && resp_size == size
                        && !self.has_pending_host_request() =>
                    {
                        break;
                    }
                    Some(BusEvent::HostRequestTimeout { addr: resp_addr }) if resp_addr == addr => {
                        return Err(DeviceError::IoError(std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            format!(
                                "Timed out writing {} at address 0x{addr:08x}",
                                access_size_name(size)
                            ),
                        )));
                    }
                    Some(_) => {}
                    None => std::thread::sleep(std::time::Duration::from_millis(1)),
                }
            }
            offset += step;
        }

        Ok(())
    }

    /// Read a memory region using host-initiated bus requests.
    ///
    /// This default implementation issues the largest request size possible at
    /// each step (word, then halfword, then byte), without touching addresses
    /// outside the requested region.
    fn read_memory_region(&mut self, start_addr: u32, size: u32) -> Result<Vec<u8>, DeviceError> {
        let mut data = Vec::with_capacity(size as usize);
        let mut offset = 0u32;
        while offset < size {
            let addr = start_addr.wrapping_add(offset);
            let remaining = size - offset;
            let (request_size, step) = if remaining >= 4 && (addr & 0x3) == 0 {
                (AccessSize::Word, 4u32)
            } else if remaining >= 2 && (addr & 0x1) == 0 {
                (AccessSize::Halfword, 2u32)
            } else {
                (AccessSize::Byte, 1u32)
            };
            self.send_host_request(BusRequest::read(addr, request_size))?;

            let read_data = loop {
                match self.poll()? {
                    Some(BusEvent::HostReadResponse {
                        addr: resp_addr,
                        data: read_data,
                        size: resp_size,
                        ..
                    }) if resp_addr == addr
                        && resp_size == request_size
                        && !self.has_pending_host_request() =>
                    {
                        break read_data;
                    }
                    Some(BusEvent::HostRequestTimeout { addr: resp_addr }) if resp_addr == addr => {
                        return Err(DeviceError::IoError(std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            format!(
                                "Timed out reading {} at address 0x{addr:08x}",
                                access_size_name(request_size)
                            ),
                        )));
                    }
                    Some(_) => {}
                    None => std::thread::sleep(std::time::Duration::from_millis(1)),
                }
            };
            let bytes = read_data.to_le_bytes();
            data.extend_from_slice(&bytes[..step as usize]);
            offset += step;
        }

        Ok(data)
    }

    /// Boot the CPU from the specified address.
    ///
    /// This performs a two-phase boot sequence:
    /// 1. Reads the system controller STATUS register to verify the CPU is in
    ///    the boot-wait state (`cpu_booting` bit set).
    /// 2. Writes the boot address to the BOOT register to start execution.
    ///
    /// This default implementation uses generic bus request functionality
    /// (`send_host_request` and `poll`) and works for any `DeviceRuntime`.
    fn boot_cpu(&mut self, boot_addr: u32) -> Result<(), DeviceError> {
        // Phase 1: Read STATUS register to verify cpu_booting bit
        let status_addr = sysctrl_status_addr();
        let request = BusRequest::read(status_addr, AccessSize::Word);
        self.send_host_request(request)?;

        // Poll until we get the STATUS read response
        let status_val = loop {
            match self.poll()? {
                Some(BusEvent::HostReadResponse { addr, data, .. })
                    if addr == status_addr && !self.has_pending_host_request() =>
                {
                    break data;
                }
                Some(BusEvent::HostRequestTimeout { addr }) if addr == status_addr => {
                    return Err(DeviceError::IoError(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        format!("Timed out reading STATUS register at 0x{:08x}", status_addr),
                    )));
                }
                Some(_) => {
                    // Continue polling (ignore other events)
                }
                None => {
                    // No event ready; yield briefly to avoid busy-waiting
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }
        };

        // Verify cpu_booting bit is set
        if (status_val & SYSCTRL_STATUS_CPU_BOOTING) == 0 {
            return Err(DeviceError::IoError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "Boot failed: cpu_booting bit not set (STATUS=0x{:08x})",
                    status_val
                ),
            )));
        }

        // Phase 2: Write boot address to BOOT register
        let boot_reg_addr = sysctrl_boot_addr();
        let request = BusRequest::write(boot_reg_addr, boot_addr, AccessSize::Word);
        self.send_host_request(request)?;

        // Poll until we get the BOOT write acknowledgment
        loop {
            match self.poll()? {
                Some(BusEvent::HostWriteResponse { addr, .. })
                    if addr == boot_reg_addr && !self.has_pending_host_request() =>
                {
                    return Ok(());
                }
                Some(BusEvent::HostRequestTimeout { addr }) if addr == boot_reg_addr => {
                    return Err(DeviceError::IoError(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        format!(
                            "Timed out writing boot address to BOOT register at 0x{:08x}",
                            boot_reg_addr
                        ),
                    )));
                }
                Some(_) => {
                    // Continue polling (ignore other events)
                }
                None => {
                    // No event ready; yield briefly to avoid busy-waiting
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }
        }
    }
}
