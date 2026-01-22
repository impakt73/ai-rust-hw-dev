//! Background simulation thread module.
//!
//! This module provides a background thread for running the CPU simulation,
//! decoupling simulation speed from UI update speed for better performance.
//!
//! The key design principle is to maintain the direct bus device callback flow:
//! - Video/Audio devices in the simulator call their callbacks during instruction execution
//! - These callbacks immediately invoke thread-safe backends
//! - Audio/video data is made available as soon as it's generated (no batching delay)

use cpu_sim::{Audio, AudioConfig, InteractiveSimulator, Video, VideoConfig, AUDIO_BASE, VIDEO_BASE};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

/// Number of instructions to execute per batch in the simulation loop.
/// Adjust this to balance between responsiveness and throughput.
const DEFAULT_BATCH_SIZE: u64 = 1000;

/// Commands sent from the main thread to the simulation thread.
#[derive(Debug)]
pub enum SimCommand {
    /// Load an ELF file and start execution
    LoadELF(PathBuf),
    /// Pause simulation
    Pause,
    /// Resume simulation
    Resume,
    /// Execute exactly N instructions then pause (for deterministic testing)
    StepInstructions(u64),
    /// Terminate the simulation thread
    Terminate,
}

/// Status updates sent from the simulation thread to the main thread.
#[derive(Debug, Clone)]
pub enum SimStatus {
    /// Simulation is idle (no program loaded)
    Idle,
    /// Simulation is running
    Running,
    /// Simulation is paused
    Paused,
    /// Program halted with tohost value
    Halted(u32),
    /// An error occurred
    Error(String),
    /// ELF loaded successfully
    ElfLoaded,
    /// Step completed (instructions executed, frames presented)
    StepCompleted { instructions: u64, frames: u64 },
}

/// Thread-safe video backend trait.
///
/// This trait extends VideoBackend with Send + Sync requirements for use
/// across thread boundaries. Implementations must be thread-safe.
pub trait ThreadSafeVideoBackend: Send + Sync {
    /// Process a video frame from the simulator (called from simulation thread)
    fn process_frame(&self, data: &[u8], config: &VideoConfig) -> Result<(), String>;

    /// Set window title (called from main thread)
    fn set_title(&self, title: &str);

    /// Check if backend is still active
    fn is_active(&self) -> bool;

    /// Update display (called from main thread)
    fn update(&self) -> Result<(), String>;
}

/// Thread-safe audio backend trait.
///
/// This trait extends AudioBackend with Send + Sync requirements for use
/// across thread boundaries. Implementations must be thread-safe.
pub trait ThreadSafeAudioBackend: Send + Sync {
    /// Push audio samples (called from simulation thread)
    fn push_samples(&self, samples: &[i16]);

    /// Update audio configuration (called from simulation thread)
    fn set_config(&self, config: &AudioConfig);
}

/// Configuration for the simulation thread.
#[derive(Clone)]
pub struct SimThreadConfig {
    /// Maximum cycles before auto-termination (0 = unlimited)
    pub max_cycles: u64,
    /// Number of instructions per batch
    pub batch_size: u64,
}

impl Default for SimThreadConfig {
    fn default() -> Self {
        Self {
            max_cycles: 0,
            batch_size: DEFAULT_BATCH_SIZE,
        }
    }
}

/// Shared state between main thread and simulation thread.
struct SharedState {
    /// Frame counter (incremented by video callback)
    frame_count: AtomicU64,
    /// Flag indicating simulation thread is running
    running: AtomicBool,
}

/// Handle to control the background simulation thread.
pub struct SimThread {
    /// Channel to send commands to the simulation thread
    command_tx: Sender<SimCommand>,
    /// Channel to receive status updates from the simulation thread
    status_rx: Receiver<SimStatus>,
    /// Handle to the simulation thread
    thread_handle: Option<JoinHandle<()>>,
    /// Shared state with the simulation thread
    shared: Arc<SharedState>,
    /// Last known status
    last_status: SimStatus,
    /// Thread-safe video backend for main thread access
    video_backend: Arc<dyn ThreadSafeVideoBackend>,
}

impl SimThread {
    /// Create and start a new simulation thread.
    ///
    /// # Arguments
    /// * `config` - Configuration for the simulation thread
    /// * `video_backend` - Thread-safe video backend
    /// * `audio_backend` - Thread-safe audio backend
    ///
    /// # Returns
    /// A handle to control the simulation thread
    pub fn new(
        config: SimThreadConfig,
        video_backend: Arc<dyn ThreadSafeVideoBackend>,
        audio_backend: Arc<dyn ThreadSafeAudioBackend>,
    ) -> Result<Self, String> {
        let (command_tx, command_rx) = mpsc::channel::<SimCommand>();
        let (status_tx, status_rx) = mpsc::channel::<SimStatus>();

        let shared = Arc::new(SharedState {
            frame_count: AtomicU64::new(0),
            running: AtomicBool::new(true),
        });
        let shared_clone = Arc::clone(&shared);

        // Clone backends for the simulation thread
        let video_backend_clone = Arc::clone(&video_backend);
        let audio_backend_clone = Arc::clone(&audio_backend);

        let thread_handle = thread::spawn(move || {
            simulation_thread_main(
                config,
                video_backend_clone,
                audio_backend_clone,
                command_rx,
                status_tx,
                shared_clone,
            );
        });

        Ok(SimThread {
            command_tx,
            status_rx,
            thread_handle: Some(thread_handle),
            shared,
            last_status: SimStatus::Idle,
            video_backend,
        })
    }

