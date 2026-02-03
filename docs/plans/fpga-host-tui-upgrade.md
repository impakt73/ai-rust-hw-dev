# FPGA Host TUI Upgrade Implementation Plan

## 1. Overview

This document describes the implementation plan for upgrading the `fpga-host` crate to provide a modern terminal user interface (TUI) experience using the `ratatui` crate. This upgrade transforms the existing command-line application into an interactive terminal application with a scrolling log view and command shell.

### 1.1 Current State

The existing `fpga-host` application:
- Accepts command-line arguments for serial device path, baud rate, and ELF file
- Opens a serial connection immediately on startup (exits on failure)
- Logs messages to stdout via `env_logger`
- Runs a bus request processing loop until Ctrl+C is pressed
- Has no interactive command input capability

### 1.2 Target State

The upgraded application will:
- Use `ratatui` for a modern TUI experience
- Switch to an alternate terminal buffer on startup
- Provide a scrolling text log covering most of the screen
- Auto-scroll to show new log messages
- Include a command prompt at the bottom for user input
- Support an internal shell with commands: `exit`, `connect`, `disconnect`
- Display connection status in the prompt
- Handle panics and shutdown gracefully with `ratatui::restore()`
- Only process FPGA bus requests when a valid serial connection exists
- Integrate with `tui-logger` for displaying log messages in the TUI

---

## 2. UI Design

### 2.1 Screen Layout

```
┌──────────────────────────────────────────────────────────────────────┐
│                         Log Window                                    │
│                                                                       │
│ [INFO] FPGA Host Interface v0.1.0                                    │
│ [INFO] Type 'help' for available commands                            │
│ [DEBUG] Initialized sparse memory                                     │
│ [INFO] Loaded ELF: entry point 0x80000000                            │
│ [INFO] Connected to /dev/ttyUSB0 @ 115200 baud                       │
│ [DEBUG] [1] READ word @ 0x80000000 => 0x00000013                     │
│ [DEBUG] [2] READ word @ 0x80000004 => 0x00000093                     │
│ ...                                                                   │
│ (auto-scrolls to bottom for new messages)                            │
├──────────────────────────────────────────────────────────────────────┤
│ [Connected: /dev/ttyUSB0] > _                                        │
└──────────────────────────────────────────────────────────────────────┘
```

### 2.2 Prompt Formats

**Disconnected state:**
```
[Disconnected] > _
```

**Connected state:**
```
[Connected: /dev/ttyUSB0] > _
```

### 2.3 Color Scheme

| Element | Color |
|---------|-------|
| Log border | Gray |
| Log messages | Default/White |
| Info level | Green |
| Debug level | Cyan |
| Warning level | Yellow |
| Error level | Red |
| Connected status | Green |
| Disconnected status | Red/Yellow |
| Command prompt | White |
| Input text | White |

---

## 3. Command Shell Specification

### 3.1 Supported Commands

| Command | Arguments | Description |
|---------|-----------|-------------|
| `exit` | None | Exit the application gracefully |
| `connect` | `<device_path> [baud_rate]` | Connect to serial port |
| `disconnect` | None | Disconnect current serial connection |
| `help` | None | Display available commands |

### 3.2 Command Details

#### 3.2.1 `exit`

**Syntax:** `exit`

**Behavior:**
1. Disconnect any active serial connection
2. Restore terminal to original state via `ratatui::restore()`
3. Exit application with code 0

**Example:**
```
[Disconnected] > exit
```

#### 3.2.2 `connect`

**Syntax:** `connect <device_path> [baud_rate]`

**Arguments:**
- `device_path` (required): Path to serial device (e.g., `/dev/ttyUSB0`)
- `baud_rate` (optional): Baud rate, defaults to 115200

**Behavior:**
1. If already connected, log error and return
2. Attempt to open serial port with specified parameters
3. On success: update connection state, log success message
4. On failure: log error message with details

**Examples:**
```
[Disconnected] > connect /dev/ttyUSB0
[INFO] Connected to /dev/ttyUSB0 @ 115200 baud

[Disconnected] > connect /dev/ttyACM0 9600
[INFO] Connected to /dev/ttyACM0 @ 9600 baud

[Connected: /dev/ttyUSB0] > connect /dev/ttyUSB1
[ERROR] Already connected. Disconnect first.
```

