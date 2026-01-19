# sim-view: Real-Time Video and Audio Viewer Implementation Plan

**Target Audience:** AI Coding Agent  
**Crate Name:** `sim-view`  
**Purpose:** GUI-based binary providing real-time video and audio output from programs running on the simulated RISC-V CPU

---

## 1. Executive Summary

This document provides a complete implementation plan for the `sim-view` crate, a new Rust binary that runs ELF programs on the simulated CPU and displays real-time video and audio output. The viewer uses existing `cpu-sim` infrastructure, specifically the Video and Audio bus devices, to capture output and render it using `minifb` (video) and `cpal` (audio).

### Key Features

1. **Real-time video rendering** using minifb window
2. **Real-time audio playback** using cpal audio stream
3. **ELF file loading** via CLI argument or drag-and-drop
4. **Hot reload** with Ctrl+R to reload last ELF file
5. **Pause/resume** simulation with Space bar
6. **Exit** with Escape key
7. **Dynamic window sizing** based on video configuration
8. **Dynamic audio format** based on audio configuration

---

## 2. Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                      sim-view Binary                         │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │                   Main Event Loop                       │ │
│  │  - Handle keyboard events (ESC, Space, Ctrl+R)         │ │
│  │  - Handle drag-and-drop events                         │ │
│  │  - Step simulation                                     │ │
│  │  - Update video window                                 │ │
│  │  - Feed audio stream                                   │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │   minifb     │  │     cpal     │  │  Event Handler   │  │
│  │   Window     │  │ Audio Stream │  │  (Keyboard/Drop) │  │
│  └──────┬───────┘  └──────┬───────┘  └─────────┬────────┘  │
│         │                 │                     │           │
│         └─────────────────┼─────────────────────┘           │
│                           │                                 │
│  ┌────────────────────────┴──────────────────────────────┐  │
│  │              Simulator Controller                      │  │
│  │  - Load/Reload ELF files                              │  │
│  │  - Reset CPU and simulation state                     │  │
│  │  - Step simulation (cycles per frame)                 │  │
│  │  - Pause/Resume control                               │  │
│  │  - Access to Video and Audio bus devices              │  │
│  └────────────────────────┬──────────────────────────────┘  │
└───────────────────────────┼─────────────────────────────────┘
                            │
                 ┌──────────┴───────────┐
                 │   cpu-sim crate      │
                 │                      │
                 │  ┌────────────────┐  │
                 │  │  SystemBus     │  │
                 │  │  - Video       │  │  ← Video::new(callback)
                 │  │  - Audio       │  │  ← Audio::new(callback, callback)
                 │  │  - DRAM        │  │
                 │  │  - FIFO        │  │
                 │  │  - DMA         │  │
                 │  └────────────────┘  │
                 │                      │
                 │  ┌────────────────┐  │
                 │  │  Simulator     │  │
                 │  │  (Verilator)   │  │
                 │  └────────────────┘  │
                 └──────────────────────┘
```

### Data Flow

1. **ELF Loading**: User provides ELF via CLI or drag-and-drop → Load into simulator memory
2. **Simulation Execution**: Main loop steps CPU cycles → CPU writes to Video/Audio bus devices
3. **Video Output**: Video device invokes callback with frame data → Convert to minifb format → Update window
4. **Audio Output**: Audio device invokes callback with audio samples → Push to cpal stream buffer
5. **User Interaction**: Keyboard/window events → Control simulation (pause, reload, exit)

---

## 3. Component Design

### 3.1 Directory Structure

```
sim-view/
├── Cargo.toml
├── src/
│   ├── main.rs           # Entry point, CLI parsing
│   ├── viewer.rs         # SimViewer struct and main loop
│   ├── video_window.rs   # minifb window management
│   ├── audio_stream.rs   # cpal audio stream management
│   └── simulator_controller.rs  # ELF loading, reset, stepping
```

### 3.2 Dependencies (Cargo.toml)

```toml
[package]
name = "sim-view"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "sim-view"
path = "src/main.rs"

[dependencies]
# Core simulation
cpu-sim = { path = "../cpu-sim" }
riscv_core = { path = "../riscv_core" }

# GUI and Audio
minifb = "0.27"
cpal = "0.15"

# CLI and logging
clap = { version = "4.4", features = ["derive"] }
log = "0.4"
env_logger = "0.10"

