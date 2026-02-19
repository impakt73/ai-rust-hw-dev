//! Background simulation thread module
//!
//! This module contains the simulation thread implementation that runs the
//! RISC-V simulator in a separate thread, decoupled from the UI thread.

use device_runtime::{
    create_device_runtime, BusDeviceRegistration, BusEvent, DeviceRuntime, DeviceRuntimeType,
    ResetKind,
};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Instant;

// Performance constants
const BATCHES_PER_PROGRESS_UPDATE: u64 = 10; // Send progress update every 10 poll batches

/// Shared frame timing metrics tracked across simulation thread and video callback
#[derive(Debug, Clone, Default)]
pub(crate) struct FrameTimingMetrics {
    /// Total number of frames presented
    pub frames_presented: u64,
    /// Total time between frame presentations (nanoseconds)
    pub total_frame_time_ns: u64,
    /// Timestamp of last frame presentation
    pub last_frame_time: Option<Instant>,
}

/// Messages sent from main thread to simulation thread
#[derive(Debug)]
pub(crate) enum SimRequest {
    /// Load an ELF file into the simulator
    LoadELF(PathBuf),
    /// Start running the simulation continuously
    Run,
    /// Terminate the simulation thread
    Terminate,
}

/// Messages sent from simulation thread to main thread
#[derive(Debug)]
pub(crate) enum SimResponse {
    /// ELF loaded successfully
    ELFLoaded,
    /// Error occurred
    Error(String),
    /// Run completed (program halted or max cycles reached)
    RunCompleted {
        tohost_value: Option<u32>,
        cycles_executed: u64,
    },
    /// Progress update during continuous run (sent periodically)
    Progress {
        cycles_executed: u64,
        frames_presented: u64,
        total_frame_time_ns: u64,
    },
    /// Simulation thread terminated
    Terminated,
}

/// Simulation thread handle and communication channels
pub(crate) struct SimulationThread {
    /// Handle to the background thread
    thread_handle: Option<JoinHandle<()>>,
    /// Channel to send requests to simulation thread
    request_tx: Sender<SimRequest>,
    /// Channel to receive responses from simulation thread
    response_rx: Receiver<SimResponse>,
}

