//! Background simulation thread for decoupling simulation speed from UI update speed.
//!
//! This module provides `SimulationThread`, which runs the CPU simulation in a background
//! thread while allowing the main thread to focus on UI updates and event handling.
//!
//! Key features:
//! - Fast simulation loop running in batches of instructions
//! - Channel-based command/response communication with main thread
//! - Direct callback to backends for audio/video (no waiting for UI loop)
//! - Single-step mode for deterministic headless testing
//!
//! # Thread Safety Design
//!
//! The simulation runs in a background thread while the main thread handles UI. Communication
//! happens through:
//! - Command channel (main → sim): Load ELF, Pause, Resume, SingleStep, Terminate
//! - State channel (sim → main): Status updates, step completions
//! - Callbacks (sim → backends): Direct calls for audio samples and video frames
//!
//! The callbacks are `Arc<Mutex<dyn FnMut + Send>>` allowing thread-safe access from
//! the background thread to the backend data structures.

use cpu_sim::{
    Audio, AudioConfig, InteractiveSimulator, Video, VideoConfig, AUDIO_BASE, VIDEO_BASE,
};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

/// Number of instructions to execute per batch in the background thread
const INSTRUCTIONS_PER_BATCH: u64 = 1000;

/// Commands sent from the main thread to the simulation thread
#[derive(Debug)]
pub enum SimCommand {
    /// Load an ELF file and reset the simulation
    LoadELF(PathBuf),
    /// Pause the simulation
    Pause,
    /// Resume the simulation
    Resume,
    /// Execute a single step (for headless testing)
    SingleStep,
    /// Terminate the simulation thread
    Terminate,
}

/// State updates sent from the simulation thread to the main thread
#[derive(Debug, Clone)]
pub enum SimState {
    /// Simulation is idle (no program loaded)
    Idle,
    /// Simulation is running
    Running,
    /// Simulation is paused
    Paused,
    /// Simulation has halted (program completed with tohost value)
    Halted(u32),
    /// An error occurred
    Error(String),
    /// ELF file loaded successfully
    ELFLoaded,
    /// A single step completed (used for headless testing)
    StepCompleted {
        /// Number of instructions executed
        instructions: u64,
        /// Whether a frame was presented during this step
        frame_presented: bool,
        /// Tohost value if program halted
        tohost: Option<u32>,
    },
}

/// Callback type for video frame presentation.
/// Called directly from the background thread when a frame is ready.
pub type VideoCallback = Arc<Mutex<dyn FnMut(&[u8], &VideoConfig) + Send>>;

/// Callback type for audio sample delivery.
/// Called directly from the background thread when audio samples are ready.
pub type AudioSampleCallback = Arc<Mutex<dyn FnMut(&[i16]) + Send>>;

/// Callback type for audio configuration changes.
/// Called directly from the background thread when audio config changes.
pub type AudioConfigCallback = Arc<Mutex<dyn FnMut(&AudioConfig) + Send>>;

/// Background simulation thread handle.
///
/// This struct manages the background thread that runs the CPU simulation.
/// It provides methods for sending commands to the thread and receiving
/// state updates.
pub struct SimulationThread {
    /// Thread handle
    thread_handle: Option<JoinHandle<()>>,
    /// Command sender
    cmd_sender: Sender<SimCommand>,
    /// State receiver
    state_receiver: Receiver<SimState>,
    /// Current state (cached)
    current_state: SimState,
    /// Whether single-step mode is enabled (for headless testing)
    single_step_mode: bool,
    /// Total cycles executed (tracked from step responses)
    total_cycles: u64,
    /// Frame count (tracked from step responses)
    frame_count: u64,
}

impl SimulationThread {
    /// Create and spawn a new simulation thread.
    ///
    /// The callbacks are called directly from the background thread when
    /// video frames or audio samples become available, ensuring minimal latency.
    ///
    /// # Arguments
    /// * `video_callback` - Called when a video frame is ready to display
    /// * `audio_sample_callback` - Called when audio samples are ready for playback
    /// * `audio_config_callback` - Called when audio configuration changes
    ///
    /// # Thread Safety
    /// All callbacks are wrapped in `Arc<Mutex<>>` to allow safe access from
    /// the background thread. The mutex is held only for the duration of the
    /// callback invocation.
    pub fn new(
        video_callback: VideoCallback,
        audio_sample_callback: AudioSampleCallback,
        audio_config_callback: AudioConfigCallback,
    ) -> Result<Self, String> {
        // Create channels for communication
        let (cmd_sender, cmd_receiver) = mpsc::channel::<SimCommand>();
        let (state_sender, state_receiver) = mpsc::channel::<SimState>();

        // Spawn the background thread
        let thread_handle = thread::spawn(move || {
            simulation_loop(
                cmd_receiver,
                state_sender,
                video_callback,
                audio_sample_callback,
                audio_config_callback,
            );
        });

        Ok(SimulationThread {
            thread_handle: Some(thread_handle),
            cmd_sender,
            state_receiver,
            current_state: SimState::Idle,
            single_step_mode: false,
            total_cycles: 0,
            frame_count: 0,
        })
    }

