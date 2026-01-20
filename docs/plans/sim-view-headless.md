# sim-view Headless Mode Implementation Plan

**Status:** Planning  
**Created:** 2026-01-20  
**Author:** GitHub Copilot Custom Agent

## Executive Summary

This document outlines a comprehensive technical plan for implementing headless mode in the `sim-view` application. Headless mode will enable automated integration testing against the core `SimViewer` structure by replacing hardware-dependent video and audio systems with dummy implementations that capture all presented data with timestamps. This will allow AI coding agents and CI/CD systems to validate changes without requiring GUI environments.

## Goals and Objectives

### Primary Goals

1. **Enable automated integration testing** of `SimViewer` application logic without requiring GUI hardware
2. **Capture all multimedia data** (video frames and audio samples) with precise timestamps
3. **Support testing of timing-critical behavior** (frame pacing, audio buffer management)
4. **Facilitate AI agent validation** through automated test execution and log-based verification

### Success Criteria

- ✅ Integration tests can create and run `SimViewer` in headless mode
- ✅ All video frames are captured with timestamps and metadata
- ✅ All audio samples are captured with timestamps
- ✅ Tests can verify frame pacing (e.g., "60 FPS" means frames are ~16.67ms apart)
- ✅ Tests can detect audio underruns/overruns
- ✅ Binary can run in headless mode with test ELF files for debugging
- ✅ No changes required to existing GUI mode functionality
- ✅ Zero clippy warnings, passes `cargo fmt --check`

## Current Architecture Analysis

### Component Overview

The current `sim-view` architecture consists of four main components:

1. **SimViewer** (`viewer.rs`) - Main application logic and event loop
   - Manages state machine (Idle → Running → Paused → Halted)
   - Coordinates between simulator, video, and audio
   - Handles keyboard events and user controls
   - Implements frame timing loop

2. **VideoWindow** (`video_window.rs`) - Hardware video output
   - Uses `minifb` for cross-platform windowing
   - Converts various pixel formats to ARGB8888
   - Handles keyboard input events
   - **Hard dependency:** Requires X11/Wayland/Windows GUI

3. **AudioStream** (`audio_stream.rs`) - Hardware audio output
   - Uses `cpal` for cross-platform audio I/O
   - Manages sample buffering with thread-safe queues
   - Converts i16 samples to device-native format
   - **Hard dependency:** Requires audio hardware/drivers

4. **SimulatorController** (`simulator_controller.rs`) - CPU simulation bridge
   - Wraps `InteractiveSimulator` from `cpu-sim`
   - Registers Video and Audio devices at memory-mapped addresses
   - Provides queues for video frames and audio samples
   - **No hardware dependencies** - Pure Rust simulation

### Current Data Flow

```
InteractiveSimulator
    ↓ (Video/Audio device callbacks)
SimulatorController
    ↓ (Frame/sample queues)
SimViewer main loop
    ├→ VideoWindow::process_video_frame() → minifb::update_with_buffer()
    └→ AudioStream::push_samples() → cpal audio callback
```

### Identified Constraints

1. **Tight coupling:** `SimViewer` directly instantiates `VideoWindow` and `AudioStream`
2. **No abstraction:** No trait-based abstraction for video/audio backends
3. **No test infrastructure:** `sim-view` has no `tests/` directory
4. **Timing assumptions:** Main loop runs at ~60 FPS, assumes real-time execution
5. **GUI requirements:** `minifb` requires window creation, fails in headless environments

## Proposed Architecture

### High-Level Design Principles

1. **Trait-based abstraction** - Define `VideoBackend` and `AudioBackend` traits
2. **Dependency injection** - `SimViewer` accepts backend implementations
3. **Zero-copy data capture** - Headless backends store references, not copies
4. **Precise timestamping** - Use `std::time::Instant` for all captured data
5. **Backward compatibility** - Existing GUI mode unchanged, new headless mode additive
6. **Test-friendly API** - Easy to construct and query in integration tests

### Component Architecture

