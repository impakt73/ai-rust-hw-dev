//! Background simulation thread for decoupled execution.
//!
//! This module provides a threaded simulation architecture that runs the CPU
//! simulator on a background thread, communicating with the main GUI thread
//! via channels. This design ensures:
//!
//! 1. Maximum simulation performance (no blocking on GUI)
//! 2. Smooth GUI updates (no blocking on simulation)
//! 3. Immediate data availability via callbacks
//!
//! Data flow is push-based: video frames and audio samples are delivered
//! directly to backends via callbacks, avoiding the delays of a pull model.

use cpu_sim::{
    Audio, AudioConfig, InteractiveSimulator, Video, VideoConfig, AUDIO_BASE, VIDEO_BASE,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

/// Commands sent from the main thread to the simulation thread
#[derive(Debug)]
pub enum SimCommand {
    /// Load an ELF file and start simulation
    LoadElf(PathBuf),
    /// Pause simulation
    Pause,
    /// Resume simulation
    Resume,
    /// Terminate simulation thread
    Terminate,
}

/// Notifications sent from the simulation thread to the main thread
#[derive(Debug, Clone)]
pub enum SimNotification {
    /// ELF loaded successfully
    ElfLoaded,
    /// ELF load failed with error message
    ElfLoadError(String),
    /// Simulation paused
    Paused,
    /// Simulation resumed
    Resumed,
    /// Program halted with tohost value
    Halted(u32),
    /// Simulation error occurred
    Error(String),
}

/// Current state of the simulation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimState {
    /// No program loaded
    Idle,
    /// Program running
    Running,
    /// Program paused
    Paused,
    /// Program halted (terminated)
    Halted,
}

/// Video frame data shared between simulation and main thread
#[derive(Debug, Clone)]
pub struct VideoFrame {
    pub data: Vec<u8>,
    pub config: VideoConfig,
}

/// Audio data shared between simulation and main thread
#[derive(Debug, Clone)]
#[allow(dead_code)] // May be used in future API expansions
pub struct AudioData {
    pub samples: Vec<i16>,
}

/// Shared state between simulation thread and main thread.
///
/// This allows push-based data delivery: the simulation callback pushes
/// data directly into shared state, and the main thread reads it.
pub struct SharedSimState {
    /// Current simulation state
    pub state: Mutex<SimState>,
    /// Pending video frame (if any)
    pub video_frame: Mutex<Option<VideoFrame>>,
    /// Pending audio samples
    pub audio_samples: Mutex<Vec<i16>>,
    /// Audio configuration (set when config changes)
    pub audio_config: Mutex<Option<AudioConfig>>,
    /// Request to stop simulation (atomic for quick checking)
    stop_requested: AtomicBool,
    /// Request to pause simulation
    pause_requested: AtomicBool,
}

impl SharedSimState {
    /// Create new shared simulation state
    pub fn new() -> Self {
        Self {
            state: Mutex::new(SimState::Idle),
            video_frame: Mutex::new(None),
            audio_samples: Mutex::new(Vec::new()),
            audio_config: Mutex::new(None),
            stop_requested: AtomicBool::new(false),
            pause_requested: AtomicBool::new(false),
        }
    }

    /// Request simulation to stop
    pub fn request_stop(&self) {
        self.stop_requested.store(true, Ordering::SeqCst);
    }

    /// Check if stop is requested
    pub fn is_stop_requested(&self) -> bool {
        self.stop_requested.load(Ordering::SeqCst)
    }

    /// Request simulation to pause
    pub fn request_pause(&self) {
        self.pause_requested.store(true, Ordering::SeqCst);
    }

    /// Request simulation to resume
    pub fn request_resume(&self) {
        self.pause_requested.store(false, Ordering::SeqCst);
    }

    /// Check if pause is requested
    pub fn is_pause_requested(&self) -> bool {
        self.pause_requested.load(Ordering::SeqCst)
    }

    /// Reset stop/pause flags
    pub fn reset_flags(&self) {
        self.stop_requested.store(false, Ordering::SeqCst);
        self.pause_requested.store(false, Ordering::SeqCst);
    }

    /// Push a video frame (called by simulation callback)
    pub fn push_video_frame(&self, data: Vec<u8>, config: VideoConfig) {
        let mut frame = self.video_frame.lock().unwrap();
        // Replace any existing pending frame (we only keep the latest)
        *frame = Some(VideoFrame { data, config });
    }

