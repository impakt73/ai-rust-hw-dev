# sim-view - RISC-V CPU Simulator Viewer

Real-time video and audio viewer for programs running on the simulated RISC-V CPU.

## Overview

`sim-view` provides an interactive GUI application that executes RISC-V ELF programs on the CPU simulator and displays live video and audio output. It leverages the `InteractiveSimulator` API from `cpu-sim` to provide step-by-step execution control with real-time multimedia output.

## Features

- **Real-time video rendering** using softbuffer/winit window (supports RGBA8, RGB8, RGB565, R8 formats)
- **Real-time audio playback** using cpal audio stream (supports i16, f32, u16 sample formats)
- **Interactive controls**:
  - **Escape**: Exit the viewer
  - **Space**: Pause/Resume simulation
  - **Ctrl+R**: Reload the last ELF file (useful during development)
- **Dynamic window sizing** based on video configuration from the program
- **Dynamic audio format** based on audio configuration from the program
- **State management**: Idle → Running → Paused → Halted transitions

## Installation

This package is part of the ai-rust-hw-dev workspace.

### Prerequisites

On Linux, the ALSA development libraries are required for audio support:

```bash
# Ubuntu/Debian
sudo apt-get update && sudo apt-get install -y libasound2-dev

# Fedora/RHEL
sudo dnf install alsa-lib-devel

# Arch Linux
sudo pacman -S alsa-lib
```

### Building

Once prerequisites are installed, build the package:

```bash
cargo build --package sim-view --release
```

## Usage

### Basic Usage

Run an ELF program on the simulator with video/audio output:

```bash
# First, build the test programs (automatically happens when running tests)
cargo build --package sim-tests

# Run with an ELF file from the automatically built test programs
# The test programs are built from rust-test-program/ and stored in cargo's build directory
cargo run --package sim-view -- $(cargo build -p sim-tests 2>&1 | grep "test_video_pattern.elf" | awk '{print $NF}')

# Alternatively, provide your own ELF file path
./target/debug/sim-view path/to/your/program.elf
```

**Note:** Test programs are now built automatically from the `rust-test-program/` directory 
when you build or test packages that depend on `sim-tests`. The precompiled `test_programs/` 
folder has been removed in favor of this cargo-native workflow.

### Command-Line Options

```
sim-view [OPTIONS] [ELF_FILE]

Arguments:
  [ELF_FILE]  Path to the RISC-V ELF executable to run on startup (optional)

Options:
  -m, --max-cycles <MAX_CYCLES>    Maximum cycles to run before auto-terminating (0 = unlimited) [default: 0]
  -v, --verbose                    Enable verbose logging
      --print-inst-trace           Print instruction trace (prints every instruction executed)
      --width <WIDTH>              Initial window width [default: 320]
      --height <HEIGHT>            Initial window height [default: 240]
      --headless                   Run in headless mode (no GUI, for testing)
  -h, --help                       Print help
```

### Headless Mode

The viewer now supports a **headless mode** for automated testing and CI/CD environments. In headless mode:

- No GUI window is created
- Video frames and audio samples are captured in memory with timestamps
- The viewer can be controlled programmatically via test commands
- Perfect for integration tests and automated validation

**Usage:**

```bash
# Run in headless mode with a test program
cargo run --package sim-view -- --headless --max-cycles 100000 <path-to-elf>

# Run headless integration tests (test programs are built automatically)
cargo test --package sim-view
```

**Example output:**

```
Headless mode completed:
  Frames captured: 42
```

Headless mode is used by the integration tests in `tests/headless_integration.rs` to validate viewer functionality without requiring a GUI environment.

### Examples

```bash
# Run with verbose logging (provide path to your ELF file)
cargo run --package sim-view -- -v path/to/program.elf

# Run with custom window size
cargo run --package sim-view -- --width 640 --height 480 path/to/program.elf

# Run with instruction trace (for debugging)
cargo run --package sim-view -- --print-inst-trace path/to/program.elf

# Run with maximum cycle limit
cargo run --package sim-view -- --max-cycles 100000 path/to/program.elf
```