# Threading and synchronization
crossbeam-channel = "0.5"
```

**Note:** Add `sim-view` to workspace members in root `Cargo.toml`:
```toml
[workspace]
members = ["cpu-sim", "riscv_core", "testbench", "riscv_protocol", "riscv_macros", "vcd-mcp", "sim-view"]
```

---

## 4. Implementation Details

### 4.1 Main Entry Point (main.rs)

```rust
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "sim-view",
    about = "RISC-V CPU Simulator with Real-time Video and Audio Output",
    long_about = "Interactive viewer for running ELF programs on a simulated RISC-V CPU with live video and audio output"
)]
struct Args {
    /// Path to the RISC-V ELF executable to run on startup (optional)
    #[arg(value_name = "ELF_FILE")]
    elf: Option<PathBuf>,

    /// Maximum cycles to run before auto-terminating (0 = unlimited)
    #[arg(short, long, default_value_t = 0)]
    max_cycles: u64,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Print instruction trace (prints every instruction executed)
    #[arg(long)]
    print_inst_trace: bool,

    /// Initial window width (default: 320)
    #[arg(long, default_value_t = 320)]
    width: u32,

    /// Initial window height (default: 240)
    #[arg(long, default_value_t = 240)]
    height: u32,
}

fn main() {
    let args = Args::parse();

    // Initialize logger
    let log_level = if args.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level))
        .init();

    log::info!("sim-view: RISC-V CPU Simulator Viewer");
    log::info!("Controls:");
    log::info!("  - Drag & Drop ELF file to load");
    log::info!("  - Ctrl+R: Reload last ELF file");
    log::info!("  - Space: Pause/Resume simulation");
    log::info!("  - Escape: Exit");

    // Create viewer configuration
    let config = viewer::ViewerConfig {
        initial_width: args.width,
        initial_height: args.height,
        max_cycles: args.max_cycles,
        print_inst_trace: args.print_inst_trace,
    };

    // Create and run viewer
    match viewer::SimViewer::new(config) {
        Ok(mut viewer) => {
            // Load initial ELF if provided
            if let Some(elf_path) = args.elf {
                if let Err(e) = viewer.load_elf(&elf_path) {
                    eprintln!("✗ Failed to load ELF: {}", e);
                    std::process::exit(1);
                }
            }

            // Run main loop
            if let Err(e) = viewer.run() {
                eprintln!("✗ Viewer error: {}", e);
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("✗ Failed to create viewer: {}", e);
            std::process::exit(1);
        }
    }
}
```

### 4.2 Main Viewer (viewer.rs)

```rust
use crate::audio_stream::AudioStream;
use crate::simulator_controller::SimulatorController;
use crate::video_window::VideoWindow;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub struct ViewerConfig {
    pub initial_width: u32,
    pub initial_height: u32,
    pub max_cycles: u64,
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
    last_frame_time: Instant,
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
            last_frame_time: Instant::now(),
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
            if let Err(e) = self.handle_events() {
                return Err(e);
            }
            
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
                        if self.config.max_cycles > 0 
                            && self.total_cycles >= self.config.max_cycles 
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
                self.video_window.process_video_frame(&frame_data, &config)?;
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

/// Window event types
enum WindowEvent {
    KeyPressed(Key, KeyModifiers),
    FileDrop(PathBuf),
    Close,
}

/// Key codes (simplified)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Key {
    Escape,
    Space,
    R,
    Unknown,
}

/// Keyboard modifiers
#[derive(Debug, Clone, Copy, Default)]
struct KeyModifiers {
    ctrl: bool,
    shift: bool,
    alt: bool,
}
```

### 4.3 Video Window (video_window.rs)

```rust
use minifb::{Window, WindowOptions, Key as MinifbKey, KeyRepeat, Scale};
use cpu_sim::{VideoConfig, VideoFormat};
use std::collections::VecDeque;

pub struct VideoWindow {
    window: Window,
    width: usize,
    height: usize,
    
    /// Frame buffer for minifb (ARGB8888 format)
    framebuffer: Vec<u32>,
    
    /// Event queue for communicating with main loop
    event_queue: VecDeque<crate::viewer::WindowEvent>,
}

impl VideoWindow {
    pub fn new(width: usize, height: usize) -> Result<Self, String> {
        let mut window = Window::new(
            "sim-view - No program loaded",
            width,
            height,
            WindowOptions {
                resize: true,
                scale: Scale::X1,
                ..WindowOptions::default()
            },
        )
        .map_err(|e| format!("Failed to create window: {}", e))?;
        
        // Set target FPS for minifb internal timing
        window.limit_update_rate(Some(std::time::Duration::from_millis(16)));
        
        // Create black framebuffer
        let framebuffer = vec![0xFF000000u32; width * height];
        
        Ok(VideoWindow {
            window,
            width,
            height,
            framebuffer,
            event_queue: VecDeque::new(),
        })
    }
    