    /// Take the pending video frame (called by main thread)
    pub fn take_video_frame(&self) -> Option<VideoFrame> {
        self.video_frame.lock().unwrap().take()
    }

    /// Push audio samples (called by simulation callback)
    pub fn push_audio_samples(&self, samples: &[i16]) {
        let mut audio = self.audio_samples.lock().unwrap();
        audio.extend_from_slice(samples);
        // Limit buffer size to prevent unbounded growth.
        // This is approximately 0.5 seconds of stereo audio at 48kHz (the highest supported rate).
        // At lower sample rates or mono audio, this provides even more buffer time.
        // If the main thread can't consume fast enough, older samples are dropped.
        const MAX_AUDIO_SAMPLES: usize = 48000;
        if audio.len() > MAX_AUDIO_SAMPLES {
            let drain_count = audio.len() - MAX_AUDIO_SAMPLES;
            log::warn!(
                "Audio buffer overflow: dropping {} samples (buffer at {} samples)",
                drain_count,
                audio.len()
            );
            audio.drain(..drain_count);
        }
    }

    /// Take pending audio samples (called by main thread)
    pub fn take_audio_samples(&self, max_samples: usize) -> Vec<i16> {
        let mut audio = self.audio_samples.lock().unwrap();
        let count = audio.len().min(max_samples);
        audio.drain(..count).collect()
    }

    /// Set audio configuration (called by simulation callback)
    pub fn set_audio_config(&self, config: AudioConfig) {
        let mut cfg = self.audio_config.lock().unwrap();
        *cfg = Some(config);
    }

    /// Take pending audio configuration change (called by main thread)
    pub fn take_audio_config(&self) -> Option<AudioConfig> {
        self.audio_config.lock().unwrap().take()
    }

    /// Set the current simulation state
    pub fn set_state(&self, new_state: SimState) {
        let mut state = self.state.lock().unwrap();
        *state = new_state;
    }

    /// Get the current simulation state
    pub fn get_state(&self) -> SimState {
        *self.state.lock().unwrap()
    }
}

impl Default for SharedSimState {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle to the simulation thread
pub struct SimulationThread {
    /// Command sender
    command_tx: Sender<SimCommand>,
    /// Notification receiver
    notification_rx: Receiver<SimNotification>,
    /// Shared state
    shared_state: Arc<SharedSimState>,
    /// Thread handle
    thread_handle: Option<JoinHandle<()>>,
}

/// Number of instructions to execute per batch before checking for commands.
///
/// This constant balances two competing concerns:
/// - **Higher values** = Better throughput (fewer context switches and command checks)
/// - **Lower values** = Better responsiveness (faster pause/terminate reaction)
///
/// At 1000 instructions per batch, the simulation thread can:
/// - Check for commands ~1000 times per second (assuming ~1M instructions/second)
/// - React to pause/terminate within ~1ms in typical scenarios
/// - Maintain near-maximum throughput with minimal overhead
const INSTRUCTIONS_PER_BATCH: u64 = 1000;

impl SimulationThread {
    /// Create and start a new simulation thread
    pub fn new() -> Result<Self, String> {
        let (command_tx, command_rx) = mpsc::channel();
        let (notification_tx, notification_rx) = mpsc::channel();
        let shared_state = Arc::new(SharedSimState::new());
        let shared_state_clone = Arc::clone(&shared_state);

        let thread_handle = thread::spawn(move || {
            simulation_thread_main(command_rx, notification_tx, shared_state_clone);
        });

        Ok(Self {
            command_tx,
            notification_rx,
            shared_state,
            thread_handle: Some(thread_handle),
        })
    }

    /// Send a command to the simulation thread
    pub fn send_command(&self, cmd: SimCommand) -> Result<(), String> {
        self.command_tx
            .send(cmd)
            .map_err(|e| format!("Failed to send command: {}", e))
    }

    /// Try to receive a notification (non-blocking)
    pub fn try_recv_notification(&self) -> Option<SimNotification> {
        match self.notification_rx.try_recv() {
            Ok(notification) => Some(notification),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                log::error!("Simulation thread disconnected");
                None
            }
        }
    }

    /// Get access to the shared state
    pub fn shared_state(&self) -> &Arc<SharedSimState> {
        &self.shared_state
    }

