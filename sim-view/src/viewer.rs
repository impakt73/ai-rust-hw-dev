use crate::backend_traits::{
    AudioBackend, EventSource, Key, KeyModifiers, TestCommand, VideoBackend, ViewerEvent,
};
use crate::shared_buffers::{SharedAudioBuffer, SharedVideoBuffer};
use crate::simulation_thread::{FrameTimingMetrics, SimRequest, SimResponse, SimulationThread};
use bus_shared::{Audio, AudioConfig, Video, VideoConfig, AUDIO_BASE, VIDEO_BASE};
use device_runtime::BusDeviceRegistration;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct ViewerConfig {
    #[allow(dead_code)] // Used by main.rs to create backends
    pub initial_width: u32,
    #[allow(dead_code)] // Used by main.rs to create backends
    pub initial_height: u32,
    pub max_cycles: u64,
    #[allow(dead_code)]
    pub print_inst_trace: bool,
}

/// Performance tracking for logging and window title
struct PerformanceMetrics {
    /// Time of last log message
    last_log_time: Instant,
    /// Number of frames presented since last log (from sim thread)
    frames_since_last_log: u64,
    /// Total frame time since last log in nanoseconds (from sim thread)
    total_frame_time_ns_since_last_log: u64,
    /// Cycles executed since last log
    cycles_since_last_log: u64,
    /// Last cycles count (to compute delta)
    last_cycles: u64,
    /// Last frame count (to compute delta)
    last_frames: u64,
    /// Last frame time total (to compute delta)
    last_frame_time_ns: u64,
    /// Current performance string (for window title)
    current_perf_string: String,
}

pub struct SimViewer<V: VideoBackend, A: AudioBackend, E: EventSource> {
    /// Simulation thread handle
    sim_thread: SimulationThread,

    /// Video backend (wrapped in Rc<RefCell<>> for backward compatibility)
    video_backend: Rc<RefCell<V>>,

    /// Audio backend (wrapped in Rc<RefCell<>> for backward compatibility)
    audio_backend: Rc<RefCell<A>>,

    /// Pending audio configuration updates from simulation callbacks
    audio_config_rx: mpsc::Receiver<AudioConfig>,

    /// Event source (generic)
    event_source: E,

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

    /// Flag to track if a frame was presented in the current step
    frame_presented_this_step: Arc<Mutex<bool>>,

