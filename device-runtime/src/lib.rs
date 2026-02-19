//! Device runtime trait and shared types for host-device communication
//!
//! This crate defines the [`DeviceRuntime`] trait for communicating with a
//! RISC-V CPU device, along with shared types like [`BusEvent`] and
//! [`DeviceError`]. Use [`create_device_runtime`] to create a runtime
//! instance for the desired backend (e.g., FPGA over serial).

mod fpga;
mod sim;

use bus_shared::BusDevice;
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

/// A custom bus device registration to install before runtime reset/init.
pub struct BusDeviceRegistration {
    /// Base address for the device in the system bus memory map.
    pub base_addr: u32,
    /// Device implementation to register.
    pub device: Box<dyn BusDevice>,
}

/// Create a device runtime for the specified backend.
pub fn create_device_runtime(
    runtime_type: DeviceRuntimeType,
    bus_devices: Option<Vec<BusDeviceRegistration>>,
) -> Result<Box<dyn DeviceRuntime>, DeviceError> {
    match runtime_type {
        DeviceRuntimeType::Fpga {
            device,
            baud,
            startup_reset,
        } => {
            let runtime =
                fpga::FpgaDeviceRuntime::connect(&device, baud, startup_reset, bus_devices)?;
            Ok(Box::new(runtime))
        }
        DeviceRuntimeType::Sim => {
            let runtime = sim::SimDeviceRuntime::new(bus_devices)
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

    /// Load an ELF file into the device memory via [`write_memory_region`].
    ///
    /// This default implementation parses loadable ELF program headers
    /// (`PT_LOAD`) and writes each loadable file-backed segment to the
    /// segment virtual address.
    ///
    /// Returns the ELF entry point address on success.
    fn load_elf(&mut self, path: &Path) -> Result<u32, DeviceError> {
        let file_data = std::fs::read(path).map_err(DeviceError::IoError)?;
        let elf_file =
            elf::ElfBytes::<elf::endian::AnyEndian>::minimal_parse(&file_data).map_err(|e| {
                DeviceError::IoError(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Failed to parse ELF: {e}"),
                ))
            })?;

        let entry_point = u32::try_from(elf_file.ehdr.e_entry).map_err(|_| {
            DeviceError::IoError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "ELF entry point 0x{:x} does not fit in u32",
                    elf_file.ehdr.e_entry
                ),
            ))
        })?;

        if let Some(phdrs) = elf_file.segments() {
            for phdr in phdrs.iter() {
                if phdr.p_type != elf::abi::PT_LOAD {
                    continue;
                }

                let vaddr = u32::try_from(phdr.p_vaddr).map_err(|_| {
                    DeviceError::IoError(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("Segment vaddr 0x{:x} does not fit in u32", phdr.p_vaddr),
                    ))
                })?;
                
                let file_size = usize::try_from(phdr.p_filesz).map_err(|_| {
                    DeviceError::IoError(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!(
                            "Segment file size 0x{:x} does not fit in usize",
                            phdr.p_filesz
                        ),
                    ))
                })?;
                
                let mem_size = usize::try_from(phdr.p_memsz).map_err(|_| {
                    DeviceError::IoError(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!(
                            "Segment mem size 0x{:x} does not fit in usize",
                            phdr.p_memsz
                        ),
                    ))
                })?;
                
                // Skip completely empty segments
                if mem_size == 0 {
                    continue;
                }
                
                // Load file data if present (file_size > 0)
                if file_size > 0 {
                    let offset = usize::try_from(phdr.p_offset).map_err(|_| {
                        DeviceError::IoError(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("Segment offset 0x{:x} does not fit in usize", phdr.p_offset),
                        ))
                    })?;
                    let end = offset.checked_add(file_size).ok_or_else(|| {
                        DeviceError::IoError(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!(
                                "Segment range overflow: offset=0x{offset:x}, size=0x{file_size:x}"
                            ),
                        ))
                    })?;
                    let segment_data = file_data.get(offset..end).ok_or_else(|| {
                        DeviceError::IoError(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!(
                                "Segment out of bounds: offset=0x{offset:x}, size=0x{file_size:x}, file_len=0x{:x}",
                                file_data.len()
                            ),
                        ))
                    })?;
                    self.write_memory_region(vaddr, segment_data, None)?;
                }
                
                // Zero-initialize remaining bytes if mem_size > file_size (BSS/stack)
                if mem_size > file_size {
                    let zero_start = vaddr.checked_add(u32::try_from(file_size).map_err(|_| {
                        DeviceError::IoError(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("file_size 0x{:x} does not fit in u32", file_size),
                        ))
                    })?).ok_or_else(|| {
                        DeviceError::IoError(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("BSS start address overflow: vaddr=0x{vaddr:x}, file_size=0x{file_size:x}"),
                        ))
                    })?;
                    let zero_size = mem_size - file_size;
                    let zeros = vec![0u8; zero_size];
                    self.write_memory_region(zero_start, &zeros, None)?;
                }
            }
        }

        Ok(entry_point)
    }

    /// Load raw program bytes into device memory at the specified address.
    ///
    /// The provided bytes are written into memory starting at `boot_pc`
    /// so they can be executed later via [`boot_cpu`] with the same address.
    ///
    /// # Arguments
    /// * `boot_pc` - Address at which to load the program.
    /// * `data` - Byte slice containing the program data (typically encoded
    ///   RISC-V instructions in little-endian format)
    ///
    /// # Errors
    /// Returns `Err(DeviceError)` if:
    /// - The requested range overflows 32-bit address space
    /// - The background thread is disconnected or times out
    fn load_program(&mut self, boot_pc: u32, data: &[u8]) -> Result<(), DeviceError> {
        self.write_memory_region(boot_pc, data, None)
    }

    /// Write a memory region using host-initiated bus requests.
    ///
    /// This default implementation issues the largest request size possible at
    /// each step (word, then halfword, then byte), without touching addresses
    /// outside the requested region.
    ///
    /// `event_callback` receives non-matching bus events encountered while waiting
    /// for host responses. If `None`, non-matching events are ignored.
    fn write_memory_region(
        &mut self,
        start_addr: u32,
        data: &[u8],
        mut event_callback: Option<&mut dyn FnMut(BusEvent)>,
    ) -> Result<(), DeviceError> {
        let len = u32::try_from(data.len()).map_err(|_| {
            DeviceError::IoError(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Requested write of {} bytes at address 0x{start_addr:08x} exceeds 32-bit addressable range",
                    data.len()
                ),
            ))
        })?;
        if len > 0 {
            let last_offset = len - 1;
            start_addr.checked_add(last_offset).ok_or_else(|| {
                DeviceError::IoError(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "Requested write of {} bytes at address 0x{start_addr:08x} overflows 32-bit address space",
                        data.len()
                    ),
                ))
            })?;
        }

        let mut offset = 0usize;
        while offset < data.len() {
            let offset_u32 = u32::try_from(offset)
                .expect("offset must fit in u32 after upfront length validation");
            let addr = start_addr
                .checked_add(offset_u32)
                .expect("address overflow prevented by upfront range validation");
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
                    Some(event) => {
                        if let Some(callback) = event_callback.as_mut() {
                            callback(event);
                        }
                    }
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
    ///
    /// `event_callback` receives non-matching bus events encountered while waiting
    /// for host responses. If `None`, non-matching events are ignored.
    fn read_memory_region(
        &mut self,
        start_addr: u32,
        size: u32,
        mut event_callback: Option<&mut dyn FnMut(BusEvent)>,
    ) -> Result<Vec<u8>, DeviceError> {
        if size > 0 {
            let last_offset = size - 1;
            start_addr.checked_add(last_offset).ok_or_else(|| {
                DeviceError::IoError(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "Requested read of {} bytes at address 0x{start_addr:08x} overflows 32-bit address space",
                        size
                    ),
                ))
            })?;
        }
        let mut data = Vec::with_capacity(size as usize);
        let mut offset = 0u32;
        while offset < size {
            let addr = start_addr
                .checked_add(offset)
                .expect("address overflow prevented by upfront range validation");
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
                    Some(event) => {
                        if let Some(callback) = event_callback.as_mut() {
                            callback(event);
                        }
                    }
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