```
                    ┌─────────────────────────────────────┐
                    │         SimViewer<V, A>             │
                    │  (Generic over VideoBackend         │
                    │   and AudioBackend traits)          │
                    └─────────────────────────────────────┘
                         ↓                          ↓
         ┌───────────────────────────┐  ┌─────────────────────────────┐
         │   VideoBackend trait      │  │   AudioBackend trait        │
         └───────────────────────────┘  └─────────────────────────────┘
              ↓                   ↓               ↓                ↓
    ┌─────────────────┐ ┌──────────────────┐  ┌───────────────┐ ┌──────────────────┐
    │  GuiVideoBackend│ │HeadlessVideoBackend│ │GuiAudioBackend│ │HeadlessAudioBackend│
    │   (minifb)      │ │ (capture frames) │  │   (cpal)      │ │ (capture samples)│
    └─────────────────┘ └──────────────────┘  └───────────────┘ └──────────────────┘
```

### Trait Definitions

#### VideoBackend Trait

```rust
use cpu_sim::VideoConfig;
use std::time::Instant;

/// Trait for video output backends (GUI or headless)
pub trait VideoBackend {
    /// Process a video frame from the simulator
    fn process_frame(&mut self, data: &[u8], config: &VideoConfig) 
        -> Result<(), String>;
    
    /// Update display/capture (called once per frame in main loop)
    fn update(&mut self) -> Result<(), String>;
    
    /// Set window title (no-op for headless)
    fn set_title(&mut self, title: &str);
    
    /// Check if backend is still active (window not closed)
    fn is_active(&self) -> bool;
}
```

#### AudioBackend Trait

```rust
/// Trait for audio output backends (GUI or headless)
pub trait AudioBackend {
    /// Push audio samples for playback/capture
    fn push_samples(&mut self, samples: &[i16]);
}
```

#### EventSource Trait

```rust
/// Trait for input event sources (keyboard, headless test driver)
pub trait EventSource {
    /// Get pending events (keyboard, close, test commands)
    fn get_events(&mut self) -> Vec<ViewerEvent>;
}

/// Unified event type
pub enum ViewerEvent {
    KeyPressed(Key, KeyModifiers),
    Close,
    TestCommand(TestCommand),  // For programmatic control in tests
}

pub enum TestCommand {
    LoadELF(PathBuf),
    Pause,
    Resume,
    StepFrames(u64),
    Terminate,
}
```

### Headless Backend Implementations

#### HeadlessVideoBackend

```rust
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct CapturedFrame {
    /// Frame data (owned copy for safety)
    pub data: Vec<u8>,
    
    /// Video configuration at capture time
    pub config: VideoConfig,
    
    /// Timestamp when frame was presented (relative to start)
    pub timestamp: Instant,
    
    /// Frame sequence number (monotonic counter)
    pub sequence: u64,
}

pub struct HeadlessVideoBackend {
    /// All captured frames
    captured_frames: Arc<Mutex<Vec<CapturedFrame>>>,
    
    /// Current frame buffer (before presentation)
    current_frame: Option<(Vec<u8>, VideoConfig)>,
    
    /// Frame sequence counter
    frame_count: u64,
    
    /// Start time for relative timestamps
    start_time: Instant,
}

impl HeadlessVideoBackend {
    pub fn new() -> Self {
        Self {
            captured_frames: Arc::new(Mutex::new(Vec::new())),
            current_frame: None,
            frame_count: 0,
            start_time: Instant::now(),
        }
    }
    
    /// Get handle to captured frames (for tests)
    pub fn get_frames_handle(&self) -> Arc<Mutex<Vec<CapturedFrame>>> {
        Arc::clone(&self.captured_frames)
    }
}

impl VideoBackend for HeadlessVideoBackend {
    fn process_frame(&mut self, data: &[u8], config: &VideoConfig) 
        -> Result<(), String> 
    {
        // Store frame data (will be presented in update())
        self.current_frame = Some((data.to_vec(), *config));
        Ok(())
    }
    
    fn update(&mut self) -> Result<(), String> {
        // Present the current frame (capture with timestamp)
        if let Some((data, config)) = self.current_frame.take() {
            let frame = CapturedFrame {
                data,
                config,
                timestamp: Instant::now(),
                sequence: self.frame_count,
            };
            
            self.captured_frames.lock().unwrap().push(frame);
            self.frame_count += 1;
        }
        
        Ok(())
    }
    
    fn set_title(&mut self, _title: &str) {
        // No-op in headless mode
    }
    
    fn is_active(&self) -> bool {
        true  // Always active in headless mode
    }
}
```

