//! Background simulation thread for decoupling simulation speed from UI updates.
//!
//! This module provides a `SimulationThread` that runs the CPU simulation in a
//! background thread, communicating with the main thread via channels.
//!
//! ## Architecture
//!
//! The simulation runs in a background thread while the main thread handles UI.
//! Communication happens through channels:
//! - Commands: main thread → simulation thread (load ELF, pause, resume, terminate)
//! - Responses: simulation thread → main thread (state changes, errors, video/audio data)
//!
//! Video frames and audio samples are sent to the main thread via channels, which
//! then forwards them to the appropriate backends. This allows the backends to
//! remain on the main thread (required for GUI backends with non-Send types).

use cpu_sim::{
    Audio, AudioConfig, InteractiveSimulator, Video, VideoConfig, AUDIO_BASE, VIDEO_BASE,
};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};

/// Instructions to step per batch in the background thread
const INSTRUCTIONS_PER_BATCH: u64 = 1000;

/// Commands sent from main thread to simulation thread
#[derive(Debug, Clone)]
pub enum SimCommand {
    /// Load an ELF file and reset the simulation
    LoadELF(PathBuf),
    /// Pause the simulation
    Pause,
    /// Resume the simulation
    Resume,
    /// Terminate the simulation thread
    Terminate,
}

/// Current state of the simulation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimState {
    /// No program loaded
    Idle,
    /// Program loaded and running
    Running,
    /// Program loaded but paused
    Paused,
    /// Program completed (tohost written)
    Halted,
}

/// Video frame data to be sent to main thread
#[derive(Debug, Clone)]
pub struct VideoFrameData {
    pub data: Vec<u8>,
    pub config: VideoConfig,
}

/// Audio data to be sent to main thread
#[derive(Debug, Clone)]
pub enum AudioData {
    /// Audio samples
    Samples(Vec<i16>),
    /// Audio configuration change
    ConfigChange(AudioConfig),
}

/// Response/status updates sent from simulation thread to main thread
#[derive(Debug, Clone)]
pub enum SimResponse {
    /// Simulation state changed
    StateChanged(SimState),
    /// ELF loaded successfully
    ELFLoaded(PathBuf),
    /// ELF load failed
    ELFLoadError(String),
    /// Simulation halted with tohost value
    Halted(u32),
    /// Simulation error
    Error(String),
    /// Max cycles reached
    MaxCyclesReached(u64),
    /// Cycle count update (for status display)
    CycleCount(u64),
    /// Video frame data
    VideoFrame(VideoFrameData),
    /// Audio data (samples or config)
    Audio(AudioData),
}

/// Handle to communicate with the background simulation thread
pub struct SimulationThread {
    /// Channel to send commands to the simulation thread
    command_tx: Sender<SimCommand>,
    /// Channel to receive responses from the simulation thread
    response_rx: Receiver<SimResponse>,
    /// Handle to the background thread (for join on drop)
    thread_handle: Option<JoinHandle<()>>,
    /// Current simulation state (cached from responses)
    current_state: SimState,
    /// Total cycles executed
    total_cycles: u64,
    /// Last loaded ELF path
    last_elf_path: Option<PathBuf>,
}

impl SimulationThread {
    /// Create and start a new simulation thread
    ///
    /// # Arguments
    /// * `max_cycles` - Maximum cycles to run (0 = unlimited)
    pub fn new(max_cycles: u64) -> Result<Self, String> {
        // Create channels for communication
        let (command_tx, command_rx) = mpsc::channel();
        let (response_tx, response_rx) = mpsc::channel();

        // Create the simulation thread
        let thread_handle = thread::spawn(move || {
            simulation_loop(command_rx, response_tx, max_cycles);
        });

        Ok(SimulationThread {
            command_tx,
            response_rx,
            thread_handle: Some(thread_handle),
            current_state: SimState::Idle,
            total_cycles: 0,
            last_elf_path: None,
        })
    }

    /// Send a command to the simulation thread
    pub fn send_command(&self, cmd: SimCommand) -> Result<(), String> {
        self.command_tx
            .send(cmd)
            .map_err(|e| format!("Failed to send command: {}", e))
    }

    /// Load an ELF file
    pub fn load_elf(&mut self, path: &std::path::Path) -> Result<(), String> {
        self.last_elf_path = Some(path.to_path_buf());
        self.send_command(SimCommand::LoadELF(path.to_path_buf()))
    }