    /// Process a video frame from the simulator controller
    /// This is called by the main viewer loop when a new frame is available
    pub fn process_video_frame(
        &mut self,
        data: &[u8],
        config: &VideoConfig,
    ) -> Result<(), String> {
        let new_width = config.width as usize;
        let new_height = config.height as usize;
        
        // Resize window if dimensions changed
        if new_width != self.width || new_height != self.height {
            log::info!("Resizing window to {}x{}", new_width, new_height);
            self.width = new_width;
            self.height = new_height;
            self.framebuffer.resize(new_width * new_height, 0xFF000000);
        }
        
        // Convert frame data to ARGB8888 format for minifb
        match config.format {
            VideoFormat::Rgba8 => {
                self.convert_rgba8(data)?;
            }
            VideoFormat::Rgb8 => {
                self.convert_rgb8(data)?;
            }
            VideoFormat::Rgb565 => {
                self.convert_rgb565(data)?;
            }
            VideoFormat::R8 => {
                self.convert_r8(data)?;
            }
        }
        
        Ok(())
    }
    
    /// Update the window display (call once per frame in main loop)
    pub fn update(&mut self) -> Result<(), String> {
        
        // Update minifb window with current framebuffer
        self.window
            .update_with_buffer(&self.framebuffer, self.width, self.height)
            .map_err(|e| format!("Failed to update window: {}", e))?;
        
        // Collect events
        self.collect_events();
        
        Ok(())
    }
    
    /// Convert RGBA8 (32-bit) to ARGB8888 for minifb
    fn convert_rgba8(&mut self, data: &[u8]) -> Result<(), String> {
        let pixel_count = self.width * self.height;
        if data.len() < pixel_count * 4 {
            return Err("Frame data too small for RGBA8 format".to_string());
        }
        
        for i in 0..pixel_count {
            let r = data[i * 4] as u32;
            let g = data[i * 4 + 1] as u32;
            let b = data[i * 4 + 2] as u32;
            let a = data[i * 4 + 3] as u32;
            
            // minifb format: 0xAARRGGBB
            self.framebuffer[i] = (a << 24) | (r << 16) | (g << 8) | b;
        }
        
        Ok(())
    }
    
    /// Convert RGB8 (24-bit) to ARGB8888 for minifb
    fn convert_rgb8(&mut self, data: &[u8]) -> Result<(), String> {
        let pixel_count = self.width * self.height;
        if data.len() < pixel_count * 3 {
            return Err("Frame data too small for RGB8 format".to_string());
        }
        
        for i in 0..pixel_count {
            let r = data[i * 3] as u32;
            let g = data[i * 3 + 1] as u32;
            let b = data[i * 3 + 2] as u32;
            
            // minifb format: 0xAARRGGBB (opaque)
            self.framebuffer[i] = 0xFF000000 | (r << 16) | (g << 8) | b;
        }
        
        Ok(())
    }
    
    /// Convert RGB565 (16-bit) to ARGB8888 for minifb
    fn convert_rgb565(&mut self, data: &[u8]) -> Result<(), String> {
        let pixel_count = self.width * self.height;
        if data.len() < pixel_count * 2 {
            return Err("Frame data too small for RGB565 format".to_string());
        }
        
        for i in 0..pixel_count {
            let pixel = u16::from_le_bytes([data[i * 2], data[i * 2 + 1]]);
            
            // Extract 5-6-5 components
            let r5 = ((pixel >> 11) & 0x1F) as u8;
            let g6 = ((pixel >> 5) & 0x3F) as u8;
            let b5 = (pixel & 0x1F) as u8;
            
            // Scale to 8-bit (preserve precision)
            let r8 = (r5 << 3) | (r5 >> 2);
            let g8 = (g6 << 2) | (g6 >> 4);
            let b8 = (b5 << 3) | (b5 >> 2);
            
            // minifb format: 0xAARRGGBB (opaque)
            self.framebuffer[i] = 0xFF000000 
                | ((r8 as u32) << 16) 
                | ((g8 as u32) << 8) 
                | (b8 as u32);
        }
        
        Ok(())
    }
    
    /// Convert R8 (8-bit grayscale) to ARGB8888 for minifb
    fn convert_r8(&mut self, data: &[u8]) -> Result<(), String> {
        let pixel_count = self.width * self.height;
        if data.len() < pixel_count {
            return Err("Frame data too small for R8 format".to_string());
        }
        
        for i in 0..pixel_count {
            let gray = data[i] as u32;
            
            // Replicate gray value to R, G, B channels
            self.framebuffer[i] = 0xFF000000 | (gray << 16) | (gray << 8) | gray;
        }
        
        Ok(())
    }
    
