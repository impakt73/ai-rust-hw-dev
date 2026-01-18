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
        // Create controller with callbacks for video and audio
        let controller = SimulatorController::new(config.print_inst_trace)?;
        
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
                // Calculate how many cycles to run this frame
                // Assume 100 MHz CPU, 60 FPS → ~1.67M cycles/frame
                // But limit to smaller chunks for responsiveness
                let cycles_per_step = 10000; // Adjust for performance
                
                match self.controller.step_cycles(cycles_per_step) {
                    Ok(result) => {
                        self.total_cycles += cycles_per_step;
                        
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
            
            // Update video window (processes callbacks from Video device)
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
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

pub struct VideoWindow {
    window: Window,
    width: usize,
    height: usize,
    
    /// Frame buffer for minifb (ARGB8888 format)
    framebuffer: Vec<u32>,
    
    /// Pending video frames from Video device callback
    pending_frames: Arc<Mutex<VecDeque<(Vec<u8>, VideoConfig)>>>,
    
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
        
        let pending_frames = Arc::new(Mutex::new(VecDeque::new()));
        
        Ok(VideoWindow {
            window,
            width,
            height,
            framebuffer,
            pending_frames,
            event_queue: VecDeque::new(),
        })
    }
    
    /// Get a callback for the Video bus device
    pub fn get_video_callback(&self) -> impl FnMut(&[u8], &VideoConfig) {
        let pending_frames = Arc::clone(&self.pending_frames);
        
        move |data: &[u8], config: &VideoConfig| {
            // Push frame to queue for processing in update()
            let mut queue = pending_frames.lock().unwrap();
            queue.push_back((data.to_vec(), *config));
            
            // Keep only last 2 frames to prevent unbounded growth
            while queue.len() > 2 {
                queue.pop_front();
            }
        }
    }
    
    /// Update the window (call once per frame in main loop)
    pub fn update(&mut self) -> Result<(), String> {
        // Process pending video frames
        if let Some((frame_data, config)) = {
            let mut queue = self.pending_frames.lock().unwrap();
            queue.pop_front()
        } {
            self.process_video_frame(&frame_data, &config)?;
        }
        
        // Update minifb window with current framebuffer
        self.window
            .update_with_buffer(&self.framebuffer, self.width, self.height)
            .map_err(|e| format!("Failed to update window: {}", e))?;
        
        // Collect events
        self.collect_events();
        
        Ok(())
    }
    
    /// Process a video frame from the Video device
    fn process_video_frame(
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
    
    /// Get a callback for the Audio bus device (sample callback)
    pub fn get_sample_callback(&self) -> impl FnMut(&[i16]) {
        let buffer = Arc::clone(&self.sample_buffer);
        
        move |samples: &[i16]| {
            let mut buf = buffer.lock().unwrap();
            
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
    }
    
    /// Get a callback for the Audio bus device (config callback)
    pub fn get_config_callback(&self) -> impl FnMut(&AudioConfig) {
        let config = Arc::clone(&self.current_config);
        
        move |new_config: &AudioConfig| {
            let mut cfg = config.lock().unwrap();
            *cfg = Some(*new_config);
            
            log::info!(
                "Audio config changed: {} Hz, {:?}, {} samples",
                new_config.sample_rate.to_hz(),
                new_config.channels,
                new_config.sample_count
            );
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
use cpu_sim::{run_elf, SimulationResult, SimulatorView, InstructionTrace};
use std::path::Path;

pub struct SimulatorController {
    /// Print instruction trace flag
    print_inst_trace: bool,
    
    /// Callbacks for video and audio (stored for reset)
    video_callback: Option<Box<dyn FnMut(&[u8], &cpu_sim::VideoConfig)>>,
    audio_sample_callback: Option<Box<dyn FnMut(&[i16])>>,
    audio_config_callback: Option<Box<dyn FnMut(&cpu_sim::AudioConfig)>>,
    
    // TODO: This design needs rework - we can't easily "step" the existing run_elf API
    // Need to expose Simulator internals or redesign cpu-sim public API
    // For now, document the limitation
}

impl SimulatorController {
    pub fn new(print_inst_trace: bool) -> Result<Self, String> {
        Ok(SimulatorController {
            print_inst_trace,
            video_callback: None,
            audio_sample_callback: None,
            audio_config_callback: None,
        })
    }
    
    /// Set video callback
    pub fn set_video_callback<F>(&mut self, callback: F)
    where
        F: FnMut(&[u8], &cpu_sim::VideoConfig) + 'static,
    {
        self.video_callback = Some(Box::new(callback));
    }
    
    /// Set audio callbacks
    pub fn set_audio_callbacks<S, C>(&mut self, sample_callback: S, config_callback: C)
    where
        S: FnMut(&[i16]) + 'static,
        C: FnMut(&cpu_sim::AudioConfig) + 'static,
    {
        self.audio_sample_callback = Some(Box::new(sample_callback));
        self.audio_config_callback = Some(Box::new(config_callback));
    }
    
    /// Load ELF file and reset simulation
    pub fn load_elf(&mut self, path: &Path) -> Result<(), String> {
        // NOTE: Current cpu-sim API doesn't support stepping
        // This is a design challenge that needs to be addressed
        
        // For MVP, we can run in a background thread and use callbacks
        // But this requires significant API changes to cpu-sim
        
        // DESIGN DECISION NEEDED:
        // Option 1: Extend cpu-sim to expose Simulator::step() method
        // Option 2: Run simulation in background thread with messaging
        // Option 3: Redesign cpu-sim API for interactive use
        
        todo!("Need to extend cpu-sim API for interactive stepping")
    }
    
    /// Step the simulation for N cycles
    pub fn step_cycles(&mut self, cycles: u64) -> Result<SimulationStepResult, String> {
        // This requires access to the internal Simulator
        todo!("Need to extend cpu-sim API for interactive stepping")
    }
}

pub struct SimulationStepResult {
    pub tohost_value: Option<u32>,
    pub elapsed_cpu_time_us: u64,
}
```

---

## 5. Design Challenges and Solutions

### Challenge 1: cpu-sim API Not Designed for Interactive Use

**Problem:** The current `cpu-sim` API uses `run_elf()` which runs to completion. We need to step the simulation incrementally while processing GUI events.

**Solutions:**

**Option A (Recommended):** Extend `cpu-sim` public API
- Add `Simulator::new_with_callbacks()` to public API
- Add `Simulator::step()` or `Simulator::step_cycles()` method
- Add `Simulator::reset()` method
- Keep existing `run_elf()` for backward compatibility

```rust
// In cpu-sim/src/lib.rs
pub use sim::Simulator; // Make Simulator public

// In cpu-sim/src/sim.rs
impl Simulator {
    /// Create a new simulator with custom callbacks (public API)
    pub fn new_with_callbacks<F, T>(
        print_inst_trace: bool,
        print_fsm_state: bool,
        inst_complete_callback: Option<F>,
        trace_callback: Option<T>,
        vcd_path: Option<&str>,
        mem_latency_cycles: u32,
    ) -> Result<Self, String>
    where
        F: FnMut(&mut SimulatorView),
        T: FnMut(&InstructionTrace),
    { ... }
    
    /// Step the simulation by one cycle
    pub fn step(&mut self) -> Result<StepResult, String> { ... }
    
    /// Step the simulation by N cycles
    pub fn step_cycles(&mut self, cycles: u64) -> Result<StepResult, String> { ... }
    
    /// Reset the CPU and simulator state
    pub fn reset(&mut self) { ... }
    
    /// Load an ELF file into memory
    pub fn load_elf(&mut self, path: &Path) -> Result<u32, String> { ... }
}
```

**Option B:** Background Thread with Message Passing
- Run simulation in background thread
- Use `crossbeam-channel` for communication
- GUI thread sends commands (load, pause, step, reset)
- Simulation thread sends updates (video frames, audio samples, status)
- More complex but doesn't require API changes

**Option C:** Fork cpu-sim for sim-view
- Create a specialized version of cpu-sim
- Not recommended (maintenance burden)

**Decision:** Go with Option A - extend the cpu-sim public API to support interactive use.

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

### Phase 1: Extend cpu-sim API (Week 1)

1. Make `Simulator` struct public in `cpu-sim/src/lib.rs`
2. Add `Simulator::new_with_callbacks()` public constructor
3. Add `Simulator::step_cycles()` method
4. Add `Simulator::reset()` method
5. Add `Simulator::load_elf()` method
6. Update `SimulatorView` to provide needed access
7. Write unit tests for new API

### Phase 2: Create sim-view Structure (Week 1)

1. Create `sim-view/` directory
2. Add to workspace members in root `Cargo.toml`
3. Create `sim-view/Cargo.toml` with dependencies
4. Create stub files: `main.rs`, `viewer.rs`, `video_window.rs`, `audio_stream.rs`, `simulator_controller.rs`
5. Verify `cargo build` works

### Phase 3: Implement Audio Stream (Week 2)

1. Implement `AudioStream::new()`
2. Implement callback generation for Audio device
3. Test with simple tone program
4. Handle different sample formats (i16, f32, u16)

### Phase 4: Implement Video Window (Week 2)

1. Implement `VideoWindow::new()`
2. Implement frame conversion (RGBA8, RGB8, RGB565, R8)
3. Implement window resize on config change
4. Implement keyboard event handling
5. Test with simple color fill program

### Phase 5: Implement Simulator Controller (Week 3)

1. Implement ELF loading via extended API
2. Implement reset functionality
3. Implement cycle stepping
4. Wire up callbacks to Audio and Video devices

### Phase 6: Implement Main Viewer Loop (Week 3)

1. Implement `SimViewer::new()`
2. Implement `SimViewer::run()` main loop
3. Implement state management (Idle, Running, Paused, Halted)
4. Implement keyboard handlers (Escape, Space, Ctrl+R)
5. Implement window title updates

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
