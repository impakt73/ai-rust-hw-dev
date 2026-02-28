//! Application state management
//!
//! This module contains the main application state and event handling logic.

use bus_shared::AccessSize;
use bus_shared::{Fifo, FifoDataSource, SharedFifoDataSource};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use device_runtime::{
    access_size_name, bytes_for_size, size_name, BusDeviceRegistration, BusEvent, DeviceRuntime,
};
use riscv_shared::bus::FIFO_BASE;
use rustyline::history::{DefaultHistory, History, SearchDirection};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

/// Maximum number of FIFO lines drained per event-loop tick to keep the TUI responsive.
const MAX_FIFO_LINES_PER_TICK: usize = 64;

/// Capacity of the bounded FIFO line channel.
/// Lines are dropped (not sent) when the channel is full, preventing unbounded memory growth.
const FIFO_CHANNEL_CAPACITY: usize = 256;

const MAX_LOG_LINES: usize = 1000;

/// Maximum number of command history entries to retain
const MAX_HISTORY_ENTRIES: usize = 500;
/// History file location under user's home directory
const HISTORY_FILENAME: &str = ".fpga-host-history";

/// A log line entry for display
#[derive(Debug, Clone)]
pub struct LogLine {
    /// Log level
    pub level: log::Level,
    /// Log message
    pub message: String,
}

/// Main application state
pub struct App {
    /// Device runtime connection state
    pub device_runtime: Option<Box<dyn DeviceRuntime>>,
    /// Command input buffer
    pub input_buffer: String,
    /// Command history for up/down navigation
    pub command_history: DefaultHistory,
    /// Current position in command history (None = not navigating)
    pub history_index: Option<usize>,
    /// Log message buffer for display (ring buffer)
    pub log_messages: VecDeque<LogLine>,
    /// Whether the application should exit
    pub should_quit: bool,
    /// Bus request statistics
    pub request_count: u64,
    /// Scroll offset for log view (0 = auto-scroll to bottom)
    pub scroll_offset: usize,
    /// Verbose logging mode
    pub verbose: bool,
    /// Last loaded ELF entry point (for boot command)
    pub last_entry_point: Option<u32>,
    /// Receiver for lines printed by the CPU program via the FIFO
    pub fifo_line_rx: Option<mpsc::Receiver<String>>,
}

impl App {
    /// Create a new application instance
    pub fn new() -> Self {
        let mut command_history = DefaultHistory::new();
        let _ = command_history.set_max_len(MAX_HISTORY_ENTRIES);
        if let Some(path) = Self::history_file_path() {
            let _ = command_history.load(&path);
        }

        Self {
            device_runtime: None,
            input_buffer: String::new(),
            command_history,
            history_index: None,
            log_messages: VecDeque::with_capacity(MAX_LOG_LINES),
            should_quit: false,
            request_count: 0,
            scroll_offset: 0,
            verbose: false,
            last_entry_point: None,
            fifo_line_rx: None,
        }
    }

    /// Set verbose mode
    pub fn set_verbose(&mut self, verbose: bool) {
        self.verbose = verbose;
    }

    /// Check if a device runtime connection is active
    pub fn is_connected(&self) -> bool {
        self.device_runtime.is_some()
    }