#### HeadlessAudioBackend

```rust
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct CapturedAudioChunk {
    /// Audio samples (owned copy)
    pub samples: Vec<i16>,
    
    /// Timestamp when samples were received (relative to start)
    pub timestamp: Instant,
    
    /// Sample sequence number (cumulative sample count)
    pub sample_offset: u64,
}

pub struct HeadlessAudioBackend {
    /// All captured audio chunks
    captured_chunks: Arc<Mutex<Vec<CapturedAudioChunk>>>,
    
    /// Cumulative sample counter
    sample_count: u64,
    
    /// Start time for relative timestamps
    start_time: Instant,
}

impl HeadlessAudioBackend {
    pub fn new() -> Self {
        Self {
            captured_chunks: Arc::new(Mutex::new(Vec::new())),
            sample_count: 0,
            start_time: Instant::now(),
        }
    }
    
    /// Get handle to captured audio chunks (for tests)
    pub fn get_chunks_handle(&self) -> Arc<Mutex<Vec<CapturedAudioChunk>>> {
        Arc::clone(&self.captured_chunks)
    }
}

impl AudioBackend for HeadlessAudioBackend {
    fn push_samples(&mut self, samples: &[i16]) {
        if samples.is_empty() {
            return;
        }
        
        let chunk = CapturedAudioChunk {
            samples: samples.to_vec(),
            timestamp: Instant::now(),
            sample_offset: self.sample_count,
        };
        
        self.captured_chunks.lock().unwrap().push(chunk);
        self.sample_count += samples.len() as u64;
    }
}
```

#### HeadlessEventSource

```rust
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

pub struct HeadlessEventSource {
    /// Event queue (shared with test driver)
    event_queue: Arc<Mutex<VecDeque<ViewerEvent>>>,
}

impl HeadlessEventSource {
    pub fn new() -> Self {
        Self {
            event_queue: Arc::new(Mutex::new(VecDeque::new())),
        }
    }
    
    /// Get handle for injecting events (for tests)
    pub fn get_event_handle(&self) -> Arc<Mutex<VecDeque<ViewerEvent>>> {
        Arc::clone(&self.event_queue)
    }
}

impl EventSource for HeadlessEventSource {
    fn get_events(&mut self) -> Vec<ViewerEvent> {
        self.event_queue.lock().unwrap().drain(..).collect()
    }
}
```

### SimViewer Refactoring

#### Generic SimViewer

```rust
pub struct SimViewer<V: VideoBackend, A: AudioBackend, E: EventSource> {
    /// Simulation controller (unchanged)
    controller: SimulatorController,
    
    /// Video backend (generic)
    video_backend: V,
    
    /// Audio backend (generic)
    audio_backend: A,
    
    /// Event source (generic)
    event_source: E,
    
    /// Current configuration (unchanged)
    config: ViewerConfig,
    
    /// Current simulation state (unchanged)
    state: ViewerState,
    
    /// Last loaded ELF file path (unchanged)
    last_elf_path: Option<PathBuf>,
    
    /// Cycle counter (unchanged)
    total_cycles: u64,
    
    /// Exit requested flag (unchanged)
    exit_requested: bool,
}

impl<V: VideoBackend, A: AudioBackend, E: EventSource> SimViewer<V, A, E> {
    /// Create new viewer with dependency injection
    pub fn new(
        config: ViewerConfig,
        video_backend: V,
        audio_backend: A,
        event_source: E,
    ) -> Result<Self, String> {
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
            exit_requested: false,
        })
    }
    
    // All other methods remain largely unchanged, but use trait methods
    // instead of concrete types
}
```

#### Type Aliases for Convenience

