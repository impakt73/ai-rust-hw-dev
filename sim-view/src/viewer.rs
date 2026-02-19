use crate::backend_traits::{
    AudioBackend, EventSource, Key, KeyModifiers, TestCommand, VideoBackend, ViewerEvent,
};
use crate::shared_buffers::{SharedAudioBuffer, SharedVideoBuffer};
use bus_shared::{Audio, AudioConfig, Video, VideoConfig, AUDIO_BASE, VIDEO_BASE};
use device_runtime::{
    create_device_runtime, BusDeviceRegistration, BusEvent, DeviceRuntime, DeviceRuntimeType,
    ResetKind,
};
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
    #[allow(dead_code)]
    pub max_cycles: u64,
    #[allow(dead_code)]
    pub print_inst_trace: bool,
    pub runtime_type: DeviceRuntimeType,
}

#[derive(Debug, Default)]
struct FrameTimingMetrics {
    frames_presented: u64,
    total_frame_time_ns: u64,
    last_frame_time: Option<Instant>,
}

/// Performance tracking for logging and window title
struct PerformanceMetrics {
    /// Time of last log message
    last_log_time: Instant,
    /// Number of frames presented since last log
    frames_since_last_log: u64,
    /// Total frame time since last log in nanoseconds
    total_frame_time_ns_since_last_log: u64,
    /// Last frame count (to compute delta)
    last_frames: u64,
    /// Last frame time total (to compute delta)
    last_frame_time_ns: u64,
    /// Current performance string (for window title)
    current_perf_string: String,
}

pub struct SimViewer<V: VideoBackend, A: AudioBackend, E: EventSource> {
    /// Device runtime backend
    runtime: Box<dyn DeviceRuntime>,

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

    /// Frame counter (incremented each time a frame is presented)
    frame_count: u64,

    /// Exit requested by user (e.g., Escape key)
    exit_requested: bool,

    /// Flag to track if a frame was presented in the current step
    frame_presented_this_step: Arc<Mutex<bool>>,

    /// Shared frame timing metrics collected by callbacks
    frame_timing: Arc<Mutex<FrameTimingMetrics>>,

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

        let frame_timing = Arc::new(Mutex::new(FrameTimingMetrics::default()));

