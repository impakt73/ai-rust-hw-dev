# FPGA Host TUI Upgrade Implementation Plan

## 1. Overview

This document describes the implementation plan for upgrading the `fpga-host` crate to provide a modern terminal user interface (TUI) experience using the `ratatui` crate. The upgrade transforms the current command-line tool into an interactive application with a scrolling log view, command shell, and proper serial port lifecycle management.

### 1.1 Current State

The existing `fpga-host` application:
- Uses `clap` for CLI argument parsing (serial device, baud rate, ELF file, verbose flag)
- Opens a serial connection immediately on startup
- Runs a single-threaded main loop processing FPGA bus requests
- Uses `env_logger` for logging to stdout
- Handles Ctrl+C via `ctrlc` crate for graceful shutdown
- Requires all arguments at startup (no interactive control)

### 1.2 Target State

The upgraded application will provide:
- Interactive TUI using `ratatui` with alternate screen buffer
- Scrolling log window displaying real-time messages
- Command prompt for interactive shell commands
- Dynamic serial port connection management (connect/disconnect at runtime)
- Support for loading ELF files after startup
- Proper panic handling and terminal restoration
- Integration with the `log` macro via `tui-logger`

---

## 2. Architecture Design

### 2.1 Module Structure

```
fpga-host/src/
├── main.rs              # Entry point, TUI initialization, main event loop
├── app.rs               # Application state management
├── ui.rs                # UI rendering logic (ratatui widgets)
├── shell.rs             # Command parsing and execution
├── serial.rs            # Serial port abstraction and bus protocol
├── memory.rs            # SparseMemory (extracted from current main.rs)
└── elf_loader.rs        # ELF loading logic (extracted from current main.rs)
```

### 2.2 Core Components

#### 2.2.1 Application State (`app.rs`)

```rust
pub struct App {
    /// Serial connection state
    serial: Option<SerialConnection>,
    
    /// Sparse memory model for DRAM
    memory: SparseMemory,
    
    /// Command input buffer
    input_buffer: String,
    
    /// Command history for potential up/down navigation
    command_history: Vec<String>,
    
    /// Log message buffer for display (ring buffer)
    log_messages: VecDeque<LogLine>,
    
    /// Maximum number of log lines to retain
    max_log_lines: usize,
    
    /// Whether the application should exit
    should_quit: bool,
    
    /// Scroll offset for log view (0 = auto-scroll to bottom)
    scroll_offset: usize,
    
    /// Bus request statistics
    request_count: u64,
}
```

#### 2.2.2 Serial Connection (`serial.rs`)

```rust
pub struct SerialConnection {
    /// Underlying serial port
    port: Box<dyn SerialPort>,
    
    /// Device path for status display
    device_path: String,
    
    /// Baud rate for status display
    baud_rate: u32,
    
    /// Host bus interface state machine
    bus_state: HostBusState,
    
    /// Current transaction being processed
    current_txn: HostBusTransaction,
}

impl SerialConnection {
    /// Create a new serial connection
    pub fn connect(device: &str, baud: u32) -> Result<Self, SerialError>;
    
    /// Close the connection (consumes self)
    pub fn disconnect(self);
    
    /// Poll for and process bus requests (non-blocking)
    /// Returns Ok(Some(event)) if a complete request was processed
    pub fn poll(&mut self, memory: &mut SparseMemory) -> Result<Option<BusEvent>, SerialError>;
}
```

#### 2.2.3 Shell Commands (`shell.rs`)

```rust
pub enum ShellCommand {
    /// Exit the application
    Exit,
    
    /// Display help information
    Help { command: Option<String> },
    
    /// Show connection status
    Status,
    
    /// Connect to serial port
    Connect { device: String, baud: u32 },
    
    /// Disconnect from serial port
    Disconnect,
    
    /// Load an ELF file into memory
    LoadElf { path: String },
}

impl ShellCommand {
    /// Parse a command string into a ShellCommand
    pub fn parse(input: &str) -> Result<Self, ParseError>;
    
    /// Execute the command and return a result message
    pub fn execute(self, app: &mut App) -> CommandResult;
}
```

### 2.3 UI Layout