    /// Send a command to the simulation thread.
    pub fn send_command(&self, command: SimCommand) -> Result<(), String> {
        self.command_tx
            .send(command)
            .map_err(|e| format!("Failed to send command: {}", e))
    }

    /// Try to receive a status update (non-blocking).
    /// Returns the latest status or None if no new status is available.
    pub fn try_recv_status(&mut self) -> Option<SimStatus> {
        match self.status_rx.try_recv() {
            Ok(status) => {
                self.last_status = status.clone();
                Some(status)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.last_status = SimStatus::Error("Thread disconnected".to_string());
                Some(self.last_status.clone())
            }
        }
    }

    /// Drain all pending status updates and return the last one.
    pub fn drain_status(&mut self) -> Option<SimStatus> {
        let mut last = None;
        while let Some(status) = self.try_recv_status() {
            last = Some(status);
        }
        last
    }

    /// Get the last known status.
    pub fn last_status(&self) -> &SimStatus {
        &self.last_status
    }

    /// Check if the thread is still running.
    pub fn is_running(&self) -> bool {
        self.shared.running.load(Ordering::Relaxed)
    }

    /// Get the current frame count.
    pub fn frame_count(&self) -> u64 {
        self.shared.frame_count.load(Ordering::Relaxed)
    }

    /// Get reference to the video backend (for main thread UI updates).
    pub fn video_backend(&self) -> &Arc<dyn ThreadSafeVideoBackend> {
        &self.video_backend
    }

