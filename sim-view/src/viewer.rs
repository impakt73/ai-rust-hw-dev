//! SimViewer - Main viewer implementation with background simulation thread.
//!
//! This module provides `SimViewer`, which manages the CPU simulation in a background
//! thread while the main thread handles UI updates and event handling.

use crate::backend_traits::{
    AudioBackend, EventSource, Key, KeyModifiers, TestCommand, VideoBackend, ViewerEvent,
};
use crate::sim_thread::{
    AudioConfigCallback, AudioSampleCallback, SimState, SimulationThread, VideoCallback,
};
use cpu_sim::{AudioConfig, VideoConfig};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Viewer configuration.
pub struct ViewerConfig {
    #[allow(dead_code)] // Used by main.rs to create backends
    pub initial_width: u32,
    #[allow(dead_code)] // Used by main.rs to create backends
    pub initial_height: u32,
    pub max_cycles: u64,
    #[allow(dead_code)]
    pub print_inst_trace: bool,
}

/// Thread-safe wrapper for video backend.
///
/// This wrapper provides thread-safe access to a VideoBackend, allowing it to be
/// called from both the main thread and the background simulation thread.
struct ThreadSafeVideoBackend<V: VideoBackend + Send> {
    inner: Arc<Mutex<V>>,
    frame_presented: Arc<Mutex<bool>>,
}

impl<V: VideoBackend + Send + 'static> ThreadSafeVideoBackend<V> {
    fn new(backend: V) -> Self {
        Self {
            inner: Arc::new(Mutex::new(backend)),
            frame_presented: Arc::new(Mutex::new(false)),
        }
    }

    /// Create a video callback that can be called from the simulation thread.
    fn create_callback(&self) -> VideoCallback {
        let inner = Arc::clone(&self.inner);
        let frame_presented = Arc::clone(&self.frame_presented);
        Arc::new(Mutex::new(move |data: &[u8], config: &VideoConfig| {
            if let Ok(mut backend) = inner.lock() {
                match backend.process_frame(data, config) {
                    Ok(()) => {
                        if let Ok(mut flag) = frame_presented.lock() {
                            *flag = true;
                        }
                    }
                    Err(e) => {
                        log::error!("Video backend process_frame error: {}", e);
                    }
                }
            }
        }))
    }

    /// Get a reference to the inner backend (for main thread operations).
    fn inner(&self) -> &Arc<Mutex<V>> {
        &self.inner
    }

    /// Check and reset the frame_presented flag.
    fn take_frame_presented(&self) -> bool {
        if let Ok(mut flag) = self.frame_presented.lock() {
            let was_presented = *flag;
            *flag = false;
            was_presented
        } else {
            false
        }
    }
}

/// Thread-safe wrapper for audio backend.
struct ThreadSafeAudioBackend<A: AudioBackend + Send + 'static> {
    inner: Arc<Mutex<A>>,
}

impl<A: AudioBackend + Send + 'static> ThreadSafeAudioBackend<A> {
    fn new(backend: A) -> Self {
        Self {
            inner: Arc::new(Mutex::new(backend)),
        }
    }

    /// Create sample callback that can be called from the simulation thread.
    fn create_sample_callback(&self) -> AudioSampleCallback {
        let inner = Arc::clone(&self.inner);
        Arc::new(Mutex::new(move |samples: &[i16]| {
            if let Ok(mut backend) = inner.lock() {
                backend.push_samples(samples);
            }
        }))
    }

    /// Create config callback that can be called from the simulation thread.
    fn create_config_callback(&self) -> AudioConfigCallback {
        let inner = Arc::clone(&self.inner);
        Arc::new(Mutex::new(move |config: &AudioConfig| {
            log::info!(
                "Audio config changed: {} Hz, {:?}, {} samples",
                config.sample_rate.to_hz(),
                config.channels,
                config.sample_count
            );
            if let Ok(mut backend) = inner.lock() {
                backend.set_config(config);
            }
        }))
    }

    /// Get a reference to the inner backend (for main thread operations).
    fn inner(&self) -> &Arc<Mutex<A>> {
        &self.inner
    }
}

/// Current viewer state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewerState {
    /// No program loaded
    Idle,
    /// Program loaded and running
    Running,
    /// Program loaded but paused
    Paused,
    /// Program completed (tohost written)
    Halted,
}

/// Main viewer struct that manages simulation in a background thread.
pub struct SimViewer<V: VideoBackend + Send + 'static, A: AudioBackend + Send + 'static, E: EventSource>
{
    /// Background simulation thread
    sim_thread: SimulationThread,

    /// Thread-safe video backend wrapper
    video_backend: ThreadSafeVideoBackend<V>,

    /// Thread-safe audio backend wrapper
    audio_backend: ThreadSafeAudioBackend<A>,

    /// Event source (main thread only)
    event_source: E,

    /// Current configuration
    config: ViewerConfig,