    /// Enable or disable single-step mode.
    ///
    /// In single-step mode, the simulation thread waits for SingleStep commands
    /// instead of running continuously. This is useful for deterministic testing
    /// where tests need to control exactly how many instruction batches execute.
    pub fn set_single_step_mode(&mut self, enabled: bool) {
        self.single_step_mode = enabled;
        // When enabling single-step mode, pause the simulation
        if enabled {
            let _ = self.cmd_sender.send(SimCommand::Pause);
        }
    }

    /// Check if single-step mode is enabled.
    pub fn is_single_step_mode(&self) -> bool {
        self.single_step_mode
    }

    /// Send a command to the simulation thread.
    pub fn send_command(&self, cmd: SimCommand) -> Result<(), String> {
        self.cmd_sender
            .send(cmd)
            .map_err(|e| format!("Failed to send command: {}", e))
    }

    /// Load an ELF file.
    pub fn load_elf(&self, path: &PathBuf) -> Result<(), String> {
        self.send_command(SimCommand::LoadELF(path.clone()))
    }

    /// Pause the simulation.
    pub fn pause(&self) -> Result<(), String> {
        self.send_command(SimCommand::Pause)
    }

    /// Resume the simulation.
    pub fn resume(&self) -> Result<(), String> {
        self.send_command(SimCommand::Resume)
    }

    /// Request a single step (for headless testing).
    ///
    /// This executes one batch of instructions (INSTRUCTIONS_PER_BATCH) and
    /// sends a StepCompleted state update.
    pub fn single_step(&self) -> Result<(), String> {
        self.send_command(SimCommand::SingleStep)
    }

    /// Poll for state updates (non-blocking).
    ///
    /// Returns the latest state if available, or None if no updates.
    /// Also updates internal tracking of cycles and frames.
    pub fn poll_state(&mut self) -> Option<SimState> {
        match self.state_receiver.try_recv() {
            Ok(state) => {
                // Track cycles and frames from step completions
                if let SimState::StepCompleted {
                    instructions,
                    frame_presented,
                    ..
                } = &state
                {
                    self.total_cycles += instructions;
                    if *frame_presented {
                        self.frame_count += 1;
                    }
                }
                self.current_state = state.clone();
                Some(state)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.current_state = SimState::Error("Thread disconnected".to_string());
                Some(self.current_state.clone())
            }
        }
    }

    /// Drain all pending state updates and return the final state.
    pub fn drain_states(&mut self) -> SimState {
        while self.poll_state().is_some() {
            // Keep polling until empty
        }
        self.current_state.clone()
    }

    /// Get the current cached state.
    pub fn current_state(&self) -> &SimState {
        &self.current_state
    }

    /// Get total cycles executed.
    pub fn total_cycles(&self) -> u64 {
        self.total_cycles
    }

    /// Get frame count.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Reset cycle and frame counters (called after ELF load).
    pub fn reset_counters(&mut self) {
        self.total_cycles = 0;
        self.frame_count = 0;
    }

    /// Terminate the simulation thread and wait for it to exit.
    pub fn terminate(&mut self) -> Result<(), String> {
        // Send terminate command
        let _ = self.cmd_sender.send(SimCommand::Terminate);

        // Wait for thread to exit
        if let Some(handle) = self.thread_handle.take() {
            handle.join().map_err(|_| "Thread panicked".to_string())?;
        }

        Ok(())
    }
}

impl Drop for SimulationThread {
    fn drop(&mut self) {
        let _ = self.terminate();
    }
}