#### 3.2.3 `disconnect`

**Syntax:** `disconnect`

**Behavior:**
1. If not connected, log warning and return
2. Close serial port
3. Update connection state
4. Log disconnection message

**Examples:**
```
[Connected: /dev/ttyUSB0] > disconnect
[INFO] Disconnected from /dev/ttyUSB0

[Disconnected] > disconnect
[WARN] Not connected
```

#### 3.2.4 `help`

**Syntax:** `help`

**Behavior:**
- Display list of available commands with brief descriptions

**Output:**
```
Available commands:
  connect <device> [baud]  - Connect to serial port (default baud: 115200)
  disconnect               - Disconnect from serial port
  exit                     - Exit the application
  help                     - Show this help message
```

---

## 4. Architecture Design

### 4.1 Module Structure

```
fpga-host/src/
├── main.rs              # Entry point, TUI initialization, main event loop
├── app.rs               # Application state and business logic
├── ui.rs                # UI rendering with ratatui
├── shell.rs             # Command parsing and execution
├── serial.rs            # Serial port management (extracted)
└── memory.rs            # Sparse memory model (extracted)
```

### 4.2 Application State

```rust
/// Application state
pub struct App {
    /// Current connection state
    connection: ConnectionState,
    
    /// Sparse memory for ELF and bus transactions
    memory: SparseMemory,
    
    /// Path to ELF file (from command line)
    elf_path: Option<PathBuf>,
    
    /// Current input buffer for command line
    input_buffer: String,
    
    /// Command history for up/down navigation
    command_history: Vec<String>,
    
    /// Current position in command history
    history_index: Option<usize>,
    
    /// Bus transaction state machine
    bus_state: HostBusState,
    
    /// Current transaction being processed
    current_txn: HostBusTransaction,
    
    /// Request counter
    request_count: u64,
    
    /// Application running flag
    running: bool,
}

/// Serial connection state
pub enum ConnectionState {
    Disconnected,
    Connected {
        port: Box<dyn SerialPort>,
        device_path: String,
        baud_rate: u32,
    },
}
```

### 4.3 Event Loop Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Main Event Loop                           │
│                                                                  │
│  1. Poll for events (keyboard, serial data) with timeout        │
│  2. Process keyboard events → update input buffer or execute    │
│  3. Process serial data → handle bus requests (if connected)    │
│  4. Render UI                                                    │
│  5. Repeat until running == false                                │
└─────────────────────────────────────────────────────────────────┘
```

### 4.4 Panic Handling

```rust
fn main() {
    // Install panic hook to restore terminal before panic message
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // Restore terminal state
        let _ = ratatui::restore();
        // Call original panic hook
        original_hook(panic_info);
    }));
    
    // Initialize TUI
    ratatui::init();
    
    // Run application
    let result = run_app();
    
    // Restore terminal
    ratatui::restore();
    
    // Handle any error from run_app
    if let Err(e) = result {
        eprintln!("Application error: {}", e);
        std::process::exit(1);
    }
}
```

---

## 5. Dependencies

### 5.1 New Dependencies

Add to `fpga-host/Cargo.toml`:

```toml
[dependencies]
# Existing
clap = { version = "4.4", features = ["derive"] }
elf = "0.7"
log = "0.4"
serialport = "4.6"
riscv_shared = { path = "../riscv_shared" }

# New - TUI
ratatui = "0.29"
crossterm = "0.28"
tui-logger = "0.14"

# Remove
# env_logger = "0.10"  # Replaced by tui-logger
# ctrlc = "3.4"        # Handled by crossterm events
```

### 5.2 Dependency Notes

| Dependency | Purpose |
|------------|---------|
| `ratatui` | Modern TUI framework |
| `crossterm` | Terminal backend for ratatui, event handling |
| `tui-logger` | Capture `log` macro output and display in TUI widget |

---

## 6. Implementation Details

### 6.1 TUI Initialization and Shutdown

```rust
use ratatui::crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::prelude::*;
use std::io;