```
┌──────────────────────────────────────────────────────────────────────────┐
│                           FPGA Host Interface                            │
├──────────────────────────────────────────────────────────────────────────┤
│ [INFO] FPGA Host Interface started                                       │
│ [INFO] Type 'help' for available commands                                │
│ [INFO] Use 'connect <device> <baud>' to connect to serial port           │
│ [DEBUG] Waiting for connection...                                        │
│ [INFO] Connected to /dev/ttyUSB0 at 115200 baud                          │
│ [INFO] [1] READ word @ 0x80000000 => 0x00000013                          │
│ [INFO] [2] WRITE word @ 0x80000004 <= 0xDEADBEEF                         │
│                                                                          │
│ (log messages scroll here, auto-scroll to bottom)                        │
│                                                                          │
├──────────────────────────────────────────────────────────────────────────┤
│ [CONNECTED] > _                                                          │
│ or                                                                       │
│ [DISCONNECTED] > _                                                       │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Implementation Tasks

### 3.1 Phase 1: Project Setup and Dependencies

#### Task 1.1: Update Cargo.toml

Add the following dependencies:

```toml
[dependencies]
# Existing dependencies
clap = { version = "4.4", features = ["derive"] }
elf = "0.7"
log = "0.4"
serialport = "4.6"
riscv_shared = { path = "../riscv_shared" }

# New dependencies
ratatui = "0.29"
crossterm = "0.28"
tui-logger = "0.14"

# Remove/replace
# env_logger = "0.10"  # Replaced by tui-logger
# ctrlc = "3.4"        # Replaced by crossterm event handling
```

**Rationale:**
- `ratatui` version 0.29 is the latest stable version with the `init()` and `restore()` APIs
- `crossterm` is required for terminal manipulation and event handling (ratatui backend)
- `tui-logger` provides integration between the `log` crate and TUI widgets
- `ctrlc` is removed because crossterm handles Ctrl+C as a keyboard event

#### Task 1.2: Create Module Files

Create the following empty module files:
- `src/app.rs`
- `src/ui.rs`
- `src/shell.rs`
- `src/serial.rs`
- `src/memory.rs`
- `src/elf_loader.rs`

Update `src/main.rs` to declare modules:

```rust
mod app;
mod elf_loader;
mod memory;
mod serial;
mod shell;
mod ui;
```

---

### 3.2 Phase 2: Extract Existing Functionality

#### Task 2.1: Extract SparseMemory to memory.rs

Move the `SparseMemory` struct and its implementation from `main.rs` to `memory.rs`:

```rust
// memory.rs
use std::collections::HashMap;

/// Sparse memory model using a byte-addressable HashMap
pub struct SparseMemory {
    data: HashMap<u32, u8>,
}

impl SparseMemory {
    pub fn new() -> Self { ... }
    pub fn read_byte(&self, addr: u32) -> u8 { ... }
    pub fn read_halfword(&self, addr: u32) -> u16 { ... }
    pub fn read_word(&self, addr: u32) -> u32 { ... }
    pub fn write_byte(&mut self, addr: u32, data: u8) { ... }
    pub fn write_halfword(&mut self, addr: u32, data: u16) { ... }
    pub fn write_word(&mut self, addr: u32, data: u32) { ... }
    
    /// Clear all memory contents (for loadelf command)
    pub fn clear(&mut self) { ... }
}

impl Default for SparseMemory {
    fn default() -> Self {
        Self::new()
    }
}
```

#### Task 2.2: Extract ELF Loading to elf_loader.rs

Move the `load_elf` function from `main.rs` to `elf_loader.rs`:

```rust
// elf_loader.rs
use crate::memory::SparseMemory;
use std::path::Path;

#[derive(Debug)]
pub enum ElfError {
    IoError(std::io::Error),
    ParseError(String),
    SegmentOutOfBounds { offset: usize, size: usize, file_len: usize },
}

impl std::fmt::Display for ElfError { ... }
impl std::error::Error for ElfError { ... }

