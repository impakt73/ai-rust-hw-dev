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
    },
    /// Software simulator
    Sim,
}

/// Create a device runtime for the specified backend.
pub fn create_device_runtime(
    runtime_type: DeviceRuntimeType,
) -> Result<Box<dyn DeviceRuntime>, DeviceError> {
    match runtime_type {
        DeviceRuntimeType::Fpga { device, baud } => {
            let runtime = fpga::FpgaDeviceRuntime::connect(&device, baud)?;
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

/// Trait defining the interface for communicating with a RISC-V CPU device.
///
/// Implementations handle the details of the communication channel (e.g.,
/// serial port for FPGA, simulation bus for software models). The runtime
/// manages background I/O and provides a non-blocking polling interface.
pub trait DeviceRuntime: std::fmt::Display {
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

    /// Load an ELF file into the device's memory.
    ///
    /// For FPGA runtimes, this populates the internal memory model.
    /// For simulator runtimes, this loads directly into the simulator's memory.
    ///
    /// Returns the ELF entry point address on success.
    fn load_elf(&mut self, path: &Path) -> Result<u32, DeviceError>;

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
