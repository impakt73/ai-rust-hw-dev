use crate::backend_traits::{
    AudioBackend, EventSource, Key, KeyModifiers, TestCommand, VideoBackend, ViewerEvent,
};
use crate::sim_thread::{AudioData, SimResponse, SimState, SimulationThread};
use cpu_sim::AudioConfig;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub struct ViewerConfig {
    #[allow(dead_code)] // Used by main.rs to create backends
    pub initial_width: u32,
    #[allow(dead_code)] // Used by main.rs to create backends
    pub initial_height: u32,
    pub max_cycles: u64,
    #[allow(dead_code)]
    pub print_inst_trace: bool,
}

/// SimViewer with background simulation thread
///
/// The simulation runs in a background thread while the main thread handles
/// UI updates and event processing. This decouples simulation speed from
/// UI update speed and allows faster simulation on multi-core machines.
///
/// Video frames and audio samples are sent from the background thread via
/// channels and forwarded to the backends on the main thread.
pub struct SimViewer<V: VideoBackend, A: AudioBackend, E: EventSource> {
    /// Background simulation thread handle
    sim_thread: SimulationThread,

    /// Video backend (on main thread)
    video_backend: V,

    /// Audio backend (on main thread)
    audio_backend: A,

    /// Event source (generic)
    event_source: E,

    /// Current configuration (stored but only max_cycles is used for simulation thread)
    #[allow(dead_code)]
    config: ViewerConfig,

    /// Frame counter (incremented each time a frame is presented)
    frame_count: u64,

    /// Exit requested by user (e.g., Escape key)
    exit_requested: bool,

    /// Frame step target (for StepFrames test command)
    frame_step_target: Option<u64>,

    /// Last loaded ELF path (cached for window title updates)
    last_elf_path: Option<PathBuf>,
}

impl<V: VideoBackend, A: AudioBackend, E: EventSource> SimViewer<V, A, E> {
    /// Create a new SimViewer with dependency injection
    ///
    /// This function sets up the simulator in a background thread. Video and audio
    /// data are sent via channels and forwarded to the backends on the main thread.
    pub fn new(
        config: ViewerConfig,
        video_backend: V,
        audio_backend: A,
        event_source: E,
    ) -> Result<Self, String> {
        // Create the background simulation thread
        let sim_thread = SimulationThread::new(config.max_cycles)?;

        Ok(SimViewer {
            sim_thread,
            video_backend,
            audio_backend,
            event_source,
            config,
            frame_count: 0,
            exit_requested: false,
            frame_step_target: None,
            last_elf_path: None,
        })
    }

