use crate::backend_traits::{
    AudioBackend, EventSource, Key, KeyModifiers, TestCommand, VideoBackend, ViewerEvent,
};
use crate::simulator_controller::SimulatorController;
use std::path::{Path, PathBuf};
use std::time::Instant;

// Performance constants
const INSTRUCTIONS_PER_FRAME: u64 = 10000; // Adjust this to control simulation speed

pub struct ViewerConfig {
    #[allow(dead_code)] // Used by main.rs to create backends
    pub initial_width: u32,
    #[allow(dead_code)] // Used by main.rs to create backends
    pub initial_height: u32,
    pub max_cycles: u64,
    #[allow(dead_code)]
    pub print_inst_trace: bool,
}

pub struct SimViewer<V: VideoBackend, A: AudioBackend, E: EventSource> {
    /// Simulation controller (manages CPU and bus)
    controller: SimulatorController,

    /// Video backend (generic)
    video_backend: V,

    /// Audio backend (generic)
    audio_backend: A,

    /// Event source (generic)
    event_source: E,

    /// Current configuration
    config: ViewerConfig,

    /// Current simulation state
    state: ViewerState,

    /// Last loaded ELF file path (for reload)
    last_elf_path: Option<PathBuf>,

    /// Cycle counter
    total_cycles: u64,

    /// Frame counter (incremented each time a frame is presented)
    frame_count: u64,

    /// Exit requested by user (e.g., Escape key)
    exit_requested: bool,

    /// Frame step target (for StepFrames test command)
    frame_step_target: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewerState {
    /// No program loaded
    Idle,
    /// Program loaded and running
    Running,
    /// Program loaded but paused
    Paused,
    /// Program completed (tohost written)
    Halted,
}

impl<V: VideoBackend, A: AudioBackend, E: EventSource> SimViewer<V, A, E> {
    /// Create a new SimViewer with dependency injection
    pub fn new(
        config: ViewerConfig,
        video_backend: V,
        audio_backend: A,
        event_source: E,
    ) -> Result<Self, String> {
        // Create controller (handles simulator setup with Video/Audio devices)
        let controller = SimulatorController::new()?;

        Ok(SimViewer {
            controller,
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
        })
    }

    /// Load an ELF file and reset the simulation
    pub fn load_elf(&mut self, path: &Path) -> Result<(), String> {
        log::info!("Loading ELF: {}", path.display());

        // Load ELF into controller (this resets the CPU)
        self.controller.load_elf(path)?;

        // Update state
        self.state = ViewerState::Running;
        self.last_elf_path = Some(path.to_path_buf());
        self.total_cycles = 0;

        // Update window title
        self.update_window_title();

        log::info!("ELF loaded successfully, simulation ready");
        Ok(())
    }