    /// Collect events from minifb window
    fn collect_events(&mut self) {
        // Check for window close
        if !self.window.is_open() {
            self.event_queue.push_back(crate::viewer::WindowEvent::Close);
            return;
        }
        
        // Check for keyboard events
        if self.window.is_key_pressed(MinifbKey::Escape, KeyRepeat::No) {
            self.event_queue.push_back(crate::viewer::WindowEvent::KeyPressed(
                crate::viewer::Key::Escape,
                crate::viewer::KeyModifiers::default(),
            ));
        }
        
        if self.window.is_key_pressed(MinifbKey::Space, KeyRepeat::No) {
            self.event_queue.push_back(crate::viewer::WindowEvent::KeyPressed(
                crate::viewer::Key::Space,
                crate::viewer::KeyModifiers::default(),
            ));
        }
        
        // Check for Ctrl+R
        let ctrl_pressed = self.window.is_key_down(MinifbKey::LeftCtrl)
            || self.window.is_key_down(MinifbKey::RightCtrl);
        if ctrl_pressed && self.window.is_key_pressed(MinifbKey::R, KeyRepeat::No) {
            self.event_queue.push_back(crate::viewer::WindowEvent::KeyPressed(
                crate::viewer::Key::R,
                crate::viewer::KeyModifiers { ctrl: true, shift: false, alt: false },
            ));
        }
        
        // NOTE: minifb doesn't directly support drag-and-drop
        // This would require platform-specific code or a different windowing library
        // For MVP, omit drag-and-drop and document as future enhancement
    }
    
    /// Get pending events
    pub fn get_events(&mut self) -> Vec<crate::viewer::WindowEvent> {
        self.event_queue.drain(..).collect()
    }
    
    /// Check if window is still open
    pub fn is_open(&self) -> bool {
        self.window.is_open()
    }
    
    /// Close the window
    pub fn close(&mut self) {
        // minifb windows close automatically when dropped
        // This is a no-op but kept for API consistency
    }
    
    /// Set window title
    pub fn set_title(&mut self, title: &str) {
        self.window.set_title(title);
    }
}
```

### 4.4 Audio Stream (audio_stream.rs)

```rust
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat, SampleRate, StreamConfig};
use cpu_sim::{AudioChannels, AudioConfig, AudioSampleRate};
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

pub struct AudioStream {
    /// Active audio output stream
    stream: cpal::Stream,
    
    /// Sample buffer queue (thread-safe)
    sample_buffer: Arc<Mutex<VecDeque<i16>>>,
    
    /// Current audio configuration
    current_config: Arc<Mutex<Option<AudioConfig>>>,
}

impl AudioStream {
    pub fn new() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("No audio output device available")?;
        
        log::info!("Using audio device: {}", device.name().unwrap_or("Unknown".to_string()));
        
        // Get default configuration
        let supported_config = device
            .default_output_config()
            .map_err(|e| format!("Failed to get default audio config: {}", e))?;
        
        let sample_format = supported_config.sample_format();
        let config: StreamConfig = supported_config.into();
        
        log::info!(
            "Audio config: {} Hz, {} channels, format: {:?}",
            config.sample_rate.0,
            config.channels,
            sample_format
        );
        
        // Shared buffer for audio samples
        let sample_buffer = Arc::new(Mutex::new(VecDeque::new()));
        let current_config = Arc::new(Mutex::new(None));
        
        // Build output stream based on sample format
        let stream = match sample_format {
            SampleFormat::I16 => {
                Self::build_i16_stream(&device, &config, Arc::clone(&sample_buffer))?
            }
            SampleFormat::F32 => {
                Self::build_f32_stream(&device, &config, Arc::clone(&sample_buffer))?
            }
            SampleFormat::U16 => {
                Self::build_u16_stream(&device, &config, Arc::clone(&sample_buffer))?
            }
            _ => {
                return Err(format!("Unsupported sample format: {:?}", sample_format));
            }
        };
        
        // Start playback
        stream
            .play()
            .map_err(|e| format!("Failed to start audio stream: {}", e))?;
        