**Note:** Test programs from `rust-test-program/` are automatically built when you run 
tests. To use them interactively, you can find the built ELF files in the cargo build 
output directory (typically under `target/debug/build/sim-tests-*/out/`).
cargo run --package sim-view -- --max-cycles 100000 test_programs/hello_world.elf
```

## Architecture

### Components

1. **VideoBackend Trait** (`backend_traits.rs`)
   - Abstraction for video output (GUI or headless)
   - Allows dependency injection for different rendering backends

2. **AudioBackend Trait** (`backend_traits.rs`)
   - Abstraction for audio output (GUI or headless)
   - Enables testing without audio hardware

3. **EventSource Trait** (`backend_traits.rs`)
   - Abstraction for input events (keyboard or programmatic)
   - Supports test-driven event injection

4. **GuiVideoBackend** (`gui_backends.rs`)
   - Wraps VideoWindow for GUI mode
   - Uses softbuffer for cross-platform software rendering
   - Uses winit for cross-platform windowing

5. **GuiAudioBackend** (`gui_backends.rs`)
   - Wraps AudioStream for GUI mode
   - Uses cpal for cross-platform audio

6. **GuiEventSource** (`gui_backends.rs`)
   - Provides keyboard events from the video window

7. **HeadlessVideoBackend** (`headless_backends.rs`)
   - Captures video frames with timestamps
   - Used for automated testing

8. **HeadlessAudioBackend** (`headless_backends.rs`)
   - Captures audio samples with timestamps
   - Used for automated testing

9. **HeadlessEventSource** (`headless_backends.rs`)
   - Allows programmatic event injection
   - Used for test control

10. **VideoWindow** (`video_window.rs`)
    - Manages the winit window and softbuffer surface
    - Converts video frames from various formats to RGB for display
    - Handles keyboard events
    - Supports dynamic window resizing

11. **AudioStream** (`audio_stream.rs`)
    - Manages the cpal audio output stream
    - Handles sample format conversions (i16, f32, u16)
    - Implements thread-safe sample buffering

12. **SimulatorController** (`simulator_controller.rs`)
    - Wraps the `InteractiveSimulator` from cpu-sim
    - Registers Video and Audio devices at `VIDEO_BASE` and `AUDIO_BASE`
    - Manages ELF loading and simulation stepping
    - Provides thread-safe queues for video/audio data

13. **SimViewer<V, A, E>** (`viewer.rs`)
    - Main application logic and event loop
    - Generic over VideoBackend, AudioBackend, and EventSource traits
    - State management (Idle, Running, Paused, Halted)
    - Coordinates between simulator and backends
    - Implements keyboard controls and UI feedback

### Memory Map

The simulator uses the following memory-mapped I/O addresses:

- `0x1000_0000` - SimControl (tohost register) - Built-in
- `0x2000_0000` - Video device (`VIDEO_BASE`)
- `0x3000_0000` - Audio device (`AUDIO_BASE`)
- `0x4000_0000` - FIFO device - Built-in
- `0x8000_0000 - 0xFFFF_FFFF` - DRAM - Built-in

Programs can write to the Video and Audio devices using these base addresses. 
See the test programs in the `rust-test-program/` directory for examples.

## Controls

### Keyboard Controls

- **Escape**: Exit the viewer
- **Space**: Pause/Resume simulation
  - When paused, the window title shows `[PAUSED]`
  - When running, the window title shows `[RUNNING]`
  - When halted (program finished), the window title shows `[HALTED]`
- **Ctrl+R**: Reload the last ELF file
  - Useful during development to quickly test changes
  - Resets the simulation state

### Window States

The viewer tracks four states:

1. **Idle**: No program loaded (initial state)
2. **Running**: Program is executing
3. **Paused**: Program is loaded but execution is paused
4. **Halted**: Program has terminated (tohost written or max cycles reached)

## Supported Formats

### Video Formats

The viewer supports the following video formats from the CPU simulator:

- **RGBA8**: 32-bit RGBA (8 bits per channel + alpha)
- **RGB8**: 24-bit RGB (8 bits per channel)
- **RGB565**: 16-bit RGB (5-6-5 bit layout)
- **R8**: 8-bit grayscale

All formats are automatically converted to 0x00RRGGBB for display in the softbuffer/winit window.

### Audio Formats

The viewer supports the following audio sample formats:

- **i16**: 16-bit signed integer samples
- **f32**: 32-bit floating-point samples (normalized to [-1.0, 1.0])
- **u16**: 16-bit unsigned integer samples

The audio device in the simulator outputs i16 samples, which are automatically converted to the host audio device's native format by cpal.

## Performance

The viewer runs at approximately 60 FPS by default. Each frame, the simulator executes ~10,000 instructions. This provides a good balance between responsiveness and simulation speed.

To adjust simulation speed:
- Increase `instructions_per_frame` in `viewer.rs` for faster simulation
- Decrease for slower, more interactive debugging
- Use `--max-cycles` to limit execution time

## Test Programs

Test programs are located in the `rust-test-program/` directory and demonstrate video and audio functionality:

- `test_video_pattern` - Displays a video pattern
- `test_audio_pattern` - Plays an audio pattern  
- `test_image_data` - Displays image data

These programs are automatically built from source when you run tests. They are no longer 
stored as precompiled binaries in the repository.

## Troubleshooting

### Window doesn't open

- Ensure you have a graphical environment (X11, Wayland, etc.)
- On headless systems, use `--headless` flag to run without GUI
- In CI/CD environments, always use `--headless` mode

### No audio output

- Check that your system has audio devices configured
- The viewer will log the audio device being used
- Use `-v` for verbose logging to see audio configuration details

### Simulation runs too fast/slow

- Adjust the `instructions_per_frame` constant in `viewer.rs`
- Use `--max-cycles` to limit execution
- Use Space to pause and step through manually (requires code changes for single-step)

## Future Enhancements

Potential improvements for future versions:

1. **Drag-and-drop support** - Load ELF files by dragging them to the window
2. **Screenshot capability** - Save current frame to PNG file
3. **Recording** - Record video/audio to file
4. **Performance overlay** - Show FPS, cycle count, instruction count
5. **Variable simulation speed** - Speed up/slow down with hotkeys
6. **Single-step mode** - Step one instruction at a time
7. **Debugger integration** - Breakpoints, register inspection
8. **VCD waveform viewer** - Integrated waveform display

## Dependencies

- **cpu-sim**: Core RISC-V CPU simulator with InteractiveSimulator API
- **riscv_core**: RISC-V instruction definitions and trace structures
- **softbuffer**: Cross-platform software framebuffer library
- **winit**: Cross-platform window creation library
- **cpal**: Cross-platform audio I/O library
- **clap**: Command-line argument parsing
- **log/env_logger**: Logging infrastructure

## License

See the root LICENSE file for license information.
