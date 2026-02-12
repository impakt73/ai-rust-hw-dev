//! FPGA device runtime implementation
//!
//! This module provides [`FpgaDeviceRuntime`], which implements the
//! [`DeviceRuntime`] trait for communicating with an FPGA-based RISC-V CPU
//! over a serial port using the host-bus-handler protocol.
//!
//! All serial port interaction, protocol handling, and CPU-initiated request
//! processing happen on a background thread. The main thread communicates
//! via channels and shared state.

use crate::{BusEvent, DeviceError, DeviceRuntime, PendingHostRequest};
use bus_shared::{is_valid_dram_range, SystemBus, DRAM_BASE, DRAM_END};
use host_bus_handler::{AccessSize, BusRequest, BusResponse, HostBusHandler};
use serialport::SerialPort;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Size of the intermediate buffers for RX and TX
const BUFFER_SIZE: usize = 64;

/// Timeout for host-initiated requests (1 second)
const HOST_REQUEST_TIMEOUT: Duration = Duration::from_secs(1);

/// Internal message sent from the main thread to the background thread
enum RuntimeCommand {
    /// Send a host-initiated bus request
    SendRequest(BusRequest),
    /// Load an ELF file into memory, with a one-shot channel for the result
    LoadElf(std::path::PathBuf, mpsc::Sender<Result<u32, String>>),
    /// Shut down the background thread
    Shutdown,
}

/// Result of a poll iteration on the background thread, sent back to main thread
enum RuntimeEvent {
    /// A bus event was produced
    Bus(BusEvent),
    /// A fatal serial error occurred
    FatalError(String),
    /// A non-fatal serial error occurred
    NonFatalError(String),
}

/// FPGA device runtime that communicates over a serial port.
///
/// Implements [`DeviceRuntime`] by running serial I/O on a background thread.
/// The main thread communicates via channels and shared state.
pub(crate) struct FpgaDeviceRuntime {
    /// Device path for status display
    device_path: String,
    /// Baud rate for status display
    baud_rate: u32,
    /// Channel to send commands to the background thread
    command_tx: mpsc::Sender<RuntimeCommand>,
    /// Channel to receive events from the background thread
    event_rx: mpsc::Receiver<RuntimeEvent>,
    /// Shared pending host request state (for querying from main thread)
    pending_host_request: Arc<Mutex<Option<PendingHostRequest>>>,
    /// Handle to the background thread
    thread_handle: Option<thread::JoinHandle<()>>,
}

impl FpgaDeviceRuntime {
    /// Create a new FpgaDeviceRuntime connected to the specified serial port.
    ///
    /// Opens the serial port and launches a background thread to handle
    /// serial I/O and protocol processing. The SystemBus is created on the
    /// background thread to avoid requiring `Send` on `SystemBus`.
    pub(crate) fn connect(device: &str, baud: u32) -> Result<Self, DeviceError> {
        let port = serialport::new(device, baud)
            .timeout(Duration::from_millis(1))
            .open()
            .map_err(|e| DeviceError::OpenFailed(Box::new(e)))?;

        let (command_tx, command_rx) = mpsc::channel::<RuntimeCommand>();
        let (event_tx, event_rx) = mpsc::channel::<RuntimeEvent>();
        let pending_host_request: Arc<Mutex<Option<PendingHostRequest>>> =
            Arc::new(Mutex::new(None));
        let pending_clone = Arc::clone(&pending_host_request);

        let thread_handle = thread::spawn(move || {
            Self::run_loop(port, command_rx, event_tx, pending_clone);
        });

        Ok(FpgaDeviceRuntime {
            device_path: device.to_string(),
            baud_rate: baud,
            command_tx,
            event_rx,
            pending_host_request,
            thread_handle: Some(thread_handle),
        })
    }