/// Load an ELF file into sparse memory
/// Returns the entry point address on success
pub fn load_elf(memory: &mut SparseMemory, path: &Path) -> Result<u32, ElfError> { ... }
```

#### Task 2.3: Extract Serial/Bus Logic to serial.rs

Move the bus protocol state machine and transaction handling to `serial.rs`:

```rust
// serial.rs
use crate::memory::SparseMemory;
use riscv_shared::bus::{DRAM_BASE, DRAM_END};
use serialport::SerialPort;
use std::time::Duration;

#[derive(Debug)]
pub enum SerialError {
    OpenFailed(serialport::Error),
    IoError(std::io::Error),
    AlreadyConnected,
    NotConnected,
}

impl std::fmt::Display for SerialError { ... }
impl std::error::Error for SerialError { ... }

/// Host bus interface state machine states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostBusState { ... }

/// Captured transaction from host bus interface
#[derive(Debug, Clone, Default)]
struct HostBusTransaction { ... }

/// Event generated when a bus transaction completes
#[derive(Debug)]
pub enum BusEvent {
    Read { addr: u32, size: u8, data: u32, is_dram: bool },
    Write { addr: u32, size: u8, data: u32, is_dram: bool },
}

/// Serial connection with bus protocol handling
pub struct SerialConnection { ... }

impl SerialConnection {
    pub fn connect(device: &str, baud: u32) -> Result<Self, SerialError> { ... }
    pub fn poll(&mut self, memory: &mut SparseMemory) -> Result<Option<BusEvent>, SerialError> { ... }
}

impl std::fmt::Display for SerialConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { ... }
}
```

---

### 3.3 Phase 3: Implement TUI Foundation

#### Task 3.1: Implement Terminal Setup in main.rs

```rust
// main.rs
use std::io;
use std::panic;
use ratatui::DefaultTerminal;

fn main() -> io::Result<()> {
    // Parse CLI arguments (still supported for initial connection)
    let args = Args::parse();
    
    // Initialize tui-logger
    tui_logger::init_logger(log::LevelFilter::Debug).unwrap();
    tui_logger::set_default_level(log::LevelFilter::Debug);
    
    // Set up panic hook to restore terminal on panic
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // Attempt to restore terminal before displaying panic
        let _ = ratatui::restore();
        original_hook(panic_info);
    }));
    
    // Initialize terminal (switches to alternate screen)
    let terminal = ratatui::init();
    
    // Run the application
    let result = run_app(terminal, args);
    
    // Restore terminal to normal state
    ratatui::restore()?;
    
    result
}

fn run_app(mut terminal: DefaultTerminal, args: Args) -> io::Result<()> {
    let mut app = App::new();
    
    // If CLI args provided serial/elf, connect and load automatically
    if let Some(serial_path) = args.serial {
        // Auto-connect based on CLI args
        ...
    }
    
    // Main event loop
    loop {
        // Draw UI
        terminal.draw(|frame| ui::render(frame, &app))?;
        
        // Handle input events (with timeout for serial polling)
        if crossterm::event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = crossterm::event::read()? {
                app.handle_key_event(key);
            }
        }
        
        // Poll serial connection if connected
        if let Some(ref mut serial) = app.serial {
            match serial.poll(&mut app.memory) {
                Ok(Some(event)) => {
                    app.log_bus_event(&event);
                    app.request_count += 1;
                }
                Ok(None) => {}
                Err(e) => {
                    log::error!("Serial error: {}", e);
                }
            }
        }
        
        // Check exit condition
        if app.should_quit {
            break;
        }
    }
    
    Ok(())
}
```

#### Task 3.2: Implement Application State in app.rs

```rust
// app.rs
use crate::memory::SparseMemory;
use crate::serial::{BusEvent, SerialConnection};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::VecDeque;

const MAX_LOG_LINES: usize = 1000;

#[derive(Debug, Clone)]
pub struct LogLine {
    pub level: log::Level,
    pub message: String,
    pub timestamp: std::time::Instant,
}

pub struct App {
    pub serial: Option<SerialConnection>,
    pub memory: SparseMemory,
    pub input_buffer: String,
    pub command_history: Vec<String>,
    pub history_index: Option<usize>,
    pub log_messages: VecDeque<LogLine>,
    pub should_quit: bool,
    pub request_count: u64,
    pub scroll_offset: usize,  // 0 = auto-scroll
}

