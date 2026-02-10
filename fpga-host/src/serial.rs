//! Serial connection and bus protocol handling
//!
//! This module provides the serial port abstraction and implements the
//! host bus interface protocol for communicating with the FPGA using
//! the host-bus-handler crate.
//!
//! The core I/O logic runs on a background thread inside [`DeviceRuntime`],
//! while [`SerialConnection`] provides the public interface for sending
//! host requests and receiving bus events.

use crate::memory::SparseMemory;
use host_bus_handler::{AccessSize, BusRequest, BusResponse, HostBusHandler};
use riscv_shared::bus::{DRAM_BASE, DRAM_END};
use serialport::SerialPort;
use std::io::{Read, Write};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
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

/// Poll interval for the background thread when idle
const POLL_INTERVAL: Duration = Duration::from_millis(1);

/// Internal message sent from the main thread to the background thread
enum RuntimeCommand {
    /// Send a host-initiated bus request
    SendRequest(BusRequest),
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

/// Background runtime that performs serial I/O on a dedicated thread.
///
/// All serial port interaction, protocol handling, and CPU-initiated request
/// processing happen on the background thread. The main thread communicates
/// via channels and shared state.
struct DeviceRuntime {
    /// Channel to send commands to the background thread
    command_tx: mpsc::Sender<RuntimeCommand>,
    /// Channel to receive events from the background thread
    event_rx: mpsc::Receiver<RuntimeEvent>,
    /// Shared pending host request state (for querying from main thread)
    pending_host_request: Arc<Mutex<Option<PendingHostRequest>>>,
    /// Handle to the background thread
    thread_handle: Option<thread::JoinHandle<()>>,
}

impl DeviceRuntime {
    /// Create a new DeviceRuntime that runs the poll loop on a background thread.
    fn new(port: Box<dyn SerialPort>, memory: Arc<Mutex<SparseMemory>>) -> Self {
        let (command_tx, command_rx) = mpsc::channel::<RuntimeCommand>();
        let (event_tx, event_rx) = mpsc::channel::<RuntimeEvent>();
        let pending_host_request: Arc<Mutex<Option<PendingHostRequest>>> =
            Arc::new(Mutex::new(None));
        let pending_clone = Arc::clone(&pending_host_request);

        let thread_handle = thread::spawn(move || {
            Self::run_loop(port, memory, command_rx, event_tx, pending_clone);
        });

        DeviceRuntime {
            command_tx,
            event_rx,
            pending_host_request,
            thread_handle: Some(thread_handle),
        }
    }