    /// Performance metrics for logging
    perf_metrics: PerformanceMetrics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewerState {
    /// No program loaded
    Idle,
    /// Program loaded and running
    Running,
    /// Program completed (tohost written)
    Halted,
}

impl<V: VideoBackend + 'static, A: AudioBackend + 'static, E: EventSource> SimViewer<V, A, E> {
    /// Create a new SimViewer with dependency injection
    ///
    /// This function sets up the simulator with Video and Audio devices that push data
    /// to shared buffers, which backends can pull from when needed.
    pub fn new(
        config: ViewerConfig,
        mut video_backend: V,
        mut audio_backend: A,
        event_source: E,
    ) -> Result<Self, String> {
        // Create shared buffers for pull-based data flow
        let video_buffer = SharedVideoBuffer::new();
        let audio_buffer = SharedAudioBuffer::new();

        // Connect backends to shared buffers
        video_backend.set_video_source(video_buffer.clone());
        audio_backend.set_audio_source(audio_buffer.clone());

        // Wrap backends in Rc<RefCell<>> for backward compatibility
        let video_backend = Rc::new(RefCell::new(video_backend));
        let audio_backend = Rc::new(RefCell::new(audio_backend));
        let frame_presented_this_step = Arc::new(Mutex::new(false));
        let (audio_config_tx, audio_config_rx) = mpsc::channel();

        // Create shared frame timing metrics for simulation thread
        let frame_timing = Arc::new(Mutex::new(FrameTimingMetrics::default()));

        // Create Video device with callback that pushes to shared buffer
        let video_buffer_for_callback = video_buffer.clone();
        let frame_presented_clone = Arc::clone(&frame_presented_this_step);
        let frame_timing_clone = Arc::clone(&frame_timing);
        let video_callback = move |data: &[u8], video_config: &VideoConfig| {
            // Push frame data to shared buffer
            video_buffer_for_callback.push_frame(data.to_vec(), *video_config);
            // Mark that a frame was presented
            if let Ok(mut frame_presented) = frame_presented_clone.lock() {
                *frame_presented = true;
            }

            // Track frame timing in simulation thread context
            if let Ok(mut metrics) = frame_timing_clone.lock() {
                let now = Instant::now();
                if let Some(last_time) = metrics.last_frame_time {
                    // Add time since last frame
                    let frame_time = now.duration_since(last_time);
                    // Use saturating conversion to avoid silent truncation
                    metrics.total_frame_time_ns +=
                        frame_time.as_nanos().min(u64::MAX as u128) as u64;
                }
                metrics.last_frame_time = Some(now);
                metrics.frames_presented += 1;
            }
        };
        let video_device = Box::new(Video::new(Some(video_callback)));

        // Create Audio device with callbacks that push to shared buffer
        let audio_buffer_for_samples = audio_buffer.clone();
        let sample_callback = move |samples: &[i16]| {
            // Push samples to shared buffer
            audio_buffer_for_samples.push_samples(samples.to_vec());
        };

        let audio_buffer_for_config = audio_buffer.clone();
        let audio_config_tx_for_callback = audio_config_tx.clone();
        let config_callback = move |audio_config: &AudioConfig| {
            log::info!(
                "Audio config changed: {} Hz, {:?}, {} samples",
                audio_config.sample_rate.to_hz(),
                audio_config.channels,
                audio_config.sample_count
            );
            // Update config in shared buffer
            audio_buffer_for_config.set_config(*audio_config);
            // Forward config update to main thread for backend reconfiguration
            if audio_config_tx_for_callback.send(*audio_config).is_err() {
                log::warn!("Failed to forward audio config update to viewer thread");
            }
        };
        let audio_device = Box::new(Audio::new(Some(sample_callback), Some(config_callback)));

        let registrations = vec![
            BusDeviceRegistration {
                base_addr: VIDEO_BASE,
                device: video_device,
            },
            BusDeviceRegistration {
                base_addr: AUDIO_BASE,
                device: audio_device,
            },
        ];

        // Create simulation thread with frame timing metrics
        let sim_thread = SimulationThread::new(registrations, config.max_cycles, frame_timing)?;

        Ok(SimViewer {
            sim_thread,
            video_backend,
            audio_backend,
            audio_config_rx,
            event_source,
            state: ViewerState::Idle,
            last_elf_path: None,
            total_cycles: 0,
            frame_count: 0,
            exit_requested: false,
            frame_presented_this_step,
            perf_metrics: PerformanceMetrics {
                last_log_time: Instant::now(),
                frames_since_last_log: 0,
                total_frame_time_ns_since_last_log: 0,
                cycles_since_last_log: 0,
                last_cycles: 0,
                last_frames: 0,
                last_frame_time_ns: 0,
                current_perf_string: String::new(),
            },
        })
    }

