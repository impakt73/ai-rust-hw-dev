//! Application state management
//!
//! This module contains the main application state and event handling logic.

use crate::memory::SparseMemory;
use crate::serial::{access_size_name, bytes_for_size, size_name, BusEvent, SerialConnection};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use host_bus_handler::AccessSize;
use std::collections::VecDeque;

/// Maximum number of log lines to retain
const MAX_LOG_LINES: usize = 1000;

/// Maximum number of command history entries to retain
const MAX_HISTORY_ENTRIES: usize = 500;

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
    /// Serial connection state
    pub serial: Option<SerialConnection>,
    /// Sparse memory model for DRAM
    pub memory: SparseMemory,
    /// Command input buffer
    pub input_buffer: String,
    /// Command history for up/down navigation
    /// VecDeque for efficient pop_front when capping at MAX_HISTORY_ENTRIES
    pub command_history: VecDeque<String>,
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
}

impl App {
    /// Create a new application instance
    pub fn new() -> Self {
        Self {
            serial: None,
            memory: SparseMemory::new(),
            input_buffer: String::new(),
            command_history: VecDeque::with_capacity(MAX_HISTORY_ENTRIES),
            history_index: None,
            log_messages: VecDeque::with_capacity(MAX_LOG_LINES),
            should_quit: false,
            request_count: 0,
            scroll_offset: 0,
            verbose: false,
        }
    }

    /// Set verbose mode
    pub fn set_verbose(&mut self, verbose: bool) {
        self.verbose = verbose;
    }

    /// Check if a serial connection is active
    pub fn is_connected(&self) -> bool {
        self.serial.is_some()
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

        // Add to history (with cap)
        if self.command_history.len() >= MAX_HISTORY_ENTRIES {
            self.command_history.pop_front();
        }
        self.command_history.push_back(input.clone());
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
                let suffix = if *is_dram { "" } else { " (non-DRAM, dropped)" };
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
            BusEvent::HostReadResponse { data, size } => {
                let width = size.byte_count() as usize * 2;
                format!(
                    "HOST READ response: 0x{:0width$x} ({})",
                    data,
                    access_size_name(*size),
                    width = width
                )
            }
            BusEvent::HostWriteResponse { size } => {
                format!("HOST WRITE acknowledged ({})", access_size_name(*size))
            }
        };
        self.add_log(log::Level::Info, msg);
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
            self.input_buffer = self.command_history[idx].clone();
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
                self.input_buffer = self.command_history[idx + 1].clone();
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
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
