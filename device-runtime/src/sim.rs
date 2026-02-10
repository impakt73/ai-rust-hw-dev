//! Simulator device runtime implementation
//!
//! This module provides [`SimDeviceRuntime`], which implements the
//! [`DeviceRuntime`] trait using the cpu-sim [`InteractiveSimulator`]
//! to run a software simulation of the RISC-V CPU.
//!
//! The simulator runs on a background thread, stepping instructions
//! continuously. Host-initiated bus requests are forwarded to the
//! simulator's host bus handler.

use crate::{BusEvent, DeviceError, DeviceRuntime, PendingHostRequest};
use cpu_sim::InteractiveSimulator;
use host_bus_handler::{BusRequest, BusResponse, HostBusHandler};
use std::path::Path;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Timeout for host-initiated requests (1 second)
const HOST_REQUEST_TIMEOUT: Duration = Duration::from_secs(1);

/// Internal message sent from the main thread to the background thread
enum RuntimeCommand {
    /// Send a host-initiated bus request
    SendRequest(BusRequest),
    /// Load an ELF file into the simulator
    LoadElf(std::path::PathBuf),
    /// Shut down the background thread
    Shutdown,
}

/// Result of a poll iteration on the background thread, sent back to main thread
enum RuntimeEvent {
    /// A bus event was produced
    Bus(BusEvent),
    /// ELF loading completed successfully
    ElfLoaded(u32),
    /// ELF loading failed
    ElfLoadError(String),
    /// A fatal error occurred
    FatalError(String),
    /// A non-fatal error occurred
    #[allow(dead_code)]
    NonFatalError(String),
}

/// Simulator device runtime that runs the CPU in software.
///
/// Implements [`DeviceRuntime`] by running the interactive simulator
/// on a background thread. The main thread communicates via channels
/// and shared state.
pub(crate) struct SimDeviceRuntime {
    /// Channel to send commands to the background thread
    command_tx: mpsc::Sender<RuntimeCommand>,
    /// Channel to receive events from the background thread
    event_rx: mpsc::Receiver<RuntimeEvent>,
    /// Shared pending host request state (for querying from main thread)
    pending_host_request: Arc<Mutex<Option<PendingHostRequest>>>,
    /// Handle to the background thread
    thread_handle: Option<thread::JoinHandle<()>>,
}