impl App {
    pub fn new() -> Self {
        Self {
            serial: None,
            memory: SparseMemory::new(),
            input_buffer: String::new(),
            command_history: Vec::new(),
            history_index: None,
            log_messages: VecDeque::with_capacity(MAX_LOG_LINES),
            should_quit: false,
            request_count: 0,
            scroll_offset: 0,
        }
    }
    
    pub fn is_connected(&self) -> bool {
        self.serial.is_some()
    }
    
    pub fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            // Ctrl+C triggers exit
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            // Enter submits command
            KeyCode::Enter => {
                self.submit_command();
            }
            // Backspace deletes character
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            // Up/Down for command history
            KeyCode::Up => {
                self.history_prev();
            }
            KeyCode::Down => {
                self.history_next();
            }
            // Page Up/Down for log scrolling
            KeyCode::PageUp => {
                self.scroll_up(10);
            }
            KeyCode::PageDown => {
                self.scroll_down(10);
            }
            // Regular character input
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
            }
            _ => {}
        }
    }
    
    fn submit_command(&mut self) {
        let input = self.input_buffer.trim().to_string();
        if input.is_empty() {
            return;
        }
        
        // Add to history
        self.command_history.push(input.clone());
        self.history_index = None;
        self.input_buffer.clear();
        
        // Parse and execute command
        match crate::shell::ShellCommand::parse(&input) {
            Ok(cmd) => {
                let result = cmd.execute(self);
                if let Some(msg) = result.message {
                    log::info!("{}", msg);
                }
            }
            Err(e) => {
                log::error!("Command error: {}", e);
            }
        }
    }
    
    pub fn add_log(&mut self, level: log::Level, message: String) {
        if self.log_messages.len() >= MAX_LOG_LINES {
            self.log_messages.pop_front();
        }
        self.log_messages.push_back(LogLine {
            level,
            message,
            timestamp: std::time::Instant::now(),
        });
        // Auto-scroll if at bottom
        if self.scroll_offset == 0 {
            // Will naturally show newest messages
        }
    }
    
    pub fn log_bus_event(&mut self, event: &BusEvent) {
        let msg = match event {
            BusEvent::Read { addr, size, data, is_dram } => {
                let suffix = if *is_dram { "" } else { " (non-DRAM)" };
                format!("[{}] READ {} @ 0x{:08x} => 0x{:0width$x}{}",
                    self.request_count + 1,
                    size_name(*size),
                    addr,
                    data,
                    suffix,
                    width = (bytes_for_size(*size) * 2) as usize
                )
            }
            BusEvent::Write { addr, size, data, is_dram } => {
                let suffix = if *is_dram { "" } else { " (non-DRAM, dropped)" };
                format!("[{}] WRITE {} @ 0x{:08x} <= 0x{:0width$x}{}",
                    self.request_count + 1,
                    size_name(*size),
                    addr,
                    data,
                    suffix,
                    width = (bytes_for_size(*size) * 2) as usize
                )
            }
        };
        log::info!("{}", msg);
    }
    
    // History navigation helpers
    fn history_prev(&mut self) { ... }
    fn history_next(&mut self) { ... }
    
    // Scroll helpers
    fn scroll_up(&mut self, lines: usize) { ... }
    fn scroll_down(&mut self, lines: usize) { ... }
}

fn size_name(size: u8) -> &'static str {
    match size {
        0 => "byte",
        1 => "halfword",
        _ => "word",
    }
}

fn bytes_for_size(size: u8) -> u8 {
    match size {
        0 => 1,
        1 => 2,
        _ => 4,
    }
}
```

---

### 3.4 Phase 4: Implement UI Rendering

#### Task 4.1: Implement UI Layout in ui.rs

```rust
// ui.rs
use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

pub fn render(frame: &mut Frame, app: &App) {
    // Create main layout: log area + input line
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),     // Log area (takes remaining space)
            Constraint::Length(3), // Input area (fixed height)
        ])
        .split(frame.area());
    
    render_log_area(frame, app, chunks[0]);
    render_input_area(frame, app, chunks[1]);
}