/// Main simulation loop running in the background thread.
///
/// This function runs the CPU simulation, processing commands from the main thread
/// and executing instructions in batches when running.
fn simulation_loop(
    cmd_receiver: Receiver<SimCommand>,
    state_sender: Sender<SimState>,
    video_callback: VideoCallback,
    audio_sample_callback: AudioSampleCallback,
    audio_config_callback: AudioConfigCallback,
) {
    // Create the simulator
    let mut simulator = match InteractiveSimulator::new() {
        Ok(sim) => sim,
        Err(e) => {
            let _ = state_sender.send(SimState::Error(e));
            return;
        }
    };

    // Track whether a frame was presented during the current batch
    let frame_presented = Arc::new(Mutex::new(false));

    // Create Video device with callback that invokes the external video callback
    let video_callback_clone = Arc::clone(&video_callback);
    let frame_presented_clone = Arc::clone(&frame_presented);
    let video_cb = move |data: &[u8], config: &VideoConfig| {
        log::debug!(
            "Video callback invoked: {}x{} {:?}, {} bytes",
            config.width,
            config.height,
            config.format,
            data.len()
        );
        if let Ok(mut cb) = video_callback_clone.lock() {
            cb(data, config);
            if let Ok(mut flag) = frame_presented_clone.lock() {
                *flag = true;
            }
        }
    };
    let video_device = Box::new(Video::new(Some(video_cb)));

    // Create Audio device with callbacks that invoke the external audio callbacks
    let audio_sample_callback_clone = Arc::clone(&audio_sample_callback);
    let sample_cb = move |samples: &[i16]| {
        if let Ok(mut cb) = audio_sample_callback_clone.lock() {
            cb(samples);
        }
    };

    let audio_config_callback_clone = Arc::clone(&audio_config_callback);
    let config_cb = move |config: &AudioConfig| {
        if let Ok(mut cb) = audio_config_callback_clone.lock() {
            cb(config);
        }
    };
    let audio_device = Box::new(Audio::new(Some(sample_cb), Some(config_cb)));

    // Register devices with the simulator
    if let Err(e) = simulator.register_device(VIDEO_BASE, video_device) {
        let _ = state_sender.send(SimState::Error(e));
        return;
    }
    if let Err(e) = simulator.register_device(AUDIO_BASE, audio_device) {
        let _ = state_sender.send(SimState::Error(e));
        return;
    }

    // Simulation state
    let mut running = false;
    let mut elf_loaded = false;

    // Send initial state
    let _ = state_sender.send(SimState::Idle);

    loop {
        // Process commands (non-blocking when running, blocking when paused/idle)
        let recv_result = if running {
            // Non-blocking check when running
            match cmd_receiver.try_recv() {
                Ok(cmd) => Ok(cmd),
                Err(TryRecvError::Empty) => Err(()),
                Err(TryRecvError::Disconnected) => {
                    // Channel disconnected, exit
                    break;
                }
            }
        } else {
            // Blocking wait when not running
            cmd_receiver.recv().map_err(|_| ())
        };

        if let Ok(cmd) = recv_result {
            match cmd {
                SimCommand::LoadELF(path) => {
                    match simulator.load_elf(&path) {
                        Ok(()) => {
                            elf_loaded = true;
                            running = false; // Start paused after loading
                            let _ = state_sender.send(SimState::ELFLoaded);
                            let _ = state_sender.send(SimState::Paused);
                        }
                        Err(e) => {
                            let _ = state_sender.send(SimState::Error(e));
                        }
                    }
                }
                SimCommand::Pause => {
                    running = false;
                    let _ = state_sender.send(SimState::Paused);
                }
                SimCommand::Resume => {
                    if elf_loaded {
                        running = true;
                        log::info!("Simulation thread: resuming execution");
                        let _ = state_sender.send(SimState::Running);
                    }
                }
                SimCommand::SingleStep => {
                    if elf_loaded {
                        execute_batch(
                            &mut simulator,
                            &frame_presented,
                            &state_sender,
                            &mut running,
                        );
                    }
                }
                SimCommand::Terminate => {
                    break;
                }
            }
        } else if !running {
            // If we're paused and recv returned error (disconnected), exit
            break;
        }

        // Execute simulation batch if running
        if running && elf_loaded {
            execute_batch(
                &mut simulator,
                &frame_presented,
                &state_sender,
                &mut running,
            );
        }
    }
}

/// Execute a batch of instructions and send a step completion.
fn execute_batch(
    simulator: &mut InteractiveSimulator,
    frame_presented: &Arc<Mutex<bool>>,
    state_sender: &Sender<SimState>,
    running: &mut bool,
) {
    // Reset frame presented flag
    if let Ok(mut flag) = frame_presented.lock() {
        *flag = false;
    }

    let mut instructions = 0u64;
    let mut tohost = None;

    for _ in 0..INSTRUCTIONS_PER_BATCH {
        match simulator.step_instruction() {
            Ok(result) => {
                instructions += 1;
                if result.tohost_value.is_some() {
                    tohost = result.tohost_value;
                    log::debug!("Simulation: tohost triggered after {} instructions", instructions);
                    break;
                }
            }
            Err(e) => {
                log::error!("Simulation error: {}", e);
                let _ = state_sender.send(SimState::Error(e));
                *running = false;
                return;
            }
        }
    }

    let frame_was_presented = frame_presented.lock().map(|f| *f).unwrap_or(false);
    
    log::debug!(
        "Batch complete: {} instructions, frame_presented={}",
        instructions,
        frame_was_presented
    );

    // Send step completion (for tracking cycles/frames)
    let _ = state_sender.send(SimState::StepCompleted {
        instructions,
        frame_presented: frame_was_presented,
        tohost,
    });

    // If halted, update state
    if let Some(value) = tohost {
        *running = false;
        let _ = state_sender.send(SimState::Halted(value));
    }
}