    /// Load an ELF file and reset the simulation
    pub fn load_elf(&mut self, path: &Path) -> Result<(), String> {
        log::info!("Loading ELF: {}", path.display());

        // Send load request to simulation thread
        self.sim_thread
            .send_request(SimRequest::LoadELF(path.to_path_buf()))?;

        // Wait for response
        match self.sim_thread.recv_response()? {
            SimResponse::ELFLoaded => {
                // Update state and start runtime execution.
                self.state = ViewerState::Running;
                self.last_elf_path = Some(path.to_path_buf());
                self.total_cycles = 0;
                self.sim_thread.send_request(SimRequest::Run)?;

                // Update window title
                self.update_window_title();

                log::info!("ELF loaded successfully, simulation ready");
                Ok(())
            }
            SimResponse::Error(e) => Err(e),
            _ => Err("Unexpected response from simulation thread".to_string()),
        }
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

    /// Update window title to reflect current state
    fn update_window_title(&mut self) {
        let base_title = match (&self.last_elf_path, self.state) {
            (Some(path), ViewerState::Running) => {
                format!("sim-view - {} [RUNNING]", path.display())
            }
            (Some(path), ViewerState::Halted) => {
                format!("sim-view - {} [HALTED]", path.display())
            }
            (Some(path), ViewerState::Idle) => {
                format!("sim-view - {} [IDLE]", path.display())
            }
            (None, _) => "sim-view - No program loaded".to_string(),
        };

        // Append performance metrics if available
        let title = if !self.perf_metrics.current_perf_string.is_empty() {
            format!("{} - {}", base_title, self.perf_metrics.current_perf_string)
        } else {
            base_title
        };

        self.video_backend.borrow_mut().set_title(&title);
    }

    /// Apply pending audio configuration updates from simulation callbacks.
    fn apply_pending_audio_config_updates(&mut self) {
        while let Ok(audio_config) = self.audio_config_rx.try_recv() {
            self.audio_backend.borrow_mut().set_config(&audio_config);
        }
    }

    /// Format a cycle count in a human-friendly way (e.g., "1.5M", "234K")
    fn format_cycles(cycles: u64) -> String {
        if cycles >= 1_000_000 {
            format!("{:.1}M", cycles as f64 / 1_000_000.0)
        } else if cycles >= 1_000 {
            format!("{:.1}K", cycles as f64 / 1_000.0)
        } else {
            format!("{}", cycles)
        }
    }

    /// Update performance metrics and log if one second has elapsed
    /// Now receives frame timing data from simulation thread progress updates
    fn update_performance_metrics(&mut self, frames_presented: u64, total_frame_time_ns: u64) {
        // Calculate deltas from last progress update
        let frames_delta = frames_presented.saturating_sub(self.perf_metrics.last_frames);
        let frame_time_ns_delta =
            total_frame_time_ns.saturating_sub(self.perf_metrics.last_frame_time_ns);
        let cycles_delta = self
            .total_cycles
            .saturating_sub(self.perf_metrics.last_cycles);

        // Accumulate for this logging interval
        self.perf_metrics.frames_since_last_log += frames_delta;
        self.perf_metrics.total_frame_time_ns_since_last_log += frame_time_ns_delta;
        self.perf_metrics.cycles_since_last_log += cycles_delta;

        // Update last values
        self.perf_metrics.last_frames = frames_presented;
        self.perf_metrics.last_frame_time_ns = total_frame_time_ns;
        self.perf_metrics.last_cycles = self.total_cycles;

        // Check if one second has elapsed
        let elapsed = self.perf_metrics.last_log_time.elapsed();
        if elapsed >= Duration::from_secs(1) {
            // Normalize cycles to cycles per second based on actual elapsed time
            let elapsed_secs = elapsed.as_secs_f64();
            let cycles_per_sec =
                (self.perf_metrics.cycles_since_last_log as f64 / elapsed_secs) as u64;
            let cycles_formatted = Self::format_cycles(cycles_per_sec);

            let perf_string = if self.perf_metrics.frames_since_last_log > 0 {
                // Calculate average frame time in milliseconds from simulation thread data
                let avg_frame_time_ms =
                    (self.perf_metrics.total_frame_time_ns_since_last_log as f64 / 1_000_000.0)
                        / self.perf_metrics.frames_since_last_log as f64;
                format!(
                    "{:.2} ms/frame, {} cycles/s",
                    avg_frame_time_ms, cycles_formatted
                )
            } else {
                // No frames presented
                format!("{} cycles/s", cycles_formatted)
            };

            // Log the performance info
            log::info!("{}", perf_string);

            // Update the current performance string for window title
            self.perf_metrics.current_perf_string = perf_string;

            // Update window title with new performance info
            self.update_window_title();

            // Reset the metrics for the next interval
            // Add the target duration to maintain consistent intervals
            self.perf_metrics.last_log_time += Duration::from_secs(1);
            self.perf_metrics.frames_since_last_log = 0;
            self.perf_metrics.total_frame_time_ns_since_last_log = 0;
            self.perf_metrics.cycles_since_last_log = 0;
        }
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

        self.apply_pending_audio_config_updates();

        // Reset frame presented flag before stepping
        if let Ok(mut frame_presented) = self.frame_presented_this_step.lock() {
            *frame_presented = false;
        }

        // When in the Idle state (no ELF loaded), step() will not advance simulation.
        if self.state == ViewerState::Idle {
            log::debug!("step() called while viewer is Idle: no simulation step will be executed");
        }

        if self.state == ViewerState::Running {
            while let Some(response) = self.sim_thread.try_recv_response()? {
                match response {
                    SimResponse::RunCompleted {
                        tohost_value,
                        cycles_executed,
                    } => {
                        self.total_cycles = cycles_executed;
                        if let Some(tohost) = tohost_value {
                            log::info!("Program halted with tohost value: 0x{:08x}", tohost);
                        } else {
                            log::info!("Max cycles reached");
                        }
                        self.state = ViewerState::Halted;
                        self.update_window_title();
                    }
                    SimResponse::Error(e) => {
                        log::error!("Simulation error: {}", e);
                        self.state = ViewerState::Halted;
                        self.update_window_title();
                    }
                    SimResponse::Progress {
                        cycles_executed,
                        frames_presented,
                        total_frame_time_ns,
                    } => {
                        self.total_cycles = cycles_executed;
                        self.update_performance_metrics(frames_presented, total_frame_time_ns);
                    }
                    _ => {}
                }
            }
        }

        // Check if a frame was presented during this step (set by video callback)
        let frame_presented = self
            .frame_presented_this_step
            .lock()
            .map(|frame_presented| *frame_presented)
            .unwrap_or(false);

        // Update video backend (pull frames from shared buffer and display/capture)
        self.video_backend.borrow_mut().update()?;

        if self.state == ViewerState::Running {
            std::thread::sleep(Duration::from_millis(1));
        }

        // Increment frame counter only if a frame was actually presented
        if frame_presented {
            self.frame_count += 1;
        }

        Ok(true)
    }

    /// Main viewer loop
    ///
    /// Sends a run request to the simulation thread and waits for completion.
    /// The main thread periodically updates the UI while the simulation runs.
    pub fn run(&mut self) -> Result<(), String> {
        log::info!("Starting viewer main loop");

        // Send run request to simulation thread
        self.sim_thread.send_request(SimRequest::Run)?;
        self.state = ViewerState::Running;
        self.update_window_title();

        loop {
            // Handle events
            self.handle_events()?;

            // Check for exit request
            if self.exit_requested {
                log::info!("Exit requested");
                break;
            }

            // Check if backend is still active
            if !self.video_backend.borrow().is_active() {
                log::info!("Backend inactive");
                break;
            }

            self.apply_pending_audio_config_updates();

            // Check for responses from simulation thread (non-blocking)
            if let Some(response) = self.sim_thread.try_recv_response()? {
                match response {
                    SimResponse::RunCompleted {
                        tohost_value,
                        cycles_executed,
                    } => {
                        self.total_cycles = cycles_executed;
                        if let Some(tohost) = tohost_value {
                            log::info!("Program halted with tohost value: 0x{:08x}", tohost);
                        } else {
                            log::info!("Max cycles reached");
                        }
                        self.state = ViewerState::Halted;
                        self.update_window_title();
                        break;
                    }
                    SimResponse::Error(e) => {
                        log::error!("Simulation error: {}", e);
                        self.state = ViewerState::Halted;
                        self.update_window_title();
                        break;
                    }
                    SimResponse::Progress {
                        cycles_executed,
                        frames_presented,
                        total_frame_time_ns,
                    } => {
                        // Update total cycles from periodic progress updates
                        self.total_cycles = cycles_executed;
                        // Update performance metrics with simulation thread data
                        self.update_performance_metrics(frames_presented, total_frame_time_ns);
                    }
                    _ => {
                        // Ignore other responses
                    }
                }
            }

            // Update video backend (pull frames from shared buffer and display/capture)
            self.video_backend.borrow_mut().update()?;
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

    /// Update audio backend to pull and capture samples (headless-specific)
    ///
    /// This should be called periodically to capture audio samples in headless mode.
    /// GUI mode doesn't need this as audio is pulled automatically via the stream callback.
    pub fn update_audio_capture(&mut self) {
        self.audio_backend.borrow_mut().update();
    }

    /// Get captured video frames (for headless mode testing)
    ///
    /// Note: This clones the frame data. This is acceptable because this method is only
    /// called infrequently (at test completion for verification). The hot path
    /// (video callback during simulation) pushes to shared buffer with zero copies.
    pub fn get_video_frames(&self) -> Vec<CapturedFrame> {
        self.video_backend.borrow().get_frames().to_vec()
    }

    /// Get captured audio chunks (for headless mode testing)
    ///
    /// Note: This clones the chunk data. This is acceptable because this method is only
    /// called infrequently (at test completion for verification). The hot path
    /// (audio callback during simulation) pushes to shared buffer with zero copies.
    pub fn get_audio_chunks(&self) -> Vec<CapturedAudioChunk> {
        self.audio_backend.borrow().get_chunks().to_vec()
    }

    /// Get current audio configuration (for headless mode testing)
    pub fn get_audio_config(&self) -> Option<AudioConfig> {
        self.audio_backend.borrow().get_current_config()
    }
}