        // Create Video device with callback that pushes to shared buffer
        let video_buffer_for_callback = video_buffer.clone();
        let frame_presented_clone = Arc::clone(&frame_presented_this_step);
        let frame_timing_clone = Arc::clone(&frame_timing);
        let video_callback = move |data: &[u8], video_config: &VideoConfig| {
            video_buffer_for_callback.push_frame(data.to_vec(), *video_config);
            if let Ok(mut frame_presented) = frame_presented_clone.lock() {
                *frame_presented = true;
            }

            if let Ok(mut metrics) = frame_timing_clone.lock() {
                let now = Instant::now();
                if let Some(last_time) = metrics.last_frame_time {
                    let frame_time = now.duration_since(last_time);
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
            audio_buffer_for_config.set_config(*audio_config);
            if audio_config_tx_for_callback.send(*audio_config).is_err() {
                log::warn!("Failed to forward audio config update to viewer thread");
            }
        };
        let audio_device = Box::new(Audio::new(Some(sample_callback), Some(config_callback)));

        let runtime = create_device_runtime(
            config.runtime_type,
            Some(vec![
                BusDeviceRegistration {
                    base_addr: VIDEO_BASE,
                    device: video_device,
                },
                BusDeviceRegistration {
                    base_addr: AUDIO_BASE,
                    device: audio_device,
                },
            ]),
        )
        .map_err(|e| format!("Failed to create device runtime: {e}"))?;

        let mut viewer = SimViewer {
            runtime,
            video_backend,
            audio_backend,
            audio_config_rx,
            event_source,
            state: ViewerState::Idle,
            last_elf_path: None,
            frame_count: 0,
            exit_requested: false,
            frame_presented_this_step,
            frame_timing,
            perf_metrics: PerformanceMetrics {
                last_log_time: Instant::now(),
                frames_since_last_log: 0,
                total_frame_time_ns_since_last_log: 0,
                last_frames: 0,
                last_frame_time_ns: 0,
                current_perf_string: String::new(),
            },
        };

        viewer.update_window_title();
        Ok(viewer)
    }

    /// Load an ELF file and boot the runtime
    pub fn load_elf(&mut self, path: &Path) -> Result<(), String> {
        log::info!("Loading ELF: {}", path.display());

        self.runtime
            .reset(ResetKind::System)
            .map_err(|e| format!("Failed to reset runtime before ELF load: {e}"))?;

        let entry = self
            .runtime
            .load_elf(path)
            .map_err(|e| format!("Failed to load ELF: {e}"))?;

        self.runtime
            .boot_cpu(entry)
            .map_err(|e| format!("Failed to boot CPU at entry point 0x{entry:08x}: {e}"))?;

        if let Ok(mut timing) = self.frame_timing.lock() {
            *timing = FrameTimingMetrics::default();
        }

        self.state = ViewerState::Running;
        self.last_elf_path = Some(path.to_path_buf());
        self.update_window_title();

        log::info!("ELF loaded and booted at entry point 0x{entry:08x}");
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

    fn update_performance_metrics(&mut self) {
        let (frames_presented, total_frame_time_ns) = if let Ok(metrics) = self.frame_timing.lock()
        {
            (metrics.frames_presented, metrics.total_frame_time_ns)
        } else {
            (0, 0)
        };

        let frames_delta = frames_presented.saturating_sub(self.perf_metrics.last_frames);
        let frame_time_ns_delta =
            total_frame_time_ns.saturating_sub(self.perf_metrics.last_frame_time_ns);

        self.perf_metrics.frames_since_last_log += frames_delta;
        self.perf_metrics.total_frame_time_ns_since_last_log += frame_time_ns_delta;

        self.perf_metrics.last_frames = frames_presented;
        self.perf_metrics.last_frame_time_ns = total_frame_time_ns;

        let elapsed = self.perf_metrics.last_log_time.elapsed();
        if elapsed >= Duration::from_secs(1) {
            let perf_string = if self.perf_metrics.frames_since_last_log > 0 {
                let avg_frame_time_ms =
                    (self.perf_metrics.total_frame_time_ns_since_last_log as f64 / 1_000_000.0)
                        / self.perf_metrics.frames_since_last_log as f64;
                format!("{avg_frame_time_ms:.2} ms/frame")
            } else {
                "No frames".to_string()
            };

            log::info!("{perf_string}");
            self.perf_metrics.current_perf_string = perf_string;
            self.update_window_title();

            self.perf_metrics.last_log_time += Duration::from_secs(1);
            self.perf_metrics.frames_since_last_log = 0;
            self.perf_metrics.total_frame_time_ns_since_last_log = 0;
        }
    }

    fn poll_runtime_events(&mut self) -> Result<(), String> {
        while let Some(event) = self
            .runtime
            .poll()
            .map_err(|e| format!("Runtime poll failed: {e}"))?
        {
            if let BusEvent::TohostTermination { value } = event {
                log::info!("Program halted with tohost value: 0x{value:08x}");
                self.state = ViewerState::Halted;
                self.update_window_title();
            }
        }
        Ok(())
    }

    /// Execute a single iteration of the viewer loop
    pub fn step(&mut self) -> Result<bool, String> {
        self.handle_events()?;

        if self.exit_requested {
            log::info!("Exit requested, terminating viewer loop");
            return Ok(false);
        }

        if !self.video_backend.borrow().is_active() {
            log::info!("Backend inactive, terminating viewer loop");
            return Ok(false);
        }

        self.apply_pending_audio_config_updates();

        if let Ok(mut frame_presented) = self.frame_presented_this_step.lock() {
            *frame_presented = false;
        }

        self.poll_runtime_events()?;

        self.video_backend.borrow_mut().update()?;

        let frame_presented = self
            .frame_presented_this_step
            .lock()
            .map(|frame_presented| *frame_presented)
            .unwrap_or(false);

        if frame_presented {
            self.frame_count += 1;
        }

        self.update_performance_metrics();
        std::thread::yield_now();

        Ok(self.state != ViewerState::Halted)
    }

    /// Main viewer loop
    pub fn run(&mut self) -> Result<(), String> {
        log::info!("Starting viewer main loop");

        if self.state == ViewerState::Idle {
            self.update_window_title();
        }

        loop {
            if !self.step()? {
                break;
            }
        }

        log::info!("Viewer loop ended");
        Ok(())
    }

    /// Handle window events (keyboard, close, test commands)
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
    pub fn update_audio_capture(&mut self) {
        self.audio_backend.borrow_mut().update();
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