    /// Background thread main loop
    fn run_loop(
        mut port: Box<dyn SerialPort>,
        memory: Arc<Mutex<SparseMemory>>,
        command_rx: mpsc::Receiver<RuntimeCommand>,
        event_tx: mpsc::Sender<RuntimeEvent>,
        pending_host_request: Arc<Mutex<Option<PendingHostRequest>>>,
    ) {
        let mut handler = HostBusHandler::new();
        let mut rx_buffer = [0u8; BUFFER_SIZE];
        let mut rx_buffer_len: usize = 0;
        let mut tx_buffer = [0u8; BUFFER_SIZE];
        let mut tx_buffer_len: usize = 0;

        loop {
            // Check for commands from the main thread (non-blocking)
            match command_rx.try_recv() {
                Ok(RuntimeCommand::Shutdown) => break,
                Ok(RuntimeCommand::SendRequest(request)) => {
                    // Store pending request info
                    {
                        let mut pending = pending_host_request.lock().unwrap();
                        *pending = Some(PendingHostRequest {
                            addr: request.addr,
                            wdata: request.wdata,
                            sent_at: Instant::now(),
                        });
                    }
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
                Err(mpsc::TryRecvError::Empty) => {} // No commands, continue polling
                Err(mpsc::TryRecvError::Disconnected) => break, // Main thread dropped sender
            }

            // Run one poll iteration
            match Self::poll_once(
                &mut port,
                &memory,
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
                    // No event, sleep briefly to avoid busy-spinning
                    thread::sleep(POLL_INTERVAL);
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
        memory: &Arc<Mutex<SparseMemory>>,
        handler: &mut HostBusHandler,
        rx_buffer: &mut [u8; BUFFER_SIZE],
        rx_buffer_len: &mut usize,
        tx_buffer: &mut [u8; BUFFER_SIZE],
        tx_buffer_len: &mut usize,
        pending_host_request: &Arc<Mutex<Option<PendingHostRequest>>>,
    ) -> Result<Option<BusEvent>, SerialError> {
        // === Check for timeout on pending host requests ===
        {
            let mut pending = pending_host_request.lock().unwrap();
            if let Some(ref p) = *pending {
                if p.sent_at.elapsed() > HOST_REQUEST_TIMEOUT {
                    let timed_out_addr = p.addr;

                    // Drain serial port to remove buffered bytes
                    let mut drain_buffer = [0u8; 256];
                    while port.read(&mut drain_buffer).is_ok() {
                        // Continue draining until no more data
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
            if pending.is_some() {
                if let Some(response) = handler.receive_response() {
                    *pending = None;
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
        }

        // Check for incoming CPU-initiated requests from FPGA
        if handler.has_incoming_request() {
            if let Ok(request) = handler.accept_request() {
                let event = Self::process_cpu_request(handler, &request, memory);
                return Ok(Some(event));
            }
        }

        Ok(None)
    }

    /// Fill rx_buffer from serial port (only reads into remaining available space)
    fn fill_rx_buffer(
        port: &mut Box<dyn SerialPort>,
        rx_buffer: &mut [u8; BUFFER_SIZE],
        rx_buffer_len: &mut usize,
    ) -> Result<(), SerialError> {
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
            Err(e) => Err(SerialError::IoError(e)),
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
    ) -> Result<(), SerialError> {
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
            Err(e) => Err(SerialError::IoError(e)),
        }
    }

    /// Process a CPU-initiated request and generate response
    fn process_cpu_request(
        handler: &mut HostBusHandler,
        request: &host_bus_handler::BusRequest,
        memory: &Arc<Mutex<SparseMemory>>,
    ) -> BusEvent {
        let size = request.size.to_size_code();
        let is_dram = is_dram_address(request.addr);

        let mut memory = memory.lock().unwrap();

        if request.we {
            if is_dram {
                match request.size {
                    AccessSize::Byte => memory.write_byte(request.addr, request.wdata as u8),
                    AccessSize::Halfword => {
                        memory.write_halfword(request.addr, request.wdata as u16)
                    }
                    AccessSize::Word => memory.write_word(request.addr, request.wdata),
                }
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
            let rdata = if is_dram {
                match request.size {
                    AccessSize::Byte => memory.read_byte(request.addr) as u32,
                    AccessSize::Halfword => memory.read_halfword(request.addr) as u32,
                    AccessSize::Word => memory.read_word(request.addr),
                }
            } else {
                0
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
}

impl Drop for DeviceRuntime {
    fn drop(&mut self) {
        // Signal the background thread to stop
        let _ = self.command_tx.send(RuntimeCommand::Shutdown);
        // Wait for the thread to finish
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

/// Serial connection with bus protocol handling.
///
/// This is the public interface for communicating with the FPGA over serial.
/// Internally, all serial I/O runs on a background thread managed by
/// [`DeviceRuntime`]. Thread synchronization is handled internally; callers
/// interact with this struct as if it were single-threaded.
pub struct SerialConnection {
    /// Device path for status display
    device_path: String,
    /// Baud rate for status display
    baud_rate: u32,
    /// Background runtime handling serial I/O
    runtime: DeviceRuntime,
}

impl SerialConnection {
    /// Create a new serial connection.
    ///
    /// Opens the serial port and launches a background thread to handle
    /// serial I/O and protocol processing. The provided memory is shared
    /// with the background thread for CPU-initiated request processing.
    pub fn connect(
        device: &str,
        baud: u32,
        memory: Arc<Mutex<SparseMemory>>,
    ) -> Result<Self, SerialError> {
        let port = serialport::new(device, baud)
            .timeout(Duration::from_millis(1))
            .open()
            .map_err(SerialError::OpenFailed)?;

        let runtime = DeviceRuntime::new(port, memory);

        Ok(SerialConnection {
            device_path: device.to_string(),
            baud_rate: baud,
            runtime,
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
        self.runtime.pending_host_request.lock().unwrap().is_some()
    }

    /// Get a clone of the pending host-initiated request information
    pub fn pending_host_request(&self) -> Option<PendingHostRequest> {
        self.runtime.pending_host_request.lock().unwrap().clone()
    }

    /// Send a host-initiated bus request
    ///
    /// Returns Ok(()) if the request was accepted, or Err if there's already
    /// a pending request or another error occurred.
    pub fn send_host_request(&mut self, request: BusRequest) -> Result<(), SerialError> {
        // Check for pending request on the main thread side
        if self.has_pending_host_request() {
            return Err(SerialError::HandlerError(
                host_bus_handler::HandlerError::RequestPending,
            ));
        }

        self.runtime
            .command_tx
            .send(RuntimeCommand::SendRequest(request))
            .map_err(|_| {
                SerialError::IoError(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "Background thread disconnected",
                ))
            })
    }

    /// Poll for bus events (non-blocking).
    ///
    /// Returns the next available event from the background thread, or None
    /// if no events are ready. Errors from the background thread are returned
    /// as `SerialError`.
    pub fn poll(&mut self) -> Result<Option<BusEvent>, SerialError> {
        match self.runtime.event_rx.try_recv() {
            Ok(RuntimeEvent::Bus(event)) => Ok(Some(event)),
            Ok(RuntimeEvent::FatalError(msg)) => Err(SerialError::IoError(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                msg,
            ))),
            Ok(RuntimeEvent::NonFatalError(msg)) => {
                Err(SerialError::IoError(std::io::Error::other(msg)))
            }
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => {
                Err(SerialError::IoError(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "Background thread terminated",
                )))
            }
        }
    }
}
