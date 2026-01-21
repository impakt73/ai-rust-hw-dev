//! Threaded viewer implementation for GUI mode.
//!
//! This module provides `ThreadedSimViewer` which runs the simulation on a
//! background thread while keeping the GUI responsive on the main thread.
//! This design solves the issues with the original `SimViewer`:
//!
//! 1. **No fixed instructions per frame** - Simulation runs as fast as possible
//! 2. **No blocking on display** - Main thread just updates the display
//! 3. **Push-based data flow** - Video/audio data is delivered immediately via callbacks
//!
//! Use this for GUI mode. For headless testing, use the original `SimViewer`.

use crate::backend_traits::{
    AudioBackend, EventSource, Key, KeyModifiers, VideoBackend, ViewerEvent,
};
use crate::simulation_thread::{SimCommand, SimNotification, SimState, SimulationThread};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Configuration for the threaded viewer
pub struct ThreadedViewerConfig {
    /// Initial window width (may be overridden by video frame size)
    pub initial_width: u32,
    /// Initial window height (may be overridden by video frame size)
    pub initial_height: u32,
    /// Whether to print instruction trace (not used in threaded mode)
    #[allow(dead_code)]
    pub print_inst_trace: bool,
}

/// Threaded simulation viewer for GUI mode.
///
/// This viewer runs the simulation on a background thread while keeping the
/// GUI responsive. Video frames and audio samples are pushed directly from
/// the simulation callbacks to the backends.
pub struct ThreadedSimViewer<V: VideoBackend, A: AudioBackend, E: EventSource> {
    /// Simulation thread
    sim_thread: SimulationThread,

    /// Video backend (generic)
    video_backend: V,

    /// Audio backend (generic)
    audio_backend: A,

    /// Event source (generic)
    event_source: E,

    /// Current configuration
    #[allow(dead_code)]
    config: ThreadedViewerConfig,

    /// Last loaded ELF file path (for reload)
    last_elf_path: Option<PathBuf>,

    /// Exit requested by user (e.g., Escape key)
    exit_requested: bool,
}

impl<V: VideoBackend, A: AudioBackend, E: EventSource> ThreadedSimViewer<V, A, E> {
    /// Create a new ThreadedSimViewer with dependency injection
    pub fn new(
        config: ThreadedViewerConfig,
        video_backend: V,
        audio_backend: A,
        event_source: E,
    ) -> Result<Self, String> {
        // Create simulation thread
        let sim_thread = SimulationThread::new()?;

        Ok(Self {
            sim_thread,
            video_backend,
            audio_backend,
            event_source,
            config,
            last_elf_path: None,
            exit_requested: false,
        })
    }

    /// Load an ELF file and start simulation
    pub fn load_elf(&mut self, path: &Path) -> Result<(), String> {
        log::info!("Loading ELF: {}", path.display());

        // Send load command to simulation thread
        self.sim_thread
            .send_command(SimCommand::LoadElf(path.to_path_buf()))?;

        // Store path for reload
        self.last_elf_path = Some(path.to_path_buf());

        // Update window title
        self.update_window_title();

        Ok(())
    }

    /// Reload the last loaded ELF file (for Ctrl+R hotkey)
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

    /// Toggle pause/resume state
    pub fn toggle_pause(&mut self) {
        let state = self.sim_thread.shared_state().get_state();
        match state {
            SimState::Running => {
                log::info!("Pausing simulation");
                let _ = self.sim_thread.send_command(SimCommand::Pause);
            }
            SimState::Paused => {
                log::info!("Resuming simulation");
                let _ = self.sim_thread.send_command(SimCommand::Resume);
            }
            _ => {
                // Idle and Halted states don't change
            }
        }
        self.update_window_title();
    }

    /// Update window title to reflect current state
    fn update_window_title(&mut self) {
        let state = self.sim_thread.shared_state().get_state();
        let title = match (&self.last_elf_path, state) {
            (Some(path), SimState::Running) => {
                format!("sim-view - {} [RUNNING]", path.display())
            }
            (Some(path), SimState::Paused) => {
                format!("sim-view - {} [PAUSED]", path.display())
            }
            (Some(path), SimState::Halted) => {
                format!("sim-view - {} [HALTED]", path.display())
            }
            (Some(path), SimState::Idle) => {
                format!("sim-view - {} [IDLE]", path.display())
            }
            (None, _) => "sim-view - No program loaded".to_string(),
        };
        self.video_backend.set_title(&title);
    }

