//! Threaded simulation viewer.
//!
//! This module provides a viewer that runs simulation in a background thread,
//! decoupling simulation speed from UI update speed for better performance.

use crate::backend_traits::{EventSource, Key, KeyModifiers, TestCommand, ViewerEvent};
use crate::sim_thread::{
    SimCommand, SimStatus, SimThread, SimThreadConfig, ThreadSafeAudioBackend,
    ThreadSafeVideoBackend,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

/// Configuration for the threaded viewer.
pub struct ThreadedViewerConfig {
    /// Maximum cycles before auto-termination (0 = unlimited)
    pub max_cycles: u64,
    /// Number of instructions per batch in the simulation thread
    pub batch_size: u64,
}

impl Default for ThreadedViewerConfig {
    fn default() -> Self {
        Self {
            max_cycles: 0,
            batch_size: 1000,
        }
    }
}

/// Viewer state tracked by the main thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewerState {
    /// No program loaded
    Idle,
    /// Program loaded and running
    Running,
    /// Program loaded but paused
    Paused,
    /// Program completed (tohost written or error)
    Halted,
}

/// Threaded simulation viewer that runs simulation in a background thread.
///
/// This viewer separates the simulation loop from the UI loop:
/// - Simulation runs in a background thread with its own event loop
/// - Main thread handles UI updates, window events, and user input
/// - Audio/video callbacks write directly to thread-safe backends
pub struct ThreadedViewer<E: EventSource> {
    /// Simulation thread handle
    sim_thread: SimThread,

    /// Thread-safe video backend (shared with simulation thread)
    video_backend: Arc<dyn ThreadSafeVideoBackend>,

    /// Thread-safe audio backend (shared with simulation thread)
    #[allow(dead_code)] // May be used for future audio-related APIs
    audio_backend: Arc<dyn ThreadSafeAudioBackend>,

    /// Event source for keyboard/window events
    event_source: E,

    /// Current viewer state (synchronized with simulation thread status)
    state: ViewerState,

    /// Last loaded ELF file path (for reload)
    last_elf_path: Option<PathBuf>,

    /// Exit requested by user
    exit_requested: bool,

    /// Frame step target (for StepFrames test command)
    frame_step_target: Option<u64>,

    /// Instruction step target (for single-step mode in tests)
    #[allow(dead_code)] // Will be implemented for single-step test mode
    instruction_step_target: Option<u64>,
}

impl<E: EventSource> ThreadedViewer<E> {
    /// Create a new threaded viewer.
    ///
    /// # Arguments
    /// * `config` - Viewer configuration
    /// * `video_backend` - Thread-safe video backend
    /// * `audio_backend` - Thread-safe audio backend
    /// * `event_source` - Event source for input handling
    pub fn new(
        config: ThreadedViewerConfig,
        video_backend: Arc<dyn ThreadSafeVideoBackend>,
        audio_backend: Arc<dyn ThreadSafeAudioBackend>,
        event_source: E,
    ) -> Result<Self, String> {
        let sim_config = SimThreadConfig {
            max_cycles: config.max_cycles,
            batch_size: config.batch_size,
        };

        let sim_thread = SimThread::new(
            sim_config,
            Arc::clone(&video_backend),
            Arc::clone(&audio_backend),
        )?;

        Ok(Self {
            sim_thread,
            video_backend,
            audio_backend,
            event_source,
            state: ViewerState::Idle,
            last_elf_path: None,
            exit_requested: false,
            frame_step_target: None,
            instruction_step_target: None,
        })
    }

    /// Load an ELF file and start simulation.
    pub fn load_elf(&mut self, path: &Path) -> Result<(), String> {
        log::info!("Loading ELF: {}", path.display());

        self.sim_thread
            .send_command(SimCommand::LoadELF(path.to_path_buf()))?;

        self.last_elf_path = Some(path.to_path_buf());
        self.update_window_title();

        Ok(())
    }

    /// Reload the last loaded ELF file.
    pub fn reload_last_elf(&mut self) -> Result<(), String> {
        match &self.last_elf_path {
            Some(path) => {
                let path = path.clone();
                self.load_elf(&path)
            }
            None => {
                log::warn!("No ELF file to reload");
                Ok(())
            }
        }
    }