fn render_log_area(frame: &mut Frame, app: &App, area: Rect) {
    // Convert log messages to list items with appropriate styling
    let items: Vec<ListItem> = app.log_messages
        .iter()
        .map(|log_line| {
            let style = match log_line.level {
                log::Level::Error => Style::default().fg(Color::Red),
                log::Level::Warn => Style::default().fg(Color::Yellow),
                log::Level::Info => Style::default().fg(Color::White),
                log::Level::Debug => Style::default().fg(Color::Gray),
                log::Level::Trace => Style::default().fg(Color::DarkGray),
            };
            
            let level_str = format!("[{:5}]", log_line.level);
            let line = Line::from(vec![
                Span::styled(level_str, style.add_modifier(Modifier::BOLD)),
                Span::raw(" "),
                Span::styled(&log_line.message, style),
            ]);
            ListItem::new(line)
        })
        .collect();
    
    let log_list = List::new(items)
        .block(Block::default()
            .title(" FPGA Host Interface ")
            .borders(Borders::ALL));
    
    // Handle scrolling - show last N items that fit in the view
    // If scroll_offset > 0, show older messages
    frame.render_widget(log_list, area);
}

fn render_input_area(frame: &mut Frame, app: &App, area: Rect) {
    // Build prompt based on connection status
    let prompt = if app.is_connected() {
        Span::styled("[CONNECTED] > ", Style::default().fg(Color::Green))
    } else {
        Span::styled("[DISCONNECTED] > ", Style::default().fg(Color::Red))
    };
    
    let input_line = Line::from(vec![
        prompt,
        Span::raw(&app.input_buffer),
        Span::styled("_", Style::default().add_modifier(Modifier::SLOW_BLINK)),
    ]);
    
    let input_paragraph = Paragraph::new(input_line)
        .block(Block::default().borders(Borders::ALL));
    
    frame.render_widget(input_paragraph, area);
}
```

---

### 3.5 Phase 5: Implement Shell Commands

#### Task 5.1: Implement Command Parser and Executor in shell.rs

```rust
// shell.rs
use crate::app::App;
use crate::elf_loader;
use crate::serial::SerialConnection;
use std::path::Path;

#[derive(Debug)]
pub enum ParseError {
    EmptyCommand,
    UnknownCommand(String),
    MissingArgument(String),
    InvalidArgument { name: String, reason: String },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::EmptyCommand => write!(f, "Empty command"),
            ParseError::UnknownCommand(cmd) => write!(f, "Unknown command: {}", cmd),
            ParseError::MissingArgument(arg) => write!(f, "Missing argument: {}", arg),
            ParseError::InvalidArgument { name, reason } => {
                write!(f, "Invalid argument '{}': {}", name, reason)
            }
        }
    }
}

pub struct CommandResult {
    pub message: Option<String>,
}

impl CommandResult {
    pub fn ok(msg: impl Into<String>) -> Self {
        Self { message: Some(msg.into()) }
    }
    
    pub fn silent() -> Self {
        Self { message: None }
    }
}

pub enum ShellCommand {
    Exit,
    Help { command: Option<String> },
    Status,
    Connect { device: String, baud: u32 },
    Disconnect,
    LoadElf { path: String },
}

impl ShellCommand {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let parts: Vec<&str> = input.trim().split_whitespace().collect();
        if parts.is_empty() {
            return Err(ParseError::EmptyCommand);
        }
        
        let command = parts[0].to_lowercase();
        let args = &parts[1..];
        