```rust
// GUI mode type alias (existing behavior)
pub type GuiSimViewer = SimViewer<
    GuiVideoBackend,
    GuiAudioBackend,
    GuiEventSource
>;

// Headless mode type alias (new)
pub type HeadlessSimViewer = SimViewer<
    HeadlessVideoBackend,
    HeadlessAudioBackend,
    HeadlessEventSource
>;
```

### Binary Mode Selection

```rust
// In main.rs
fn main() -> Result<(), String> {
    // Parse CLI arguments
    let args = CliArgs::parse();
    
    if args.headless {
        run_headless_mode(args)
    } else {
        run_gui_mode(args)
    }
}

fn run_gui_mode(args: CliArgs) -> Result<(), String> {
    // Create GUI backends
    let video = GuiVideoBackend::new(args.width, args.height)?;
    let audio = GuiAudioBackend::new()?;
    let events = GuiEventSource::new();
    
    // Create and run viewer
    let mut viewer = GuiSimViewer::new(
        args.into_config(),
        video,
        audio,
        events,
    )?;
    
    if let Some(elf_path) = args.elf_file {
        viewer.load_elf(&elf_path)?;
    }
    
    viewer.run()
}

fn run_headless_mode(args: CliArgs) -> Result<(), String> {
    // Create headless backends
    let video = HeadlessVideoBackend::new();
    let audio = HeadlessAudioBackend::new();
    let events = HeadlessEventSource::new();
    
    // Get handles for inspection
    let frames = video.get_frames_handle();
    let chunks = audio.get_chunks_handle();
    
    // Create and run viewer
    let mut viewer = HeadlessSimViewer::new(
        args.into_config(),
        video,
        audio,
        events,
    )?;
    
    if let Some(elf_path) = args.elf_file {
        viewer.load_elf(&elf_path)?;
    }
    
    viewer.run()?;
    
    // Print summary
    let frame_count = frames.lock().unwrap().len();
    let sample_count: usize = chunks.lock()
        .unwrap()
        .iter()
        .map(|c| c.samples.len())
        .sum();
    
    println!("Headless mode completed:");
    println!("  Frames captured: {}", frame_count);
    println!("  Audio samples captured: {}", sample_count);
    
    Ok(())
}
```

## Testing Strategy

### Integration Test Structure