impl SimDeviceRuntime {
    /// Create a new SimDeviceRuntime.
    ///
    /// Initializes the interactive simulator and launches a background thread
    /// to step through instructions.
    pub(crate) fn new() -> Result<Self, String> {
        let (command_tx, command_rx) = mpsc::channel::<RuntimeCommand>();
        let (event_tx, event_rx) = mpsc::channel::<RuntimeEvent>();
        let pending_host_request: Arc<Mutex<Option<PendingHostRequest>>> =
            Arc::new(Mutex::new(None));
        let pending_clone = Arc::clone(&pending_host_request);
        let elf_loaded: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));

        let thread_handle = thread::spawn(move || {
            Self::run_loop(command_rx, event_tx, pending_clone, elf_loaded);
        });

        Ok(SimDeviceRuntime {
            command_tx,
            event_rx,
            pending_host_request,
            thread_handle: Some(thread_handle),
        })
    }

    /// Background thread main loop
    fn run_loop(
        command_rx: mpsc::Receiver<RuntimeCommand>,
        event_tx: mpsc::Sender<RuntimeEvent>,
        pending_host_request: Arc<Mutex<Option<PendingHostRequest>>>,
        elf_loaded: Arc<Mutex<bool>>,
    ) {
        // Create the interactive simulator
        let mut simulator = match InteractiveSimulator::new() {
            Ok(sim) => sim,
            Err(e) => {
                let _ = event_tx.send(RuntimeEvent::FatalError(format!(
                    "Failed to create simulator: {}",
                    e
                )));
                return;
            }
        };

        let mut handler = HostBusHandler::new();

        loop {
            // Check for commands from the main thread (non-blocking)
            match command_rx.try_recv() {
                Ok(RuntimeCommand::Shutdown) => break,
                Ok(RuntimeCommand::SendRequest(request)) => {
                    if let Err(e) = handler.send_request(request) {
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
                Ok(RuntimeCommand::LoadElf(path)) => {
                    match simulator.load_elf(&path) {
                        Ok(()) => {
                            // Extract entry point by reading the ELF header again
                            // (InteractiveSimulator::load_elf doesn't return it)
                            // We need to parse the ELF ourselves for the entry point
                            match Self::read_elf_entry_point(&path) {
                                Ok(entry_point) => {
                                    *elf_loaded.lock().unwrap() = true;
                                    let _ = event_tx.send(RuntimeEvent::ElfLoaded(entry_point));
                                }
                                Err(e) => {
                                    let _ = event_tx.send(RuntimeEvent::ElfLoadError(e));
                                }
                            }
                        }
                        Err(e) => {
                            let _ = event_tx.send(RuntimeEvent::ElfLoadError(e));
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => break,
            }

            // Only step the simulator if an ELF has been loaded
            let is_loaded = *elf_loaded.lock().unwrap();
            if is_loaded {
                // Step one instruction
                match simulator.step_instruction() {
                    Ok(result) => {
                        if let Some(tohost) = result.tohost_value {
                            let _ = event_tx.send(RuntimeEvent::Bus(BusEvent::Write {
                                addr: 0x40000000, // SIM_CONTROL_BASE
                                size: 2,          // word
                                data: tohost,
                                is_dram: false,
                            }));
                            // Program has terminated, stop stepping
                            *elf_loaded.lock().unwrap() = false;
                        }
                    }
                    Err(e) => {
                        let _ = event_tx
                            .send(RuntimeEvent::FatalError(format!("Simulation error: {}", e)));
                        break;
                    }
                }
            } else {
                // No ELF loaded, sleep briefly to avoid busy-waiting
                thread::sleep(Duration::from_millis(1));
            }

            // Check for timeout on pending host requests
            {
                let mut pending = pending_host_request.lock().unwrap();
                if let Some(ref p) = *pending {
                    if p.sent_at.elapsed() > HOST_REQUEST_TIMEOUT {
                        let timed_out_addr = p.addr;
                        *pending = None;
                        handler.reset();
                        let _ = event_tx.send(RuntimeEvent::Bus(BusEvent::HostRequestTimeout {
                            addr: timed_out_addr,
                        }));
                    }
                }
            }

            // Check for completed host-initiated responses
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
                        let _ = event_tx.send(RuntimeEvent::Bus(event));
                    }
                }
            }

            // Check for incoming CPU-initiated requests
            if handler.has_incoming_request() {
                if let Ok(request) = handler.accept_request() {
                    let size = request.size.to_size_code();
                    let event = if request.we {
                        let response = BusResponse::write_ack(request.size);
                        if let Err(e) = handler.complete_request(response) {
                            log::error!("Failed to complete write request: {:?}", e);
                        }
                        BusEvent::Write {
                            addr: request.addr,
                            size,
                            data: request.wdata,
                            is_dram: false,
                        }
                    } else {
                        let response = BusResponse::read_data(0, request.size);
                        if let Err(e) = handler.complete_request(response) {
                            log::error!("Failed to complete read request: {:?}", e);
                        }
                        BusEvent::Read {
                            addr: request.addr,
                            size,
                            data: 0,
                            is_dram: false,
                        }
                    };
                    let _ = event_tx.send(RuntimeEvent::Bus(event));
                }
            }
        }
    }

    /// Read the entry point from an ELF file
    fn read_elf_entry_point(path: &Path) -> Result<u32, String> {
        let file_data = std::fs::read(path).map_err(|e| format!("Failed to read ELF: {}", e))?;
        let elf_file = elf::ElfBytes::<elf::endian::AnyEndian>::minimal_parse(&file_data)
            .map_err(|e| format!("Failed to parse ELF: {}", e))?;
        elf_file.ehdr.e_entry.try_into().map_err(|_| {
            format!(
                "ELF entry point 0x{:x} does not fit in u32",
                elf_file.ehdr.e_entry
            )
        })
    }
}

impl DeviceRuntime for SimDeviceRuntime {
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
            Ok(RuntimeEvent::ElfLoaded(_)) | Ok(RuntimeEvent::ElfLoadError(_)) => {
                // These are handled internally by load_elf
                Ok(None)
            }
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
        // Send the ELF load command to the background thread
        self.command_tx
            .send(RuntimeCommand::LoadElf(path.to_path_buf()))
            .map_err(|e| {
                DeviceError::IoError(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    format!("Background thread disconnected: {}", e),
                ))
            })?;

        // Wait synchronously for the result
        loop {
            match self.event_rx.recv_timeout(Duration::from_secs(30)) {
                Ok(RuntimeEvent::ElfLoaded(entry_point)) => return Ok(entry_point),
                Ok(RuntimeEvent::ElfLoadError(e)) => {
                    return Err(DeviceError::OpenFailed(Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        e,
                    ))));
                }
                Ok(RuntimeEvent::FatalError(msg)) => {
                    return Err(DeviceError::IoError(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        msg,
                    )));
                }
                Ok(other_event) => {
                    // Consume other events (Bus events, etc.) during loading
                    match other_event {
                        RuntimeEvent::NonFatalError(msg) => {
                            log::warn!("Non-fatal error during ELF load: {}", msg);
                        }
                        RuntimeEvent::Bus(_) => {
                            log::debug!("Received bus event during load_elf: dropping");
                        }
                        _ => {
                            log::debug!("Received unexpected event during load_elf: dropping");
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    return Err(DeviceError::IoError(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "Timed out waiting for ELF load to complete",
                    )));
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(DeviceError::IoError(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        "Background thread terminated during ELF load",
                    )));
                }
            }
        }
    }
}

impl std::fmt::Display for SimDeviceRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Simulator")
    }
}

impl Drop for SimDeviceRuntime {
    fn drop(&mut self) {
        let _ = self.command_tx.send(RuntimeCommand::Shutdown);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}