/// Initialize the TUI
fn init_tui() -> io::Result<Terminal<impl Backend>> {
    // ratatui::init() handles:
    // - enable raw mode
    // - enter alternate screen
    // - enable mouse capture (optional)
    // - create terminal with crossterm backend
    let terminal = ratatui::init();
    Ok(terminal)
}

/// Restore terminal state
fn restore_tui() {
    // ratatui::restore() handles:
    // - disable raw mode
    // - leave alternate screen
    // - show cursor
    let _ = ratatui::restore();
}
```

### 6.2 Log Widget with tui-logger

```rust
use tui_logger::{TuiLoggerWidget, TuiLoggerLevelOutput};

/// Initialize logging to capture via tui-logger
fn init_logging() {
    // Initialize tui-logger at the start of the application
    tui_logger::init_logger(log::LevelFilter::Debug).unwrap();
    
    // Set default level for all targets
    tui_logger::set_default_level(log::LevelFilter::Debug);
}

/// Render the log widget
fn render_log(frame: &mut Frame, area: Rect) {
    let log_widget = TuiLoggerWidget::default()
        .block(Block::default().borders(Borders::ALL).title("Log"))
        .output_separator('|')
        .output_timestamp(Some("%H:%M:%S".to_string()))
        .output_level(Some(TuiLoggerLevelOutput::Long))
        .output_target(false)
        .output_file(false)
        .output_line(false)
        .style(Style::default().fg(Color::White));
    
    frame.render_widget(log_widget, area);
}
```

### 6.3 Input Widget

```rust
/// Render the input prompt
fn render_input(frame: &mut Frame, area: Rect, app: &App) {
    let prompt = match &app.connection {
        ConnectionState::Disconnected => {
            Span::styled("[Disconnected]", Style::default().fg(Color::Yellow))
        }
        ConnectionState::Connected { device_path, .. } => {
            Span::styled(
                format!("[Connected: {}]", device_path),
                Style::default().fg(Color::Green),
            )
        }
    };
    
    let input_line = Line::from(vec![
        prompt,
        Span::raw(" > "),
        Span::raw(&app.input_buffer),
        Span::styled("_", Style::default().add_modifier(Modifier::SLOW_BLINK)),
    ]);
    
    let input_widget = Paragraph::new(input_line)
        .block(Block::default().borders(Borders::ALL));
    
    frame.render_widget(input_widget, area);
}
```

### 6.4 Main UI Layout

```rust
fn ui(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(10),      // Log window (flexible, takes remaining space)
            Constraint::Length(3),    // Input line (fixed height)
        ])
        .split(frame.area());
    
    render_log(frame, chunks[0]);
    render_input(frame, chunks[1], app);
}
```

### 6.5 Event Handling

```rust
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use std::time::Duration;