        match command.as_str() {
            "exit" | "quit" | "q" => Ok(ShellCommand::Exit),
            
            "help" | "?" => {
                let cmd = args.first().map(|s| s.to_string());
                Ok(ShellCommand::Help { command: cmd })
            }
            
            "status" => Ok(ShellCommand::Status),
            
            "connect" => {
                if args.is_empty() {
                    return Err(ParseError::MissingArgument("device".into()));
                }
                let device = args[0].to_string();
                let baud = if args.len() > 1 {
                    args[1].parse().map_err(|_| ParseError::InvalidArgument {
                        name: "baud".into(),
                        reason: "must be a valid number".into(),
                    })?
                } else {
                    115200 // Default baud rate
                };
                Ok(ShellCommand::Connect { device, baud })
            }
            
            "disconnect" => Ok(ShellCommand::Disconnect),
            
            "loadelf" => {
                if args.is_empty() {
                    return Err(ParseError::MissingArgument("path".into()));
                }
                Ok(ShellCommand::LoadElf { path: args[0].to_string() })
            }
            
            _ => Err(ParseError::UnknownCommand(command)),
        }
    }
    
    pub fn execute(self, app: &mut App) -> CommandResult {
        match self {
            ShellCommand::Exit => {
                log::info!("Exiting...");
                app.should_quit = true;
                CommandResult::silent()
            }
            
            ShellCommand::Help { command } => {
                execute_help(command)
            }
            
            ShellCommand::Status => {
                execute_status(app)
            }
            
            ShellCommand::Connect { device, baud } => {
                execute_connect(app, &device, baud)
            }
            
            ShellCommand::Disconnect => {
                execute_disconnect(app)
            }
            
            ShellCommand::LoadElf { path } => {
                execute_loadelf(app, &path)
            }
        }
    }
}

fn execute_help(command: Option<String>) -> CommandResult {
    if let Some(cmd) = command {
        let help_text = match cmd.to_lowercase().as_str() {
            "exit" => "exit - Exit the application (Ctrl+C also works)",
            "help" => "help [command] - Display help information",
            "status" => "status - Show current serial connection status",
            "connect" => "connect <device> [baud] - Connect to serial port\n  \
                          Example: connect /dev/ttyUSB0 115200\n  \
                          Default baud rate is 115200",
            "disconnect" => "disconnect - Close the current serial connection",
            "loadelf" => "loadelf <path> - Load an ELF file into memory\n  \
                          Example: loadelf ./program.elf",
            _ => return CommandResult::ok(format!("Unknown command: {}", cmd)),
        };
        CommandResult::ok(help_text)
    } else {
        let help_text = "\
Available commands:
  exit       - Exit the application (Ctrl+C also works)
  help       - Display this help message
  status     - Show current serial connection status
  connect    - Connect to a serial port
  disconnect - Close the current serial connection
  loadelf    - Load an ELF file into memory

Type 'help <command>' for detailed help on a specific command.
Use Page Up/Down to scroll the log. Press Enter to submit commands.";
        CommandResult::ok(help_text)
    }
}

fn execute_status(app: &App) -> CommandResult {
    if let Some(ref serial) = app.serial {
        CommandResult::ok(format!(
            "Connected to {}\nTotal bus requests: {}",
            serial,
            app.request_count
        ))
    } else {
        CommandResult::ok("Not connected. Use 'connect <device> [baud]' to connect.")
    }
}

fn execute_connect(app: &mut App, device: &str, baud: u32) -> CommandResult {
    if app.serial.is_some() {
        return CommandResult::ok("Already connected. Disconnect first.");
    }
    
    match SerialConnection::connect(device, baud) {
        Ok(serial) => {
            log::info!("Connected to {} at {} baud", device, baud);
            app.serial = Some(serial);
            CommandResult::ok(format!("Connected to {} at {} baud", device, baud))
        }
        Err(e) => {
            CommandResult::ok(format!("Failed to connect: {}", e))
        }
    }
}

fn execute_disconnect(app: &mut App) -> CommandResult {
    if let Some(serial) = app.serial.take() {
        let device = serial.to_string();
        drop(serial);  // Explicitly close
        log::info!("Disconnected from {}", device);
        CommandResult::ok(format!("Disconnected from {}", device))
    } else {
        CommandResult::ok("Not connected.")
    }
}