```rust
// tests/headless_integration.rs

use sim_view::{
    HeadlessSimViewer, HeadlessVideoBackend, HeadlessAudioBackend,
    HeadlessEventSource, ViewerConfig, ViewerEvent, TestCommand
};
use std::path::PathBuf;
use std::time::Duration;

#[test]
fn test_headless_video_capture() {
    let _ = env_logger::builder().is_test(true).try_init();
    
    // Create headless backends
    let video = HeadlessVideoBackend::new();
    let audio = HeadlessAudioBackend::new();
    let events = HeadlessEventSource::new();
    
    // Get handles for verification
    let frames = video.get_frames_handle();
    let event_queue = events.get_event_handle();
    
    // Create viewer
    let config = ViewerConfig {
        initial_width: 320,
        initial_height: 240,
        max_cycles: 100_000,  // Limit execution
        print_inst_trace: false,
    };
    
    let mut viewer = HeadlessSimViewer::new(
        config,
        video,
        audio,
        events,
    ).expect("Failed to create headless viewer");
    
    // Load test ELF
    let elf_path = test_program_path("test_video_pattern.elf");
    viewer.load_elf(&elf_path).expect("Failed to load ELF");
    
    // Inject termination command after 60 frames (~1 second at 60 FPS)
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(1));
        event_queue.lock().unwrap().push_back(
            ViewerEvent::TestCommand(TestCommand::Terminate)
        );
    });
    
    // Run viewer
    viewer.run().expect("Viewer execution failed");
    
    // Verify captured frames
    let captured = frames.lock().unwrap();
    assert!(captured.len() > 0, "Should capture at least one frame");
    assert!(captured.len() <= 70, "Should capture ~60 frames in 1 second");
    
    // Verify frame metadata
    for (i, frame) in captured.iter().enumerate() {
        assert_eq!(frame.sequence, i as u64, "Frame sequence should be monotonic");
        assert_eq!(frame.config.width, 320, "Width should match config");
        assert_eq!(frame.config.height, 240, "Height should match config");
    }
}

#[test]
fn test_frame_pacing() {
    let _ = env_logger::builder().is_test(true).try_init();
    
    // Create headless viewer (similar setup as above)
    let video = HeadlessVideoBackend::new();
    let frames = video.get_frames_handle();
    
    // ... create and run viewer ...
    
    // Analyze frame timing
    let captured = frames.lock().unwrap();
    let frame_times: Vec<Duration> = captured
        .windows(2)
        .map(|pair| pair[1].timestamp.duration_since(pair[0].timestamp))
        .collect();
    
    // At 60 FPS, frames should be ~16.67ms apart
    let expected_frame_time = Duration::from_secs_f64(1.0 / 60.0);
    let tolerance = Duration::from_millis(5);  // ±5ms tolerance
    
    for (i, &dt) in frame_times.iter().enumerate() {
        let diff = if dt > expected_frame_time {
            dt - expected_frame_time
        } else {
            expected_frame_time - dt
        };
        
        assert!(
            diff < tolerance,
            "Frame {} timing off: expected ~{:?}, got {:?} (diff: {:?})",
            i, expected_frame_time, dt, diff
        );
    }
}

#[test]
fn test_audio_no_underrun() {
    let _ = env_logger::builder().is_test(true).try_init();
    
    // Create headless viewer
    let video = HeadlessVideoBackend::new();
    let audio = HeadlessAudioBackend::new();
    let chunks = audio.get_chunks_handle();
    
    // ... create and run viewer with audio test program ...
    
    // Analyze audio timing
    let captured = chunks.lock().unwrap();
    
    // At 48kHz, we need 48000 samples/second
    // If chunks arrive at ~60 FPS, each chunk should have ~800 samples
    // Verify chunks are arriving fast enough to prevent underruns
    
    for (i, chunk) in captured.iter().enumerate() {
        if i > 0 {
            let prev = &captured[i - 1];
            let time_between = chunk.timestamp.duration_since(prev.timestamp);
            let samples_per_sec = chunk.samples.len() as f64 / time_between.as_secs_f64();
            
            // Should deliver samples faster than consumption rate
            assert!(
                samples_per_sec > 40000.0,  // Some margin below 48kHz
                "Audio delivery too slow: {} samples/sec (chunk {})",
                samples_per_sec, i
            );
        }
    }
}

fn test_program_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("test_programs")
        .join(name)
}
```

### Unit Test Strategy

Each backend component should have unit tests:

```rust
// In headless_backends.rs

#[cfg(test)]
mod tests {
    use super::*;
    use cpu_sim::{VideoConfig, VideoFormat};
    
    #[test]
    fn test_headless_video_captures_frames() {
        let mut backend = HeadlessVideoBackend::new();
        let frames = backend.get_frames_handle();
        
        let data = vec![0xFF; 320 * 240 * 4];
        let config = VideoConfig {
            width: 320,
            height: 240,
            format: VideoFormat::Rgba8,
        };
        
        backend.process_frame(&data, &config).unwrap();
        backend.update().unwrap();
        
        let captured = frames.lock().unwrap();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].data.len(), data.len());
        assert_eq!(captured[0].sequence, 0);
    }
    
    #[test]
    fn test_headless_audio_captures_samples() {
        let mut backend = HeadlessAudioBackend::new();
        let chunks = backend.get_chunks_handle();
        
        let samples = vec![100i16, 200, 300];
        backend.push_samples(&samples);
        
        let captured = chunks.lock().unwrap();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].samples, samples);
        assert_eq!(captured[0].sample_offset, 0);
    }
}
```

## Implementation Roadmap

### Phase 1: Trait Abstraction (Week 1)

**Goal:** Introduce traits without breaking existing GUI mode

1. **Define traits** (`src/backend_traits.rs`)
   - `VideoBackend` trait
   - `AudioBackend` trait
   - `EventSource` trait
   - `ViewerEvent` enum (unified events)