/// Handle keyboard input
fn handle_key_event(app: &mut App, key: event::KeyEvent) -> io::Result<()> {
    match (key.modifiers, key.code) {
        // Ctrl+C - exit
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            app.running = false;
        }
        
        // Enter - execute command
        (_, KeyCode::Enter) => {
            let command = app.input_buffer.drain(..).collect::<String>();
            if !command.is_empty() {
                app.command_history.push(command.clone());
                app.history_index = None;
                execute_command(app, &command);
            }
        }
        
        // Backspace - delete character
        (_, KeyCode::Backspace) => {
            app.input_buffer.pop();
        }
        
        // Up arrow - previous command
        (_, KeyCode::Up) => {
            if !app.command_history.is_empty() {
                let new_index = match app.history_index {
                    Some(i) if i > 0 => Some(i - 1),
                    Some(i) => Some(i),
                    None => Some(app.command_history.len() - 1),
                };
                app.history_index = new_index;
                if let Some(i) = new_index {
                    app.input_buffer = app.command_history[i].clone();
                }
            }
        }
        
        // Down arrow - next command
        (_, KeyCode::Down) => {
            if let Some(i) = app.history_index {
                if i + 1 < app.command_history.len() {
                    app.history_index = Some(i + 1);
                    app.input_buffer = app.command_history[i + 1].clone();
                } else {
                    app.history_index = None;
                    app.input_buffer.clear();
                }
            }
        }
        
        // Regular character input
        (_, KeyCode::Char(c)) => {
            app.input_buffer.push(c);
        }
        
        _ => {}
    }
    
    Ok(())
}
```

### 6.6 Serial Data Processing (Conditional)

```rust
/// Process serial data if connected
fn process_serial_data(app: &mut App) -> io::Result<()> {
    // Only process if connected
    let port = match &mut app.connection {
        ConnectionState::Connected { port, .. } => port,
        ConnectionState::Disconnected => return Ok(()),
    };
    
    // Non-blocking read with small buffer
    let mut byte_buf = [0u8; 1];
    
    // Process bus requests using existing state machine logic
    match app.bus_state {
        HostBusState::WaitHeader => {
            match port.read(&mut byte_buf) {
                Ok(1) => {
                    // Parse header and transition state
                    let header = byte_buf[0];
                    app.current_txn.we = (header & 0x01) != 0;
                    app.current_txn.size = (header >> 2) & 0x03;
                    app.current_txn.addr = 0;
                    app.current_txn.wdata = 0;
                    app.current_txn.rdata = 0;
                    
                    log::debug!(
                        "Received header: 0x{:02x} (we={}, size={})",
                        header,
                        app.current_txn.we,
                        size_name(app.current_txn.size)
                    );
                    app.bus_state = HostBusState::RxAddr { byte_idx: 0 };
                }
                Ok(0) | Err(_) => {
                    // No data, continue
                }
                Ok(_) => unreachable!(),
            }
        }
        // ... (other states from existing implementation)
    }
    
    Ok(())
}
```

### 6.7 Main Event Loop

```rust
fn run_app(terminal: &mut Terminal<impl Backend>, app: &mut App) -> io::Result<()> {
    while app.running {
        // Render UI
        terminal.draw(|frame| ui(frame, app))?;
        
        // Poll for events with short timeout to allow serial processing
        if event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = event::read()? {
                handle_key_event(app, key)?;
            }
        }
        
        // Process serial data (only when connected)
        process_serial_data(app)?;
    }
    
    Ok(())
}
```

---

## 7. Command Line Arguments

### 7.1 Updated CLI Structure

The CLI arguments should be updated to make the serial connection optional at startup:

```rust
#[derive(Parser)]
#[command(author, version, about = "FPGA Host Interface for RISC-V CPU")]
struct Args {
    /// Path to the RISC-V ELF executable to load
    #[arg(short, long)]
    elf: PathBuf,

    /// Path to the serial device for auto-connect on startup (optional)
    #[arg(short, long)]
    serial: Option<PathBuf>,

    /// Baud rate for serial communication (used with --serial)
    #[arg(short, long, default_value_t = 115200)]
    baud: u32,

    /// Enable verbose logging (debug level)
    #[arg(short, long)]
    verbose: bool,
}
```

### 7.2 Startup Behavior

1. Load ELF file into sparse memory
2. Initialize TUI
3. If `--serial` provided:
   - Attempt to connect to specified serial port
   - Log success or failure (don't exit on failure)
4. Enter main event loop

---

## 8. Error Handling

### 8.1 Serial Connection Errors

```rust
/// Attempt to connect to serial port
fn connect_serial(
    device_path: &str,
    baud_rate: u32,
) -> Result<Box<dyn SerialPort>, serialport::Error> {
    serialport::new(device_path, baud_rate)
        .timeout(Duration::from_millis(100))
        .open()
}