    /// Reload the last loaded ELF file (for Ctrl+R hotkey)
    pub fn reload_last_elf(&mut self) -> Result<(), String> {
        match &self.last_elf_path {
            Some(path) => {
                let path = path.clone(); // Avoid borrow issue
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
        self.state = match self.state {
            ViewerState::Running => {
                log::info!("Simulation paused");
                ViewerState::Paused
            }
            ViewerState::Paused => {
                log::info!("Simulation resumed");
                ViewerState::Running
            }
            other => other, // Idle and Halted states don't change
        };
        self.update_window_title();
    }

    /// Update window title to reflect current state
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
        self.video_backend.set_title(&title);
    }

    /// Execute a single iteration of the viewer loop
    ///
    /// Returns `Ok(true)` if the viewer should continue running,
    /// or `Ok(false)` if the viewer should terminate.
    pub fn step(&mut self) -> Result<bool, String> {
        // Handle window events (keyboard, close, test commands)
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

        // Step simulation if running
        if self.state == ViewerState::Running {
            // Step simulation by multiple instructions per frame for performance
            match self.controller.step_instructions(INSTRUCTIONS_PER_FRAME) {
                Ok(result) => {
                    // Increment instruction counter
                    self.total_cycles += INSTRUCTIONS_PER_FRAME;

                    // Check if simulation halted
                    if result.tohost_value.is_some() {
                        log::info!(
                            "Program halted with tohost value: 0x{:08x}",
                            result.tohost_value.unwrap()
                        );
                        self.state = ViewerState::Halted;
                        self.update_window_title();
                    }

                    // Check if max cycles reached
                    if self.config.max_cycles > 0 && self.total_cycles >= self.config.max_cycles {
                        log::info!("Max cycles reached: {}", self.total_cycles);
                        self.state = ViewerState::Halted;
                        self.update_window_title();
                    }
                }
                Err(e) => {
                    log::error!("Simulation error: {}", e);
                    self.state = ViewerState::Halted;
                    self.update_window_title();
                }
            }
        }

        // Pull video frames from controller and send to backend
        let frame_presented = if let Some((frame_data, config)) = self.controller.get_video_frame()
        {
            self.video_backend.process_frame(&frame_data, &config)?;
            true
        } else {
            false
        };

        // Pull audio samples from controller and send to audio backend
        let audio_samples = self.controller.get_audio_samples(4096);
        if !audio_samples.is_empty() {
            log::debug!(
                "Pushing {} audio samples to backend (cycle: {})",
                audio_samples.len(),
                self.total_cycles
            );
            self.audio_backend.push_samples(&audio_samples);
        }

        // Update video backend (display or capture)
        self.video_backend.update()?;

        // Increment frame counter only if a frame was actually presented
        if frame_presented {
            self.frame_count += 1;

            // Check if frame step target reached
            if let Some(target) = self.frame_step_target {
                if self.frame_count >= target {
                    log::info!("Frame step target reached: {} frames", self.frame_count);
                    self.frame_step_target = None;
                    self.state = ViewerState::Paused;
                    self.update_window_title();
                }
            }
        }

        Ok(true)
    }

    /// Main viewer loop
    pub fn run(&mut self) -> Result<(), String> {
        log::info!("Starting viewer main loop");

        // Record startup time to compute total elapsed time per iteration
        let startup_time = Instant::now();

        loop {
            let frame_start = Instant::now();

            // Execute one step of the viewer loop
            if !self.step()? {
                break;
            }

            // Print timing info: total elapsed since startup (s) and current iteration duration (ms)
            let total_elapsed_s = startup_time.elapsed().as_secs_f64();
            let iteration_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
            log::info!(
                "Elapsed: {:.2} s (iteration: {:.2} ms)",
                total_elapsed_s,
                iteration_ms
            );
        }

        log::info!("Viewer loop ended");
        Ok(())
    }

    /// Handle window events (keyboard, close, test commands)
    fn handle_events(&mut self) -> Result<(), String> {
        // Get events from event source
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

    /// Handle test commands (for headless mode)
    fn handle_test_command(&mut self, cmd: TestCommand) -> Result<(), String> {
        match cmd {
            TestCommand::LoadELF(path) => {
                log::info!("Test command: Load ELF {:?}", path);
                self.load_elf(&path)?;
            }
            TestCommand::Pause => {
                log::info!("Test command: Pause");
                if self.state == ViewerState::Running {
                    self.state = ViewerState::Paused;
                    self.update_window_title();
                }
            }
            TestCommand::Resume => {
                log::info!("Test command: Resume");
                if self.state == ViewerState::Paused {
                    self.state = ViewerState::Running;
                    self.update_window_title();
                }
            }
            TestCommand::StepFrames(count) => {
                log::info!("Test command: Step {} frames", count);
                // Set frame step target and resume execution
                self.frame_step_target = Some(self.frame_count + count);
                if self.state == ViewerState::Paused || self.state == ViewerState::Idle {
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

// Specialized methods for HeadlessSimViewer
use crate::headless_backends::{
    CapturedFrame, HeadlessAudioBackend, HeadlessEventSource, HeadlessVideoBackend,
};

impl SimViewer<HeadlessVideoBackend, HeadlessAudioBackend, HeadlessEventSource> {
    /// Push an event into the headless event source
    pub fn push_event(&mut self, event: ViewerEvent) -> Result<(), String> {
        self.event_source.push_event(event);
        Ok(())
    }

    /// Get captured video frames (for headless mode testing)
    pub fn get_video_frames(&self) -> &[CapturedFrame] {
        self.video_backend.get_frames()
    }
}