    /// Terminate the simulation thread and wait for it to finish
    pub fn terminate(mut self) {
        // Request stop
        self.shared_state.request_stop();

        // Send terminate command (if channel is still open)
        let _ = self.command_tx.send(SimCommand::Terminate);

        // Wait for thread to finish
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for SimulationThread {
    fn drop(&mut self) {
        // Request stop on drop
        self.shared_state.request_stop();
        let _ = self.command_tx.send(SimCommand::Terminate);

        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

/// Main function for the simulation thread
fn simulation_thread_main(
    command_rx: Receiver<SimCommand>,
    notification_tx: Sender<SimNotification>,
    shared_state: Arc<SharedSimState>,
) {
    log::info!("Simulation thread started");

    // Create simulator
    let mut simulator = match create_simulator(Arc::clone(&shared_state)) {
        Ok(sim) => sim,
        Err(e) => {
            log::error!("Failed to create simulator: {}", e);
            let _ = notification_tx.send(SimNotification::Error(e));
            return;
        }
    };

    // Main simulation loop
    loop {
        // Check for stop request
        if shared_state.is_stop_requested() {
            log::info!("Simulation thread stopping (stop requested)");
            break;
        }

        // Check for commands (non-blocking first, then blocking if paused/idle)
        match command_rx.try_recv() {
            Ok(cmd) => {
                if !handle_command(cmd, &mut simulator, &shared_state, &notification_tx) {
                    break;
                }
            }
            Err(TryRecvError::Disconnected) => {
                log::info!("Command channel disconnected");
                break;
            }
            Err(TryRecvError::Empty) => {
                // No command, continue with simulation
            }
        }

        // Execute simulation if running
        let state = shared_state.get_state();
        match state {
            SimState::Running => {
                // Execute a batch of instructions
                match execute_batch(&mut simulator, INSTRUCTIONS_PER_BATCH) {
                    Ok(Some(tohost)) => {
                        // Program halted
                        log::info!("Program halted with tohost: 0x{:08x}", tohost);
                        shared_state.set_state(SimState::Halted);
                        let _ = notification_tx.send(SimNotification::Halted(tohost));
                    }
                    Ok(None) => {
                        // Batch completed normally, continue
                    }
                    Err(e) => {
                        log::error!("Simulation error: {}", e);
                        shared_state.set_state(SimState::Halted);
                        let _ = notification_tx.send(SimNotification::Error(e));
                    }
                }

                // Check pause request after batch
                if shared_state.is_pause_requested() {
                    log::info!("Simulation paused");
                    shared_state.set_state(SimState::Paused);
                    let _ = notification_tx.send(SimNotification::Paused);
                }
            }
            SimState::Idle | SimState::Paused | SimState::Halted => {
                // When not running, wait for commands (with timeout to check stop flag)
                match command_rx.recv_timeout(std::time::Duration::from_millis(10)) {
                    Ok(cmd) => {
                        if !handle_command(cmd, &mut simulator, &shared_state, &notification_tx) {
                            break;
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        // Check stop flag
                        if shared_state.is_stop_requested() {
                            break;
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        log::info!("Command channel disconnected while waiting");
                        break;
                    }
                }
            }
        }
    }

    log::info!("Simulation thread exiting");
}

/// Create a new simulator with video/audio callbacks that push to shared state
fn create_simulator(shared_state: Arc<SharedSimState>) -> Result<InteractiveSimulator, String> {
    let mut simulator = InteractiveSimulator::new()?;

    // Create Video device with callback that pushes to shared state
    let video_shared = Arc::clone(&shared_state);
    let video_callback = move |data: &[u8], config: &VideoConfig| {
        video_shared.push_video_frame(data.to_vec(), *config);
    };
    let video_device = Box::new(Video::new(Some(video_callback)));

    // Create Audio device with callbacks that push to shared state
    let audio_samples_shared = Arc::clone(&shared_state);
    let sample_callback = move |samples: &[i16]| {
        audio_samples_shared.push_audio_samples(samples);
    };

    let audio_config_shared = Arc::clone(&shared_state);
    let config_callback = move |config: &AudioConfig| {
        audio_config_shared.set_audio_config(*config);
    };

    let audio_device = Box::new(Audio::new(Some(sample_callback), Some(config_callback)));

    // Register devices
    simulator.register_device(VIDEO_BASE, video_device)?;
    simulator.register_device(AUDIO_BASE, audio_device)?;

    Ok(simulator)
}

/// Handle a command from the main thread.
/// Returns true to continue, false to exit.
fn handle_command(
    cmd: SimCommand,
    simulator: &mut InteractiveSimulator,
    shared_state: &Arc<SharedSimState>,
    notification_tx: &Sender<SimNotification>,
) -> bool {
    match cmd {
        SimCommand::LoadElf(path) => {
            log::info!("Loading ELF: {:?}", path);

            // Clear any pending data
            let _ = shared_state.take_video_frame();
            let _ = shared_state.take_audio_samples(usize::MAX);
            let _ = shared_state.take_audio_config();

            // Reset flags
            shared_state.reset_flags();

            // Need to create a new simulator to reload
            // First, recreate the simulator with fresh devices
            match create_simulator(Arc::clone(shared_state)) {
                Ok(mut new_sim) => {
                    // Load ELF
                    match new_sim.load_elf(&path) {
                        Ok(()) => {
                            *simulator = new_sim;
                            shared_state.set_state(SimState::Running);
                            let _ = notification_tx.send(SimNotification::ElfLoaded);
                        }
                        Err(e) => {
                            log::error!("Failed to load ELF: {}", e);
                            shared_state.set_state(SimState::Idle);
                            let _ = notification_tx.send(SimNotification::ElfLoadError(e));
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to create simulator: {}", e);
                    let _ = notification_tx.send(SimNotification::Error(e));
                }
            }
            true
        }
        SimCommand::Pause => {
            shared_state.request_pause();
            true
        }
        SimCommand::Resume => {
            let current = shared_state.get_state();
            if current == SimState::Paused {
                shared_state.request_resume();
                shared_state.set_state(SimState::Running);
                let _ = notification_tx.send(SimNotification::Resumed);
            }
            true
        }
        SimCommand::Terminate => {
            log::info!("Terminate command received");
            false
        }
    }
}

/// Execute a batch of instructions.
/// Returns Ok(Some(tohost)) if program halted, Ok(None) if batch completed normally.
fn execute_batch(simulator: &mut InteractiveSimulator, count: u64) -> Result<Option<u32>, String> {
    for _ in 0..count {
        let result = simulator.step_instruction()?;
        if let Some(tohost) = result.tohost_value {
            return Ok(Some(tohost));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_shared_state_video_push_take() {
        let state = SharedSimState::new();

        // No frame initially
        assert!(state.take_video_frame().is_none());

        // Push a frame
        let data = vec![1, 2, 3, 4];
        let config = VideoConfig {
            width: 2,
            height: 2,
            format: cpu_sim::VideoFormat::Rgba8,
        };
        state.push_video_frame(data.clone(), config);

        // Take the frame
        let frame = state.take_video_frame().expect("Should have frame");
        assert_eq!(frame.data, data);
        assert_eq!(frame.config.width, 2);

        // Frame should be consumed
        assert!(state.take_video_frame().is_none());
    }

    #[test]
    fn test_shared_state_audio_push_take() {
        let state = SharedSimState::new();

        // No samples initially
        assert!(state.take_audio_samples(100).is_empty());

        // Push samples
        state.push_audio_samples(&[100, 200, 300]);
        state.push_audio_samples(&[400, 500]);

        // Take partial samples
        let samples = state.take_audio_samples(3);
        assert_eq!(samples, vec![100, 200, 300]);

        // Take remaining
        let samples = state.take_audio_samples(100);
        assert_eq!(samples, vec![400, 500]);

        // All consumed
        assert!(state.take_audio_samples(100).is_empty());
    }

    #[test]
    fn test_shared_state_flags() {
        let state = SharedSimState::new();

        assert!(!state.is_stop_requested());
        assert!(!state.is_pause_requested());

        state.request_stop();
        assert!(state.is_stop_requested());

        state.request_pause();
        assert!(state.is_pause_requested());

        state.reset_flags();
        assert!(!state.is_stop_requested());
        assert!(!state.is_pause_requested());
    }

    #[test]
    fn test_simulation_thread_creation() {
        let thread = SimulationThread::new().expect("Failed to create simulation thread");

        // State should be idle initially
        assert_eq!(thread.shared_state().get_state(), SimState::Idle);

        // Terminate
        thread.terminate();
    }

    #[test]
    fn test_simulation_thread_terminate() {
        let thread = SimulationThread::new().expect("Failed to create simulation thread");

        // Send terminate command
        thread
            .send_command(SimCommand::Terminate)
            .expect("Failed to send command");

        // Give thread time to process
        std::thread::sleep(Duration::from_millis(50));

        // Terminate should complete
        thread.terminate();
    }
}