    /// Current viewer state (synced with sim_thread)
    state: ViewerState,

    /// Last loaded ELF file path (for reload)
    last_elf_path: Option<PathBuf>,

    /// Cycle counter (synced from sim_thread)
    total_cycles: u64,

    /// Frame counter (synced from sim_thread)
    frame_count: u64,

    /// Exit requested by user
    exit_requested: bool,

    /// Frame step target (for StepFrames test command)
    frame_step_target: Option<u64>,

    /// Single-step mode enabled (for deterministic headless testing)
    single_step_mode: bool,
}

impl<V: VideoBackend + Send + 'static, A: AudioBackend + Send + 'static, E: EventSource>
    SimViewer<V, A, E>
{
    /// Create a new SimViewer with dependency injection.
    ///
    /// This function sets up the simulation thread with callbacks that directly
    /// write to the provided backends, ensuring audio and video data is available
    /// immediately without waiting for UI updates.
    pub fn new(
        config: ViewerConfig,
        video_backend: V,
        audio_backend: A,
        event_source: E,
    ) -> Result<Self, String> {
        // Wrap backends in thread-safe wrappers
        let video_backend = ThreadSafeVideoBackend::new(video_backend);
        let audio_backend = ThreadSafeAudioBackend::new(audio_backend);

        // Create callbacks that will be called from the simulation thread
        let video_callback = video_backend.create_callback();
        let audio_sample_callback = audio_backend.create_sample_callback();
        let audio_config_callback = audio_backend.create_config_callback();

        // Create the simulation thread
        let sim_thread =
            SimulationThread::new(video_callback, audio_sample_callback, audio_config_callback)?;

        Ok(SimViewer {
            sim_thread,
            video_backend,
            audio_backend,
            event_source,
            config,
            state: ViewerState::Idle,
            last_elf_path: None,
            total_cycles: 0,
            frame_count: 0,
            exit_requested: false,
            frame_step_target: None,
            single_step_mode: false,
        })
    }

    /// Enable or disable single-step mode for deterministic testing.
    ///
    /// When enabled, the simulation only advances when `step()` is called,
    /// executing a fixed batch of instructions per step. This is useful for
    /// headless tests that need deterministic behavior.
    pub fn set_single_step_mode(&mut self, enabled: bool) {
        self.single_step_mode = enabled;
        self.sim_thread.set_single_step_mode(enabled);
    }

    /// Load an ELF file and reset the simulation.
    pub fn load_elf(&mut self, path: &Path) -> Result<(), String> {
        log::info!("Loading ELF: {}", path.display());

        // Send load command to simulation thread
        self.sim_thread.load_elf(&path.to_path_buf())?;

        // Wait for load to complete (with timeout)
        let start = Instant::now();
        let timeout = std::time::Duration::from_secs(30);
        loop {
            if let Some(state) = self.sim_thread.poll_state() {
                match state {
                    SimState::ELFLoaded => {
                        log::info!("ELF loaded successfully, simulation ready");
                        break;
                    }
                    SimState::Error(e) => {
                        return Err(e);
                    }
                    _ => {}
                }
            }
            if start.elapsed() > timeout {
                return Err("Timeout waiting for ELF load".to_string());
            }
            std::thread::yield_now();
        }

        // Update state
        self.state = ViewerState::Running;
        self.last_elf_path = Some(path.to_path_buf());
        self.total_cycles = 0;
        self.frame_count = 0;
        self.sim_thread.reset_counters();

        // Resume simulation after loading (unless in single-step mode)
        if !self.single_step_mode {
            self.sim_thread.resume()?;
        }

        // Update window title
        self.update_window_title();

        Ok(())
    }

    /// Reload the last loaded ELF file (for Ctrl+R hotkey).
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
    pub fn toggle_pause(&mut self) {
        self.state = match self.state {
            ViewerState::Running => {
                log::info!("Simulation paused");
                let _ = self.sim_thread.pause();
                ViewerState::Paused
            }
            ViewerState::Paused => {
                log::info!("Simulation resumed");
                let _ = self.sim_thread.resume();
                ViewerState::Running
            }
            other => other,
        };
        self.update_window_title();
    }

    /// Update window title to reflect current state.
    fn update_window_title(&mut self) {
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
        if let Ok(mut backend) = self.video_backend.inner().lock() {
            backend.set_title(&title);
        }
    }

    /// Execute a single iteration of the viewer loop.
    ///
    /// Returns `Ok(true)` if the viewer should continue running,
    /// or `Ok(false)` if the viewer should terminate.
    pub fn step(&mut self) -> Result<bool, String> {
        // Handle window events
        self.handle_events()?;

        // Terminate if exit was requested
        if self.exit_requested {
            log::info!("Exit requested, terminating viewer loop");
            return Ok(false);
        }

        // Check if backend is still active
        if let Ok(backend) = self.video_backend.inner().lock() {
            if !backend.is_active() {
                log::info!("Backend inactive, terminating viewer loop");
                return Ok(false);
            }
        }

        // In single-step mode, request a step from the simulation thread
        if self.single_step_mode && self.state == ViewerState::Running {
            self.sim_thread.single_step()?;
        }

        // Brief yield to allow background thread to execute
        // This is important for tests running in a tight loop
        std::thread::sleep(std::time::Duration::from_micros(100));

        // Poll for state updates from the simulation thread
        while let Some(state) = self.sim_thread.poll_state() {
            match state {
                SimState::StepCompleted {
                    instructions,
                    frame_presented,
                    tohost,
                } => {
                    self.total_cycles += instructions;

                    if frame_presented {
                        self.frame_count += 1;
                    }

                    if let Some(value) = tohost {
                        log::info!("Program halted with tohost value: 0x{:08x}", value);
                        self.state = ViewerState::Halted;
                        self.update_window_title();
                    }

                    if self.config.max_cycles > 0 && self.total_cycles >= self.config.max_cycles {
                        log::info!("Max cycles reached: {}", self.total_cycles);
                        self.state = ViewerState::Halted;
                        let _ = self.sim_thread.pause();
                        self.update_window_title();
                    }
                }
                SimState::Halted(value) => {
                    log::info!("Simulation halted with tohost: 0x{:08x}", value);
                    self.state = ViewerState::Halted;
                    self.update_window_title();
                }
                SimState::Error(e) => {
                    log::error!("Simulation error: {}", e);
                    self.state = ViewerState::Halted;
                    self.update_window_title();
                }
                SimState::Paused => {
                    if self.state == ViewerState::Running {
                        self.state = ViewerState::Paused;
                    }
                }
                SimState::Running => {
                    if self.state == ViewerState::Paused {
                        self.state = ViewerState::Running;
                    }
                }
                _ => {}
            }
        }

        // Check frame presented from video callback (for GUI mode where we need to track)
        let frame_presented = self.video_backend.take_frame_presented();

        // Update video backend
        if let Ok(mut backend) = self.video_backend.inner().lock() {
            backend.update()?;
        }

        // Handle frame stepping in non-single-step mode
        if !self.single_step_mode && frame_presented {
            if let Some(target) = self.frame_step_target {
                if self.frame_count >= target {
                    log::info!("Frame step target reached: {} frames", self.frame_count);
                    self.frame_step_target = None;
                    self.state = ViewerState::Paused;
                    let _ = self.sim_thread.pause();
                    self.update_window_title();
                }
            }
        }

        Ok(true)
    }

    /// Main viewer loop.
    pub fn run(&mut self) -> Result<(), String> {
        log::info!("Starting viewer main loop");

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

        log::info!("Viewer loop ended");

        // Terminate simulation thread
        self.sim_thread.terminate()?;

        Ok(())
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
                    log::info!("Close event received, exit requested");
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
                if self.state == ViewerState::Running {
                    self.sim_thread.pause()?;
                    self.state = ViewerState::Paused;
                    self.update_window_title();
                }
            }
            TestCommand::Resume => {
                log::info!("Test command: Resume");
                if self.state == ViewerState::Paused {
                    self.sim_thread.resume()?;
                    self.state = ViewerState::Running;
                    self.update_window_title();
                }
            }
            TestCommand::StepFrames(count) => {
                log::info!("Test command: Step {} frames", count);
                self.frame_step_target = Some(self.frame_count + count);
                if self.state == ViewerState::Paused || self.state == ViewerState::Idle {
                    self.sim_thread.resume()?;
                    self.state = ViewerState::Running;
                    self.update_window_title();
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

// Specialized methods for HeadlessSimViewer
use crate::headless_backends::{
    CapturedAudioChunk, CapturedFrame, HeadlessAudioBackend, HeadlessEventSource,
    HeadlessVideoBackend,
};

impl SimViewer<HeadlessVideoBackend, HeadlessAudioBackend, HeadlessEventSource> {
    /// Push an event into the headless event source.
    pub fn push_event(&mut self, event: ViewerEvent) -> Result<(), String> {
        self.event_source.push_event(event);
        Ok(())
    }

    /// Get captured video frames (for headless mode testing).
    pub fn get_video_frames(&self) -> Vec<CapturedFrame> {
        if let Ok(backend) = self.video_backend.inner().lock() {
            backend.get_frames().to_vec()
        } else {
            Vec::new()
        }
    }

    /// Get captured audio chunks (for headless mode testing).
    pub fn get_audio_chunks(&self) -> Vec<CapturedAudioChunk> {
        if let Ok(backend) = self.audio_backend.inner().lock() {
            backend.get_chunks().to_vec()
        } else {
            Vec::new()
        }
    }

    /// Get current audio configuration (for headless mode testing).
    pub fn get_audio_config(&self) -> Option<AudioConfig> {
        if let Ok(backend) = self.audio_backend.inner().lock() {
            backend.get_current_config()
        } else {
            None
        }
    }
}