    /// Reload the last loaded ELF file
    #[allow(dead_code)]
    pub fn reload_last_elf(&mut self) -> Result<(), String> {
        match &self.last_elf_path {
            Some(path) => {
                let path = path.clone();
                self.send_command(SimCommand::LoadELF(path))
            }
            None => {
                log::warn!("No ELF file to reload");
                Ok(())
            }
        }
    }

    /// Pause the simulation
    pub fn pause(&self) -> Result<(), String> {
        self.send_command(SimCommand::Pause)
    }

    /// Resume the simulation
    pub fn resume(&self) -> Result<(), String> {
        self.send_command(SimCommand::Resume)
    }

    /// Terminate the simulation thread
    pub fn terminate(&self) -> Result<(), String> {
        self.send_command(SimCommand::Terminate)
    }

    /// Get the current simulation state
    pub fn state(&self) -> SimState {
        self.current_state
    }

    /// Get the total cycles executed
    #[allow(dead_code)]
    pub fn total_cycles(&self) -> u64 {
        self.total_cycles
    }

    /// Get the last loaded ELF path
    #[allow(dead_code)]
    pub fn last_elf_path(&self) -> Option<&PathBuf> {
        self.last_elf_path.as_ref()
    }

    /// Poll for responses from the simulation thread
    ///
    /// Returns all available responses without blocking
    pub fn poll_responses(&mut self) -> Vec<SimResponse> {
        let mut responses = Vec::new();

        loop {
            match self.response_rx.try_recv() {
                Ok(response) => {
                    // Update cached state based on response
                    match &response {
                        SimResponse::StateChanged(state) => {
                            self.current_state = *state;
                        }
                        SimResponse::CycleCount(count) => {
                            self.total_cycles = *count;
                        }
                        SimResponse::ELFLoaded(path) => {
                            self.last_elf_path = Some(path.clone());
                        }
                        _ => {}
                    }
                    responses.push(response);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    log::warn!("Simulation thread disconnected");
                    break;
                }
            }
        }

        responses
    }

    /// Check if the simulation thread is still running
    pub fn is_running(&self) -> bool {
        self.thread_handle
            .as_ref()
            .is_some_and(|h| !h.is_finished())
    }
}

impl Drop for SimulationThread {
    fn drop(&mut self) {
        // Send terminate command (ignore errors if already disconnected)
        let _ = self.send_command(SimCommand::Terminate);

        // Wait for the thread to finish
        if let Some(handle) = self.thread_handle.take() {
            if let Err(e) = handle.join() {
                log::error!("Simulation thread panicked: {:?}", e);
            }
        }
    }
}

