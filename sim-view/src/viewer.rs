use crate::audio_stream::AudioStream;
use crate::simulator_controller::SimulatorController;
use crate::video_window::{Key, KeyModifiers, VideoWindow, WindowEvent};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub struct ViewerConfig {
    pub initial_width: u32,
    pub initial_height: u32,
    pub max_cycles: u64,
    #[allow(dead_code)]
    pub print_inst_trace: bool,
}

pub struct SimViewer {
    /// Simulation controller (manages CPU and bus)
    controller: SimulatorController,

    /// Video window for display
    video_window: VideoWindow,

    /// Audio output stream
    audio_stream: AudioStream,

    /// Current configuration
    config: ViewerConfig,

    /// Current simulation state
    state: ViewerState,

    /// Last loaded ELF file path (for reload)
    last_elf_path: Option<PathBuf>,

    /// Cycle counter
    total_cycles: u64,

    /// FPS timing
    _last_frame_time: Instant,
    target_frame_duration: Duration,
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

impl SimViewer {
    /// Create a new SimViewer with the given configuration
    pub fn new(config: ViewerConfig) -> Result<Self, String> {
        // Create controller (handles simulator setup with Video/Audio devices)
        let controller = SimulatorController::new()?;

        // Create video window with initial size
        let video_window = VideoWindow::new(
            config.initial_width as usize,
            config.initial_height as usize,
        )?;

        // Create audio stream
        let audio_stream = AudioStream::new()?;

        // Target 60 FPS
        let target_frame_duration = Duration::from_millis(16); // ~60 FPS

        Ok(SimViewer {
            controller,
            video_window,
            audio_stream,
            config,
            state: ViewerState::Idle,
            last_elf_path: None,
            total_cycles: 0,
            _last_frame_time: Instant::now(),
            target_frame_duration,
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
        self.video_window.set_title(&title);
    }

    /// Main viewer loop
    pub fn run(&mut self) -> Result<(), String> {
        log::info!("Starting viewer main loop");

        while self.video_window.is_open() {
            let frame_start = Instant::now();

            // Handle window events (keyboard, drag-and-drop, close)
            self.handle_events()?;

            // Step simulation if running
            if self.state == ViewerState::Running {
                // Step simulation by multiple instructions per frame for performance
                // Adjust this value based on desired simulation speed
                let instructions_per_frame = 10000; // ~10K instructions per frame

                match self.controller.step_instructions(instructions_per_frame) {
                    Ok(result) => {
                        // Increment instruction counter
                        self.total_cycles += instructions_per_frame;

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
                        if self.config.max_cycles > 0 && self.total_cycles >= self.config.max_cycles
                        {
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

            // Pull video frames from controller and send to window
            if let Some((frame_data, config)) = self.controller.get_video_frame() {
                self.video_window
                    .process_video_frame(&frame_data, &config)?;
            }

            // Pull audio samples from controller and send to audio stream
            let audio_samples = self.controller.get_audio_samples(4096);
            if !audio_samples.is_empty() {
                self.audio_stream.push_samples(&audio_samples);
            }

            // Update video window display
            self.video_window.update()?;

            // Frame pacing to maintain target FPS
            let elapsed = frame_start.elapsed();
            if elapsed < self.target_frame_duration {
                std::thread::sleep(self.target_frame_duration - elapsed);
            }
        }

        log::info!("Viewer loop ended");
        Ok(())
    }

    /// Handle window events (keyboard, drag-and-drop, close)
    fn handle_events(&mut self) -> Result<(), String> {
        // Get events from window
        let events = self.video_window.get_events();

        for event in events {
            match event {
                WindowEvent::KeyPressed(key, modifiers) => {
                    self.handle_key_press(key, modifiers)?;
                }
                WindowEvent::FileDrop(path) => {
                    self.handle_file_drop(path)?;
                }
                WindowEvent::Close => {
                    log::info!("Window close requested");
                    self.video_window.close();
                }
            }
        }

        Ok(())
    }

    /// Handle keyboard input
    fn handle_key_press(&mut self, key: Key, modifiers: KeyModifiers) -> Result<(), String> {
        match key {
            Key::Escape => {
                log::info!("Escape pressed, exiting");
                self.video_window.close();
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

    /// Handle file drop event
    fn handle_file_drop(&mut self, path: PathBuf) -> Result<(), String> {
        log::info!("File dropped: {}", path.display());

        // Check if file has .elf extension
        if path.extension().and_then(|s| s.to_str()) != Some("elf") {
            log::warn!("Dropped file is not an ELF file (expected .elf extension)");
            return Ok(());
        }

        // Load the ELF file
        self.load_elf(&path)
    }
}
