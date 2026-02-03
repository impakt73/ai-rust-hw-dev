//! Shell command parsing and execution
//!
//! This module provides the interactive command shell functionality.

use crate::app::App;
use crate::elf_loader;
use crate::serial::SerialConnection;
use std::path::Path;

/// Default baud rate for serial connections
const DEFAULT_BAUD_RATE: u32 = 115200;

/// Errors that can occur during command parsing
#[derive(Debug)]
pub enum ParseError {
    /// Empty command string
    EmptyCommand,
    /// Unknown command
    UnknownCommand(String),
    /// Missing required argument
    MissingArgument(String),
    /// Invalid argument value
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

impl std::error::Error for ParseError {}

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

/// Available shell commands
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
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let parts: Vec<&str> = input.split_whitespace().collect();
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
                    DEFAULT_BAUD_RATE
                };
                Ok(ShellCommand::Connect { device, baud })
            }

            "disconnect" => Ok(ShellCommand::Disconnect),

            "loadelf" => {
                if args.is_empty() {
                    return Err(ParseError::MissingArgument("path".into()));
                }
                Ok(ShellCommand::LoadElf {
                    path: args[0].to_string(),
                })
            }

            _ => Err(ParseError::UnknownCommand(command)),
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

            ShellCommand::Help { command } => execute_help(command),

            ShellCommand::Status => execute_status(app),

            ShellCommand::Connect { device, baud } => execute_connect(app, &device, baud),

            ShellCommand::Disconnect => execute_disconnect(app),

            ShellCommand::LoadElf { path } => execute_loadelf(app, &path),
        }
    }
}

/// Execute the help command
fn execute_help(command: Option<String>) -> CommandResult {
    if let Some(cmd) = command {
        let help_text = match cmd.to_lowercase().as_str() {
            "exit" | "quit" | "q" => "exit - Exit the application (Ctrl+C also works)".to_string(),
            "help" | "?" => "help [command] - Display help information".to_string(),
            "status" => "status - Show current serial connection status".to_string(),
            "connect" => {
                format!(
                    "connect <device> [baud] - Connect to serial port\n  \
                     Example: connect /dev/ttyUSB0 {}\n  \
                     Default baud rate is {}",
                    DEFAULT_BAUD_RATE, DEFAULT_BAUD_RATE
                )
            }
            "disconnect" => "disconnect - Close the current serial connection".to_string(),
            "loadelf" => "loadelf <path> - Load an ELF file into memory\n  \
                 Example: loadelf ./program.elf"
                .to_string(),
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
Use Page Up/Down to scroll the log. Press Escape to reset scroll.";
        CommandResult::ok(help_text)
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
    fn test_parse_help() {
        assert!(matches!(
            ShellCommand::parse("help"),
            Ok(ShellCommand::Help { command: None })
        ));
        let result = ShellCommand::parse("help connect");
        assert!(matches!(
            result,
            Ok(ShellCommand::Help {
                command: Some(ref cmd)
            }) if cmd == "connect"
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
        assert!(matches!(
            ShellCommand::parse("connect"),
            Err(ParseError::MissingArgument(_))
        ));
    }

    #[test]
    fn test_parse_connect_invalid_baud() {
        assert!(matches!(
            ShellCommand::parse("connect /dev/ttyUSB0 abc"),
            Err(ParseError::InvalidArgument { .. })
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
        assert!(matches!(
            ShellCommand::parse("loadelf"),
            Err(ParseError::MissingArgument(_))
        ));
    }

    #[test]
    fn test_parse_unknown_command() {
        assert!(matches!(
            ShellCommand::parse("unknown"),
            Err(ParseError::UnknownCommand(_))
        ));
    }

    #[test]
    fn test_parse_empty() {
        assert!(matches!(
            ShellCommand::parse(""),
            Err(ParseError::EmptyCommand)
        ));
    }
}