    /// Background thread main loop
    fn run_loop(
        mut port: Box<dyn SerialPort>,
        command_rx: mpsc::Receiver<RuntimeCommand>,
        event_tx: mpsc::Sender<RuntimeEvent>,
        pending_host_request: Arc<Mutex<Option<PendingHostRequest>>>,
    ) {
        let mut bus = SystemBus::new();
        let mut handler = HostBusHandler::new();
        let mut rx_buffer = [0u8; BUFFER_SIZE];
        let mut rx_buffer_len: usize = 0;
        let mut tx_buffer = [0u8; BUFFER_SIZE];
        let mut tx_buffer_len: usize = 0;

        // Reset all bus devices at startup
        bus.reset_all_devices();

        loop {
            // Check for commands from the main thread (non-blocking)
            match command_rx.try_recv() {
                Ok(RuntimeCommand::Shutdown) => break,
                Ok(RuntimeCommand::SendRequest(request)) => {
                    // pending_host_request is already set by the sender before
                    // enqueuing this command, so we only need to forward to handler
                    if let Err(e) = handler.send_request(request) {
                        // Clear pending on failure
                        {
                            let mut pending = pending_host_request.lock().unwrap();
                            *pending = None;
                        }
                        let _ = event_tx.send(RuntimeEvent::NonFatalError(format!(
                            "Handler error: {:?}",
                            e
                        )));
                    }
                }
                Ok(RuntimeCommand::LoadElf(path, result_tx)) => {
                    match Self::load_elf_into_bus(&path) {
                        Ok((entry_point, new_bus)) => {
                            bus = new_bus;
                            let _ = result_tx.send(Ok(entry_point));
                        }
                        Err(e) => {
                            let _ = result_tx.send(Err(e));
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {} // No commands, continue polling
                Err(mpsc::TryRecvError::Disconnected) => break, // Main thread dropped sender
            }

            // Run one poll iteration
            match Self::poll_once(
                &mut port,
                &mut bus,
                &mut handler,
                &mut rx_buffer,
                &mut rx_buffer_len,
                &mut tx_buffer,
                &mut tx_buffer_len,
                &pending_host_request,
            ) {
                Ok(Some(event)) => {
                    if event_tx.send(RuntimeEvent::Bus(event)).is_err() {
                        break; // Main thread dropped receiver
                    }
                }
                Ok(None) => {
                    // No event produced this iteration. The serial port read
                    // timeout (set during connect) naturally paces the loop
                    // when no data is available, so no explicit sleep is needed.
                }
                Err(e) => {
                    let is_fatal = e.is_fatal();
                    let msg = e.to_string();
                    let runtime_event = if is_fatal {
                        RuntimeEvent::FatalError(msg)
                    } else {
                        RuntimeEvent::NonFatalError(msg)
                    };
                    if event_tx.send(runtime_event).is_err() {
                        break;
                    }
                    if is_fatal {
                        break;
                    }
                }
            }
        }
    }

    /// Single poll iteration (extracted from old SerialConnection::poll)
    #[allow(clippy::too_many_arguments)]
    fn poll_once(
        port: &mut Box<dyn SerialPort>,
        bus: &mut SystemBus,
        handler: &mut HostBusHandler,
        rx_buffer: &mut [u8; BUFFER_SIZE],
        rx_buffer_len: &mut usize,
        tx_buffer: &mut [u8; BUFFER_SIZE],
        tx_buffer_len: &mut usize,
        pending_host_request: &Arc<Mutex<Option<PendingHostRequest>>>,
    ) -> Result<Option<BusEvent>, DeviceError> {
        // === Check for timeout on pending host requests ===
        {
            let mut pending = pending_host_request.lock().unwrap();
            if let Some(ref p) = *pending {
                if p.sent_at.elapsed() > HOST_REQUEST_TIMEOUT {
                    let timed_out_addr = p.addr;

                    // Drain serial port to remove buffered bytes
                    let mut drain_buffer = [0u8; 256];
                    loop {
                        match port.read(&mut drain_buffer) {
                            Ok(0) => break,
                            Ok(_) => continue,
                            Err(ref e)
                                if e.kind() == std::io::ErrorKind::TimedOut
                                    || e.kind() == std::io::ErrorKind::WouldBlock =>
                            {
                                break;
                            }
                            Err(_) => break,
                        }
                    }

                    // Clear rx/tx buffers
                    *rx_buffer_len = 0;
                    *tx_buffer_len = 0;

                    // Clear pending request
                    *pending = None;

                    // Reset handler
                    handler.reset();

                    return Ok(Some(BusEvent::HostRequestTimeout {
                        addr: timed_out_addr,
                    }));
                }
            }
        }

        // === RX Path: Serial Port -> rx_buffer -> Handler ===
        Self::fill_rx_buffer(port, rx_buffer, rx_buffer_len)?;
        Self::drain_rx_buffer_to_handler(handler, rx_buffer, rx_buffer_len);

        // === TX Path: Handler -> tx_buffer -> Serial Port ===
        Self::fill_tx_buffer_from_handler(handler, tx_buffer, tx_buffer_len);
        Self::drain_tx_buffer(port, tx_buffer, tx_buffer_len)?;

        // === Check for completed transactions ===

        // Check for a response to a host-initiated request
        {
            let mut pending = pending_host_request.lock().unwrap();
            if let Some(ref p) = *pending {
                if let Some(response) = handler.receive_response() {
                    let req_addr = p.addr;
                    let req_wdata = p.wdata;
                    *pending = None;
                    let event = if response.we {
                        BusEvent::HostWriteResponse {
                            addr: req_addr,
                            wdata: req_wdata,
                            size: response.size,
                        }
                    } else {
                        BusEvent::HostReadResponse {
                            addr: req_addr,
                            data: response.rdata,
                            size: response.size,
                        }
                    };
                    return Ok(Some(event));
                }
            }
        }

        // Check for incoming CPU-initiated requests from FPGA
        if handler.has_incoming_request() {
            if let Ok(request) = handler.accept_request() {
                let event = Self::process_cpu_request(handler, &request, bus);

                // Tick all bus devices after handling the request
                bus.clock_cycle_all_devices();

                // Check for tohost termination after processing request
                if let Some(value) = bus.sim_control.acknowledge_termination() {
                    return Ok(Some(BusEvent::TohostTermination { value }));
                }

                return Ok(Some(event));
            }
        }

        // Tick all bus devices even when no transaction occurred
        bus.clock_cycle_all_devices();

        // Check for tohost termination outside of CPU request processing
        if let Some(value) = bus.sim_control.acknowledge_termination() {
            return Ok(Some(BusEvent::TohostTermination { value }));
        }

        Ok(None)
    }

    /// Fill rx_buffer from serial port (only reads into remaining available space)
    fn fill_rx_buffer(
        port: &mut Box<dyn SerialPort>,
        rx_buffer: &mut [u8; BUFFER_SIZE],
        rx_buffer_len: &mut usize,
    ) -> Result<(), DeviceError> {
        let available_space = BUFFER_SIZE - *rx_buffer_len;
        if available_space == 0 {
            return Ok(());
        }

        match port.read(&mut rx_buffer[*rx_buffer_len..]) {
            Ok(n) => {
                *rx_buffer_len += n;
                Ok(())
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::TimedOut
                    || e.kind() == std::io::ErrorKind::WouldBlock =>
            {
                Ok(())
            }
            Err(e) => Err(DeviceError::IoError(e)),
        }
    }

    /// Transfer bytes from rx_buffer to handler until handler can't accept more
    fn drain_rx_buffer_to_handler(
        handler: &mut HostBusHandler,
        rx_buffer: &mut [u8; BUFFER_SIZE],
        rx_buffer_len: &mut usize,
    ) {
        let mut consumed = 0;
        for byte in rx_buffer.iter().take(*rx_buffer_len) {
            if !handler.can_accept_rx() {
                break;
            }
            if let Err(e) = handler.transfer_rx_byte(*byte) {
                log::warn!("Handler rejected byte: {:?}", e);
                break;
            }
            consumed += 1;
        }

        if consumed > 0 && consumed < *rx_buffer_len {
            rx_buffer.copy_within(consumed..*rx_buffer_len, 0);
        }
        *rx_buffer_len -= consumed;
    }

    /// Fill tx_buffer from handler until buffer is full or handler has no more data
    fn fill_tx_buffer_from_handler(
        handler: &mut HostBusHandler,
        tx_buffer: &mut [u8; BUFFER_SIZE],
        tx_buffer_len: &mut usize,
    ) {
        while *tx_buffer_len < BUFFER_SIZE {
            if let Some(byte) = handler.transfer_tx_byte() {
                tx_buffer[*tx_buffer_len] = byte;
                *tx_buffer_len += 1;
            } else {
                break;
            }
        }
    }

    /// Drain tx_buffer to serial port, keeping any bytes that couldn't be written
    fn drain_tx_buffer(
        port: &mut Box<dyn SerialPort>,
        tx_buffer: &mut [u8; BUFFER_SIZE],
        tx_buffer_len: &mut usize,
    ) -> Result<(), DeviceError> {
        if *tx_buffer_len == 0 {
            return Ok(());
        }

        match port.write(&tx_buffer[..*tx_buffer_len]) {
            Ok(n) => {
                if n > 0 && n < *tx_buffer_len {
                    tx_buffer.copy_within(n..*tx_buffer_len, 0);
                }
                *tx_buffer_len -= n;
                Ok(())
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::TimedOut
                    || e.kind() == std::io::ErrorKind::WouldBlock =>
            {
                Ok(())
            }
            Err(e) => Err(DeviceError::IoError(e)),
        }
    }

    /// Process a CPU-initiated request and generate response
    fn process_cpu_request(
        handler: &mut HostBusHandler,
        request: &BusRequest,
        bus: &mut SystemBus,
    ) -> BusEvent {
        let size = request.size.to_size_code();
        let is_dram = is_valid_dram_range(request.addr, request.size.byte_count() as u32);

        if request.we {
            match request.size {
                AccessSize::Byte => bus.write_byte(request.addr, request.wdata as u8),
                AccessSize::Halfword => bus.write_halfword(request.addr, request.wdata as u16),
                AccessSize::Word => bus.write_word(request.addr, request.wdata),
            }
            let response = BusResponse::write_ack(request.size);
            if let Err(e) = handler.complete_request(response) {
                log::error!("Failed to complete write request: {:?}", e);
            }

            BusEvent::Write {
                addr: request.addr,
                size,
                data: request.wdata,
                is_dram,
            }
        } else {
            let rdata = match request.size {
                AccessSize::Byte => bus.read_byte(request.addr) as u32,
                AccessSize::Halfword => bus.read_halfword(request.addr) as u32,
                AccessSize::Word => bus.read_word(request.addr),
            };

            let response = BusResponse::read_data(rdata, request.size);
            if let Err(e) = handler.complete_request(response) {
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

    /// Load an ELF file into a new SystemBus, returning the entry point and bus.
    ///
    /// This loads into a temporary SystemBus instance first, so the existing bus is
    /// preserved if loading fails. ELF segments are validated to be within the DRAM
    /// address range and written directly to the bus's backing memory.
    fn load_elf_into_bus(path: &Path) -> Result<(u32, SystemBus), String> {
        let file_data = std::fs::read(path).map_err(|e| format!("Failed to read ELF: {}", e))?;
        let elf_file = elf::ElfBytes::<elf::endian::AnyEndian>::minimal_parse(&file_data)
            .map_err(|e| format!("Failed to parse ELF: {}", e))?;

        let entry_point: u32 = elf_file.ehdr.e_entry.try_into().map_err(|_| {
            format!(
                "ELF entry point 0x{:x} does not fit in u32",
                elf_file.ehdr.e_entry
            )
        })?;

        let mut new_bus = SystemBus::new();

        if let Some(phdrs) = elf_file.segments() {
            for phdr in phdrs.iter() {
                if phdr.p_type == elf::abi::PT_LOAD {
                    let vaddr = phdr.p_vaddr as u32;
                    let file_size = phdr.p_filesz as usize;
                    let offset = phdr.p_offset as usize;

                    if file_size > 0 {
                        // Validate segment address range is within DRAM
                        let end_addr = vaddr.checked_add(file_size as u32).ok_or_else(|| {
                            format!(
                                "Segment address overflow: vaddr=0x{:08x}, size=0x{:x}",
                                vaddr, file_size
                            )
                        })?;
                        if !is_valid_dram_range(vaddr, file_size as u32) {
                            return Err(format!(
                                "Segment [0x{:08x}, 0x{:08x}) is outside DRAM range [0x{:08x}, 0x{:08x}]",
                                vaddr, end_addr, DRAM_BASE, DRAM_END
                            ));
                        }

                        let end = match offset.checked_add(file_size) {
                            Some(end) if end <= file_data.len() => end,
                            _ => {
                                return Err(format!(
                                    "Segment out of bounds: offset=0x{:x}, size=0x{:x}, file_len=0x{:x}",
                                    offset, file_size, file_data.len()
                                ));
                            }
                        };

                        // Write directly to the bus's backing memory (DRAM-only, no device routing)
                        let segment_data = &file_data[offset..end];
                        for (i, &byte) in segment_data.iter().enumerate() {
                            new_bus.memory.write_byte(vaddr + i as u32, byte);
                        }
                        log::info!(
                            "Loaded segment: vaddr=0x{:08x}, size=0x{:x} bytes",
                            vaddr,
                            file_size
                        );
                    }
                }
            }
        }

        Ok((entry_point, new_bus))
    }
}

impl DeviceRuntime for FpgaDeviceRuntime {
    fn send_host_request(&mut self, request: BusRequest) -> Result<(), DeviceError> {
        // Atomically check and set pending request under the same lock
        {
            let mut pending = self.pending_host_request.lock().unwrap();
            if pending.is_some() {
                return Err(DeviceError::HandlerError(
                    host_bus_handler::HandlerError::RequestPending,
                ));
            }
            *pending = Some(PendingHostRequest {
                addr: request.addr,
                wdata: request.wdata,
                sent_at: Instant::now(),
            });
        }

        // Enqueue the command; if the channel send fails, clear the pending flag
        if let Err(e) = self.command_tx.send(RuntimeCommand::SendRequest(request)) {
            let mut pending = self.pending_host_request.lock().unwrap();
            *pending = None;
            return Err(DeviceError::IoError(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                format!("Background thread disconnected: {}", e),
            )));
        }

        Ok(())
    }

    fn poll(&mut self) -> Result<Option<BusEvent>, DeviceError> {
        match self.event_rx.try_recv() {
            Ok(RuntimeEvent::Bus(event)) => Ok(Some(event)),
            Ok(RuntimeEvent::FatalError(msg)) => Err(DeviceError::IoError(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                msg,
            ))),
            Ok(RuntimeEvent::NonFatalError(msg)) => {
                Err(DeviceError::IoError(std::io::Error::other(msg)))
            }
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => {
                Err(DeviceError::IoError(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "Background thread terminated",
                )))
            }
        }
    }

    fn has_pending_host_request(&self) -> bool {
        self.pending_host_request.lock().unwrap().is_some()
    }

    fn load_elf(&mut self, path: &Path) -> Result<u32, DeviceError> {
        // Create a one-shot channel for the ELF load result
        let (result_tx, result_rx) = mpsc::channel::<Result<u32, String>>();

        // Send the ELF load command to the background thread
        self.command_tx
            .send(RuntimeCommand::LoadElf(path.to_path_buf(), result_tx))
            .map_err(|e| {
                DeviceError::IoError(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    format!("Background thread disconnected: {}", e),
                ))
            })?;

        // Wait for the result on the dedicated channel (does not consume bus events)
        match result_rx.recv_timeout(Duration::from_secs(30)) {
            Ok(Ok(entry_point)) => Ok(entry_point),
            Ok(Err(e)) => Err(DeviceError::IoError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e,
            ))),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(DeviceError::IoError(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Timed out waiting for ELF load to complete",
            ))),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(DeviceError::IoError(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "Background thread terminated during ELF load",
                )))
            }
        }
    }
}

impl std::fmt::Display for FpgaDeviceRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({} baud)", self.device_path, self.baud_rate)
    }
}

impl Drop for FpgaDeviceRuntime {
    fn drop(&mut self) {
        // Signal the background thread to stop
        let _ = self.command_tx.send(RuntimeCommand::Shutdown);
        // Wait for the thread to finish
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}
