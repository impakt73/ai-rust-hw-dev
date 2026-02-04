//! Shell command parsing and execution
//!
//! This module provides the interactive command shell functionality.

use crate::app::App;
use crate::elf_loader;
use crate::serial::SerialConnection;
use clap::{Parser, Subcommand};
use std::path::Path;

/// Default baud rate for serial connections
const DEFAULT_BAUD_RATE: u32 = 115200;

/// Result of executing a command
pub struct CommandResult {
    /// Optional message to display
    pub message: Option<String>,
    /// Log level for the message
    pub level: log::Level,
}

impl CommandResult {
    /// Create a successful result with an info-level message
    pub fn ok(msg: impl Into<String>) -> Self {
        Self {
            message: Some(msg.into()),
            level: log::Level::Info,
        }
    }

    /// Create an error result with an error-level message
    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            message: Some(msg.into()),
            level: log::Level::Error,
        }
    }

    /// Create a result with no message
    pub fn silent() -> Self {
        Self {
            message: None,
            level: log::Level::Info,
        }
    }
}

/// Shell CLI for interactive commands
#[derive(Parser)]
#[command(name = "", no_binary_name = true)]
struct ShellCli {
    #[command(subcommand)]
    command: ShellCommand,
}

/// Available shell commands
#[derive(Subcommand)]
pub enum ShellCommand {
    /// Exit the application (Ctrl+C also works)
    #[command(visible_aliases = ["quit", "q"])]
    Exit,
    /// Show current serial connection status
    Status,
    /// Connect to a serial port
    Connect {
        /// Serial device path (e.g., /dev/ttyUSB0)
        device: String,
        /// Baud rate (default: 115200)
        #[arg(default_value_t = DEFAULT_BAUD_RATE)]
        baud: u32,
    },
    /// Close the current serial connection
    Disconnect,
    /// Load an ELF file into memory
    #[command(name = "loadelf")]
    LoadElf {
        /// Path to the ELF file
        path: String,
    },
}

impl ShellCommand {
    /// Parse a command string into a ShellCommand
    pub fn parse(input: &str) -> Result<Self, String> {
        let input = input.trim();
        if input.is_empty() {
            return Err("Empty command".to_string());
        }

        // Split the input into arguments, respecting shell-like parsing
        let args = match shell_words::split(input) {
            Ok(args) => args,
            Err(e) => return Err(format!("Failed to parse command: {}", e)),
        };

        // Parse using clap
        match ShellCli::try_parse_from(args) {
            Ok(cli) => Ok(cli.command),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Execute the command
    pub fn execute(self, app: &mut App) -> CommandResult {
        match self {
            ShellCommand::Exit => {
                app.add_log(log::Level::Info, "Exiting...".to_string());
                app.should_quit = true;
                CommandResult::silent()
            }

            ShellCommand::Status => execute_status(app),

            ShellCommand::Connect { device, baud } => execute_connect(app, &device, baud),

            ShellCommand::Disconnect => execute_disconnect(app),

            ShellCommand::LoadElf { path } => execute_loadelf(app, &path),
        }
    }
}

/// Execute the status command
fn execute_status(app: &App) -> CommandResult {
    if let Some(ref serial) = app.serial {
        CommandResult::ok(format!(
            "Connected to {} at {} baud\nTotal bus requests: {}",
            serial.device_path(),
            serial.baud_rate(),
            app.request_count
        ))
    } else {
        CommandResult::ok("Not connected. Use 'connect <device> [baud]' to connect.")
    }
}

/// Execute the connect command
fn execute_connect(app: &mut App, device: &str, baud: u32) -> CommandResult {
    if app.serial.is_some() {
        return CommandResult::error("Already connected. Disconnect first.");
    }

    match SerialConnection::connect(device, baud) {
        Ok(serial) => {
            app.serial = Some(serial);
            CommandResult::ok(format!("Connected to {} at {} baud", device, baud))
        }
        Err(e) => CommandResult::error(format!("Failed to connect: {}", e)),
    }
}

/// Execute the disconnect command
fn execute_disconnect(app: &mut App) -> CommandResult {
    if let Some(serial) = app.serial.take() {
        let device = serial.device_path().to_string();
        drop(serial); // Explicitly close
        CommandResult::ok(format!("Disconnected from {}", device))
    } else {
        CommandResult::ok("Not connected.")
    }
}

/// Execute the loadelf command
fn execute_loadelf(app: &mut App, path: &str) -> CommandResult {
    let path = Path::new(path);

    if !path.exists() {
        return CommandResult::error(format!("File not found: {}", path.display()));
    }

    // Load into a temporary memory instance first, so we don't
    // discard the existing memory contents if loading fails.
    let mut new_memory = Default::default();

    match elf_loader::load_elf(&mut new_memory, path) {
        Ok(entry_point) => {
            // Loading succeeded; commit the new memory.
            app.memory = new_memory;
            CommandResult::ok(format!(
                "Loaded {} successfully\nEntry point: 0x{:08x}",
                path.display(),
                entry_point
            ))
        }
        Err(e) => CommandResult::error(format!("Failed to load ELF: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_exit() {
        assert!(matches!(
            ShellCommand::parse("exit"),
            Ok(ShellCommand::Exit)
        ));
        assert!(matches!(
            ShellCommand::parse("quit"),
            Ok(ShellCommand::Exit)
        ));
        assert!(matches!(ShellCommand::parse("q"), Ok(ShellCommand::Exit)));
    }

    #[test]
    fn test_parse_status() {
        assert!(matches!(
            ShellCommand::parse("status"),
            Ok(ShellCommand::Status)
        ));
    }

    #[test]
    fn test_parse_connect() {
        let result = ShellCommand::parse("connect /dev/ttyUSB0");
        assert!(matches!(
            result,
            Ok(ShellCommand::Connect { ref device, baud: 115200 }) if device == "/dev/ttyUSB0"
        ));

        let result = ShellCommand::parse("connect /dev/ttyUSB0 9600");
        assert!(matches!(
            result,
            Ok(ShellCommand::Connect { ref device, baud: 9600 }) if device == "/dev/ttyUSB0"
        ));
    }

    #[test]
    fn test_parse_connect_missing_device() {
        assert!(ShellCommand::parse("connect").is_err());
    }

    #[test]
    fn test_parse_connect_invalid_baud() {
        assert!(ShellCommand::parse("connect /dev/ttyUSB0 abc").is_err());
    }

    #[test]
    fn test_parse_disconnect() {
        assert!(matches!(
            ShellCommand::parse("disconnect"),
            Ok(ShellCommand::Disconnect)
        ));
    }

    #[test]
    fn test_parse_loadelf() {
        let result = ShellCommand::parse("loadelf test.elf");
        assert!(matches!(
            result,
            Ok(ShellCommand::LoadElf { ref path }) if path == "test.elf"
        ));
    }

    #[test]
    fn test_parse_loadelf_missing_path() {
        assert!(ShellCommand::parse("loadelf").is_err());
    }

    #[test]
    fn test_parse_unknown_command() {
        assert!(ShellCommand::parse("unknown").is_err());
    }

    #[test]
    fn test_parse_empty() {
        assert!(ShellCommand::parse("").is_err());
    }
}