        Ok(AudioStream {
            stream,
            sample_buffer,
            current_config,
        })
    }
    
    /// Push audio samples to the buffer for playback
    /// This is called by the main viewer loop with samples from the simulator
    pub fn push_samples(&self, samples: &[i16]) {
        let mut buf = self.sample_buffer.lock().unwrap();
        
        // Add samples to buffer
        for &sample in samples {
            buf.push_back(sample);
        }
        
        // Limit buffer size to prevent unbounded growth
        // Keep at most 0.5 seconds of audio (48000 Hz * 2 channels * 0.5 = 48000 samples)
        while buf.len() > 48000 {
            buf.pop_front();
        }
    }
    
    /// Build i16 output stream
    fn build_i16_stream(
        device: &cpal::Device,
        config: &StreamConfig,
        buffer: Arc<Mutex<VecDeque<i16>>>,
    ) -> Result<cpal::Stream, String> {
        device
            .build_output_stream(
                config,
                move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                    let mut buf = buffer.lock().unwrap();
                    
                    for sample_slot in data.iter_mut() {
                        *sample_slot = buf.pop_front().unwrap_or(0);
                    }
                },
                |err| eprintln!("Audio stream error: {}", err),
                None,
            )
            .map_err(|e| format!("Failed to build i16 stream: {}", e))
    }
    
    /// Build f32 output stream
    fn build_f32_stream(
        device: &cpal::Device,
        config: &StreamConfig,
        buffer: Arc<Mutex<VecDeque<i16>>>,
    ) -> Result<cpal::Stream, String> {
        device
            .build_output_stream(
                config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let mut buf = buffer.lock().unwrap();
                    
                    for sample_slot in data.iter_mut() {
                        let sample_i16 = buf.pop_front().unwrap_or(0);
                        // Convert i16 to f32 in range [-1.0, 1.0]
                        *sample_slot = sample_i16 as f32 / i16::MAX as f32;
                    }
                },
                |err| eprintln!("Audio stream error: {}", err),
                None,
            )
            .map_err(|e| format!("Failed to build f32 stream: {}", e))
    }
    
    /// Build u16 output stream
    fn build_u16_stream(
        device: &cpal::Device,
        config: &StreamConfig,
        buffer: Arc<Mutex<VecDeque<i16>>>,
    ) -> Result<cpal::Stream, String> {
        device
            .build_output_stream(
                config,
                move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                    let mut buf = buffer.lock().unwrap();
                    
                    for sample_slot in data.iter_mut() {
                        let sample_i16 = buf.pop_front().unwrap_or(0);
                        // Convert i16 to u16 (shift range)
                        let shifted = (sample_i16 as i32) + (i16::MAX as i32) + 1;
                        *sample_slot = shifted.clamp(0, u16::MAX as i32) as u16;
                    }
                },
                |err| eprintln!("Audio stream error: {}", err),
                None,
            )
            .map_err(|e| format!("Failed to build u16 stream: {}", e))
    }
}

// Stream is automatically stopped when dropped
impl Drop for AudioStream {
    fn drop(&mut self) {
        log::debug!("Audio stream dropped");
    }
}
```

### 4.5 Simulator Controller (simulator_controller.rs)

```rust
use cpu_sim::{
    Audio, InteractiveSimulator, SimulationStepResult, Video, VideoConfig, AudioConfig,
};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

pub struct SimulatorController {
    /// Interactive simulator instance
    simulator: InteractiveSimulator,
    
    /// Video frame queue (shared with Video device callback)
    video_frames: Arc<Mutex<VecDeque<(Vec<u8>, VideoConfig)>>>,
    
    /// Audio sample queue (shared with Audio device callback)
    audio_samples: Arc<Mutex<VecDeque<i16>>>,
    
    /// Audio config (shared with Audio device callback)
    audio_config: Arc<Mutex<Option<AudioConfig>>>,
}

impl SimulatorController {
    /// Create a new simulator controller with video and audio support
    ///
    /// NOTE: This implementation assumes InteractiveSimulator will be extended
    /// to support device registration. If not available, use the alternative
    /// implementation shown in the "Fallback Implementation" section below.
    pub fn new() -> Result<Self, String> {
        // Create the interactive simulator
        let mut simulator = InteractiveSimulator::new()?;
        
        // Create shared queues for video and audio data
        let video_frames = Arc::new(Mutex::new(VecDeque::new()));
        let audio_samples = Arc::new(Mutex::new(VecDeque::new()));
        let audio_config = Arc::new(Mutex::new(None));
        
        // Create Video device with callback
        let video_frames_clone = Arc::clone(&video_frames);
        let video_callback = move |data: &[u8], config: &VideoConfig| {
            let mut frames = video_frames_clone.lock().unwrap();
            frames.push_back((data.to_vec(), *config));
            
            // Keep only last 2 frames to prevent unbounded growth
            while frames.len() > 2 {
                frames.pop_front();
            }
        };
        let video_device = Video::new(Some(video_callback));
        
        // Create Audio device with callbacks
        let audio_samples_clone = Arc::clone(&audio_samples);
        let sample_callback = move |samples: &[i16]| {
            let mut buf = audio_samples_clone.lock().unwrap();
            for &sample in samples {
                buf.push_back(sample);
            }
            
            // Limit buffer size (0.5 seconds at 48kHz stereo)
            while buf.len() > 48000 {
                buf.pop_front();
            }
        };
        
        let audio_config_clone = Arc::clone(&audio_config);
        let config_callback = move |config: &AudioConfig| {
            let mut cfg = audio_config_clone.lock().unwrap();
            *cfg = Some(*config);
        };
        let audio_device = Audio::new(Some(sample_callback), Some(config_callback));
        
        // Register devices with simulator
        // ASSUMPTION: InteractiveSimulator will provide register_device() method
        // If not available yet, see "Fallback Implementation" below
        simulator.register_device(0x10000000, Box::new(video_device))?;
        simulator.register_device(0x10001000, Box::new(audio_device))?;
        
        Ok(SimulatorController {
            simulator,
            video_frames,
            audio_samples,
            audio_config,
        })
    }
    