fn execute_loadelf(app: &mut App, path: &str) -> CommandResult {
    let path = Path::new(path);
    
    if !path.exists() {
        return CommandResult::ok(format!("File not found: {}", path.display()));
    }
    
    // Clear existing memory before loading
    app.memory.clear();
    
    match elf_loader::load_elf(&mut app.memory, path) {
        Ok(entry_point) => {
            log::info!("ELF loaded: {} (entry: 0x{:08x})", path.display(), entry_point);
            CommandResult::ok(format!(
                "Loaded {} successfully\nEntry point: 0x{:08x}",
                path.display(),
                entry_point
            ))
        }
        Err(e) => {
            CommandResult::ok(format!("Failed to load ELF: {}", e))
        }
    }
}
```

---

### 3.6 Phase 6: Integrate tui-logger

#### Task 6.1: Configure tui-logger Integration

The application needs to capture log messages from the `log` crate and display them in the TUI. `tui-logger` provides this capability through its widget.

**Alternative approach** (if tui-logger widget is not desired):

Instead of using the tui-logger widget directly, we can implement a custom log subscriber that feeds into our `App.log_messages` buffer:

```rust
// In main.rs or a new logging.rs module

use log::{Log, Metadata, Record, Level, LevelFilter};
use std::sync::mpsc::{self, Sender, Receiver};

pub struct TuiLogger {
    sender: Sender<(Level, String)>,
}

impl TuiLogger {
    pub fn new() -> (Self, Receiver<(Level, String)>) {
        let (tx, rx) = mpsc::channel();
        (Self { sender: tx }, rx)
    }
}

impl Log for TuiLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Debug
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let _ = self.sender.send((record.level(), record.args().to_string()));
        }
    }

    fn flush(&self) {}
}
```

**In the main event loop:**

```rust
// Drain any pending log messages
while let Ok((level, message)) = log_receiver.try_recv() {
    app.add_log(level, message);
}
```

---

### 3.7 Phase 7: CLI Argument Compatibility

#### Task 7.1: Update Args Structure for Optional Arguments

```rust
// main.rs or cli.rs

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about = "FPGA Host Interface for RISC-V CPU")]
pub struct Args {
    /// Path to the serial device (e.g., /dev/ttyUSB0)
    /// If provided, auto-connect on startup
    #[arg(short, long)]
    pub serial: Option<PathBuf>,

    /// Baud rate for serial communication
    #[arg(short, long, default_value_t = 115200)]
    pub baud: u32,

    /// Path to the RISC-V ELF executable to load
    /// If provided, auto-load on startup
    #[arg(short, long)]
    pub elf: Option<PathBuf>,