    /// Load an ELF file and reset the simulation
    pub fn load_elf(&mut self, path: &Path) -> Result<(), String> {
        log::info!("Loading ELF: {}", path.display());

        // Send load command to simulation thread
        self.sim_thread.load_elf(path)?;
        self.last_elf_path = Some(path.to_path_buf());

        // Update window title
        self.update_window_title();

        log::info!("ELF load command sent, simulation ready");
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
    pub fn toggle_pause(&mut self) -> Result<(), String> {
        let current_state = self.sim_thread.state();
        match current_state {
            SimState::Running => {
                self.sim_thread.pause()?;
                log::info!("Simulation paused");
            }
            SimState::Paused => {
                self.sim_thread.resume()?;
                log::info!("Simulation resumed");
            }
            _ => {} // Idle and Halted states don't change
        }
        self.update_window_title();
        Ok(())
    }

    /// Update window title to reflect current state
    fn update_window_title(&mut self) {
        let state = self.sim_thread.state();
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
        // Handle window events (keyboard, close, test commands)
        self.handle_events()?;

        // Terminate if an exit was requested
        if self.exit_requested {
            log::info!("Exit requested, terminating viewer loop");
            self.sim_thread.terminate()?;
            return Ok(false);
        }

        // Check if backend is still active (for GUI mode)
        if !self.video_backend.is_active() {
            log::info!("Backend inactive, terminating viewer loop");
            self.sim_thread.terminate()?;
            return Ok(false);
        }

        // Check if simulation thread is still running
        if !self.sim_thread.is_running() {
            log::info!("Simulation thread stopped, terminating viewer loop");
            return Ok(false);
        }

        // Track if a frame was presented during this step
        let mut frame_presented = false;

        // Poll for responses from simulation thread and forward data to backends
        let responses = self.sim_thread.poll_responses();
        let had_responses = !responses.is_empty();

        for response in responses {
            match response {
                SimResponse::StateChanged(state) => {
                    log::debug!("State changed to {:?}", state);
                    self.update_window_title();
                }
                SimResponse::ELFLoaded(path) => {
                    log::info!("ELF loaded: {}", path.display());
                    self.last_elf_path = Some(path);
                    self.update_window_title();
                }
                SimResponse::ELFLoadError(e) => {
                    log::error!("ELF load error: {}", e);
                }
                SimResponse::Halted(tohost) => {
                    log::info!("Program halted with tohost: 0x{:08x}", tohost);
                    self.update_window_title();
                }
                SimResponse::Error(e) => {
                    log::error!("Simulation error: {}", e);
                    self.update_window_title();
                }
                SimResponse::MaxCyclesReached(cycles) => {
                    log::info!("Max cycles reached: {}", cycles);
                    self.update_window_title();
                }
                SimResponse::CycleCount(cycles) => {
                    log::debug!("Cycle count: {}", cycles);
                }
                SimResponse::VideoFrame(frame_data) => {
                    // Forward video frame to backend
                    if let Err(e) = self
                        .video_backend
                        .process_frame(&frame_data.data, &frame_data.config)
                    {
                        log::error!("Video backend process_frame error: {}", e);
                    } else {
                        frame_presented = true;
                    }
                }
                SimResponse::Audio(audio_data) => {
                    // Forward audio data to backend
                    match audio_data {
                        AudioData::Samples(samples) => {
                            self.audio_backend.push_samples(&samples);
                        }
                        AudioData::ConfigChange(config) => {
                            self.audio_backend.set_config(&config);
                        }
                    }
                }
            }
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
                    self.sim_thread.pause()?;
                    self.update_window_title();
                }
            }
        }

        // If no responses were received, yield to the simulation thread briefly
        // This prevents busy-waiting when the simulation thread is still processing
        if !had_responses {
            std::thread::sleep(std::time::Duration::from_millis(1));
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
                if self.sim_thread.state() == SimState::Running {
                    self.sim_thread.pause()?;
                    self.update_window_title();
                }
            }
            TestCommand::Resume => {
                log::info!("Test command: Resume");
                if self.sim_thread.state() == SimState::Paused {
                    self.sim_thread.resume()?;
                    self.update_window_title();
                }
            }
            TestCommand::StepFrames(count) => {
                log::info!("Test command: Step {} frames", count);
                // Set frame step target and resume execution
                self.frame_step_target = Some(self.frame_count + count);
                let state = self.sim_thread.state();
                if state == SimState::Paused || state == SimState::Idle {
                    self.sim_thread.resume()?;
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
}

// Specialized methods for HeadlessSimViewer
use crate::headless_backends::{
    CapturedAudioChunk, CapturedFrame, HeadlessAudioBackend, HeadlessEventSource,
    HeadlessVideoBackend,
};

impl SimViewer<HeadlessVideoBackend, HeadlessAudioBackend, HeadlessEventSource> {
    /// Push an event into the headless event source
    pub fn push_event(&mut self, event: ViewerEvent) -> Result<(), String> {
        self.event_source.push_event(event);
        Ok(())
    }

    /// Get captured video frames (for headless mode testing)
    ///
    /// Note: This clones the frame data. This is acceptable because this method is only
    /// called infrequently (at test completion for verification). The hot path
    /// (video callback during simulation) writes directly to the backend with zero copies.
    pub fn get_video_frames(&self) -> Vec<CapturedFrame> {
        self.video_backend.get_frames().to_vec()
    }

    /// Get captured audio chunks (for headless mode testing)
    ///
    /// Note: This clones the chunk data. This is acceptable because this method is only
    /// called infrequently (at test completion for verification). The hot path
    /// (audio callback during simulation) writes directly to the backend with zero copies.
    pub fn get_audio_chunks(&self) -> Vec<CapturedAudioChunk> {
        self.audio_backend.get_chunks().to_vec()
    }

    /// Get current audio configuration (for headless mode testing)
    pub fn get_audio_config(&self) -> Option<AudioConfig> {
        self.audio_backend.get_current_config()
    }
}
