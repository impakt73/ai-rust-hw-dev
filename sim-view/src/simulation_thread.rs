//! Background simulation thread module
//!
//! This module contains the simulation thread implementation that runs the
//! RISC-V simulator in a separate thread, decoupled from the UI thread.

use cpu_sim::InteractiveSimulator;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Instant;

// Performance constants
const INSTRUCTIONS_PER_BATCH: u64 = 10000; // Instructions per batch in background thread
const BATCHES_PER_PROGRESS_UPDATE: u64 = 10; // Send progress update every 10 batches (~100K instructions)

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
    /// Execute a single batch of instructions
    Step,
    /// Pause the simulation
    Pause,
    /// Resume the simulation
    Resume,
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
    /// Step completed
    StepCompleted {
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
        simulator: InteractiveSimulator,
        max_cycles: u64,
        frame_timing: Arc<Mutex<FrameTimingMetrics>>,
    ) -> Result<Self, String> {
        let (request_tx, request_rx) = mpsc::channel();
        let (response_tx, response_rx) = mpsc::channel();

        let thread_handle = thread::spawn(move || {
            Self::simulation_thread_main(
                simulator,
                request_rx,
                response_tx,
                max_cycles,
                frame_timing,
            );
        });

        Ok(SimulationThread {
            thread_handle: Some(thread_handle),
            request_tx,
            response_rx,
        })
    }

    /// Main loop for the simulation thread
    fn simulation_thread_main(
        mut simulator: InteractiveSimulator,
        request_rx: Receiver<SimRequest>,
        response_tx: Sender<SimResponse>,
        max_cycles: u64,
        frame_timing: Arc<Mutex<FrameTimingMetrics>>,
    ) {
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
                        match simulator.load_elf(&path) {
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
                    SimRequest::Step => {
                        // Execute one batch and respond immediately
                        // By default preserve the current running state; it may be cleared below on halt, max-cycles, or error
                        match Self::execute_batch(&mut simulator, INSTRUCTIONS_PER_BATCH) {
                            Ok((cycles, tohost)) => {
                                total_cycles += cycles;
                                let _ = response_tx.send(SimResponse::StepCompleted {
                                    tohost_value: tohost,
                                    cycles_executed: cycles,
                                });

                                // If program halted, stop running
                                if tohost.is_some() {
                                    running = false;
                                }

                                // Check max cycles
                                if max_cycles > 0 && total_cycles >= max_cycles {
                                    running = false;
                                }
                            }
                            Err(e) => {
                                let _ = response_tx.send(SimResponse::Error(e));
                                running = false;
                            }
                        }
                        // Don't execute in continuous mode this iteration since we just stepped
                        continue;
                    }
                    SimRequest::Pause => {
                        running = false;
                    }
                    SimRequest::Resume => {
                        running = true;
                    }
                    SimRequest::Terminate => {
                        let _ = response_tx.send(SimResponse::Terminated);
                        break;
                    }
                }
            }

            // Execute simulation if running (and no Step request was just handled)
            if running {
                match Self::execute_batch(&mut simulator, INSTRUCTIONS_PER_BATCH) {
                    Ok((cycles, tohost)) => {
                        total_cycles += cycles;
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

                        // Check if program halted
                        if tohost.is_some() {
                            running = false;
                            let _ = response_tx.send(SimResponse::RunCompleted {
                                tohost_value: tohost,
                                cycles_executed: total_cycles,
                            });
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
                    Err(e) => {
                        running = false;
                        let _ = response_tx.send(SimResponse::Error(e));
                    }
                }
            }
        }
    }

    /// Execute a batch of instructions
    fn execute_batch(
        simulator: &mut InteractiveSimulator,
        count: u64,
    ) -> Result<(u64, Option<u32>), String> {
        for i in 0..count {
            let result = simulator.step_instruction()?;

            // If program terminated, return early
            if let Some(tohost) = result.tohost_value {
                return Ok((i + 1, Some(tohost)));
            }
        }

        Ok((count, None))
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
