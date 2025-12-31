# Real-Time Video and Audio Output Implementation Plan

## Executive Summary

This document outlines a comprehensive plan to add real-time video and audio output capabilities to the RISC-V CPU simulator. The solution enables user-written ELF executables running on the simulated CPU to generate video and audio data visible through a GUI application. This is achieved through memory-mapped I/O regions combined with a new `sim-viewer` binary that provides windowing and audio output capabilities.

## Table of Contents

1. [Overview and Objectives](#overview-and-objectives)
2. [Architecture Design](#architecture-design)
3. [Memory Map Specification](#memory-map-specification)
4. [Implementation Components](#implementation-components)
5. [Technology Stack](#technology-stack)
6. [Implementation Phases](#implementation-phases)
7. [User Workflow](#user-workflow)
8. [Testing Strategy](#testing-strategy)
9. [Future Enhancements](#future-enhancements)
10. [Risk Assessment](#risk-assessment)

---

## Overview and Objectives

### Primary Goals

1. **Video Output**: Enable programs running on the simulated CPU to generate real-time video output by writing pixel data to a dedicated memory region
2. **Audio Output**: Enable programs to generate real-time audio output by writing audio samples to a dedicated memory region
3. **GUI Viewer Application**: Create a new Rust binary (`sim-viewer`) similar to `cpu-sim` but with GUI rendering capabilities
4. **Developer Experience**: Provide a simple, intuitive API for developers writing programs for the simulated CPU

### Use Cases

- **Educational**: Teaching computer graphics and audio programming on bare-metal systems
- **Demo Programs**: Creating visual and audio demonstrations on the simulated RISC-V CPU
- **Hardware Development**: Prototyping and testing video/audio accelerator designs before RTL implementation
- **Retro Computing**: Emulating simple retro game/demo platforms on modern systems

---

## Architecture Design

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    sim-viewer (GUI)                      │
│  ┌────────────┐  ┌──────────────┐  ┌─────────────────┐ │
│  │  minifb    │  │  cpal Audio  │  │  Event Handler  │ │
│  │  Window    │  │  Stream      │  │  (Keyboard/Esc) │ │
│  └─────┬──────┘  └──────┬───────┘  └────────┬────────┘ │
│        │                │                    │          │
│  ┌─────┴────────────────┴────────────────────┴────┐    │
│  │          Simulation Controller                  │    │
│  │  - Manages CPU lifecycle                        │    │
│  │  - Polls video/audio memory regions             │    │
│  │  - Updates display/audio stream                 │    │
│  └─────────────────┬───────────────────────────────┘    │
└────────────────────┼────────────────────────────────────┘
                     │
          ┌──────────┴──────────┐
          │   System Bus        │
          │  (Memory Router)    │
          ├─────────────────────┤
          │  0x0000_0000        │  DRAM (program code/data)
          │  0x4000_0000        │  FIFO (existing)
          │  0x5000_0000        │  Video Framebuffer
          │  0x5010_0000        │  Video Control Registers
          │  0x6000_0000        │  Audio Buffer
          │  0x6010_0000        │  Audio Control Registers
          │  0xFFFF_FFF0        │  TOHOST (halt signal)
          └─────────────────────┘
                     │
          ┌──────────┴──────────┐
          │   RISC-V CPU Core   │
          │   (Verilated RTL)   │
          └─────────────────────┘
```

### Data Flow

1. **CPU → Video Output**:
   - CPU writes pixel data to video framebuffer (0x5000_0000)
   - CPU updates control register to signal frame ready
   - `sim-viewer` polls control register each frame
   - On ready signal, `sim-viewer` copies framebuffer to minifb window
   - Display updates at target framerate (e.g., 60 FPS)

2. **CPU → Audio Output**:
   - CPU writes audio samples to audio buffer (0x6000_0000)
   - CPU updates write pointer in control register
   - `cpal` audio callback reads samples from buffer
   - Audio stream continuously drains buffer at sample rate (e.g., 48 kHz)

---

## Memory Map Specification

### Complete Memory Map

| Address Range              | Size      | Description                           | Access   |
|----------------------------|-----------|---------------------------------------|----------|
| 0x0000_0000 - 0x3FFF_FFFF  | 1 GB      | DRAM (code, data, stack, heap)        | R/W      |
| 0x4000_0000 - 0x4000_0004  | 8 bytes   | FIFO DATA register (existing)         | R/W      |
| 0x4000_0004 - 0x4000_0008  | 4 bytes   | FIFO STATUS register (existing)       | R        |
| 0x5000_0000 - 0x500F_FFFF  | 1 MB      | **Video Framebuffer**                 | W        |
| 0x5010_0000 - 0x5010_0000  | 4 bytes   | **Video Control** (frame ready flag)  | R/W      |
| 0x5010_0004 - 0x5010_0004  | 4 bytes   | **Video Width** (default: 320)        | R/W      |
| 0x5010_0008 - 0x5010_0008  | 4 bytes   | **Video Height** (default: 240)       | R/W      |
| 0x5010_000C - 0x5010_000C  | 4 bytes   | **Video Format** (RGB888, RGB565...)  | R/W      |
| 0x6000_0000 - 0x600F_FFFF  | 1 MB      | **Audio Sample Buffer** (ring buffer) | W        |
| 0x6010_0000 - 0x6010_0000  | 4 bytes   | **Audio Write Pointer**               | R/W      |
| 0x6010_0004 - 0x6010_0004  | 4 bytes   | **Audio Read Pointer** (read-only)    | R        |
| 0x6010_0008 - 0x6010_0008  | 4 bytes   | **Audio Sample Rate** (Hz)            | R/W      |
| 0x6010_000C - 0x6010_000C  | 4 bytes   | **Audio Format** (mono/stereo, bits)  | R/W      |
| 0xFFFF_FFF0 - 0xFFFF_FFFF  | 16 bytes  | TOHOST (program termination signal)   | W        |

### Video Framebuffer Details

**Default Configuration:**
- **Resolution**: 320x240 pixels (QVGA)
- **Format**: RGB888 (24-bit color, 3 bytes per pixel)
- **Total size**: 320 × 240 × 3 = 230,400 bytes (~225 KB)
- **Layout**: Row-major, top-to-bottom, left-to-right
- **Byte order**: R, G, B (little-endian systems)

**Pixel addressing:**
```
pixel_offset = (y * width + x) * 3
address = 0x5000_0000 + pixel_offset
```

**Control Register (0x5010_0000) Bits:**
- Bit 0: Frame ready flag (1 = new frame ready, cleared by viewer)
- Bit 1: VSync wait (1 = CPU waits for next frame before writing)
- Bits 2-31: Reserved

**Supported Formats** (via 0x5010_000C):
- 0: RGB888 (24-bit, 3 bytes per pixel) - default
- 1: RGB565 (16-bit, 2 bytes per pixel)
- 2: Grayscale (8-bit, 1 byte per pixel)

### Audio Buffer Details

**Default Configuration:**
- **Sample Rate**: 48,000 Hz (configurable)
- **Format**: 16-bit signed PCM, stereo
- **Buffer Size**: 1 MB (allows ~10 seconds of audio at 48 kHz stereo)
- **Ring Buffer**: Circular buffer with read/write pointers

**Sample Format** (16-bit stereo):
```
struct AudioSample {
    left: i16,   // Left channel (-32768 to 32767)
    right: i16,  // Right channel
}
```

**Write Pointer (0x6010_0000):**
- Byte offset into audio buffer where CPU should write next sample
- CPU increments after each write
- Wraps to 0 at buffer end

**Read Pointer (0x6010_0004):**
- Byte offset where viewer is currently reading (read-only for CPU)
- CPU should avoid overwriting data ahead of read pointer
- Maintained by audio callback

**Audio Format Register (0x6010_000C) Bits:**
- Bits 0-1: Channels (0=mono, 1=stereo, 2=quad)
- Bits 2-3: Sample size (0=8-bit, 1=16-bit, 2=24-bit, 3=32-bit)
- Bits 4-31: Reserved

---

## Implementation Components

### Component 1: Video Device Module (`cpu-sim/src/video.rs`)

**Purpose**: Manages video framebuffer and control registers

**Key Structures:**
```rust
pub struct VideoDevice {
    /// Framebuffer memory (max 1 MB)
    framebuffer: Vec<u8>,
    
    /// Current resolution
    width: u32,
    height: u32,
    
    /// Pixel format (RGB888, RGB565, Grayscale)
    format: VideoFormat,
    
    /// Control flags
    frame_ready: bool,
    vsync_wait: bool,
}

pub enum VideoFormat {
    RGB888,   // 24-bit RGB
    RGB565,   // 16-bit RGB
    Grayscale, // 8-bit luminance
}
```

**Key Methods:**
```rust
impl VideoDevice {
    pub fn new() -> Self;
    pub fn write_pixel(&mut self, offset: u32, data: u32);
    pub fn read_control(&self) -> u32;
    pub fn write_control(&mut self, value: u32);
    pub fn get_framebuffer(&self) -> &[u8];
    pub fn clear_frame_ready(&mut self);
    pub fn set_resolution(&mut self, width: u32, height: u32);
    pub fn set_format(&mut self, format: VideoFormat);
}
```

### Component 2: Audio Device Module (`cpu-sim/src/audio.rs`)

**Purpose**: Manages audio ring buffer and control registers

**Key Structures:**
```rust
pub struct AudioDevice {
    /// Ring buffer for audio samples (max 1 MB)
    buffer: Vec<u8>,
    
    /// Current write position (CPU updates this)
    write_ptr: u32,
    
    /// Current read position (viewer updates this)
    read_ptr: u32,
    
    /// Audio configuration
    sample_rate: u32,
    format: AudioFormat,
}

pub struct AudioFormat {
    channels: u8,      // 1=mono, 2=stereo
    sample_bits: u8,   // 8, 16, 24, or 32
}
```

**Key Methods:**
```rust
impl AudioDevice {
    pub fn new() -> Self;
    pub fn write_sample(&mut self, offset: u32, data: u32);
    pub fn read_samples(&mut self, count: usize) -> Vec<i16>;
    pub fn get_write_ptr(&self) -> u32;
    pub fn set_write_ptr(&mut self, ptr: u32);
    pub fn get_read_ptr(&self) -> u32;
    pub fn advance_read_ptr(&mut self, bytes: u32);
    pub fn available_samples(&self) -> usize;
}
```

### Component 3: Enhanced System Bus (`cpu-sim/src/bus.rs`)

**Updates Required:**
```rust
pub struct SystemBus {
    pub dram: Dram,
    pub fifo: Fifo,
    pub video: VideoDevice,  // NEW
    pub audio: AudioDevice,  // NEW
}

impl SystemBus {
    // Update read_word to route video/audio addresses
    pub fn read_word(&mut self, addr: u32) -> u32 {
        match addr {
            // FIFO (existing)
            0x4000_0000..=0x4000_0007 => { /* existing FIFO logic */ }
            
            // Video control registers
            0x5010_0000 => self.video.read_control(),
            0x5010_0004 => self.video.width,
            0x5010_0008 => self.video.height,
            0x5010_000C => self.video.format as u32,
            
            // Audio control registers
            0x6010_0000 => self.video.get_write_ptr(),
            0x6010_0004 => self.audio.get_read_ptr(),
            0x6010_0008 => self.audio.sample_rate,
            0x6010_000C => self.audio.format_to_u32(),
            
            // DRAM (default)
            _ => self.dram.read_word(addr),
        }
    }
    
    // Update write_word to route video/audio addresses
    pub fn write_word(&mut self, addr: u32, data: u32) {
        match addr {
            // Video framebuffer (write-only)
            0x5000_0000..=0x500F_FFFF => {
                let offset = addr - 0x5000_0000;
                self.video.write_pixel(offset, data);
            }
            
            // Video control registers
            0x5010_0000 => self.video.write_control(data),
            0x5010_0004 => self.video.set_width(data),
            0x5010_0008 => self.video.set_height(data),
            0x5010_000C => self.video.set_format(data),
            
            // Audio buffer
            0x6000_0000..=0x600F_FFFF => {
                let offset = addr - 0x6000_0000;
                self.audio.write_sample(offset, data);
            }
            
            // Audio control registers
            0x6010_0000 => self.audio.set_write_ptr(data),
            0x6010_0008 => self.audio.set_sample_rate(data),
            0x6010_000C => self.audio.set_format(data),
            
            // Rest handled by existing logic
            _ => { /* existing logic */ }
        }
    }
}
```

### Component 4: GUI Viewer Application (`sim-viewer/`)

**New Workspace Member**: Create `sim-viewer` directory as a new binary crate

**Directory Structure:**
```
sim-viewer/
├── Cargo.toml
├── src/
│   ├── main.rs          # Entry point, argument parsing
│   ├── viewer.rs        # Main viewer loop
│   ├── display.rs       # minifb window management
│   └── audio_output.rs  # cpal audio stream
```

**Cargo.toml:**
```toml
[package]
name = "sim-viewer"
version = "0.1.0"
edition = "2021"

[dependencies]
riscv_core = { path = "../riscv_core" }
cpu-sim = { path = "../cpu-sim" }
clap = { version = "4.4", features = ["derive"] }
minifb = "0.27"
cpal = "0.15"
log = "0.4"
env_logger = "0.10"
```

**Main Viewer Loop (`viewer.rs`):**
```rust
pub struct SimViewer {
    simulator: Simulator,
    window: minifb::Window,
    audio_stream: cpal::Stream,
    target_fps: u32,
}

impl SimViewer {
    pub fn new(elf_path: &Path, config: ViewerConfig) -> Result<Self, String>;
    
    pub fn run(&mut self) -> Result<SimulationResult, String> {
        loop {
            // 1. Run CPU for N cycles (enough for ~1 frame at target FPS)
            let cycles_per_frame = self.calculate_cycles_per_frame();
            self.run_cpu_cycles(cycles_per_frame)?;
            
            // 2. Check for frame ready flag
            if self.simulator.bus.video.frame_ready {
                self.update_display()?;
                self.simulator.bus.video.clear_frame_ready();
            }
            
            // 3. Handle window events (ESC to quit, etc.)
            if !self.window.is_open() || self.handle_events()? {
                break;
            }
            
            // 4. Check for program termination
            if self.simulator.is_halted() {
                break;
            }
            
            // 5. Sleep to maintain target framerate
            self.sleep_for_frame();
        }
        
        Ok(self.simulator.get_result())
    }
    
    fn update_display(&mut self) -> Result<(), String>;
    fn handle_events(&mut self) -> Result<bool, String>;
}
```

**Display Management (`display.rs`):**
```rust
pub fn create_window(width: usize, height: usize) -> Result<minifb::Window, String> {
    let mut window = minifb::Window::new(
        "RISC-V Simulator - Video Output",
        width,
        height,
        minifb::WindowOptions {
            resize: false,
            scale: minifb::Scale::X2,  // 2x scaling for better visibility
            ..Default::default()
        },
    ).map_err(|e| format!("Failed to create window: {}", e))?;
    
    // Set target FPS (e.g., 60 FPS)
    window.limit_update_rate(Some(std::time::Duration::from_millis(16)));
    
    Ok(window)
}

pub fn convert_framebuffer(
    framebuffer: &[u8],
    format: VideoFormat,
    width: usize,
    height: usize,
) -> Vec<u32> {
    // Convert from VideoFormat to minifb's u32 RGB format (0x00RRGGBB)
    match format {
        VideoFormat::RGB888 => {
            framebuffer.chunks(3)
                .map(|rgb| {
                    let r = rgb[0] as u32;
                    let g = rgb[1] as u32;
                    let b = rgb[2] as u32;
                    (r << 16) | (g << 8) | b
                })
                .collect()
        }
        VideoFormat::RGB565 => { /* convert 16-bit to 32-bit */ }
        VideoFormat::Grayscale => { /* replicate to RGB */ }
    }
}
```

**Audio Output (`audio_output.rs`):**
```rust
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};

pub struct AudioOutput {
    stream: cpal::Stream,
    buffer_ref: Arc<Mutex<AudioDevice>>,
}

impl AudioOutput {
    pub fn new(audio_device: Arc<Mutex<AudioDevice>>) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host.default_output_device()
            .ok_or("No audio output device available")?;
        
        let config = device.default_output_config()
            .map_err(|e| format!("Failed to get audio config: {}", e))?;
        
        let buffer_ref = audio_device.clone();
        
        let stream = device.build_output_stream(
            &config.into(),
            move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                let mut buffer = buffer_ref.lock().unwrap();
                let samples = buffer.read_samples(data.len());
                
                // Copy samples to output buffer
                for (i, sample) in samples.iter().enumerate() {
                    if i < data.len() {
                        data[i] = *sample;
                    }
                }
                
                // Fill remaining with silence if needed
                for i in samples.len()..data.len() {
                    data[i] = 0;
                }
            },
            |err| eprintln!("Audio stream error: {}", err),
            None,
        ).map_err(|e| format!("Failed to build audio stream: {}", e))?;
        
        stream.play().map_err(|e| format!("Failed to start audio: {}", e))?;
        
        Ok(AudioOutput { stream, buffer_ref })
    }
}
```

**Main Entry Point (`main.rs`):**
```rust
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about = "RISC-V CPU Simulator with Video/Audio Output")]
struct Args {
    /// Path to the RISC-V ELF executable
    elf: PathBuf,
    
    /// Maximum cycles to run (default: 1000000)
    #[arg(short, long, default_value_t = 1000000)]
    max_cycles: u64,
    
    /// Target framerate (FPS)
    #[arg(long, default_value_t = 60)]
    fps: u32,
    
    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
    
    /// Enable audio output
    #[arg(long, default_value_t = true)]
    audio: bool,
}

fn main() {
    let args = Args::parse();
    
    // Initialize logger
    let log_level = if args.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or(log_level)
    ).init();
    
    log::info!("RISC-V Simulator Viewer");
    log::info!("Loading ELF: {}", args.elf.display());
    
    // Create and run viewer
    let config = ViewerConfig {
        target_fps: args.fps,
        enable_audio: args.audio,
        max_cycles: args.max_cycles,
    };
    
    match SimViewer::new(&args.elf, config) {
        Ok(mut viewer) => {
            match viewer.run() {
                Ok(result) => {
                    println!("✓ Simulation completed in {} cycles", result.cycles);
                }
                Err(e) => {
                    eprintln!("✗ Simulation error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("✗ Failed to create viewer: {}", e);
            std::process::exit(1);
        }
    }
}
```

### Component 5: Example Programs

**Example 1: Simple Color Fill (`test_programs/video_test.s`)**
```assembly
# Video test - fills screen with red color
.global _start

_start:
    # Set video resolution to 320x240
    lui x1, 0x50100      # x1 = 0x50100000 (video control base)
    li x2, 320           # width
    sw x2, 4(x1)         # Write to VIDEO_WIDTH
    li x2, 240           # height
    sw x2, 8(x1)         # Write to VIDEO_HEIGHT
    
    # Fill framebuffer with red (RGB888 format)
    lui x3, 0x50000      # x3 = 0x50000000 (framebuffer base)
    li x4, 0xFF0000      # Red color (0x00RRGGBB)
    li x5, 230400        # Total bytes (320*240*3)
    
fill_loop:
    sw x4, 0(x3)         # Write pixel
    addi x3, x3, 4       # Next pixel (assuming word writes)
    addi x5, x5, -4      # Decrement counter
    bnez x5, fill_loop   # Continue if not done
    
    # Signal frame ready
    li x6, 1
    sw x6, 0(x1)         # Set frame_ready flag
    
    # Halt
    lui x7, 0xFFFF0      # x7 = 0xFFFF0000
    addi x7, x7, -16     # x7 = 0xFFFFFFF0 (tohost)
    li x8, 42            # Exit code
    sw x8, 0(x7)         # Write to tohost
```

**Example 2: Rust Program with Video Output**
```rust
// rust-test-program/src/video_demo.rs

#![no_std]
#![no_main]

const VIDEO_FB_BASE: usize = 0x5000_0000;
const VIDEO_CTRL_BASE: usize = 0x5010_0000;
const VIDEO_WIDTH: usize = 320;
const VIDEO_HEIGHT: usize = 240;

struct VideoDevice {
    framebuffer: *mut u8,
    ctrl_reg: *mut u32,
    width: u32,
    height: u32,
}

impl VideoDevice {
    fn new() -> Self {
        VideoDevice {
            framebuffer: VIDEO_FB_BASE as *mut u8,
            ctrl_reg: VIDEO_CTRL_BASE as *mut u32,
            width: VIDEO_WIDTH as u32,
            height: VIDEO_HEIGHT as u32,
        }
    }
    
    fn set_pixel(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8) {
        unsafe {
            let offset = ((y * self.width + x) * 3) as isize;
            *self.framebuffer.offset(offset) = r;
            *self.framebuffer.offset(offset + 1) = g;
            *self.framebuffer.offset(offset + 2) = b;
        }
    }
    
    fn present_frame(&mut self) {
        unsafe {
            core::ptr::write_volatile(self.ctrl_reg, 1); // Set frame_ready
        }
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let mut video = VideoDevice::new();
    
    // Draw a gradient pattern
    for y in 0..VIDEO_HEIGHT as u32 {
        for x in 0..VIDEO_WIDTH as u32 {
            let r = ((x * 255) / VIDEO_WIDTH as u32) as u8;
            let g = ((y * 255) / VIDEO_HEIGHT as u32) as u8;
            let b = 128;
            video.set_pixel(x, y, r, g, b);
        }
    }
    
    // Present the frame
    video.present_frame();
    
    // Halt simulation
    unsafe {
        let tohost = 0xFFFF_FFF0 as *mut u32;
        core::ptr::write_volatile(tohost, 42);
    }
    
    loop {}
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
```

**Example 3: Audio Tone Generator**
```rust
// rust-test-program/src/audio_demo.rs

const AUDIO_BUFFER_BASE: usize = 0x6000_0000;
const AUDIO_CTRL_BASE: usize = 0x6010_0000;
const SAMPLE_RATE: u32 = 48000;

struct AudioDevice {
    buffer: *mut i16,
    write_ptr: *mut u32,
    read_ptr: *const u32,
}

impl AudioDevice {
    fn new() -> Self {
        AudioDevice {
            buffer: AUDIO_BUFFER_BASE as *mut i16,
            write_ptr: AUDIO_CTRL_BASE as *mut u32,
            read_ptr: (AUDIO_CTRL_BASE + 4) as *const u32,
        }
    }
    
    fn write_sample(&mut self, left: i16, right: i16) {
        unsafe {
            let ptr = core::ptr::read_volatile(self.write_ptr) as isize;
            let sample_ptr = self.buffer.offset(ptr / 2); // i16 offset
            core::ptr::write_volatile(sample_ptr, left);
            core::ptr::write_volatile(sample_ptr.offset(1), right);
            
            // Advance write pointer
            let new_ptr = (ptr + 4) % (1024 * 1024); // Wrap at buffer size
            core::ptr::write_volatile(self.write_ptr, new_ptr as u32);
        }
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let mut audio = AudioDevice::new();
    
    // Generate a 440 Hz sine wave (A note)
    let frequency = 440.0;
    let amplitude = 16000.0; // ~50% of i16 max
    
    for i in 0..48000 { // 1 second of audio
        let t = i as f32 / SAMPLE_RATE as f32;
        let sample = (amplitude * (2.0 * 3.14159 * frequency * t).sin()) as i16;
        audio.write_sample(sample, sample); // Mono (same both channels)
    }
    
    // Halt
    unsafe {
        let tohost = 0xFFFF_FFF0 as *mut u32;
        core::ptr::write_volatile(tohost, 42);
    }
    
    loop {}
}
```

---

## Technology Stack

### Core Dependencies

| Crate       | Version | Purpose                                      |
|-------------|---------|----------------------------------------------|
| `minifb`    | 0.27    | Cross-platform windowing and pixel rendering |
| `cpal`      | 0.15    | Cross-platform audio I/O                     |
| `clap`      | 4.4     | Command-line argument parsing                |
| `log`       | 0.4     | Logging framework                            |
| `env_logger`| 0.10    | Logger implementation                        |

### Why minifb?

- **Lightweight**: Minimal dependencies, fast compilation
- **Cross-platform**: Works on Windows, macOS, Linux
- **Simple API**: Direct framebuffer access (perfect for our use case)
- **No GPU required**: Software rendering (matches our simulation context)
- **Proven**: Used in many emulator and retro projects

**Alternatives considered:**
- `pixels` (WebGPU-based, more complex)
- `sdl2` (requires system SDL2 library)
- `winit + softbuffer` (more complex setup)

### Why cpal?

- **Cross-platform**: Works on all major platforms
- **Low-latency**: Designed for real-time audio
- **Simple API**: Stream-based audio output
- **Standard in Rust ecosystem**: Well-maintained

**Alternatives considered:**
- `rodio` (higher-level, adds complexity)
- `sdl2` (requires system library)

---

## Implementation Phases

### Phase 1: Core Infrastructure (Week 1-2)

**Objectives:**
- Add video and audio device modules
- Update system bus with new memory regions
- Create basic `sim-viewer` structure (no GUI yet)

**Tasks:**
1. ✅ Create `cpu-sim/src/video.rs` with `VideoDevice` struct
2. ✅ Create `cpu-sim/src/audio.rs` with `AudioDevice` struct
3. ✅ Update `cpu-sim/src/bus.rs` to route video/audio addresses
4. ✅ Add public API to expose video/audio devices from `SystemBus`
5. ✅ Create `sim-viewer/` directory and basic Cargo.toml
6. ✅ Implement CLI argument parsing in `sim-viewer/src/main.rs`
7. ✅ Write unit tests for video/audio device read/write operations

**Success Criteria:**
- Unit tests pass for video/audio devices
- Memory routing correctly directs video/audio addresses
- `cargo build` succeeds for all workspace members

### Phase 2: Video Output (Week 2-3)

**Objectives:**
- Implement minifb window creation and rendering
- Connect simulator to display output

**Tasks:**
1. ✅ Add `minifb` dependency to `sim-viewer`
2. ✅ Implement `display.rs` with window creation and framebuffer conversion
3. ✅ Implement `viewer.rs` with main simulation loop
4. ✅ Add frame timing and FPS limiting
5. ✅ Test with simple assembly program (solid color fill)
6. ✅ Implement support for multiple pixel formats (RGB888, RGB565)

**Success Criteria:**
- `sim-viewer` displays a window with video output
- Assembly test program successfully renders colors
- Framerate is stable at target FPS
- Window handles close/escape events

### Phase 3: Audio Output (Week 3-4)

**Objectives:**
- Implement cpal audio stream
- Connect simulator to audio output

**Tasks:**
1. ✅ Add `cpal` dependency to `sim-viewer`
2. ✅ Implement `audio_output.rs` with audio stream setup
3. ✅ Connect audio device buffer to cpal callback
4. ✅ Handle ring buffer wraparound and underruns
5. ✅ Test with simple tone generator program

**Success Criteria:**
- Audio output works without crackling or stuttering
- Ring buffer properly handles read/write pointers
- Audio plays for duration of simulation
- Graceful handling when buffer is empty

### Phase 4: Testing and Examples (Week 4-5)

**Objectives:**
- Create comprehensive test programs
- Document usage and API

**Tasks:**
1. ✅ Write assembly test programs:
   - `video_color_test.s` (solid colors)
   - `video_pattern_test.s` (gradients, patterns)
   - `audio_tone_test.s` (simple sine wave)
2. ✅ Write Rust test programs:
   - `video_demo.rs` (complex graphics)
   - `audio_demo.rs` (tone generator)
   - `combined_demo.rs` (video + audio together)
3. ✅ Add automated tests in `cpu-sim/src/tests.rs`
4. ✅ Update documentation:
   - `sim-viewer/README.md` (usage guide)
   - `docs/video-audio-api.md` (memory map and API reference)
   - Update root `README.md` with new features

**Success Criteria:**
- All test programs run successfully
- Documentation is clear and comprehensive
- Examples demonstrate key features

### Phase 5: Polish and Optimization (Week 5-6)

**Objectives:**
- Performance optimization
- Error handling improvements
- Edge case handling

**Tasks:**
1. ✅ Profile simulation performance with video/audio active
2. ✅ Optimize hot paths in video/audio device code
3. ✅ Add comprehensive error messages
4. ✅ Handle edge cases:
   - Invalid resolutions
   - Audio buffer overflow/underflow
   - Unsupported formats
5. ✅ Add command-line options for debugging
6. ✅ CI/CD integration (ensure tests pass)

**Success Criteria:**
- Simulation runs smoothly at 60 FPS with video+audio
- Clear error messages for common problems
- All CI checks pass
- Code review feedback addressed

---

## User Workflow

### For End Users (Running Programs)

1. **Write a program** (Assembly or Rust) that outputs video/audio
2. **Compile to ELF** using RISC-V toolchain
3. **Run with sim-viewer**:
   ```bash
   cargo run --package sim-viewer -- my_program.elf --fps 60
   ```
4. **View output** in the window that opens
5. **Exit** by closing window or pressing ESC

### For Developers (Creating Video/Audio Programs)

**Assembly Example:**
```assembly
# Set pixel at (100, 50) to red
lui x1, 0x50000          # Framebuffer base
li x2, 15000             # Offset = (50 * 320 + 100) * 3
add x1, x1, x2
li x3, 0xFF              # Red component
sb x3, 0(x1)
sb x0, 1(x1)             # Green = 0
sb x0, 2(x1)             # Blue = 0

# Signal frame ready
lui x4, 0x50100
li x5, 1
sw x5, 0(x4)
```

**Rust Example:**
```rust
// Using a helper library (to be created)
use riscv_sim_io::{Video, Audio};

let mut video = Video::new();
video.clear(0x000000); // Clear to black
video.draw_rect(50, 50, 100, 100, 0xFF0000); // Red rectangle
video.present();

let mut audio = Audio::new();
audio.play_tone(440, 1.0); // 440 Hz for 1 second
```

---

## Testing Strategy

### Unit Tests

**Location**: `cpu-sim/src/tests.rs` (existing test infrastructure)

**Tests to Add:**
1. ✅ `test_video_device_basic_write` - Verify pixel writes
2. ✅ `test_video_device_control_registers` - Test control register R/W
3. ✅ `test_video_device_format_conversion` - Test different pixel formats
4. ✅ `test_audio_device_ring_buffer` - Verify ring buffer logic
5. ✅ `test_audio_device_wraparound` - Test buffer wraparound
6. ✅ `test_bus_video_routing` - Verify bus routes video addresses
7. ✅ `test_bus_audio_routing` - Verify bus routes audio addresses

### Integration Tests

**Location**: `sim-viewer/tests/`

**Tests to Add:**
1. ✅ `test_simple_video_output` - Run ELF that fills screen with color
2. ✅ `test_multiple_frames` - Run ELF that outputs multiple frames
3. ✅ `test_audio_output` - Run ELF that generates audio samples
4. ✅ `test_combined_video_audio` - Run ELF with both outputs
5. ✅ `test_format_switching` - Test changing video formats at runtime

### Manual Testing

**Test Scenarios:**
1. Run video demo and verify smooth rendering at 60 FPS
2. Run audio demo and verify clear sound without artifacts
3. Run combined demo and verify synchronization
4. Test window resize/close behaviors
5. Test with various resolutions (160x120, 320x240, 640x480)
6. Test on different platforms (Linux, macOS, Windows)

---

## Future Enhancements

### Near-Term Enhancements (Next 3-6 months)

1. **Keyboard Input**
   - Memory-mapped keyboard status register
   - Scancode buffer for keypresses
   - Enable interactive programs/games

2. **Hardware Acceleration Simulation**
   - Memory-mapped GPU-like device
   - Sprite rendering commands
   - Tile-based backgrounds
   - Hardware scrolling

3. **Additional Video Formats**
   - Indexed color (8-bit palette mode)
   - YUV color space
   - Compressed formats

4. **Performance Improvements**
   - Frame skipping for slower programs
   - Adjustable simulation speed
   - Record/replay capability

### Long-Term Enhancements (6-12 months)

1. **Complete Retro System Simulation**
   - Simulated 2D graphics chip (sprites, tiles)
   - Simulated sound chip (square wave, noise, etc.)
   - Simulated DMA controller
   - Complete retro game platform

2. **Recording/Export**
   - Record video output to file (MP4, GIF)
   - Export audio to WAV
   - Screenshot capability

3. **Network I/O**
   - Memory-mapped network device
   - UDP packet send/receive
   - Multiplayer support

4. **Advanced Features**
   - Debugger integration with video/audio visualization
   - Performance profiling overlays
   - Live memory inspection while rendering

---

## Risk Assessment

### Technical Risks

| Risk | Impact | Likelihood | Mitigation |
|------|--------|------------|------------|
| **Performance bottleneck** - Simulator too slow for real-time video | High | Medium | Start with low resolution (320x240), optimize hot paths, use release builds |
| **Audio buffer underruns** - Choppy audio playback | Medium | Medium | Use large ring buffer (1 MB), monitor buffer levels, add buffering |
| **Platform compatibility issues** - minifb/cpal don't work on some systems | Medium | Low | Both crates are well-tested cross-platform, fallback to headless mode |
| **Memory consumption** - Large framebuffers use too much RAM | Low | Low | 320x240x3 = 225 KB is minimal, can limit resolution |
| **Synchronization issues** - Video/audio out of sync | Medium | Medium | Use cycle-accurate timing, careful buffer management |

### Implementation Risks

| Risk | Impact | Likelihood | Mitigation |
|------|--------|------------|------------|
| **Scope creep** - Adding too many features initially | Medium | High | Stick to MVP: basic video + audio only, defer enhancements |
| **Testing complexity** - Difficult to automate GUI/audio tests | Medium | Medium | Focus on unit tests for devices, manual testing for integration |
| **Documentation lag** - Docs don't keep up with changes | Low | Medium | Update docs in same PR as code changes |
| **Breaking existing code** - Changes to bus/simulator break tests | High | Low | Maintain backward compatibility, run all tests frequently |

### Mitigation Strategies

1. **Start Small**: Begin with lowest resolution and simplest format (RGB888)
2. **Frequent Testing**: Run existing tests after each change to catch regressions
3. **Performance Monitoring**: Profile early and often, optimize before moving forward
4. **Incremental Development**: Implement video first, then audio separately
5. **Clear Milestones**: Use phased approach with clear success criteria

---

## Appendix A: Memory Map Quick Reference

```
Address         Size      Device              Register/Region
────────────────────────────────────────────────────────────────
0x0000_0000     1 GB      DRAM                Program code/data
0x4000_0000     4 bytes   FIFO                DATA (R/W)
0x4000_0004     4 bytes   FIFO                STATUS (R)
0x5000_0000     1 MB      Video               Framebuffer (W)
0x5010_0000     4 bytes   Video Control       FRAME_READY (R/W)
0x5010_0004     4 bytes   Video Control       WIDTH (R/W)
0x5010_0008     4 bytes   Video Control       HEIGHT (R/W)
0x5010_000C     4 bytes   Video Control       FORMAT (R/W)
0x6000_0000     1 MB      Audio               Sample Buffer (W)
0x6010_0000     4 bytes   Audio Control       WRITE_PTR (R/W)
0x6010_0004     4 bytes   Audio Control       READ_PTR (R)
0x6010_0008     4 bytes   Audio Control       SAMPLE_RATE (R/W)
0x6010_000C     4 bytes   Audio Control       FORMAT (R/W)
0xFFFF_FFF0     4 bytes   TOHOST              Halt signal (W)
```

## Appendix B: Video Format Specifications

### RGB888 (24-bit)
- 3 bytes per pixel
- Byte order: R, G, B
- Range: 0-255 per channel
- Total colors: 16,777,216

### RGB565 (16-bit)
- 2 bytes per pixel
- Bit layout: RRRR RGGG GGGB BBBB
- Red: 5 bits, Green: 6 bits, Blue: 5 bits
- Total colors: 65,536

### Grayscale (8-bit)
- 1 byte per pixel
- Range: 0-255 (0=black, 255=white)
- Total shades: 256

## Appendix C: Audio Format Specifications

### 16-bit Stereo PCM (Default)
- 2 channels (left, right)
- 16-bit signed samples (-32768 to 32767)
- 4 bytes per frame (2 bytes × 2 channels)
- Sample rate: Configurable (default 48 kHz)

### 16-bit Mono PCM
- 1 channel
- 16-bit signed samples
- 2 bytes per frame

## Appendix D: Example Build Commands

### Building the Viewer
```bash
# Build sim-viewer
cargo build --package sim-viewer --release

# Run with example program
cargo run --package sim-viewer --release -- test_programs/video_demo.elf
```

### Building Test Programs (Assembly)
```bash
# Assemble and link
riscv64-unknown-elf-as -march=rv32i -mabi=ilp32 -o video_test.o video_test.s
riscv64-unknown-elf-ld -T linker.ld -m elf32lriscv -o video_test.elf video_test.o
```

### Building Test Programs (Rust)
```bash
cd rust-test-program
cargo build --release --target riscv32i-unknown-none-elf --bin video_demo
cp target/riscv32i-unknown-none-elf/release/video_demo ../test_programs/video_demo.elf
```

---

## Revision History

| Version | Date       | Author        | Changes                          |
|---------|------------|---------------|----------------------------------|
| 1.0     | 2025-12-31 | GitHub Copilot| Initial technical plan           |

---

**End of Document**