impl SimulationThread {
    /// Create a new simulation thread with the given simulator
    pub(crate) fn new(
        registrations: Vec<BusDeviceRegistration>,
        max_cycles: u64,
        frame_timing: Arc<Mutex<FrameTimingMetrics>>,
    ) -> Result<Self, String> {
        let (request_tx, request_rx) = mpsc::channel();
        let (response_tx, response_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();

        let thread_handle = thread::spawn(move || {
            Self::simulation_thread_main(
                registrations,
                request_rx,
                response_tx,
                ready_tx,
                max_cycles,
                frame_timing,
            );
        });

        match ready_rx.recv() {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(e),
            Err(e) => {
                return Err(format!(
                    "Failed to receive simulation thread initialization status: {}",
                    e
                ));
            }
        }

        Ok(SimulationThread {
            thread_handle: Some(thread_handle),
            request_tx,
            response_rx,
        })
    }

    /// Main loop for the simulation thread
    fn simulation_thread_main(
        registrations: Vec<BusDeviceRegistration>,
        request_rx: Receiver<SimRequest>,
        response_tx: Sender<SimResponse>,
        ready_tx: Sender<Result<(), String>>,
        max_cycles: u64,
        frame_timing: Arc<Mutex<FrameTimingMetrics>>,
    ) {
        let mut runtime = match create_device_runtime(DeviceRuntimeType::Sim, Some(registrations)) {
            Ok(runtime) => runtime,
            Err(e) => {
                let _ = ready_tx.send(Err(format!("Failed to create device runtime: {}", e)));
                return;
            }
        };
        let _ = ready_tx.send(Ok(()));

        let mut total_cycles: u64 = 0;
        let mut running = false;
        let mut batch_count: u64 = 0; // Track batches for progress updates

        loop {
            // Check for requests from main thread
            let request = if running {
                // Non-blocking check when running
                match request_rx.try_recv() {
                    Ok(req) => Some(req),
                    Err(mpsc::TryRecvError::Empty) => None,
                    Err(mpsc::TryRecvError::Disconnected) => break,
                }
            } else {
                // Blocking wait when paused/idle
                match request_rx.recv() {
                    Ok(req) => Some(req),
                    Err(_) => break,
                }
            };

            // Handle request if any
            if let Some(request) = request {
                match request {
                    SimRequest::LoadELF(path) => {
                        match Self::load_elf_into_runtime(runtime.as_mut(), &path) {
                            Ok(_entry_point) => {
                                total_cycles = 0;
                                batch_count = 0;
                                running = false; // Don't auto-start

                                // Reset frame timing metrics for the new program
                                if let Ok(mut timing) = frame_timing.lock() {
                                    *timing = FrameTimingMetrics::default();
                                }

                                let _ = response_tx.send(SimResponse::ELFLoaded);
                            }
                            Err(e) => {
                                let _ = response_tx.send(SimResponse::Error(e));
                            }
                        }
                    }
                    SimRequest::Run => {
                        running = true;
                        batch_count = 0; // Reset batch counter when starting

                        // Reset frame timing metrics so measurements start fresh for this run
                        if let Ok(mut timing) = frame_timing.lock() {
                            *timing = FrameTimingMetrics::default();
                        }
                    }
                    SimRequest::Terminate => {
                        let _ = response_tx.send(SimResponse::Terminated);
                        break;
                    }
                }
            }

            // Execute simulation if running (and no Step request was just handled)
            if running {
                match runtime.poll() {
                    Ok(Some(BusEvent::TohostTermination { value })) => {
                        running = false;
                        let _ = response_tx.send(SimResponse::RunCompleted {
                            tohost_value: Some(value),
                            cycles_executed: total_cycles,
                        });
                    }
                    Ok(Some(_)) => {
                        // Ignore other bus events in sim-view's main runtime loop.
                    }
                    Ok(None) => {
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                    Err(e) => {
                        running = false;
                        let _ = response_tx.send(SimResponse::Error(format!(
                            "Device runtime polling failed: {}",
                            e
                        )));
                    }
                }

                total_cycles += 1;
                batch_count += 1;

                // Send periodic progress updates
                if batch_count >= BATCHES_PER_PROGRESS_UPDATE {
                    // Get current frame timing metrics
                    let (frames, frame_time_ns) = if let Ok(metrics) = frame_timing.lock() {
                        (metrics.frames_presented, metrics.total_frame_time_ns)
                    } else {
                        (0, 0)
                    };

                    let _ = response_tx.send(SimResponse::Progress {
                        cycles_executed: total_cycles,
                        frames_presented: frames,
                        total_frame_time_ns: frame_time_ns,
                    });
                    batch_count = 0;
                }

                // Check if max cycles reached
                if max_cycles > 0 && total_cycles >= max_cycles {
                    running = false;
                    let _ = response_tx.send(SimResponse::RunCompleted {
                        tohost_value: None,
                        cycles_executed: total_cycles,
                    });
                }
            }
        }
    }

    /// Load an ELF file into the runtime and boot the CPU at the entry point.
    fn load_elf_into_runtime(runtime: &mut dyn DeviceRuntime, path: &Path) -> Result<u32, String> {
        runtime
            .reset(ResetKind::System)
            .map_err(|e| format!("Reset failed before ELF load: {}", e))?;
        let entry_point = runtime
            .load_elf(path)
            .map_err(|e| format!("Failed to load ELF: {}", e))?;
        runtime
            .boot_cpu(entry_point)
            .map_err(|e| format!("Failed to boot CPU: {}", e))?;
        Ok(entry_point)
    }

    /// Send a request to the simulation thread
    pub(crate) fn send_request(&self, request: SimRequest) -> Result<(), String> {
        self.request_tx
            .send(request)
            .map_err(|e| format!("Failed to send request: {}", e))
    }

    /// Try to receive a response from the simulation thread (non-blocking)
    pub(crate) fn try_recv_response(&self) -> Result<Option<SimResponse>, String> {
        match self.response_rx.try_recv() {
            Ok(response) => Ok(Some(response)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("Simulation thread disconnected".to_string())
            }
        }
    }

    /// Wait for a response from the simulation thread (blocking)
    pub(crate) fn recv_response(&self) -> Result<SimResponse, String> {
        self.response_rx
            .recv()
            .map_err(|e| format!("Failed to receive response: {}", e))
    }
}

impl Drop for SimulationThread {
    fn drop(&mut self) {
        // Send terminate request
        let _ = self.request_tx.send(SimRequest::Terminate);

        // Wait for thread to finish
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}