/// Main simulation loop running in the background thread
fn simulation_loop(
    command_rx: Receiver<SimCommand>,
    response_tx: Sender<SimResponse>,
    max_cycles: u64,
) {
    log::info!("Simulation thread started");

    // Create the simulator
    let mut simulator = match InteractiveSimulator::new() {
        Ok(sim) => sim,
        Err(e) => {
            let _ = response_tx.send(SimResponse::Error(format!(
                "Failed to create simulator: {}",
                e
            )));
            return;
        }
    };

    // Clone response_tx for video callback
    let video_tx = response_tx.clone();
    let video_callback = move |data: &[u8], config: &VideoConfig| {
        let frame_data = VideoFrameData {
            data: data.to_vec(),
            config: *config,
        };
        let _ = video_tx.send(SimResponse::VideoFrame(frame_data));
    };
    let video_device = Box::new(Video::new(Some(video_callback)));

    // Clone response_tx for audio callbacks
    let audio_sample_tx = response_tx.clone();
    let sample_callback = move |samples: &[i16]| {
        let _ = audio_sample_tx.send(SimResponse::Audio(AudioData::Samples(samples.to_vec())));
    };

    let audio_config_tx = response_tx.clone();
    let config_callback = move |config: &AudioConfig| {
        log::info!(
            "Audio config changed: {} Hz, {:?}, {} samples",
            config.sample_rate.to_hz(),
            config.channels,
            config.sample_count
        );
        let _ = audio_config_tx.send(SimResponse::Audio(AudioData::ConfigChange(*config)));
    };
    let audio_device = Box::new(Audio::new(Some(sample_callback), Some(config_callback)));

    // Register devices
    if let Err(e) = simulator.register_device(VIDEO_BASE, video_device) {
        let _ = response_tx.send(SimResponse::Error(format!(
            "Failed to register video device: {}",
            e
        )));
        return;
    }
    if let Err(e) = simulator.register_device(AUDIO_BASE, audio_device) {
        let _ = response_tx.send(SimResponse::Error(format!(
            "Failed to register audio device: {}",
            e
        )));
        return;
    }

    let mut state = SimState::Idle;
    let mut total_cycles: u64 = 0;
    let mut last_cycle_report: u64 = 0;

    // Main loop
    loop {
        // Check for commands (non-blocking)
        match command_rx.try_recv() {
            Ok(cmd) => match cmd {
                SimCommand::LoadELF(path) => {
                    log::info!("Loading ELF: {}", path.display());
                    match simulator.load_elf(&path) {
                        Ok(()) => {
                            state = SimState::Running;
                            total_cycles = 0;
                            last_cycle_report = 0;
                            let _ = response_tx.send(SimResponse::ELFLoaded(path));
                            let _ = response_tx.send(SimResponse::StateChanged(state));
                            log::info!("ELF loaded successfully");
                        }
                        Err(e) => {
                            let _ = response_tx.send(SimResponse::ELFLoadError(e));
                        }
                    }
                }
                SimCommand::Pause => {
                    if state == SimState::Running {
                        state = SimState::Paused;
                        let _ = response_tx.send(SimResponse::StateChanged(state));
                        log::info!("Simulation paused");
                    }
                }
                SimCommand::Resume => {
                    if state == SimState::Paused {
                        state = SimState::Running;
                        let _ = response_tx.send(SimResponse::StateChanged(state));
                        log::info!("Simulation resumed");
                    }
                }
                SimCommand::Terminate => {
                    log::info!("Simulation thread terminating");
                    return;
                }
            },
            Err(TryRecvError::Empty) => {
                // No command, continue with simulation
            }
            Err(TryRecvError::Disconnected) => {
                log::info!("Command channel disconnected, terminating");
                return;
            }
        }

        // Execute simulation steps if running
        if state == SimState::Running {
            for _ in 0..INSTRUCTIONS_PER_BATCH {
                match simulator.step_instruction() {
                    Ok(result) => {
                        total_cycles += 1;

                        // Check for program termination
                        if let Some(tohost) = result.tohost_value {
                            state = SimState::Halted;
                            let _ = response_tx.send(SimResponse::Halted(tohost));
                            let _ = response_tx.send(SimResponse::StateChanged(state));
                            log::info!("Program halted with tohost: 0x{:08x}", tohost);
                            break;
                        }

                        // Check max cycles
                        if max_cycles > 0 && total_cycles >= max_cycles {
                            state = SimState::Halted;
                            let _ = response_tx.send(SimResponse::MaxCyclesReached(total_cycles));
                            let _ = response_tx.send(SimResponse::StateChanged(state));
                            log::info!("Max cycles reached: {}", total_cycles);
                            break;
                        }
                    }
                    Err(e) => {
                        state = SimState::Halted;
                        let _ = response_tx.send(SimResponse::Error(e.clone()));
                        let _ = response_tx.send(SimResponse::StateChanged(state));
                        log::error!("Simulation error: {}", e);
                        break;
                    }
                }
            }

            // Send periodic cycle count updates (every 100k cycles)
            if total_cycles - last_cycle_report >= 100_000 {
                let _ = response_tx.send(SimResponse::CycleCount(total_cycles));
                last_cycle_report = total_cycles;
            }
        } else {
            // When not running, sleep briefly to avoid busy-waiting
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simulation_thread_creation() {
        let sim_thread = SimulationThread::new(0);
        assert!(sim_thread.is_ok());

        let sim_thread = sim_thread.unwrap();
        assert_eq!(sim_thread.state(), SimState::Idle);
        assert!(sim_thread.is_running());

        // Terminate
        sim_thread.terminate().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    #[test]
    fn test_simulation_thread_pause_resume() {
        let mut sim_thread = SimulationThread::new(0).unwrap();

        // Pause (should work even in Idle state, just won't change state)
        sim_thread.pause().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        sim_thread.poll_responses();
        // State should still be Idle since no ELF is loaded
        assert_eq!(sim_thread.state(), SimState::Idle);

        // Terminate
        sim_thread.terminate().unwrap();
    }
}
