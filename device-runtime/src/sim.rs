//! Simulator device runtime implementation
//!
//! This module provides [`SimDeviceRuntime`], which implements the
//! [`DeviceRuntime`] trait using the cpu-sim [`InteractiveSimulator`]
//! to run a software simulation of the RISC-V CPU.
//!
//! The simulator runs on a background thread, stepping instructions
//! continuously. Host-initiated bus requests are forwarded through
//! `InteractiveSimulator::send_bus_request`, which performs internal
//! address-based routing.

use crate::{
    classify_host_request_route, BusDeviceRegistration, BusEvent, DeviceError, DeviceRuntime,
    HostRequestRoute, PendingHostRequest, ResetKind,
};
use cpu_sim::InteractiveSimulator;
use host_bus_handler::{AccessSize, BusRequest, HandlerError};
use riscv_shared::bus::{sysctrl_reset_addr, SYSCTRL_RESET_CPU};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Timeout for host-initiated requests (1 second)
const HOST_REQUEST_TIMEOUT: Duration = Duration::from_secs(1);
/// Timeout for simulator runtime initialization handshake.
const RUNTIME_INIT_TIMEOUT: Duration = Duration::from_secs(30);
/// Timeout for reset command completion.
const RESET_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

/// Internal message sent from the main thread to the background thread
enum RuntimeCommand {
    /// Send a host-initiated bus request
    SendRequest(BusRequest),
    /// Perform a simulator-wide reset with boot deferred.
    Reset(mpsc::Sender<Result<(), String>>),
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
    pub(crate) fn new(bus_devices: Option<Vec<BusDeviceRegistration>>) -> Result<Self, String> {
        let (command_tx, command_rx) = mpsc::channel::<RuntimeCommand>();
        let (event_tx, event_rx) = mpsc::channel::<RuntimeEvent>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let pending_host_request: Arc<Mutex<Option<PendingHostRequest>>> =
            Arc::new(Mutex::new(None));
        let pending_clone = Arc::clone(&pending_host_request);

        let thread_handle = thread::spawn(move || {
            Self::run_loop(command_rx, event_tx, pending_clone, ready_tx, bus_devices);
        });

        match ready_rx.recv_timeout(RUNTIME_INIT_TIMEOUT) {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(e),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Err("Timed out waiting for simulator runtime initialization".to_string());
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("Simulator runtime initialization channel disconnected".to_string());
            }
        }

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
        ready_tx: mpsc::Sender<Result<(), String>>,
        bus_devices: Option<Vec<BusDeviceRegistration>>,
    ) {
        // Create the interactive simulator
        let mut simulator = match InteractiveSimulator::new() {
            Ok(sim) => sim,
            Err(e) => {
                let _ = ready_tx.send(Err(format!("Failed to create simulator: {}", e)));
                let _ = event_tx.send(RuntimeEvent::FatalError(format!(
                    "Failed to create simulator: {}",
                    e
                )));
                return;
            }
        };
        if let Some(bus_devices) = bus_devices {
            for registration in bus_devices {
                if let Err(e) =
                    simulator.register_device(registration.base_addr, registration.device)
                {
                    let message = format!(
                        "Failed to register device at 0x{:08x}: {}",
                        registration.base_addr, e
                    );
                    let _ = ready_tx.send(Err(message.clone()));
                    let _ = event_tx.send(RuntimeEvent::FatalError(message));
                    return;
                }
            }
        }

        // Initialize simulator reset/controller state before serving runtime requests.
        if let Err(e) = simulator.reset() {
            let _ = ready_tx.send(Err(format!(
                "Failed to initialize simulator reset state: {}",
                e
            )));
            let _ = event_tx.send(RuntimeEvent::FatalError(format!(
                "Failed to initialize simulator reset state: {}",
                e
            )));
            return;
        }
        let _ = ready_tx.send(Ok(()));

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
                Ok(RuntimeCommand::Reset(result_tx)) => {
                    let _ = result_tx.send(
                        simulator
                            .reset()
                            .map_err(|e| format!("Reset failed: {}", e)),
                    );
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => break,
            }

            // Step one cycle for fine-grained control
            match simulator.step_cycle() {
                Ok(result) => {
                    if let Some(value) = result.tohost_value {
                        let _ =
                            event_tx.send(RuntimeEvent::Bus(BusEvent::TohostTermination { value }));
                    }
                }
                Err(e) => {
                    let _ =
                        event_tx.send(RuntimeEvent::FatalError(format!("Simulation error: {}", e)));
                    break;
                }
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
                                burst_data: response.data,
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
    fn supports_riscv_f_extension(&self) -> bool {
        true
    }

    fn send_host_request(&mut self, request: BusRequest) -> Result<(), DeviceError> {
        if classify_host_request_route(&request) == HostRequestRoute::InvalidSpanningRegion {
            return Err(DeviceError::HandlerError(HandlerError::InvalidAddressRange));
        }

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

    fn reset(&mut self, kind: ResetKind) -> Result<(), DeviceError> {
        if self.has_pending_host_request() {
            return Err(DeviceError::HandlerError(
                host_bus_handler::HandlerError::RequestPending,
            ));
        }

        match kind {
            ResetKind::Cpu => {
                let reset_addr = sysctrl_reset_addr();
                let request = BusRequest::write(reset_addr, SYSCTRL_RESET_CPU, AccessSize::Word);
                self.send_host_request(request)?;

                loop {
                    match self.poll()? {
                        Some(BusEvent::HostWriteResponse { addr, .. }) if addr == reset_addr => {
                            return Ok(());
                        }
                        Some(BusEvent::HostRequestTimeout { addr }) if addr == reset_addr => {
                            return Err(DeviceError::IoError(std::io::Error::new(
                                std::io::ErrorKind::TimedOut,
                                format!(
                                    "Timed out writing CPU reset to RESET register at 0x{:08x}",
                                    reset_addr
                                ),
                            )));
                        }
                        Some(_) => {}
                        None => thread::sleep(Duration::from_millis(1)),
                    }
                }
            }
            ResetKind::System => {
                let (result_tx, result_rx) = mpsc::channel::<Result<(), String>>();
                self.command_tx
                    .send(RuntimeCommand::Reset(result_tx))
                    .map_err(|e| {
                        DeviceError::IoError(std::io::Error::new(
                            std::io::ErrorKind::BrokenPipe,
                            format!("Background thread disconnected: {}", e),
                        ))
                    })?;

                match result_rx.recv_timeout(RESET_COMMAND_TIMEOUT) {
                    Ok(Ok(())) => Ok(()),
                    Ok(Err(e)) => Err(DeviceError::IoError(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        e,
                    ))),
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        Err(DeviceError::IoError(std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            "Timed out waiting for system reset",
                        )))
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        Err(DeviceError::IoError(std::io::Error::new(
                            std::io::ErrorKind::BrokenPipe,
                            "Background thread terminated during system reset",
                        )))
                    }
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