2. **Wrap existing implementations**
   - `GuiVideoBackend` (wraps existing `VideoWindow`)
   - `GuiAudioBackend` (wraps existing `AudioStream`)
   - `GuiEventSource` (wraps event collection logic)

3. **Update `SimViewer`**
   - Add generic type parameters `<V, A, E>`
   - Replace concrete types with trait bounds
   - Add type alias `GuiSimViewer` for backward compatibility

4. **Update `main.rs`**
   - Extract GUI construction to `run_gui_mode()`
   - Ensure existing behavior unchanged

**Validation:**
- ✅ Existing GUI mode compiles and runs
- ✅ No functional regressions
- ✅ `cargo clippy -- -D warnings` passes
- ✅ `cargo fmt --check` passes

### Phase 2: Headless Backends (Week 2)

**Goal:** Implement headless backends with capture functionality

1. **Implement `HeadlessVideoBackend`** (`src/headless_backends.rs`)
   - `CapturedFrame` struct with timestamp
   - Frame capture with sequence numbers
   - Unit tests for frame capture

2. **Implement `HeadlessAudioBackend`** (`src/headless_backends.rs`)
   - `CapturedAudioChunk` struct with timestamp
   - Sample capture with offsets
   - Unit tests for sample capture

3. **Implement `HeadlessEventSource`** (`src/headless_backends.rs`)
   - `TestCommand` enum for programmatic control
   - Event injection API
   - Unit tests for event handling

4. **Add headless binary mode** (`src/main.rs`)
   - Add `--headless` CLI flag
   - Implement `run_headless_mode()`
   - Print summary on exit

**Validation:**
- ✅ Headless mode compiles
- ✅ Can run test ELF in headless mode
- ✅ Captures are populated
- ✅ Unit tests pass
- ✅ No clippy warnings

### Phase 3: Integration Tests (Week 3)

**Goal:** Write comprehensive integration tests

1. **Test infrastructure** (`tests/headless_integration.rs`)
   - `test_program_path()` helper
   - Test setup/teardown patterns
   - Shared test utilities

2. **Basic functionality tests**
   - `test_headless_video_capture()` - Verify frame capture
   - `test_headless_audio_capture()` - Verify sample capture
   - `test_headless_load_and_run()` - End-to-end smoke test

3. **Timing verification tests**
   - `test_frame_pacing()` - Verify ~60 FPS frame rate
   - `test_audio_timing()` - Verify sample delivery rate
   - `test_no_audio_underrun()` - Verify sufficient buffering

4. **Edge case tests**
   - `test_dynamic_resolution_change()` - Handle video config changes
   - `test_early_termination()` - Handle program halt
   - `test_max_cycles_limit()` - Verify cycle limit enforcement

**Validation:**
- ✅ All integration tests pass
- ✅ Tests cover key scenarios
- ✅ Tests run reliably in CI
- ✅ Tests complete in <30 seconds

### Phase 4: Documentation and Polish (Week 4)

**Goal:** Complete documentation and ensure production quality

1. **Update README.md**
   - Document `--headless` flag
   - Provide headless mode examples
   - Explain use cases (testing, CI, debugging)

2. **Add integration test examples** (`tests/examples/`)
   - Example: Basic frame capture test
   - Example: Frame timing verification
   - Example: Audio underrun detection

3. **Code quality sweep**
   - Run `cargo clippy -- -D warnings`
   - Run `cargo fmt`
   - Address all warnings
   - Add `#[must_use]` attributes where appropriate
   - Add comprehensive doc comments

4. **CI/CD integration**
   - Add headless tests to CI pipeline
   - Ensure tests run on all platforms
   - Add test coverage reporting

**Validation:**
- ✅ Documentation is complete and accurate
- ✅ All tests pass in CI
- ✅ Zero clippy warnings
- ✅ Code coverage >80% for new code
- ✅ `cargo fmt --check` passes

## Technical Considerations

### Performance

