use crate::backend_traits::{
    AudioBackend, EventSource, Key, KeyModifiers, TestCommand, VideoBackend, ViewerEvent,
};
use cpu_sim::{
    Audio, AudioConfig, InteractiveSimulator, Video, VideoConfig, AUDIO_BASE, VIDEO_BASE,
};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

// Performance constants
const INSTRUCTIONS_PER_BATCH: u64 = 1000;
const SIM_EVENT_WAIT_TIMEOUT: Duration = Duration::from_millis(1);

enum SimCommand {
    LoadElf {
        path: PathBuf,
        response: mpsc::Sender<Result<(), String>>,
    },
    Pause,
    Resume,
    Terminate,
}

enum SimEvent {
    VideoFrame { data: Vec<u8>, config: VideoConfig },
    AudioSamples(Vec<i16>),
    AudioConfig(AudioConfig),
    Halted(Option<u32>),
    MaxCyclesReached(u64),
    Error(String),
    Terminated,
}

struct SimThreadHandle {
    command_tx: Option<mpsc::Sender<SimCommand>>,
    event_rx: mpsc::Receiver<SimEvent>,
    join_handle: Option<thread::JoinHandle<()>>,
}

struct SimThreadState {
    simulator: InteractiveSimulator,
    is_running: bool,
    total_cycles: u64,
    max_cycles: u64,
}