    /// Execute a single iteration of the viewer loop
    ///
    /// Returns `Ok(true)` if the viewer should continue running,
    /// or `Ok(false)` if the viewer should terminate.
    pub fn step(&mut self) -> Result<bool, String> {
        // Handle window events (keyboard, close)
        self.handle_events()?;

        // Terminate if an exit was requested
        if self.exit_requested {
            log::info!("Exit requested, terminating viewer loop");
            return Ok(false);
        }

        // Check if backend is still active (for GUI mode)
        if !self.video_backend.is_active() {
            log::info!("Backend inactive, terminating viewer loop");
            return Ok(false);
        }

        // Process notifications from simulation thread
        self.process_notifications();

        // Pull and process video frame from shared state (push model - data is already there)
        let shared_state = Arc::clone(self.sim_thread.shared_state());
        if let Some(frame) = shared_state.take_video_frame() {
            self.video_backend
                .process_frame(&frame.data, &frame.config)?;
        }

        // Pull and process audio config changes
        if let Some(config) = shared_state.take_audio_config() {
            log::info!(
                "Audio config changed: {} Hz, {:?}, {} samples",
                config.sample_rate.to_hz(),
                config.channels,
                config.sample_count
            );
            self.audio_backend.set_config(&config);
        }

        // Pull and process audio samples
        let audio_samples = shared_state.take_audio_samples(4096);
        if !audio_samples.is_empty() {
            self.audio_backend.push_samples(&audio_samples);
        }

        // Update video backend (display)
        self.video_backend.update()?;

        Ok(true)
    }

    /// Main viewer loop
    pub fn run(&mut self) -> Result<(), String> {
        log::info!("Starting threaded viewer main loop");

        loop {
            // Execute one step of the viewer loop
            if !self.step()? {
                break;
            }
        }

        log::info!("Viewer loop ended");

        // Terminate simulation thread
        self.sim_thread.shared_state().request_stop();
        let _ = self.sim_thread.send_command(SimCommand::Terminate);

        Ok(())
    }

    /// Process notifications from the simulation thread
    fn process_notifications(&mut self) {
        while let Some(notification) = self.sim_thread.try_recv_notification() {
            match notification {
                SimNotification::ElfLoaded => {
                    log::info!("ELF loaded successfully");
                    self.update_window_title();
                }
                SimNotification::ElfLoadError(e) => {
                    log::error!("Failed to load ELF: {}", e);
                    self.update_window_title();
                }
                SimNotification::Paused => {
                    log::info!("Simulation paused");
                    self.update_window_title();
                }
                SimNotification::Resumed => {
                    log::info!("Simulation resumed");
                    self.update_window_title();
                }
                SimNotification::Halted(tohost) => {
                    log::info!("Program halted with tohost: 0x{:08x}", tohost);
                    self.update_window_title();
                }
                SimNotification::Error(e) => {
                    log::error!("Simulation error: {}", e);
                    self.update_window_title();
                }
            }
        }
    }

    /// Handle window events (keyboard, close)
    fn handle_events(&mut self) -> Result<(), String> {
        let events = self.event_source.get_events();

        for event in events {
            match event {
                ViewerEvent::KeyPressed(key, modifiers) => {
                    self.handle_key_press(key, modifiers)?;
                }
                ViewerEvent::Close => {
                    log::info!("Close event received, exit requested");
                    self.exit_requested = true;
                }
                ViewerEvent::TestCommand(_) => {
                    // Test commands are not used in threaded viewer
                    log::debug!("Test command ignored in threaded viewer");
                }
            }
        }

        Ok(())
    }

    /// Handle keyboard input
    fn handle_key_press(&mut self, key: Key, modifiers: KeyModifiers) -> Result<(), String> {
        match key {
            Key::Escape => {
                log::info!("Escape pressed, exit requested");
                self.exit_requested = true;
            }
            Key::Space => {
                self.toggle_pause();
            }
            Key::R if modifiers.ctrl => {
                log::info!("Ctrl+R pressed, reloading ELF");
                self.reload_last_elf()?;
            }
            _ => {}
        }
        Ok(())
    }
}

impl<V: VideoBackend, A: AudioBackend, E: EventSource> Drop for ThreadedSimViewer<V, A, E> {
    fn drop(&mut self) {
        // Ensure simulation thread is stopped
        self.sim_thread.shared_state().request_stop();
        let _ = self.sim_thread.send_command(SimCommand::Terminate);
    }
}