**Frame Capture Overhead:**
- Each frame requires a `Vec<u8>` allocation (320×240×4 = ~300KB)
- At 60 FPS, this is ~18 MB/s allocation rate
- For 10-second test: ~180 MB memory usage
- **Mitigation:** Limit test duration, add frame retention limits

**Audio Capture Overhead:**
- 48kHz mono i16 samples = 96 KB/s
- For 10-second test: ~960 KB memory usage
- **Mitigation:** Chunk-based storage, minimal overhead

**Test Execution Time:**
- Headless mode removes ~16ms frame vsync delay
- Tests may run faster than real-time
- **Mitigation:** Add frame rate limiting in headless mode if needed

### Memory Safety

**Thread Safety:**
- `Arc<Mutex<Vec<T>>>` for shared capture buffers
- Backend handles remain valid for entire viewer lifetime
- No `unsafe` code required

**Lifetime Concerns:**
- Captured frames are owned (`Vec<u8>`), not borrowed
- No lifetime parameters needed
- Simple `'static` trait bounds

**Data Races:**
- All shared state protected by `Mutex`
- Capture happens on main thread only
- No concurrent writes to capture buffers

### Compatibility

**Platform Support:**
- Headless mode is pure Rust, no platform dependencies
- Works on Linux, macOS, Windows
- Works in Docker, CI, and headless servers

**Rust Version:**
- Requires Rust 2021 edition (already used)
- No unstable features required
- Compatible with stable Rust

**Dependency Changes:**
- No new dependencies required
- `minifb` and `cpal` become optional in headless mode
- Consider feature flags: `gui` (default), `headless`

### Error Handling

**Trait Error Propagation:**
- All trait methods return `Result<(), String>`
- Errors propagated to viewer main loop
- Tests can assert on error conditions

**Backend Failure Modes:**
- Headless backends are infallible (always succeed)
- GUI backends can fail (window closed, audio device lost)
- Tests should verify graceful degradation

## Cargo Feature Flags

### Optional Feature Structure

```toml
# Cargo.toml
[features]
default = ["gui"]
gui = ["minifb", "cpal"]
headless = []
```

### Conditional Compilation

```rust
// src/lib.rs

#[cfg(feature = "gui")]
pub mod gui_backends;

#[cfg(feature = "headless")]
pub mod headless_backends;

pub mod backend_traits;
pub mod viewer;

// Re-exports
#[cfg(feature = "gui")]
pub use gui_backends::{GuiVideoBackend, GuiAudioBackend, GuiEventSource};

#[cfg(feature = "headless")]
pub use headless_backends::{
    HeadlessVideoBackend, HeadlessAudioBackend, HeadlessEventSource,
    CapturedFrame, CapturedAudioChunk
};
```

**Benefits:**
- Reduce binary size for headless-only builds
- Avoid requiring GUI dependencies in CI
- Enable pure headless Docker images

## AI Agent Integration

### Running Tests Programmatically

AI agents can validate changes by:

1. **Running integration tests:**
   ```bash
   cargo test --package sim-view --test headless_integration
   ```

2. **Running headless binary with debug logging:**
   ```bash
   cargo run --package sim-view -- --headless -v test_programs/test_video.elf
   ```

3. **Parsing output for verification:**
   - Check "Frames captured: N" matches expectation
   - Check "Audio samples captured: N" is non-zero
   - Verify no errors in log output

### Test-Driven Development Workflow

1. **Agent identifies change needed** (e.g., fix frame timing)
2. **Agent writes failing integration test** (e.g., `test_frame_pacing()`)
3. **Agent modifies code** to fix issue
4. **Agent runs headless tests** to verify fix
5. **Agent confirms** test passes before submitting PR

### Debug Logging Integration

```rust
// In headless backend
impl HeadlessVideoBackend {
    fn update(&mut self) -> Result<(), String> {
        if let Some((data, config)) = self.current_frame.take() {
            log::debug!(
                "Captured frame {} ({}x{}, {:?})",
                self.frame_count,
                config.width,
                config.height,
                config.format
            );
            
            // ... capture frame ...
        }
        Ok(())
    }
}
```

**Agent can analyze logs to verify:**
- Frame sequence is correct
- Frame dimensions match expectations
- No dropped frames
- Timing is consistent

