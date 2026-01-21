use crate::backend_traits::{
    AudioBackend, EventSource, Key, KeyModifiers, TestCommand, VideoBackend, ViewerEvent,
};
use cpu_sim::{
    Audio, AudioConfig, InteractiveSimulator, Video, VideoConfig, AUDIO_BASE, VIDEO_BASE,
};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
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
    /// Interactive simulator instance
    simulator: InteractiveSimulator,

    /// Video backend (wrapped in Rc<RefCell<>> for callback access)
    video_backend: Rc<RefCell<V>>,

    /// Audio backend (wrapped in Rc<RefCell<>> for callback access)
    audio_backend: Rc<RefCell<A>>,

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

    /// Flag to track if a frame was presented in the current step
    frame_presented_this_step: Rc<RefCell<bool>>,
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

impl<V: VideoBackend + 'static, A: AudioBackend + 'static, E: EventSource> SimViewer<V, A, E> {
    /// Create a new SimViewer with dependency injection
    ///
    /// This function sets up the simulator with Video and Audio devices that directly
    /// write to the provided backends, eliminating intermediate data copies.
    pub fn new(
        config: ViewerConfig,
        video_backend: V,
        audio_backend: A,
        event_source: E,
    ) -> Result<Self, String> {
        // Wrap backends in Rc<RefCell<>> for shared ownership with callbacks
        let video_backend = Rc::new(RefCell::new(video_backend));
        let audio_backend = Rc::new(RefCell::new(audio_backend));
        let frame_presented_this_step = Rc::new(RefCell::new(false));

        // Create the interactive simulator
        let mut simulator = InteractiveSimulator::new()?;

        // Create Video device with callback that writes directly to video backend
        let video_backend_clone = Rc::clone(&video_backend);
        let frame_presented_clone = Rc::clone(&frame_presented_this_step);
        let video_callback = move |data: &[u8], video_config: &VideoConfig| {
            // Directly call process_frame on the video backend
            if let Err(e) = video_backend_clone
                .borrow_mut()
                .process_frame(data, video_config)
            {
                log::error!("Video backend process_frame error: {}", e);
            }
            // Mark that a frame was presented
            *frame_presented_clone.borrow_mut() = true;
        };
        let video_device = Box::new(Video::new(Some(video_callback)));

        // Create Audio device with callbacks that write directly to audio backend
        let audio_backend_for_samples = Rc::clone(&audio_backend);
        let sample_callback = move |samples: &[i16]| {
            // Directly call push_samples on the audio backend
            audio_backend_for_samples.borrow_mut().push_samples(samples);
        };

        let audio_backend_for_config = Rc::clone(&audio_backend);
        let config_callback = move |audio_config: &AudioConfig| {
            log::info!(
                "Audio config changed: {} Hz, {:?}, {} samples",
                audio_config.sample_rate.to_hz(),
                audio_config.channels,
                audio_config.sample_count
            );
            // Directly call set_config on the audio backend
            audio_backend_for_config
                .borrow_mut()
                .set_config(audio_config);
        };
        let audio_device = Box::new(Audio::new(Some(sample_callback), Some(config_callback)));

        // Register devices with simulator at their standard base addresses
        simulator.register_device(VIDEO_BASE, video_device)?;
        simulator.register_device(AUDIO_BASE, audio_device)?;

        Ok(SimViewer {
            simulator,
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
            frame_presented_this_step,
        })
    }

    /// Load an ELF file and reset the simulation
    pub fn load_elf(&mut self, path: &Path) -> Result<(), String> {
        log::info!("Loading ELF: {}", path.display());

        // Load ELF into simulator (this resets the CPU)
        self.simulator.load_elf(path)?;

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
        self.video_backend.borrow_mut().set_title(&title);
    }

    /// Step the simulation for N instructions
    ///
    /// Returns the result of the last instruction executed, which may contain
    /// a tohost termination value if the program halted.
    fn step_instructions(&mut self, count: u64) -> Result<cpu_sim::SimulationStepResult, String> {
        let mut last_result = None;

        for _ in 0..count {
            let result = self.simulator.step_instruction()?;

            // If program terminated, return early
            if result.tohost_value.is_some() {
                return Ok(result);
            }

            last_result = Some(result);
        }

        // Return last result (or error if no steps were taken)
        last_result.ok_or_else(|| "No instructions executed".to_string())
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
        if !self.video_backend.borrow().is_active() {
            log::info!("Backend inactive, terminating viewer loop");
            return Ok(false);
        }

        // Reset frame presented flag before stepping
        *self.frame_presented_this_step.borrow_mut() = false;

        // Step simulation if running
        if self.state == ViewerState::Running {
            // Step simulation by multiple instructions per frame for performance
            // Note: Video and audio callbacks write directly to backends during step
            match self.step_instructions(INSTRUCTIONS_PER_FRAME) {
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

        // Check if a frame was presented during this step (set by video callback)
        let frame_presented = *self.frame_presented_this_step.borrow();

        // Update video backend (display or capture)
        self.video_backend.borrow_mut().update()?;

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
    pub fn get_video_frames(&self) -> Vec<CapturedFrame> {
        self.video_backend.borrow().get_frames().to_vec()
    }

    /// Get captured audio chunks (for headless mode testing)
    pub fn get_audio_chunks(&self) -> Vec<CapturedAudioChunk> {
        self.audio_backend.borrow().get_chunks().to_vec()
    }

    /// Get current audio configuration (for headless mode testing)
    pub fn get_audio_config(&self) -> Option<AudioConfig> {
        self.audio_backend.borrow().get_current_config()
    }
}