    /// Handle a keyboard event
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
            // Escape to reset scroll to bottom
            KeyCode::Esc => {
                self.scroll_offset = 0;
            }
            // Regular character input
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
            }
            _ => {}
        }
    }

    /// Submit the current command in the input buffer
    fn submit_command(&mut self) {
        let input = self.input_buffer.trim().to_string();
        if input.is_empty() {
            return;
        }

        // Add to history
        if self.command_history.add(&input).is_ok() {
            self.save_history();
        }
        self.history_index = None;
        self.input_buffer.clear();

        // Parse and execute command
        match crate::shell::ShellCommand::parse(&input) {
            Ok(crate::shell::ParseResult::Command(cmd)) => {
                let result = cmd.execute(self);
                if let Some(msg) = result.message {
                    // Split multi-line messages into separate log entries
                    // Use the level from the CommandResult
                    for line in msg.lines() {
                        self.add_log(result.level, line.to_string());
                    }
                }
            }
            Ok(crate::shell::ParseResult::HelpText(text)) => {
                // Help/version text should be displayed at Info level, not Error
                for line in text.lines() {
                    self.add_log(log::Level::Info, line.to_string());
                }
            }
            Err(e) => {
                // Split error text across multiple lines in case clap provides
                // multi-line error messages
                for line in e.lines() {
                    self.add_log(log::Level::Error, line.to_string());
                }
            }
        }
    }

    /// Add a log message
    pub fn add_log(&mut self, level: log::Level, message: String) {
        // Filter debug/trace logs if not in verbose mode
        if !self.verbose && matches!(level, log::Level::Debug | log::Level::Trace) {
            return;
        }
        if self.log_messages.len() >= MAX_LOG_LINES {
            self.log_messages.pop_front();
        }
        self.log_messages.push_back(LogLine { level, message });
    }

    /// Log a bus event
    pub fn log_bus_event(&mut self, event: &BusEvent) {
        let msg = match event {
            BusEvent::Read {
                addr,
                size,
                data,
                is_dram,
            } => {
                let suffix = if *is_dram { "" } else { " (non-DRAM)" };
                format!(
                    "[{}] READ {} @ 0x{:08x} => 0x{:0width$x}{}",
                    self.request_count + 1,
                    size_name(*size),
                    addr,
                    data,
                    suffix,
                    width = (bytes_for_size(*size) * 2) as usize
                )
            }
            BusEvent::Write {
                addr,
                size,
                data,
                is_dram,
            } => {
                let suffix = if *is_dram { "" } else { " (non-DRAM)" };
                format!(
                    "[{}] WRITE {} @ 0x{:08x} <= 0x{:0width$x}{}",
                    self.request_count + 1,
                    size_name(*size),
                    addr,
                    data,
                    suffix,
                    width = (bytes_for_size(*size) * 2) as usize
                )
            }
            BusEvent::HostReadResponse {
                addr, data, size, ..
            } => {
                let width = size.byte_count() as usize * 2;
                format!(
                    "HOST READ {} @ 0x{:08x} => 0x{:0width$x}",
                    access_size_name(*size),
                    addr,
                    data,
                    width = width
                )
            }
            BusEvent::HostWriteResponse { addr, wdata, size } => {
                let width = size.byte_count() as usize * 2;
                format!(
                    "HOST WRITE {} @ 0x{:08x} <= 0x{:0width$x} (acknowledged)",
                    access_size_name(*size),
                    addr,
                    wdata,
                    width = width
                )
            }
            BusEvent::HostRequestTimeout { addr } => {
                format!("HOST REQUEST TIMEOUT @ 0x{:08x}", addr)
            }
            BusEvent::TohostTermination { value } => {
                format!("TOHOST TERMINATION (value: 0x{:08x})", value)
            }
        };
        let level = match event {
            BusEvent::Read { .. } | BusEvent::Write { .. } => log::Level::Debug,
            _ => log::Level::Info,
        };
        self.add_log(level, msg);
    }

    /// Log a host-initiated read response with the request details
    pub fn log_host_read_response(&mut self, addr: u32, data: u32, size: AccessSize) {
        let width = size.byte_count() as usize * 2;
        let msg = format!(
            "HOST READ {} @ 0x{:08x} => 0x{:0width$x}",
            access_size_name(size),
            addr,
            data,
            width = width
        );
        self.add_log(log::Level::Info, msg);
    }

    /// Log a host-initiated write response with the request details
    pub fn log_host_write_response(&mut self, addr: u32, data: u32, size: AccessSize) {
        let width = size.byte_count() as usize * 2;
        let msg = format!(
            "HOST WRITE {} @ 0x{:08x} <= 0x{:0width$x} (acknowledged)",
            access_size_name(size),
            addr,
            data,
            width = width
        );
        self.add_log(log::Level::Info, msg);
    }

    /// Drain any lines received from the CPU program via the FIFO and log them.
    ///
    /// Lines are buffered in the FIFO callback until a newline is received.
    /// At most [`MAX_FIFO_LINES_PER_TICK`] lines are drained per call to avoid
    /// UI lag when the CPU prints bursts of output.
    /// This should be called each iteration of the main event loop.
    pub fn poll_fifo(&mut self) {
        let mut lines = Vec::new();
        let mut should_clear = false;
        if let Some(ref rx) = self.fifo_line_rx {
            for _ in 0..MAX_FIFO_LINES_PER_TICK {
                match rx.try_recv() {
                    Ok(line) => lines.push(line),
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        should_clear = true;
                        break;
                    }
                }
            }
        }
        for line in lines {
            self.add_log(log::Level::Info, line);
        }
        if should_clear {
            self.fifo_line_rx = None;
        }
    }

    /// Navigate to previous command in history
    fn history_prev(&mut self) {
        if self.command_history.is_empty() {
            return;
        }

        let new_index = match self.history_index {
            None => {
                // Start from the end
                Some(self.command_history.len() - 1)
            }
            Some(idx) if idx > 0 => Some(idx - 1),
            Some(_) => {
                // Already at the beginning
                return;
            }
        };

        if let Some(idx) = new_index {
            self.history_index = Some(idx);
            if let Some(entry) = self.history_entry(idx) {
                self.input_buffer = entry;
            }
        }
    }

    /// Navigate to next command in history
    fn history_next(&mut self) {
        if self.command_history.is_empty() {
            return;
        }

        match self.history_index {
            Some(idx) if idx < self.command_history.len() - 1 => {
                self.history_index = Some(idx + 1);
                if let Some(entry) = self.history_entry(idx + 1) {
                    self.input_buffer = entry;
                }
            }
            Some(_) => {
                // At the end of history, clear input
                self.history_index = None;
                self.input_buffer.clear();
            }
            None => {
                // Not in history mode
            }
        }
    }

    /// Scroll log view up
    fn scroll_up(&mut self, lines: usize) {
        if self.log_messages.is_empty() {
            return;
        }
        let max_offset = self.log_messages.len() - 1;
        self.scroll_offset = (self.scroll_offset + lines).min(max_offset);
    }

    /// Scroll log view down
    fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    fn history_entry(&self, index: usize) -> Option<String> {
        let entry = match self.command_history.get(index, SearchDirection::Forward) {
            Ok(Some(entry)) => entry,
            _ => return None,
        };
        Some(entry.entry.into_owned())
    }

    fn history_file_path() -> Option<PathBuf> {
        if cfg!(test) {
            return None;
        }
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|path| path.join(HISTORY_FILENAME))
    }

    fn save_history(&mut self) {
        if let Some(path) = Self::history_file_path() {
            let _ = self.command_history.save(&path);
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a FIFO bus device and a channel for receiving lines printed by the CPU program.
///
/// The FIFO callback buffers incoming bytes until a newline character is received,
/// then attempts to send the completed line through a bounded sync channel. Lines
/// are silently dropped when the channel is full ([`FIFO_CHANNEL_CAPACITY`]) so
/// that a fast-printing CPU never blocks the bus thread.
///
/// Returns a `(BusDeviceRegistration, Receiver<String>)` pair. The registration
/// should be passed to [`create_device_runtime`][device_runtime::create_device_runtime].
/// The receiver should be stored in [`App::fifo_line_rx`] so the main event loop
/// can drain and display CPU program output.
pub fn create_fifo_device() -> (BusDeviceRegistration, mpsc::Receiver<String>) {
    let (tx, rx) = mpsc::sync_channel::<String>(FIFO_CHANNEL_CAPACITY);
    let data_source: SharedFifoDataSource = Arc::new(Mutex::new(FifoDataSource::new()));
    let mut line_buffer: Vec<u8> = Vec::new();
    let callback: bus_shared::FifoDataReceivedCallback = Box::new(move |byte: u8| {
        if byte == b'\n' {
            let line = String::from_utf8_lossy(&line_buffer).into_owned();
            // try_send: drop the line rather than blocking the bus thread when full
            let _ = tx.try_send(line);
            line_buffer.clear();
        } else if byte != b'\r' {
            // Cap line buffer to avoid unbounded growth if CPU never sends a newline
            if line_buffer.len() < 4096 {
                line_buffer.push(byte);
            }
        }
    });
    let fifo = Fifo::new_with_callback(data_source, callback);
    let registration = BusDeviceRegistration {
        base_addr: FIFO_BASE,
        device: Box::new(fifo),
    };
    (registration, rx)
}

#[cfg(test)]
mod tests {
    use super::App;

    #[test]
    fn test_command_history_navigation() {
        let mut app = App::new();
        app.input_buffer = "status".to_string();
        app.submit_command();
        app.input_buffer = "boot 0x80000000".to_string();
        app.submit_command();

        app.history_prev();
        assert_eq!(app.input_buffer, "boot 0x80000000");

        app.history_prev();
        assert_eq!(app.input_buffer, "status");

        app.history_next();
        assert_eq!(app.input_buffer, "boot 0x80000000");

        app.history_next();
        assert!(app.input_buffer.is_empty());
    }
}
