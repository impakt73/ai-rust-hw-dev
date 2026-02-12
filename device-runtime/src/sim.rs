//! Simulator device runtime implementation
//!
//! This module provides [`SimDeviceRuntime`], which implements the
//! [`DeviceRuntime`] trait using the cpu-sim [`InteractiveSimulator`]
//! to run a software simulation of the RISC-V CPU.
//!
//! The simulator runs on a background thread, stepping instructions
//! continuously. Host-initiated bus requests are forwarded directly
//! to the simulator's internal host bus handler.

use crate::{BusEvent, DeviceError, DeviceRuntime, PendingHostRequest};
use cpu_sim::InteractiveSimulator;
use host_bus_handler::BusRequest;
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
    /// Load an ELF file into the simulator, with a one-shot channel for the result
    LoadElf(std::path::PathBuf, mpsc::Sender<Result<u32, String>>),
    /// Shut down the background thread
    Shutdown,
}

/// Result of a poll iteration on the background thread, sent back to main thread
enum RuntimeEvent {
    /// A bus event was produced
    Bus(BusEvent),
    /// A fatal error occurred
    FatalError(String),
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

        let thread_handle = thread::spawn(move || {
            Self::run_loop(command_rx, event_tx, pending_clone);
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

        let mut elf_loaded = false;

        loop {
            // Check for commands from the main thread (non-blocking)
            match command_rx.try_recv() {
                Ok(RuntimeCommand::Shutdown) => break,
                Ok(RuntimeCommand::SendRequest(request)) => {
                    if let Err(e) = simulator.send_bus_request(request) {
                        // Clear pending and notify caller with a failure event
                        let mut pending = pending_host_request.lock().unwrap();
                        if let Some(ref p) = *pending {
                            let failed_addr = p.addr;
                            let _ =
                                event_tx.send(RuntimeEvent::Bus(BusEvent::HostRequestTimeout {
                                    addr: failed_addr,
                                }));
                        }
                        *pending = None;
                        log::warn!("Host request rejected by simulator: {}", e);
                    }
                }
                Ok(RuntimeCommand::LoadElf(path, result_tx)) => {
                    match simulator.load_elf_no_boot(&path) {
                        Ok(entry_point) => {
                            elf_loaded = true;
                            let _ = result_tx.send(Ok(entry_point));
                        }
                        Err(e) => {
                            let _ = result_tx.send(Err(e));
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => break,
            }

            // Only step the simulator if an ELF has been loaded
            if elf_loaded {
                // Step one cycle for fine-grained control
                match simulator.step_cycle() {
                    Ok(result) => {
                        if let Some(value) = result.tohost_value {
                            let _ = event_tx
                                .send(RuntimeEvent::Bus(BusEvent::TohostTermination { value }));
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
                        let _ = event_tx.send(RuntimeEvent::Bus(BusEvent::HostRequestTimeout {
                            addr: timed_out_addr,
                        }));
                    }
                }
            }

            // Check for completed host-initiated responses from the simulator
            {
                let mut pending = pending_host_request.lock().unwrap();
                if pending.is_some() {
                    if let Some(response) = simulator.receive_bus_response() {
                        let p = pending.as_ref().unwrap();
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
        }
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
            Ok(RuntimeEvent::FatalError(msg)) => Err(DeviceError::IoError(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                msg,
            ))),
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