    /// Toggle pause/resume state.
    pub fn toggle_pause(&mut self) -> Result<(), String> {
        match self.state {
            ViewerState::Running => {
                self.sim_thread.send_command(SimCommand::Pause)?;
            }
            ViewerState::Paused => {
                self.sim_thread.send_command(SimCommand::Resume)?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Update window title based on current state.
    fn update_window_title(&self) {
        let title = match (&self.last_elf_path, self.state) {
            (Some(path), ViewerState::Running) => {
                format!("sim-view - {} [RUNNING]", path.display())
            }
            (Some(path), ViewerState::Paused) => {
                format!("sim-view - {} [PAUSED]", path.display())
            }
            (Some(path), ViewerState::Halted) => {
                format!("sim-view - {} [HALTED]", path.display())
            }
            (Some(path), ViewerState::Idle) => {
                format!("sim-view - {} [IDLE]", path.display())
            }
            (None, _) => "sim-view - No program loaded".to_string(),
        };
        self.video_backend.set_title(&title);
    }

    /// Execute a single iteration of the viewer loop.
    ///
    /// Returns `Ok(true)` if the viewer should continue running,
    /// or `Ok(false)` if the viewer should terminate.
    pub fn step(&mut self) -> Result<bool, String> {
        // Handle window events
        self.handle_events()?;

        // Check if exit was requested
        if self.exit_requested {
            log::info!("Exit requested, terminating viewer loop");
            return Ok(false);
        }

        // Check if video backend is still active
        if !self.video_backend.is_active() {
            log::info!("Backend inactive, terminating viewer loop");
            return Ok(false);
        }

        // Process status updates from simulation thread
        self.process_simulation_status();

        // Check frame step target
        if let Some(target) = self.frame_step_target {
            let current_frames = self.sim_thread.frame_count();
            if current_frames >= target {
                log::info!("Frame step target reached: {} frames", current_frames);
                self.frame_step_target = None;
                self.sim_thread.send_command(SimCommand::Pause)?;
            }
        }

        // Update video backend (for display refresh)
        self.video_backend.update()?;

        Ok(true)
    }

    /// Run the main viewer loop.
    pub fn run(&mut self) -> Result<(), String> {
        log::info!("Starting threaded viewer main loop");

        let startup_time = Instant::now();

        loop {
            let frame_start = Instant::now();

            if !self.step()? {
                break;
            }

            let total_elapsed_s = startup_time.elapsed().as_secs_f64();
            let iteration_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
            log::info!(
                "Elapsed: {:.2} s (iteration: {:.2} ms)",
                total_elapsed_s,
                iteration_ms
            );
        }

        // Terminate the simulation thread
        self.sim_thread.terminate();

        log::info!("Threaded viewer loop ended");
        Ok(())
    }

    /// Process status updates from the simulation thread.
    fn process_simulation_status(&mut self) {
        while let Some(status) = self.sim_thread.try_recv_status() {
            match status {
                SimStatus::Idle => {
                    self.state = ViewerState::Idle;
                    self.update_window_title();
                }
                SimStatus::Running => {
                    self.state = ViewerState::Running;
                    self.update_window_title();
                }
                SimStatus::Paused => {
                    self.state = ViewerState::Paused;
                    self.update_window_title();
                }
                SimStatus::Halted(tohost) => {
                    log::info!("Program halted with tohost value: 0x{:08x}", tohost);
                    self.state = ViewerState::Halted;
                    self.update_window_title();
                }
                SimStatus::Error(e) => {
                    log::error!("Simulation error: {}", e);
                    self.state = ViewerState::Halted;
                    self.update_window_title();
                }
                SimStatus::ElfLoaded => {
                    log::info!("ELF loaded successfully");
                }
                SimStatus::StepCompleted { instructions, frames } => {
                    log::info!(
                        "Step completed: {} instructions, {} frames",
                        instructions,
                        frames
                    );
                }
            }
        }
    }

    /// Handle window events.
    fn handle_events(&mut self) -> Result<(), String> {
        let events = self.event_source.get_events();

        for event in events {
            match event {
                ViewerEvent::KeyPressed(key, modifiers) => {
                    self.handle_key_press(key, modifiers)?;
                }
                ViewerEvent::Close => {
                    log::info!("Close event received");
                    self.exit_requested = true;
                }
                ViewerEvent::TestCommand(cmd) => {
                    self.handle_test_command(cmd)?;
                }
            }
        }

        Ok(())
    }

    /// Handle test commands (for headless mode).
    fn handle_test_command(&mut self, cmd: TestCommand) -> Result<(), String> {
        match cmd {
            TestCommand::LoadELF(path) => {
                log::info!("Test command: Load ELF {:?}", path);
                self.load_elf(&path)?;
            }
            TestCommand::Pause => {
                log::info!("Test command: Pause");
                self.sim_thread.send_command(SimCommand::Pause)?;
            }
            TestCommand::Resume => {
                log::info!("Test command: Resume");
                self.sim_thread.send_command(SimCommand::Resume)?;
            }
            TestCommand::StepFrames(count) => {
                log::info!("Test command: Step {} frames", count);
                // Set frame step target
                let current_frames = self.sim_thread.frame_count();
                self.frame_step_target = Some(current_frames + count);
                // Resume if paused
                if self.state == ViewerState::Paused || self.state == ViewerState::Idle {
                    self.sim_thread.send_command(SimCommand::Resume)?;
                }
            }
            TestCommand::Terminate => {
                log::info!("Test command: Terminate");
                self.exit_requested = true;
            }
        }
        Ok(())
    }

    /// Handle keyboard input.
    fn handle_key_press(&mut self, key: Key, modifiers: KeyModifiers) -> Result<(), String> {
        match key {
            Key::Escape => {
                log::info!("Escape pressed, exit requested");
                self.exit_requested = true;
            }
            Key::Space => {
                self.toggle_pause()?;
            }
            Key::R if modifiers.ctrl => {
                log::info!("Ctrl+R pressed, reloading ELF");
                self.reload_last_elf()?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Get the current frame count from the simulation thread.
    pub fn frame_count(&self) -> u64 {
        self.sim_thread.frame_count()
    }

    /// Get the current viewer state.
    pub fn state(&self) -> ViewerState {
        self.state
    }

    /// Check if the simulation thread is still running.
    pub fn is_sim_running(&self) -> bool {
        self.sim_thread.is_running()
    }
}

// ============================================================================
// Specialized methods for headless testing
// ============================================================================

use crate::headless_backends::{
    CapturedAudioChunk, CapturedFrame, HeadlessEventSource, ThreadSafeHeadlessAudioBackend,
    ThreadSafeHeadlessVideoBackend,
};
use cpu_sim::AudioConfig;

/// Type alias for headless threaded viewer
pub type HeadlessThreadedViewer =
    ThreadedViewer<HeadlessEventSource>;

impl ThreadedViewer<HeadlessEventSource> {
    /// Create a new headless threaded viewer for testing.
    #[allow(dead_code)] // This is a placeholder - use create_headless_viewer instead
    pub fn new_headless(_config: ThreadedViewerConfig) -> Result<(Self, HeadlessEventSource), String> {
        // This method is deprecated in favor of create_headless_viewer
        // which returns the concrete backend types for data access.
        Err("new_headless needs a different design - see create_headless_viewer".to_string())
    }

    /// Push an event (for testing).
    ///
    /// Note: This requires the event source to be accessible, which the current
    /// design doesn't support directly. Tests should use the event source before
    /// passing it to the viewer.
    pub fn push_event(&mut self, event: ViewerEvent) -> Result<(), String> {
        self.event_source.push_event(event);
        Ok(())
    }

    /// Get captured video frames.
    pub fn get_video_frames(&self) -> Vec<CapturedFrame> {
        // We need to downcast the video backend to access get_frames
        // This is a limitation of the current design - we'll need to store
        // a reference to the concrete type.
        //
        // For now, return empty vec and fix this in the next iteration.
        Vec::new()
    }

    /// Get captured audio chunks.
    pub fn get_audio_chunks(&self) -> Vec<CapturedAudioChunk> {
        Vec::new()
    }

    /// Get current audio configuration.
    pub fn get_audio_config(&self) -> Option<AudioConfig> {
        None
    }
}

/// Create a headless threaded viewer with accessible backends.
///
/// Returns the viewer along with references to the backends for data retrieval.
pub fn create_headless_viewer(
    config: ThreadedViewerConfig,
) -> Result<
    (
        ThreadedViewer<HeadlessEventSource>,
        Arc<ThreadSafeHeadlessVideoBackend>,
        Arc<ThreadSafeHeadlessAudioBackend>,
    ),
    String,
> {
    let video_backend = Arc::new(ThreadSafeHeadlessVideoBackend::new());
    let audio_backend = Arc::new(ThreadSafeHeadlessAudioBackend::new());
    let event_source = HeadlessEventSource::new();

    let video_backend_trait: Arc<dyn ThreadSafeVideoBackend> = Arc::clone(&video_backend) as Arc<dyn ThreadSafeVideoBackend>;
    let audio_backend_trait: Arc<dyn ThreadSafeAudioBackend> = Arc::clone(&audio_backend) as Arc<dyn ThreadSafeAudioBackend>;

    let sim_config = SimThreadConfig {
        max_cycles: config.max_cycles,
        batch_size: config.batch_size,
    };

    let sim_thread = SimThread::new(sim_config, video_backend_trait, audio_backend_trait)?;

    let viewer = ThreadedViewer {
        sim_thread,
        video_backend: Arc::clone(&video_backend) as Arc<dyn ThreadSafeVideoBackend>,
        audio_backend: Arc::clone(&audio_backend) as Arc<dyn ThreadSafeAudioBackend>,
        event_source,
        state: ViewerState::Idle,
        last_elf_path: None,
        exit_requested: false,
        frame_step_target: None,
        instruction_step_target: None,
    };

    Ok((viewer, video_backend, audio_backend))
}