    /// Enable verbose logging (debug level)
    #[arg(short, long)]
    pub verbose: bool,
}
```

**Auto-connect/load logic in run_app:**

```rust
fn run_app(mut terminal: DefaultTerminal, args: Args) -> io::Result<()> {
    let mut app = App::new();
    
    log::info!("FPGA Host Interface started");
    log::info!("Type 'help' for available commands");
    
    // Handle CLI-provided ELF file
    if let Some(ref elf_path) = args.elf {
        match elf_loader::load_elf(&mut app.memory, elf_path) {
            Ok(entry) => {
                log::info!("Loaded ELF: {} (entry: 0x{:08x})", elf_path.display(), entry);
            }
            Err(e) => {
                log::error!("Failed to load ELF: {}", e);
            }
        }
    }
    
    // Handle CLI-provided serial connection
    if let Some(ref serial_path) = args.serial {
        let path_str = serial_path.to_string_lossy();
        match SerialConnection::connect(&path_str, args.baud) {
            Ok(serial) => {
                log::info!("Connected to {} at {} baud", path_str, args.baud);
                app.serial = Some(serial);
            }
            Err(e) => {
                log::error!("Failed to connect: {}", e);
            }
        }
    }
    
    // Main event loop...
}
```

---

## 4. Error Handling and Edge Cases

### 4.1 Panic Handling

The panic hook must be set up before `ratatui::init()` to ensure terminal restoration:

```rust
fn main() -> io::Result<()> {
    // Set up panic hook FIRST
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // Restore terminal before printing panic
        let _ = ratatui::restore();
        original_hook(panic_info);
    }));
    
    // Now initialize terminal
    let terminal = ratatui::init();
    // ...
}
```

### 4.2 Graceful Shutdown

Ensure all resources are properly cleaned up on exit:

1. **Ctrl+C**: Captured by crossterm as a key event, sets `app.should_quit = true`
2. **`exit` command**: Sets `app.should_quit = true`
3. **Main loop exit**: After loop ends, `ratatui::restore()` is called
4. **Serial connection**: Dropped automatically when `App` goes out of scope

### 4.3 Serial Connection Errors

Handle serial port errors gracefully without crashing:

```rust
// In the main loop
match app.serial.as_mut().map(|s| s.poll(&mut app.memory)) {
    Some(Ok(Some(event))) => {
        app.log_bus_event(&event);
        app.request_count += 1;
    }
    Some(Ok(None)) => {
        // No data available, continue
    }
    Some(Err(e)) => {
        log::error!("Serial error: {}", e);
        // Optionally disconnect on certain errors
        if e.is_fatal() {
            app.serial = None;
            log::warn!("Disconnected due to error");
        }
    }
    None => {
        // Not connected, continue
    }
}
```

---

## 5. Testing Strategy

### 5.1 Unit Tests

Create unit tests for:
- `ShellCommand::parse()` with various valid and invalid inputs
- `SparseMemory` read/write operations (already tested implicitly)
- `ElfLoader` error handling

### 5.2 Integration Tests

Due to the interactive nature of the TUI, integration testing is limited. Consider:
- Testing the shell command execution logic with a mock App
- Testing serial protocol state machine with mock data

### 5.3 Manual Testing Checklist

- [ ] Application starts and displays TUI
- [ ] Ctrl+C exits cleanly
- [ ] `exit` command exits cleanly
- [ ] `help` displays command list
- [ ] `help <command>` displays specific help
- [ ] `connect` with valid device connects
- [ ] `connect` when already connected shows error
- [ ] `disconnect` when connected disconnects
- [ ] `disconnect` when not connected shows message
- [ ] `status` shows connection info when connected
- [ ] `status` shows not connected message when disconnected
- [ ] `loadelf` with valid ELF loads successfully
- [ ] `loadelf` with invalid path shows error
- [ ] Log messages appear in scrolling log area
- [ ] Prompt changes color based on connection status
- [ ] Page Up/Down scrolls log history
- [ ] Up/Down arrows navigate command history
- [ ] Terminal is properly restored on panic
- [ ] CLI arguments work for auto-connect/load

---

## 6. Implementation Order

Recommended order of implementation to enable incremental testing:

1. **Phase 1**: Update Cargo.toml, create module files
2. **Phase 2**: Extract existing code (memory.rs, elf_loader.rs, serial.rs)
3. **Phase 3**: Implement basic TUI framework (main.rs with ratatui init/restore)
4. **Phase 4**: Implement UI rendering (ui.rs)
5. **Phase 5**: Implement App state management (app.rs)
6. **Phase 6**: Implement shell commands (shell.rs)
7. **Phase 7**: Integrate logging with tui-logger or custom logger
8. **Phase 8**: CLI argument compatibility
9. **Phase 9**: Testing and polish

---

## 7. Dependencies Summary

### New Dependencies to Add

```toml
ratatui = "0.29"
crossterm = "0.28"
tui-logger = "0.14"
```

### Dependencies to Remove

```toml
# Remove these:
env_logger = "0.10"
ctrlc = "3.4"
```

### Dependencies to Keep

```toml
clap = { version = "4.4", features = ["derive"] }
elf = "0.7"
log = "0.4"
serialport = "4.6"
riscv_shared = { path = "../riscv_shared" }
```

---

## 8. Migration Notes

### 8.1 Breaking Changes

- The application no longer exits immediately if serial device is not provided
- Serial connection is now optional and can be established after startup
- ELF loading can now be done interactively

### 8.2 Backward Compatibility

- CLI arguments (`-s`, `-b`, `-e`, `-v`) still work for automated usage
- The application can still be used non-interactively by providing all arguments

---

## 9. Future Enhancements (Out of Scope)

These features are not part of this implementation but could be added later:

- Memory dump/inspect commands
- Breakpoint support
- CPU reset command
- Multiple serial connection profiles
- Configuration file support
- Color themes
- Mouse support for scrolling
- Export log to file

---

**Document Version:** 1.0  
**Created:** 2026-02-03  
**Status:** Ready for Implementation