impl SimThreadState {
    fn new(simulator: InteractiveSimulator, max_cycles: u64) -> Self {
        Self {
            simulator,
            is_running: false,
            total_cycles: 0,
            max_cycles,
        }
    }
}

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
    /// Background simulation thread handle
    sim_thread: SimThreadHandle,

    /// Video backend (wrapped in Rc<RefCell<>> for callback access)
    video_backend: Rc<RefCell<V>>,

    /// Audio backend (wrapped in Rc<RefCell<>> for callback access)
    audio_backend: Rc<RefCell<A>>,

    /// Event source (generic)
    event_source: E,

    /// Current configuration
    #[allow(dead_code)]
    config: ViewerConfig,

    /// Current simulation state
    state: ViewerState,

    /// Last loaded ELF file path (for reload)
    last_elf_path: Option<PathBuf>,

    /// Frame counter (incremented each time a frame is presented)
    frame_count: u64,

    /// Exit requested by user (e.g., Escape key)
    exit_requested: bool,

    /// Frame step target (for StepFrames test command)
    frame_step_target: Option<u64>,

    /// Number of frames presented in the current step
    frame_presented_this_step: Rc<RefCell<u64>>,
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
        let frame_presented_this_step = Rc::new(RefCell::new(0_u64));

        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();

        let max_cycles = config.max_cycles;
        let sim_thread = thread::Builder::new()
            .name("sim-view-sim".to_string())
            .spawn(move || {
                let mut simulator = match InteractiveSimulator::new() {
                    Ok(sim) => sim,
                    Err(e) => {
                        let _ = event_tx.send(SimEvent::Error(e));
                        let _ = event_tx.send(SimEvent::Terminated);
                        return;
                    }
                };

                let video_tx = event_tx.clone();
                let video_callback = move |data: &[u8], video_config: &VideoConfig| {
                    let event = SimEvent::VideoFrame {
                        data: data.to_vec(),
                        config: *video_config,
                    };
                    if video_tx.send(event).is_err() {
                        log::warn!("Simulation video callback channel closed");
                    }
                };
                let video_device = Box::new(Video::new(Some(video_callback)));

                let audio_tx = event_tx.clone();
                let sample_callback = move |samples: &[i16]| {
                    if samples.is_empty() {
                        return;
                    }
                    if audio_tx
                        .send(SimEvent::AudioSamples(samples.to_vec()))
                        .is_err()
                    {
                        log::warn!("Simulation audio sample channel closed");
                    }
                };

                let audio_config_tx = event_tx.clone();
                let config_callback = move |audio_config: &AudioConfig| {
                    let event = SimEvent::AudioConfig(*audio_config);
                    if audio_config_tx.send(event).is_err() {
                        log::warn!("Simulation audio config channel closed");
                    }
                };
                let audio_device =
                    Box::new(Audio::new(Some(sample_callback), Some(config_callback)));

                if let Err(e) = simulator
                    .register_device(VIDEO_BASE, video_device)
                    .and_then(|_| simulator.register_device(AUDIO_BASE, audio_device))
                {
                    let _ = event_tx.send(SimEvent::Error(e));
                    let _ = event_tx.send(SimEvent::Terminated);
                    return;
                }

                let mut state = SimThreadState::new(simulator, max_cycles);
                run_simulation_thread(&mut state, command_rx, event_tx);
            })
            .map_err(|e| e.to_string())?;

        Ok(SimViewer {
            sim_thread: SimThreadHandle {
                command_tx: Some(command_tx),
                event_rx,
                join_handle: Some(sim_thread),
            },
            video_backend,
            audio_backend,
            event_source,
            config,
            state: ViewerState::Idle,
            last_elf_path: None,
            frame_count: 0,
            exit_requested: false,
            frame_step_target: None,
            frame_presented_this_step,
        })
    }

    fn send_command(&self, command: SimCommand) -> Result<(), String> {
        match &self.sim_thread.command_tx {
            Some(sender) => sender
                .send(command)
                .map_err(|_| "Simulation thread unavailable".to_string()),
            None => Err("Simulation thread not running".to_string()),
        }
    }

    fn handle_sim_events(&mut self, wait_for_event: bool) {
        if wait_for_event {
            match self
                .sim_thread
                .event_rx
                .recv_timeout(SIM_EVENT_WAIT_TIMEOUT)
            {
                Ok(event) => self.handle_sim_event(event),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    self.exit_requested = true;
                }
            }
        }

        while let Ok(event) = self.sim_thread.event_rx.try_recv() {
            self.handle_sim_event(event);
        }
    }

    fn handle_sim_event(&mut self, event: SimEvent) {
        match event {
            SimEvent::VideoFrame { data, config } => {
                let result = self
                    .video_backend
                    .borrow_mut()
                    .process_frame(&data, &config);
                match result {
                    Ok(()) => {
                        *self.frame_presented_this_step.borrow_mut() += 1;
                    }
                    Err(e) => {
                        log::error!("Video backend process_frame error: {}", e);
                    }
                }
            }
            SimEvent::AudioSamples(samples) => {
                self.audio_backend.borrow_mut().push_samples(&samples);
            }
            SimEvent::AudioConfig(config) => {
                log::info!(
                    "Audio config changed: {} Hz, {:?}, {} samples",
                    config.sample_rate.to_hz(),
                    config.channels,
                    config.sample_count
                );
                self.audio_backend.borrow_mut().set_config(&config);
            }
            SimEvent::Halted(tohost) => {
                if let Some(value) = tohost {
                    log::info!("Program halted with tohost value: 0x{:08x}", value);
                } else {
                    log::info!("Program halted");
                }
                self.state = ViewerState::Halted;
                self.update_window_title();
            }
            SimEvent::MaxCyclesReached(cycles) => {
                log::info!("Max cycles reached: {}", cycles);
                self.state = ViewerState::Halted;
                self.update_window_title();
            }
            SimEvent::Error(message) => {
                log::error!("Simulation error: {}", message);
                self.state = ViewerState::Halted;
                self.update_window_title();
            }
            SimEvent::Terminated => {
                self.exit_requested = true;
            }
        }
    }

    /// Load an ELF file and reset the simulation
    pub fn load_elf(&mut self, path: &Path) -> Result<(), String> {
        log::info!("Loading ELF: {}", path.display());
        let (response_tx, response_rx) = mpsc::channel();
        self.send_command(SimCommand::LoadElf {
            path: path.to_path_buf(),
            response: response_tx,
        })?;

        let result = response_rx
            .recv()
            .map_err(|_| "Simulation thread dropped ELF load response".to_string())?;

        if result.is_ok() {
            self.state = ViewerState::Running;
            self.last_elf_path = Some(path.to_path_buf());
            self.update_window_title();
            log::info!("ELF loaded successfully, simulation ready");
        }

        result
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
                let _ = self.send_command(SimCommand::Pause);
                ViewerState::Paused
            }
            ViewerState::Paused => {
                log::info!("Simulation resumed");
                let _ = self.send_command(SimCommand::Resume);
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

    /// Execute a single iteration of the viewer loop
    ///
    /// Returns `Ok(true)` if the viewer should continue running,
    /// or `Ok(false)` if the viewer should terminate.
    pub fn step(&mut self) -> Result<bool, String> {
        // Handle window events (keyboard, close, test commands)
        self.handle_events()?;

        // Reset frame presented flag before processing new simulation events
        *self.frame_presented_this_step.borrow_mut() = 0;

        let wait_for_event = self.state == ViewerState::Running;
        self.handle_sim_events(wait_for_event);

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

        // Check if a frame was presented during this step (set by video callback)
        let frame_presented = *self.frame_presented_this_step.borrow();

        // Update video backend (display or capture)
        self.video_backend.borrow_mut().update()?;

        // Increment frame counter only if a frame was actually presented
        if frame_presented > 0 {
            self.frame_count = self.frame_count.saturating_add(frame_presented);

            // Check if frame step target reached
            if let Some(target) = self.frame_step_target {
                if self.frame_count >= target {
                    log::info!("Frame step target reached: {} frames", self.frame_count);
                    self.frame_step_target = None;
                    self.state = ViewerState::Paused;
                    self.update_window_title();
                    let _ = self.send_command(SimCommand::Pause);
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
                    let _ = self.send_command(SimCommand::Terminate);
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
                    let _ = self.send_command(SimCommand::Pause);
                }
            }
            TestCommand::Resume => {
                log::info!("Test command: Resume");
                if self.state == ViewerState::Paused {
                    self.state = ViewerState::Running;
                    self.update_window_title();
                    let _ = self.send_command(SimCommand::Resume);
                }
            }
            TestCommand::StepFrames(count) => {
                log::info!("Test command: Step {} frames", count);
                // Set frame step target and resume execution
                self.frame_step_target = Some(self.frame_count + count);
                if self.state == ViewerState::Paused || self.state == ViewerState::Idle {
                    self.state = ViewerState::Running;
                    self.update_window_title();
                    let _ = self.send_command(SimCommand::Resume);
                }
            }
            TestCommand::Terminate => {
                log::info!("Test command: Terminate");
                self.exit_requested = true;
                let _ = self.send_command(SimCommand::Terminate);
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
                let _ = self.send_command(SimCommand::Terminate);
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
    ///
    /// Note: This clones the frame data. This is acceptable because this method is only
    /// called infrequently (at test completion for verification). The hot path
    /// (video callback during simulation) writes directly to the backend with zero copies.
    pub fn get_video_frames(&self) -> Vec<CapturedFrame> {
        self.video_backend.borrow().get_frames().to_vec()
    }

    /// Get captured audio chunks (for headless mode testing)
    ///
    /// Note: This clones the chunk data. This is acceptable because this method is only
    /// called infrequently (at test completion for verification). The hot path
    /// (audio callback during simulation) writes directly to the backend with zero copies.
    pub fn get_audio_chunks(&self) -> Vec<CapturedAudioChunk> {
        self.audio_backend.borrow().get_chunks().to_vec()
    }

    /// Get current audio configuration (for headless mode testing)
    pub fn get_audio_config(&self) -> Option<AudioConfig> {
        self.audio_backend.borrow().get_current_config()
    }
}

impl<V: VideoBackend, A: AudioBackend, E: EventSource> Drop for SimViewer<V, A, E> {
    fn drop(&mut self) {
        if let Some(sender) = self.sim_thread.command_tx.take() {
            let _ = sender.send(SimCommand::Terminate);
        }
        if let Some(handle) = self.sim_thread.join_handle.take() {
            let _ = handle.join();
        }
    }
}

fn run_simulation_thread(
    state: &mut SimThreadState,
    command_rx: mpsc::Receiver<SimCommand>,
    event_tx: mpsc::Sender<SimEvent>,
) {
    let mut should_run = true;
    while should_run {
        loop {
            match command_rx.try_recv() {
                Ok(command) => {
                    if handle_sim_command(state, command) {
                        should_run = false;
                        break;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    should_run = false;
                    break;
                }
            }
        }

        if !should_run {
            break;
        }

        if state.is_running {
            let mut halted = None;
            for _ in 0..INSTRUCTIONS_PER_BATCH {
                match state.simulator.step_instruction() {
                    Ok(result) => {
                        state.total_cycles += 1;
                        if result.tohost_value.is_some() {
                            halted = result.tohost_value;
                            state.is_running = false;
                            break;
                        }
                        if state.max_cycles > 0 && state.total_cycles >= state.max_cycles {
                            state.is_running = false;
                            let _ = event_tx.send(SimEvent::MaxCyclesReached(state.total_cycles));
                            break;
                        }
                    }
                    Err(e) => {
                        state.is_running = false;
                        let _ = event_tx.send(SimEvent::Error(e));
                        break;
                    }
                }
            }

            if let Some(value) = halted {
                let _ = event_tx.send(SimEvent::Halted(Some(value)));
            }
        } else {
            match command_rx.recv_timeout(SIM_EVENT_WAIT_TIMEOUT) {
                Ok(command) => {
                    if handle_sim_command(state, command) {
                        should_run = false;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    should_run = false;
                }
            }
        }
    }

    let _ = event_tx.send(SimEvent::Terminated);
}

fn handle_sim_command(state: &mut SimThreadState, command: SimCommand) -> bool {
    match command {
        SimCommand::LoadElf { path, response } => {
            let result = state.simulator.load_elf(&path);
            state.total_cycles = 0;
            state.is_running = result.is_ok();
            let _ = response.send(result);
            false
        }
        SimCommand::Pause => {
            state.is_running = false;
            false
        }
        SimCommand::Resume => {
            state.is_running = true;
            false
        }
        SimCommand::Terminate => true,
    }
}