    /// Load an ELF file and reset the simulation
    pub fn load_elf(&mut self, path: &Path) -> Result<(), String> {
        // Clear any pending frames/samples from previous program
        self.video_frames.lock().unwrap().clear();
        self.audio_samples.lock().unwrap().clear();
        *self.audio_config.lock().unwrap() = None;
        
        // Load ELF into simulator (this resets the CPU)
        self.simulator.load_elf(path)?;
        
        Ok(())
    }
    
    /// Step the simulation for N instructions
    ///
    /// Returns the result of the last instruction executed, which may contain
    /// a tohost termination value if the program halted.
    pub fn step_instructions(&mut self, count: u64) -> Result<SimulationStepResult, String> {
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
    
    /// Get the next available video frame, if any
    pub fn get_video_frame(&self) -> Option<(Vec<u8>, VideoConfig)> {
        self.video_frames.lock().unwrap().pop_front()
    }
    
    /// Get available audio samples (up to max_samples)
    pub fn get_audio_samples(&self, max_samples: usize) -> Vec<i16> {
        let mut samples = self.audio_samples.lock().unwrap();
        let count = samples.len().min(max_samples);
        samples.drain(..count).collect()
    }
    
    /// Get current audio configuration, if set
    pub fn get_audio_config(&self) -> Option<AudioConfig> {
        *self.audio_config.lock().unwrap()
    }
}

// ============================================================================
// FALLBACK IMPLEMENTATION (if InteractiveSimulator doesn't support devices)
// ============================================================================
//
// If InteractiveSimulator.register_device() is not available, use this
// alternative approach with run_program and background thread:
//
// ```rust
// use std::thread;
// use crossbeam_channel::{bounded, Sender, Receiver};
// 
// enum SimCommand {
//     LoadElf(PathBuf),
//     Step(u64),
//     Stop,
// }
// 
// enum SimResponse {
//     Result(SimulationStepResult),
//     Error(String),
// }
// 
// pub struct SimulatorController {
//     command_tx: Sender<SimCommand>,
//     response_rx: Receiver<SimResponse>,
//     sim_thread: Option<thread::JoinHandle<()>>,
//     video_frames: Arc<Mutex<VecDeque<(Vec<u8>, VideoConfig)>>>,
//     audio_samples: Arc<Mutex<VecDeque<i16>>>,
// }
// 
// impl SimulatorController {
//     pub fn new() -> Result<Self, String> {
//         let (cmd_tx, cmd_rx) = bounded(10);
//         let (resp_tx, resp_rx) = bounded(10);
//         
//         let video_frames = Arc::new(Mutex::new(VecDeque::new()));
//         let audio_samples = Arc::new(Mutex::new(VecDeque::new()));
//         
//         // Clone for thread
//         let video_frames_thread = Arc::clone(&video_frames);
//         let audio_samples_thread = Arc::clone(&audio_samples);
//         
//         // Spawn simulation thread
//         let sim_thread = thread::spawn(move || {
//             // Thread implementation with run_program...
//         });
//         
//         Ok(SimulatorController {
//             command_tx: cmd_tx,
//             response_rx: resp_rx,
//             sim_thread: Some(sim_thread),
//             video_frames,
//             audio_samples,
//         })
//     }
// }
// ```
```

---

## 5. Design Challenges and Solutions

### Challenge 1: cpu-sim API Not Designed for Interactive Use

**Problem:** The original `cpu-sim` API used `run_elf()` which runs to completion. We need to step the simulation incrementally while processing GUI events, and we need to register Video/Audio devices with callbacks for real-time output.

**✅ PARTIALLY SOLVED:** The `InteractiveSimulator` API has been added to cpu-sim, providing:
- `InteractiveSimulator::new()` - Create simulator instance
- `InteractiveSimulator::load_elf()` - Load ELF file and reset CPU
- `InteractiveSimulator::step_instruction()` - Step execution by one instruction

**⚠️ REMAINING REQUIREMENT:** To complete sim-view, we need to register Video and Audio bus devices with callbacks. This requires ONE of the following:

**Option A (Recommended):** Extend `InteractiveSimulator` to support device registration
```rust
// In cpu-sim/src/lib.rs - extend InteractiveSimulator
impl InteractiveSimulator {
    /// Register a custom bus device before loading ELF
    /// This must be called before load_elf()
    pub fn register_device(
        &mut self,
        base_addr: u32,
        device: Box<dyn BusDevice>,
    ) -> Result<(), String> {
        // Delegate to internal simulator's bus
    }
    