/// Execute connect command
fn cmd_connect(app: &mut App, args: &[&str]) {
    // Check if already connected
    if matches!(app.connection, ConnectionState::Connected { .. }) {
        log::error!("Already connected. Disconnect first.");
        return;
    }
    
    // Parse arguments
    let device_path = match args.first() {
        Some(path) => *path,
        None => {
            log::error!("Usage: connect <device_path> [baud_rate]");
            return;
        }
    };
    
    let baud_rate = args.get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(115200);
    
    // Attempt connection
    match connect_serial(device_path, baud_rate) {
        Ok(port) => {
            log::info!("Connected to {} @ {} baud", device_path, baud_rate);
            app.connection = ConnectionState::Connected {
                port,
                device_path: device_path.to_string(),
                baud_rate,
            };
            // Reset bus state for new connection
            app.bus_state = HostBusState::WaitHeader;
            app.current_txn = HostBusTransaction::default();
        }
        Err(e) => {
            log::error!("Failed to connect to {}: {}", device_path, e);
        }
    }
}
```

### 8.2 Disconnect Handling

```rust
/// Handle disconnection (including unexpected)
fn disconnect(app: &mut App) {
    if let ConnectionState::Connected { device_path, .. } = &app.connection {
        log::info!("Disconnected from {}", device_path);
    }
    app.connection = ConnectionState::Disconnected;
    app.bus_state = HostBusState::WaitHeader;
    app.current_txn = HostBusTransaction::default();
}
```

---

## 9. Testing Strategy

### 9.1 Manual Testing Checklist

Since this is a TUI application with hardware dependencies, focus on manual testing:

- [ ] Application starts and displays TUI correctly
- [ ] Log messages appear in the log window
- [ ] Auto-scroll works when new messages arrive
- [ ] Prompt shows "[Disconnected]" when not connected
- [ ] `help` command displays available commands
- [ ] `connect` with invalid device shows error
- [ ] `connect` with valid device shows success and updates prompt
- [ ] `disconnect` works and updates prompt
- [ ] `exit` command exits cleanly
- [ ] Ctrl+C exits cleanly
- [ ] Panic restores terminal properly
- [ ] Command history (up/down arrows) works
- [ ] Backspace deletes characters
- [ ] Bus requests are processed when connected
- [ ] Bus requests are ignored when disconnected

### 9.2 Unit Tests

Extract testable logic into separate functions:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_connect_command() {
        let (path, baud) = parse_connect_args(&["connect", "/dev/ttyUSB0"]).unwrap();
        assert_eq!(path, "/dev/ttyUSB0");
        assert_eq!(baud, 115200);  // default
        
        let (path, baud) = parse_connect_args(&["connect", "/dev/ttyACM0", "9600"]).unwrap();
        assert_eq!(path, "/dev/ttyACM0");
        assert_eq!(baud, 9600);
    }
    
    #[test]
    fn test_sparse_memory_operations() {
        // Existing memory tests
    }
    
    #[test]
    fn test_elf_loading() {
        // Existing ELF loading tests
    }
}
```

---

## 10. Implementation Order

### Phase 1: Project Setup and Dependencies
1. Update `Cargo.toml` with new dependencies
2. Remove `env_logger` and `ctrlc` dependencies
3. Verify dependencies compile

### Phase 2: Module Extraction
1. Extract `SparseMemory` to `memory.rs`
2. Extract serial-related types and functions to `serial.rs`
3. Create `app.rs` with `App` struct and `ConnectionState`
4. Verify existing functionality still works

### Phase 3: TUI Infrastructure
1. Create `ui.rs` with basic layout
2. Add TUI initialization/restoration to `main.rs`
3. Implement panic hook for terminal restoration
4. Integrate `tui-logger` for log capture
5. Render basic log widget and input prompt

### Phase 4: Command Shell
1. Create `shell.rs` with command parsing
2. Implement `help` command
3. Implement `exit` command
4. Implement `connect` command
5. Implement `disconnect` command
6. Add command history support

### Phase 5: Event Loop Integration
1. Implement main event loop with polling
2. Add keyboard event handling
3. Integrate serial data processing (conditional on connection)
4. Port existing bus state machine logic

### Phase 6: Polish and Testing
1. Add color styling to log levels
2. Add cursor blinking effect
3. Manual testing of all features
4. Code cleanup and documentation

---

## 11. Validation Checklist

Before marking complete:

- [ ] All Rust code formatted: `cargo fmt`
- [ ] Clippy auto-fix run: `cargo clippy --fix --allow-dirty`
- [ ] No clippy warnings: `cargo clippy -- -D warnings`
- [ ] Application builds: `cargo build -p fpga-host`
- [ ] TUI displays correctly on startup
- [ ] All commands work as specified
- [ ] Terminal restores properly on exit
- [ ] Terminal restores properly on panic
- [ ] Log messages display correctly
- [ ] Connection state reflected in prompt
- [ ] Serial processing only occurs when connected

---

## 12. Risk Assessment

### 12.1 Terminal Compatibility
- **Risk**: Some terminals may not support all features
- **Mitigation**: Use crossterm backend (wide terminal support), test on common terminals

### 12.2 Serial Port Behavior
- **Risk**: Non-blocking reads may behave differently across platforms
- **Mitigation**: Use short timeouts, handle all error cases gracefully

### 12.3 Performance
- **Risk**: Frequent redraws may cause flickering
- **Mitigation**: Use ratatui's differential rendering, poll with appropriate timeout

### 12.4 Log Volume
- **Risk**: High-frequency bus requests may flood the log
- **Mitigation**: Use debug level for individual requests, allow log level filtering

---

## 13. Future Enhancements (Out of Scope)

The following features are not part of this implementation but could be added later:

- Scrollable log view (manual scroll up/down)
- Log filtering by level
- Log search functionality
- Multiple serial connection support
- File logging alongside TUI display
- Configuration file for default settings
- Memory view/dump commands
- Breakpoint/debug commands

---

## 14. Appendix: Code Snippets

### 14.1 Complete Cargo.toml

```toml
[package]
name = "fpga-host"
version = "0.1.0"
edition = "2021"
description = "FPGA host interface for RISC-V CPU over serial connection"

[[bin]]
name = "fpga-host"
path = "src/main.rs"

[dependencies]
clap = { version = "4.4", features = ["derive"] }
elf = "0.7"
log = "0.4"
serialport = "4.6"
riscv_shared = { path = "../riscv_shared" }

# TUI dependencies
ratatui = "0.29"
crossterm = "0.28"
tui-logger = "0.14"
```

### 14.2 Minimal main.rs Structure

```rust
//! FPGA Host Interface with TUI
//!
//! Interactive terminal application for communicating with a RISC-V CPU
//! running on an FPGA over a serial connection.

mod app;
mod memory;
mod serial;
mod shell;
mod ui;

use app::App;
use clap::Parser;
use std::io;
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about = "FPGA Host Interface for RISC-V CPU")]
struct Args {
    /// Path to the RISC-V ELF executable to load
    #[arg(short, long)]
    elf: PathBuf,

    /// Path to the serial device for auto-connect on startup
    #[arg(short, long)]
    serial: Option<PathBuf>,

    /// Baud rate for serial communication
    #[arg(short, long, default_value_t = 115200)]
    baud: u32,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

fn main() {
    // Parse arguments before TUI initialization
    let args = Args::parse();
    
    // Initialize tui-logger
    let level = if args.verbose {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };
    tui_logger::init_logger(level).expect("Failed to initialize logger");
    tui_logger::set_default_level(level);
    
    // Install panic hook to restore terminal
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = ratatui::restore();
        original_hook(panic_info);
    }));
    
    // Initialize TUI
    let mut terminal = ratatui::init();
    
    // Create and run application
    let result = run(args, &mut terminal);
    
    // Restore terminal
    ratatui::restore();
    
    // Handle result
    if let Err(e) = result {
        eprintln!("Application error: {}", e);
        std::process::exit(1);
    }
}

fn run(
    args: Args,
    terminal: &mut ratatui::DefaultTerminal,
) -> io::Result<()> {
    // Create application
    let mut app = App::new(&args.elf)?;
    
    log::info!("FPGA Host Interface v{}", env!("CARGO_PKG_VERSION"));
    log::info!("Type 'help' for available commands");
    
    // Auto-connect if serial specified
    if let Some(ref serial_path) = args.serial {
        app.connect(&serial_path.to_string_lossy(), args.baud);
    }
    
    // Main event loop
    while app.is_running() {
        terminal.draw(|frame| ui::render(frame, &app))?;
        app.handle_events()?;
    }
    
    Ok(())
}
```