## Open Questions and Decisions

### Q1: Should we add a "fast-forward" mode for tests?

**Context:** Tests might want to run faster than 60 FPS

**Options:**
1. Add `--no-frame-limit` flag to run as fast as possible
2. Add `--target-fps` flag to control simulation rate
3. Keep current behavior (tests run at ~60 FPS)

**Recommendation:** Option 1 (fast-forward mode) for faster test execution

### Q2: How long should we retain captured data?

**Context:** Long-running tests could accumulate GBs of frames

**Options:**
1. Unlimited retention (simple, but memory risk)
2. Sliding window (keep last N frames)
3. Configurable retention policy

**Recommendation:** Option 1 for now, add Option 3 if memory becomes an issue

### Q3: Should we add frame comparison utilities?

**Context:** Tests might want to compare frames pixel-by-pixel

**Options:**
1. Add `assert_frame_eq!(expected, actual, tolerance)` helper
2. Add image diffing utilities
3. Leave to test authors

**Recommendation:** Option 3 initially, add Option 1 if common pattern emerges

### Q4: Should headless backends be `Send + Sync`?

**Context:** Might want to run multiple tests in parallel

**Options:**
1. Make backends `Send + Sync` (requires `Arc<Mutex<>>` everywhere)
2. Keep single-threaded (simpler, current pattern)

**Recommendation:** Option 2 (single-threaded), tests can run in separate processes

## Success Metrics

### Code Quality Metrics

- ✅ Zero clippy warnings (`cargo clippy -- -D warnings`)
- ✅ 100% formatted (`cargo fmt --check`)
- ✅ >80% test coverage on new code
- ✅ All public APIs have doc comments

### Functional Metrics

- ✅ 10+ integration tests covering key scenarios
- ✅ All tests pass in CI on Linux, macOS, Windows
- ✅ Tests complete in <30 seconds
- ✅ No flaky tests (run 100 times without failure)

### Documentation Metrics

- ✅ README.md has headless mode section
- ✅ All new APIs have rustdoc examples
- ✅ Integration test examples provided
- ✅ AI agent workflow documented

## Risk Assessment

### High-Impact Risks

1. **Backward Compatibility Break** (High probability, High impact)
   - **Mitigation:** Extensive testing of GUI mode, type aliases, careful refactoring
   - **Fallback:** Feature flag to disable changes if issues found

2. **Performance Regression in GUI Mode** (Medium probability, High impact)
   - **Mitigation:** Benchmark before/after, profile hot paths
   - **Fallback:** Zero-cost abstractions, inline trait methods

3. **Test Flakiness** (Medium probability, Medium impact)
   - **Mitigation:** Careful timing tolerance, retry logic, deterministic tests
   - **Fallback:** Disable flaky tests, investigate root cause

### Medium-Impact Risks

4. **Increased Binary Size** (Low probability, Medium impact)
   - **Mitigation:** Feature flags to exclude GUI deps in headless builds
   - **Fallback:** Accept size increase if minimal

5. **Complex Generic Constraints** (Medium probability, Low impact)
   - **Mitigation:** Type aliases, clear documentation
   - **Fallback:** Simplify trait hierarchy if too complex

## Conclusion

This plan provides a comprehensive roadmap for implementing headless mode in `sim-view` with the following key benefits:

1. **Automated testing** of viewer application logic
2. **CI/CD integration** for continuous validation
3. **AI agent enablement** for automated change verification
4. **Timing analysis** capabilities for performance testing
5. **Zero impact** on existing GUI mode functionality

The phased implementation approach ensures:
- Minimal risk to existing functionality
- Clear validation criteria at each phase
- Incremental value delivery
- Easy rollback if issues arise

**Estimated Timeline:** 4 weeks  
**Estimated Effort:** 1 engineer, full-time  
**Risk Level:** Low (careful design, extensive testing)

**Next Steps:**
1. Review and approve this plan
2. Create implementation issues for each phase
3. Begin Phase 1: Trait Abstraction
4. Review after each phase before proceeding