    /// Terminate the simulation thread and wait for it to finish.
    pub fn terminate(&mut self) {
        // Send terminate command (ignore errors if channel is closed)
        let _ = self.command_tx.send(SimCommand::Terminate);

        // Wait for the thread to finish
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for SimThread {
    fn drop(&mut self) {
        self.terminate();
    }
}

/// Main loop for the simulation thread.
fn simulation_thread_main(
    config: SimThreadConfig,
    video_backend: Arc<dyn ThreadSafeVideoBackend>,
    audio_backend: Arc<dyn ThreadSafeAudioBackend>,
    command_rx: Receiver<SimCommand>,
    status_tx: Sender<SimStatus>,
    shared: Arc<SharedState>,
) {
    // Create the simulator with devices wired to thread-safe backends
    let simulator = match create_simulator_with_backends(
        Arc::clone(&video_backend),
        Arc::clone(&audio_backend),
        Arc::clone(&shared),
    ) {
        Ok(sim) => sim,
        Err(e) => {
            let _ = status_tx.send(SimStatus::Error(e));
            shared.running.store(false, Ordering::Relaxed);
            return;
        }
    };

    let mut state = SimulationState {
        simulator,
        is_running: false,
        is_paused: false,
        total_cycles: 0,
        max_cycles: config.max_cycles,
        batch_size: config.batch_size,
        step_target: None,
    };

    // Send initial status
    let _ = status_tx.send(SimStatus::Idle);

    // Main simulation loop
    loop {
        // Check for commands (non-blocking when running, blocking when paused/idle)
        let command = if state.is_running && !state.is_paused {
            // Non-blocking check when running
            command_rx.try_recv().ok()
        } else {
            // Blocking wait when paused or idle
            match command_rx.recv() {
                Ok(cmd) => Some(cmd),
                Err(_) => {
                    // Channel closed, exit
                    break;
                }
            }
        };

        // Handle command if received
        if let Some(cmd) = command {
            match cmd {
                SimCommand::Terminate => {
                    log::info!("Simulation thread: Terminate command received");
                    break;
                }
                SimCommand::LoadELF(path) => {
                    log::info!("Simulation thread: Loading ELF {:?}", path);
                    match state.simulator.load_elf(&path) {
                        Ok(()) => {
                            state.is_running = true;
                            state.is_paused = false;
                            state.total_cycles = 0;
                            shared.frame_count.store(0, Ordering::Relaxed);
                            let _ = status_tx.send(SimStatus::ElfLoaded);
                            let _ = status_tx.send(SimStatus::Running);
                        }
                        Err(e) => {
                            let _ = status_tx.send(SimStatus::Error(format!(
                                "Failed to load ELF: {}",
                                e
                            )));
                        }
                    }
                }
                SimCommand::Pause => {
                    if state.is_running {
                        state.is_paused = true;
                        let _ = status_tx.send(SimStatus::Paused);
                        log::info!("Simulation thread: Paused");
                    }
                }
                SimCommand::Resume => {
                    if state.is_running && state.is_paused {
                        state.is_paused = false;
                        let _ = status_tx.send(SimStatus::Running);
                        log::info!("Simulation thread: Resumed");
                    }
                }
                SimCommand::StepInstructions(count) => {
                    log::info!("Simulation thread: Step {} instructions", count);
                    state.step_target = Some(state.total_cycles + count);
                    if state.is_paused {
                        state.is_paused = false;
                        let _ = status_tx.send(SimStatus::Running);
                    }
                }
            }
        }

        // Execute simulation if running and not paused
        if state.is_running && !state.is_paused {
            // Determine how many instructions to execute this batch
            let batch_count = if let Some(target) = state.step_target {
                let remaining = target.saturating_sub(state.total_cycles);
                if remaining == 0 {
                    // Step target reached, pause
                    state.is_paused = true;
                    state.step_target = None;
                    let _ = status_tx.send(SimStatus::StepCompleted {
                        instructions: state.total_cycles,
                        frames: shared.frame_count.load(Ordering::Relaxed),
                    });
                    let _ = status_tx.send(SimStatus::Paused);
                    continue;
                }
                remaining.min(state.batch_size)
            } else {
                state.batch_size
            };

            // Execute batch of instructions
            let mut halted = false;
            let mut tohost_value = 0u32;

            for _ in 0..batch_count {
                match state.simulator.step_instruction() {
                    Ok(result) => {
                        state.total_cycles += 1;

                        if let Some(tohost) = result.tohost_value {
                            halted = true;
                            tohost_value = tohost;
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = status_tx.send(SimStatus::Error(e));
                        state.is_running = false;
                        break;
                    }
                }
            }

            // Check for halt or max cycles
            if halted {
                log::info!(
                    "Simulation thread: Halted with tohost=0x{:08x}",
                    tohost_value
                );
                state.is_running = false;
                let _ = status_tx.send(SimStatus::Halted(tohost_value));
            } else if state.max_cycles > 0 && state.total_cycles >= state.max_cycles {
                log::info!(
                    "Simulation thread: Max cycles reached ({})",
                    state.total_cycles
                );
                state.is_running = false;
                let _ = status_tx.send(SimStatus::Halted(0));
            }
        }
    }

    shared.running.store(false, Ordering::Relaxed);
    log::info!("Simulation thread: Exiting");
}

/// Internal state for the simulation thread.
struct SimulationState {
    simulator: InteractiveSimulator,
    is_running: bool,
    is_paused: bool,
    total_cycles: u64,
    max_cycles: u64,
    batch_size: u64,
    step_target: Option<u64>,
}

/// Create the simulator with audio/video devices wired to thread-safe backends.
///
/// The devices use callbacks that directly invoke the thread-safe backends,
/// ensuring audio/video data is available immediately when generated.
fn create_simulator_with_backends(
    video_backend: Arc<dyn ThreadSafeVideoBackend>,
    audio_backend: Arc<dyn ThreadSafeAudioBackend>,
    shared: Arc<SharedState>,
) -> Result<InteractiveSimulator, String> {
    let mut simulator = InteractiveSimulator::new()?;

    // Create Video device with callback that writes to thread-safe backend
    let video_backend_for_callback = Arc::clone(&video_backend);
    let shared_for_callback = Arc::clone(&shared);
    let video_callback = move |data: &[u8], config: &VideoConfig| {
        // Directly invoke the thread-safe backend
        if let Err(e) = video_backend_for_callback.process_frame(data, config) {
            log::error!("Video backend process_frame error: {}", e);
        } else {
            // Increment frame count on success
            shared_for_callback.frame_count.fetch_add(1, Ordering::Relaxed);
        }
    };
    let video_device = Box::new(Video::new(Some(video_callback)));

    // Create Audio device with callbacks that write to thread-safe backend
    let audio_backend_for_samples = Arc::clone(&audio_backend);
    let sample_callback = move |samples: &[i16]| {
        // Directly invoke the thread-safe backend
        audio_backend_for_samples.push_samples(samples);
    };

    let audio_backend_for_config = Arc::clone(&audio_backend);
    let config_callback = move |config: &AudioConfig| {
        log::info!(
            "Audio config changed: {} Hz, {:?}, {} samples",
            config.sample_rate.to_hz(),
            config.channels,
            config.sample_count
        );
        // Directly invoke the thread-safe backend
        audio_backend_for_config.set_config(config);
    };
    let audio_device = Box::new(Audio::new(Some(sample_callback), Some(config_callback)));

    // Register devices with simulator
    simulator.register_device(VIDEO_BASE, video_device)?;
    simulator.register_device(AUDIO_BASE, audio_device)?;

    Ok(simulator)
}