    /// Alternative: Provide access to SimulatorView for device setup
    pub fn with_setup<F>(&mut self, setup: F) -> Result<(), String>
    where
        F: FnOnce(&mut SimulatorView),
    {
        // Call setup function with access to SimulatorView
        // This allows registering devices, writing to memory, etc.
    }
}
```

**Option B:** Use `run_program` with background thread
- Keep using existing `run_program()` API
- Run simulation in background thread
- Use `crossbeam-channel` for GUI ↔ simulation communication
- More complex but works with current API

**Option C:** Direct Simulator usage (requires making Simulator public)
- Make `Simulator` struct public in cpu-sim
- Construct with callbacks directly
- More flexible but exposes internal complexity

**Decision for Implementation:** 
- **Short-term:** Use Option A with a PR to extend `InteractiveSimulator` API
- **Alternative:** If API extension is not feasible, fall back to Option B (background thread)

### Challenge 2: Drag-and-Drop Support

**Problem:** `minifb` doesn't natively support drag-and-drop events.

**Solutions:**

**Option A:** Use platform-specific code
- Use `winit` instead of `minifb` (supports drag-and-drop via events)
- More complex setup but better event handling

**Option B:** Omit for MVP
- Document as future enhancement
- Users can still load via CLI or Ctrl+R reload

**Decision:** Omit drag-and-drop for MVP, document as future enhancement. Use CLI loading and Ctrl+R reload for now.

### Challenge 3: Audio Buffer Management

**Problem:** Audio callback runs in separate thread at unpredictable intervals. Need thread-safe buffer that doesn't block.

**Solution:** Use `Arc<Mutex<VecDeque<i16>>>` for sample buffer
- Audio device callback pushes samples
- cpal callback pops samples
- Mutex provides safety
- VecDeque allows efficient push/pop
- Limit buffer size to prevent unbounded growth

### Challenge 4: Video Frame Timing

**Problem:** CPU may generate frames faster or slower than 60 FPS.

**Solution:** Frame buffer with drop-old policy
- Keep only last 2 frames in queue
- Drop oldest if queue full
- Prevents memory growth
- Ensures latest frame is displayed

---

## 6. Implementation Sequence

### Phase 1: Extend InteractiveSimulator API (Prerequisite)

**Status:** ✅ **PARTIALLY COMPLETE** - `InteractiveSimulator` exists with `new()`, `load_elf()`, and `step_instruction()`

**Remaining Work:**

1. Add device registration capability to `InteractiveSimulator` - **Option A (Recommended)**:
   - Add `register_device(base_addr, device)` method to `InteractiveSimulator`
   - OR add `with_setup(callback)` method for one-time setup before loading ELF
2. Alternatively, implement using background thread approach - **Option B**:
   - Use existing `run_program()` with Video/Audio callbacks
   - Run in background thread with message passing
3. Write unit tests for device registration
4. Update cpu-sim documentation

**Implementation Note:** The simulator controller code in this plan assumes Option A. If using Option B, refer to the "Fallback Implementation" comment in Section 4.5.

### Phase 2: Create sim-view Structure (Week 1)

1. Create `sim-view/` directory
2. Add to workspace members in root `Cargo.toml`
3. Create `sim-view/Cargo.toml` with dependencies
4. Create stub files: `main.rs`, `viewer.rs`, `video_window.rs`, `audio_stream.rs`, `simulator_controller.rs`
5. Verify `cargo build` works

### Phase 3: Implement Audio Stream (Week 2)

1. Implement `AudioStream::new()`
2. Implement `push_samples()` for receiving samples from controller
3. Test with simple tone program
4. Handle different sample formats (i16, f32, u16)

### Phase 4: Implement Video Window (Week 2)

1. Implement `VideoWindow::new()`
2. Implement `process_video_frame()` for frame data from controller
3. Implement frame conversion (RGBA8, RGB8, RGB565, R8)
4. Implement window resize on config change
5. Implement keyboard event handling
6. Test with simple color fill program

### Phase 5: Implement Simulator Controller (Week 3)

1. Implement device registration with Video/Audio devices
2. Implement ELF loading via `InteractiveSimulator::load_elf()`
3. Implement `step_instructions()` wrapper around `InteractiveSimulator::step_instruction()`
4. Implement frame/sample extraction methods (`get_video_frame()`, `get_audio_samples()`)
5. Wire up thread-safe queues for Video and Audio data

### Phase 6: Implement Main Viewer Loop (Week 3)

1. Implement `SimViewer::new()`
2. Implement `SimViewer::run()` main loop with frame pulling
3. Implement state management (Idle, Running, Paused, Halted)
4. Implement keyboard handlers (Escape, Space, Ctrl+R)
5. Implement window title updates
6. Wire up video frame and audio sample flow from controller to window/stream

### Phase 7: Integration and Testing (Week 4)

1. Create test ELF programs (video, audio, combined)
2. Test all hotkeys and interactions
3. Test different video formats and resolutions
4. Test audio playback quality
5. Test pause/resume functionality
6. Test reload functionality

### Phase 8: Documentation and Polish (Week 4)

1. Write `sim-view/README.md` usage guide
2. Update root `README.md` with sim-view
3. Add `--help` documentation
4. Add logging for debugging
5. Error handling improvements

---

## 7. Testing Strategy

### Unit Tests

- `AudioStream` buffer management
- `VideoWindow` format conversion
- Event handling logic

### Integration Tests

Create test ELF programs:

1. **video_solid_color.elf** - Fill screen with red, verify display
2. **video_pattern.elf** - Draw gradient pattern
3. **audio_tone.elf** - Generate 440 Hz sine wave
4. **video_audio_combined.elf** - Both video and audio output
5. **video_resize.elf** - Change resolution mid-program
6. **pause_test.elf** - Long-running program to test pause

### Manual Tests

- Load ELF via CLI
- Reload with Ctrl+R
- Pause/Resume with Space
- Exit with Escape
- Window close button
- Different resolutions (160x120, 320x240, 640x480)
- Different pixel formats
- Audio quality (no crackling)

---

## 8. Future Enhancements

### Near-Term (Post-MVP)

1. **Drag-and-drop support** - Switch to `winit` or add platform-specific code
2. **Screenshot capability** - Save current frame to PNG
3. **Recording** - Record video/audio to file
4. **Performance overlay** - Show FPS, cycle count
5. **Adjustable simulation speed** - Speed up/slow down

### Long-Term

1. **Debugger integration** - Step through instructions while viewing output
2. **Memory inspector** - View DRAM/registers while running
3. **Network I/O** - Multiplayer support
4. **VCD waveform viewer** - Integrated waveform display

---

## 9. AI Agent Implementation Notes

### Key Points for AI Agent

1. **Start with cpu-sim API extension** - This is prerequisite for everything else
2. **Follow Rust best practices** - Use proper error handling, no `unwrap()` in production code
3. **Thread safety** - Use `Arc<Mutex<>>` for shared state between threads
4. **Callbacks** - Use closures with `FnMut` traits for Video/Audio device callbacks
5. **No Box::leak()** - Use proper ownership patterns (`Arc`, `Rc`, callbacks with lifetimes)

### Critical Dependencies

- `cpu-sim` must expose `Simulator` and stepping API
- `minifb` for video window
- `cpal` for audio stream
- `crossbeam-channel` for thread communication (if using background thread approach)

### Common Pitfalls to Avoid

1. Don't use `unwrap()` or `expect()` in main loop - handle errors gracefully
2. Don't let buffers grow unbounded - limit queue sizes
3. Don't block GUI thread - keep operations fast
4. Don't leak memory - use RAII and proper Drop implementations
5. Don't forget to run `cargo fmt` and `cargo clippy` before committing

### Testing Commands

```bash
# Build everything
cargo build --package sim-view

# Run with test program
cargo run --package sim-view -- test_programs/video_test.elf

# Run tests
cargo test --package sim-view

# Check formatting
cargo fmt --package sim-view -- --check

# Check lints
cargo clippy --package sim-view -- -D warnings
```

---

## 10. Success Criteria

### MVP is complete when:

- [ ] User can run `cargo run --package sim-view -- program.elf`
- [ ] Window opens with video output from simulated CPU
- [ ] Audio plays from simulated CPU
- [ ] Space bar pauses/resumes simulation
- [ ] Ctrl+R reloads last ELF file
- [ ] Escape key exits program
- [ ] Window resizes based on video configuration
- [ ] Title bar shows loaded file and status (RUNNING/PAUSED/HALTED)
- [ ] All cargo tests pass
- [ ] No clippy warnings
- [ ] Code is formatted with cargo fmt

---

**End of Implementation Plan**
