//! Shell command parsing and execution
//!
//! This module provides the interactive command shell functionality.

use crate::app::{create_fifo_device, App};
use clap::{error::ErrorKind as ClapErrorKind, Parser, Subcommand, ValueEnum};
use device_runtime::{access_size_name, create_device_runtime, DeviceRuntimeType, ResetKind};
use host_bus_handler::{AccessSize, BusRequest};
use std::path::Path;

/// Default baud rate for device connections
const DEFAULT_BAUD_RATE: u32 = 115200;

/// Access size argument for commands
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SizeArg {
    /// Byte access (1 byte)
    Byte,
    /// Halfword access (2 bytes)
    #[value(alias = "half")]
    Halfword,
    /// Word access (4 bytes)
    Word,
}

impl SizeArg {
    /// Convert to AccessSize
    fn to_access_size(self) -> AccessSize {
        match self {
            SizeArg::Byte => AccessSize::Byte,
            SizeArg::Halfword => AccessSize::Halfword,
            SizeArg::Word => AccessSize::Word,
        }
    }
}

/// Result of parsing a shell command (success cases only)
#[derive(Debug)]
pub enum ParseResult {
    /// Successfully parsed a command
    Command(ShellCommand),
    /// Help text to display (not an error)
    HelpText(String),
}

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
#[derive(Debug, Subcommand)]
pub enum ShellCommand {
    /// Exit the application (Ctrl+C also works)
    #[command(visible_aliases = ["quit", "q"])]
    Exit,
    /// Show current device connection status
    Status,
    /// Connect to a device
    Connect {
        #[command(subcommand)]
        runtime: ConnectRuntime,
    },
    /// Close the current device connection
    Disconnect,
    /// Load an ELF file into memory
    #[command(name = "loadelf")]
    LoadElf {
        /// Path to the ELF file
        path: String,
    },
    /// Read from a memory address on the FPGA
    #[command(name = "read")]
    Read {
        /// Memory address to read from (hex with 0x prefix or decimal)
        #[arg(value_parser = parse_hex_or_decimal)]
        address: u32,
        /// Access size
        #[arg(value_enum, default_value_t = SizeArg::Word)]
        size: SizeArg,
    },
    /// Write to a memory address on the FPGA
    #[command(name = "write")]
    Write {
        /// Memory address to write to (hex with 0x prefix or decimal)
        #[arg(value_parser = parse_hex_or_decimal)]
        address: u32,
        /// Data to write (hex with 0x prefix or decimal)
        #[arg(value_parser = parse_hex_or_decimal)]
        data: u32,
        /// Access size
        #[arg(value_enum, default_value_t = SizeArg::Word)]
        size: SizeArg,
    },
    /// Trigger a reset via the system controller
    Reset {
        /// Perform a hard system-level reset (default is soft CPU-only reset)
        #[arg(long)]
        hard: bool,
    },
    /// Boot the CPU from a specified address or the last loaded ELF entry point
    Boot {
        /// Boot address (hex with 0x prefix or decimal). If not provided, use last ELF entry point
        #[arg(value_parser = parse_hex_or_decimal)]
        address: Option<u32>,
    },
}

/// Supported runtime configuration for connect
#[derive(Debug, Subcommand)]
pub enum ConnectRuntime {
    /// Connect to an FPGA over a serial link
    Fpga {
        /// Device path (e.g., /dev/ttyUSB0 for FPGA serial)
        device: String,
        /// Baud rate (default: 115200)
        #[arg(default_value_t = DEFAULT_BAUD_RATE)]
        baud: u32,
    },
    /// Connect to the software simulator
    Sim,
}

/// Parse a string as either hex (with 0x prefix) or decimal
fn parse_hex_or_decimal(s: &str) -> Result<u32, String> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16).map_err(|e| format!("Invalid hex number: {}", e))
    } else {
        s.parse::<u32>()
            .map_err(|e| format!("Invalid decimal number: {}", e))
    }
}

impl ShellCommand {
    /// Parse a command string into a ShellCommand
    ///
    /// Returns Ok(ParseResult) to distinguish between:
    /// - Successfully parsed commands (ParseResult::Command)
    /// - Help/version text (ParseResult::HelpText) - displayed at Info level
    ///
    /// Returns Err(String) for actual parse errors - displayed at Error level
    pub fn parse(input: &str) -> Result<ParseResult, String> {
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
            Ok(cli) => Ok(ParseResult::Command(cli.command)),
            Err(e) => {
                // Check if this is help/version output (not an actual error)
                match e.kind() {
                    ClapErrorKind::DisplayHelp | ClapErrorKind::DisplayVersion => {
                        Ok(ParseResult::HelpText(e.to_string()))
                    }
                    _ => Err(e.to_string()),
                }
            }
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

            ShellCommand::Connect { runtime } => match runtime {
                ConnectRuntime::Fpga { device, baud } => execute_connect_fpga(app, &device, baud),
                ConnectRuntime::Sim => execute_connect_sim(app),
            },

            ShellCommand::Disconnect => execute_disconnect(app),

            ShellCommand::LoadElf { path } => execute_loadelf(app, &path),

            ShellCommand::Read { address, size } => execute_read(app, address, size),

            ShellCommand::Write {
                address,
                data,
                size,
            } => execute_write(app, address, data, size),

            ShellCommand::Reset { hard } => execute_reset(app, hard),

            ShellCommand::Boot { address } => execute_boot(app, address),
        }
    }
}

/// Execute the status command
fn execute_status(app: &App) -> CommandResult {
    let mut status = String::new();

    if let Some(ref runtime) = app.device_runtime {
        status.push_str(&format!(
            "Connected to {}\nTotal bus requests: {}",
            runtime, app.request_count
        ));

        if runtime.has_pending_host_request() {
            status.push_str("\nPending host request: YES");
        }
    } else {
        status.push_str("Not connected. Use 'connect fpga <device> [baud]' to connect.");
    }

    CommandResult::ok(status)
}

/// Execute the connect fpga command
fn execute_connect_fpga(app: &mut App, device: &str, baud: u32) -> CommandResult {
    if app.device_runtime.is_some() {
        return CommandResult::error("Already connected. Disconnect first.");
    }

    let runtime_type = DeviceRuntimeType::Fpga {
        device: device.to_string(),
        baud,
        startup_reset: device_runtime::StartupReset::None,
    };
    let (fifo_reg, fifo_rx) = create_fifo_device();
    match create_device_runtime(runtime_type, Some(vec![fifo_reg])) {
        Ok(runtime) => {
            app.fifo_line_rx = Some(fifo_rx);
            app.device_runtime = Some(runtime);
            CommandResult::ok(format!("Connected to {} at {} baud", device, baud))
        }
        Err(e) => CommandResult::error(format!("Failed to connect: {}", e)),
    }
}

/// Execute the connect sim command
fn execute_connect_sim(app: &mut App) -> CommandResult {
    if app.device_runtime.is_some() {
        return CommandResult::error("Already connected. Disconnect first.");
    }

    let (fifo_reg, fifo_rx) = create_fifo_device();
    match create_device_runtime(DeviceRuntimeType::Sim, Some(vec![fifo_reg])) {
        Ok(runtime) => {
            app.fifo_line_rx = Some(fifo_rx);
            app.device_runtime = Some(runtime);
            CommandResult::ok("Connected to Simulator".to_string())
        }
        Err(e) => CommandResult::error(format!("Failed to create simulator: {}", e)),
    }
}

/// Execute the disconnect command
fn execute_disconnect(app: &mut App) -> CommandResult {
    if let Some(runtime) = app.device_runtime.take() {
        let device = runtime.to_string();
        drop(runtime); // Explicitly close
        app.fifo_line_rx = None;
        CommandResult::ok(format!("Disconnected from {}", device))
    } else {
        CommandResult::ok("Not connected.")
    }
}

/// Execute the loadelf command
fn execute_loadelf(app: &mut App, path: &str) -> CommandResult {
    let runtime = match app.device_runtime.as_mut() {
        Some(r) => r,
        None => return CommandResult::error("Not connected. Use 'connect' first."),
    };

    let path = Path::new(path);

    if !path.exists() {
        return CommandResult::error(format!("File not found: {}", path.display()));
    }

    match runtime.load_elf(path) {
        Ok(entry_point) => {
            app.last_entry_point = Some(entry_point);
            CommandResult::ok(format!(
                "Loaded {} successfully\nEntry point: 0x{:08x}",
                path.display(),
                entry_point
            ))
        }
        Err(e) => CommandResult::error(format!("Failed to load ELF: {}", e)),
    }
}

/// Check address alignment for a given access size
/// Returns an error message if misaligned, None if aligned
fn check_alignment(address: u32, size: SizeArg) -> Option<String> {
    match size {
        SizeArg::Byte => None, // Byte access has no alignment requirements
        SizeArg::Halfword => {
            if address & 1 != 0 {
                Some(format!(
                    "Address 0x{:08x} is not halfword-aligned (must be 2-byte aligned)",
                    address
                ))
            } else {
                None
            }
        }
        SizeArg::Word => {
            if address & 3 != 0 {
                Some(format!(
                    "Address 0x{:08x} is not word-aligned (must be 4-byte aligned)",
                    address
                ))
            } else {
                None
            }
        }
    }
}

/// Check if data value fits within the specified access size
/// Returns an error message if too large, None if valid
fn check_data_size(data: u32, size: SizeArg) -> Option<String> {
    match size {
        SizeArg::Byte => {
            if data > 0xFF {
                Some(format!(
                    "Data value 0x{:x} exceeds byte size (max 0xFF)",
                    data
                ))
            } else {
                None
            }
        }
        SizeArg::Halfword => {
            if data > 0xFFFF {
                Some(format!(
                    "Data value 0x{:x} exceeds halfword size (max 0xFFFF)",
                    data
                ))
            } else {
                None
            }
        }
        SizeArg::Word => None, // u32 always fits in a word
    }
}

/// Execute the read command
fn execute_read(app: &mut App, address: u32, size: SizeArg) -> CommandResult {
    let runtime = match app.device_runtime.as_mut() {
        Some(r) => r,
        None => return CommandResult::error("Not connected. Use 'connect' first."),
    };

    if runtime.has_pending_host_request() {
        return CommandResult::error("A host request is already pending. Wait for response.");
    }

    // Check address alignment
    if let Some(err) = check_alignment(address, size) {
        return CommandResult::error(err);
    }

    let access_size = size.to_access_size();
    let request = BusRequest::read(address, access_size);

    match runtime.send_host_request(request) {
        Ok(()) => CommandResult::ok(format!(
            "Sent read request for 0x{:08x} ({})",
            address,
            access_size_name(access_size)
        )),
        Err(e) => CommandResult::error(format!("Failed to send read request: {}", e)),
    }
}

/// Execute the write command
fn execute_write(app: &mut App, address: u32, data: u32, size: SizeArg) -> CommandResult {
    let runtime = match app.device_runtime.as_mut() {
        Some(r) => r,
        None => return CommandResult::error("Not connected. Use 'connect' first."),
    };

    if runtime.has_pending_host_request() {
        return CommandResult::error("A host request is already pending. Wait for response.");
    }

    // Check address alignment
    if let Some(err) = check_alignment(address, size) {
        return CommandResult::error(err);
    }

    // Check data size
    if let Some(err) = check_data_size(data, size) {
        return CommandResult::error(err);
    }

    let access_size = size.to_access_size();
    let request = BusRequest::write(address, data, access_size);

    match runtime.send_host_request(request) {
        Ok(()) => {
            let width = access_size.byte_count() as usize * 2;
            CommandResult::ok(format!(
                "Sent write request for 0x{:08x} <= 0x{:0width$x} ({})",
                address,
                data,
                access_size_name(access_size),
                width = width
            ))
        }
        Err(e) => CommandResult::error(format!("Failed to send write request: {}", e)),
    }
}

/// Execute the reset command
fn execute_reset(app: &mut App, hard: bool) -> CommandResult {
    let runtime = match app.device_runtime.as_mut() {
        Some(r) => r,
        None => return CommandResult::error("Not connected. Use 'connect' first."),
    };

    if runtime.has_pending_host_request() {
        return CommandResult::error("A host request is already pending. Wait for response.");
    }

    let kind = if hard {
        ResetKind::System
    } else {
        ResetKind::Cpu
    };
    match runtime.reset(kind) {
        Ok(()) => CommandResult::ok(if hard {
            "System reset completed.".to_string()
        } else {
            "CPU reset completed.".to_string()
        }),
        Err(e) => CommandResult::error(format!("Failed to reset device: {}", e)),
    }
}

/// Execute the boot command
fn execute_boot(app: &mut App, address: Option<u32>) -> CommandResult {
    let runtime = match app.device_runtime.as_mut() {
        Some(r) => r,
        None => return CommandResult::error("Not connected. Use 'connect' first."),
    };

    if runtime.has_pending_host_request() {
        return CommandResult::error("A host request is already pending. Wait for response.");
    }

    // Determine boot address
    let boot_addr = if let Some(addr) = address {
        addr
    } else if let Some(entry) = app.last_entry_point {
        entry
    } else {
        return CommandResult::error(
            "No boot address provided and no ELF file loaded.\nUse 'boot <address>' or load an ELF file first with 'loadelf <file>'."
        );
    };

    match runtime.boot_cpu(boot_addr) {
        Ok(()) => CommandResult::ok(format!("CPU booted at address 0x{:08x}", boot_addr)),
        Err(e) => CommandResult::error(format!("Boot failed: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_exit() {
        assert!(matches!(
            ShellCommand::parse("exit"),
            Ok(ParseResult::Command(ShellCommand::Exit))
        ));
        assert!(matches!(
            ShellCommand::parse("quit"),
            Ok(ParseResult::Command(ShellCommand::Exit))
        ));
        assert!(matches!(
            ShellCommand::parse("q"),
            Ok(ParseResult::Command(ShellCommand::Exit))
        ));
    }

    #[test]
    fn test_parse_status() {
        assert!(matches!(
            ShellCommand::parse("status"),
            Ok(ParseResult::Command(ShellCommand::Status))
        ));
    }

    #[test]
    fn test_parse_connect() {
        let result = ShellCommand::parse("connect fpga /dev/ttyUSB0");
        assert!(matches!(
            result,
            Ok(ParseResult::Command(ShellCommand::Connect {
                runtime: ConnectRuntime::Fpga { ref device, baud: 115200 }
            })) if device == "/dev/ttyUSB0"
        ));

        let result = ShellCommand::parse("connect fpga /dev/ttyUSB0 9600");
        assert!(matches!(
            result,
            Ok(ParseResult::Command(ShellCommand::Connect {
                runtime: ConnectRuntime::Fpga { ref device, baud: 9600 }
            })) if device == "/dev/ttyUSB0"
        ));
    }

    #[test]
    fn test_parse_connect_missing_device() {
        assert!(ShellCommand::parse("connect").is_err());
        assert!(ShellCommand::parse("connect fpga").is_err());
    }

    #[test]
    fn test_parse_connect_sim() {
        let result = ShellCommand::parse("connect sim");
        assert!(matches!(
            result,
            Ok(ParseResult::Command(ShellCommand::Connect {
                runtime: ConnectRuntime::Sim
            }))
        ));
    }

    #[test]
    fn test_parse_connect_invalid_baud() {
        assert!(ShellCommand::parse("connect fpga /dev/ttyUSB0 abc").is_err());
    }

    #[test]
    fn test_parse_disconnect() {
        assert!(matches!(
            ShellCommand::parse("disconnect"),
            Ok(ParseResult::Command(ShellCommand::Disconnect))
        ));
    }

    #[test]
    fn test_parse_loadelf() {
        let result = ShellCommand::parse("loadelf test.elf");
        assert!(matches!(
            result,
            Ok(ParseResult::Command(ShellCommand::LoadElf { ref path })) if path == "test.elf"
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

    #[test]
    fn test_parse_read_hex() {
        let result = ShellCommand::parse("read 0x50000000");
        assert!(matches!(
            result,
            Ok(ParseResult::Command(ShellCommand::Read {
                address: 0x50000000,
                size: SizeArg::Word
            }))
        ));
    }

    #[test]
    fn test_parse_read_decimal() {
        let result = ShellCommand::parse("read 256");
        assert!(matches!(
            result,
            Ok(ParseResult::Command(ShellCommand::Read {
                address: 256,
                size: SizeArg::Word
            }))
        ));
    }

    #[test]
    fn test_parse_read_with_size() {
        let result = ShellCommand::parse("read 0x50000000 byte");
        assert!(matches!(
            result,
            Ok(ParseResult::Command(ShellCommand::Read {
                address: 0x50000000,
                size: SizeArg::Byte
            }))
        ));

        let result = ShellCommand::parse("read 0x50000000 halfword");
        assert!(matches!(
            result,
            Ok(ParseResult::Command(ShellCommand::Read {
                address: 0x50000000,
                size: SizeArg::Halfword
            }))
        ));

        let result = ShellCommand::parse("read 0x50000000 half");
        assert!(matches!(
            result,
            Ok(ParseResult::Command(ShellCommand::Read {
                address: 0x50000000,
                size: SizeArg::Halfword
            }))
        ));
    }

    #[test]
    fn test_parse_write_hex() {
        let result = ShellCommand::parse("write 0x50000000 0xDEADBEEF");
        assert!(matches!(
            result,
            Ok(ParseResult::Command(ShellCommand::Write {
                address: 0x50000000,
                data: 0xDEADBEEF,
                size: SizeArg::Word
            }))
        ));
    }

    #[test]
    fn test_parse_write_with_size() {
        let result = ShellCommand::parse("write 0x50000000 0xAB byte");
        assert!(matches!(
            result,
            Ok(ParseResult::Command(ShellCommand::Write {
                address: 0x50000000,
                data: 0xAB,
                size: SizeArg::Byte
            }))
        ));
    }

    #[test]
    fn test_parse_read_missing_address() {
        assert!(ShellCommand::parse("read").is_err());
    }

    #[test]
    fn test_parse_write_missing_args() {
        assert!(ShellCommand::parse("write").is_err());
        assert!(ShellCommand::parse("write 0x50000000").is_err());
    }

    #[test]
    fn test_parse_hex_or_decimal() {
        assert_eq!(parse_hex_or_decimal("0x100"), Ok(256));
        assert_eq!(parse_hex_or_decimal("0X100"), Ok(256));
        assert_eq!(parse_hex_or_decimal("256"), Ok(256));
        assert_eq!(parse_hex_or_decimal("0xDEADBEEF"), Ok(0xDEADBEEF));
        assert!(parse_hex_or_decimal("0xGGGG").is_err());
        assert!(parse_hex_or_decimal("abc").is_err());
    }

    #[test]
    fn test_check_alignment_byte() {
        // Byte access has no alignment requirements
        assert!(check_alignment(0x00000000, SizeArg::Byte).is_none());
        assert!(check_alignment(0x00000001, SizeArg::Byte).is_none());
        assert!(check_alignment(0x00000003, SizeArg::Byte).is_none());
    }

    #[test]
    fn test_check_alignment_halfword() {
        // Halfword requires 2-byte alignment
        assert!(check_alignment(0x00000000, SizeArg::Halfword).is_none());
        assert!(check_alignment(0x00000002, SizeArg::Halfword).is_none());
        assert!(check_alignment(0x00000001, SizeArg::Halfword).is_some());
        assert!(check_alignment(0x00000003, SizeArg::Halfword).is_some());
    }

    #[test]
    fn test_check_alignment_word() {
        // Word requires 4-byte alignment
        assert!(check_alignment(0x00000000, SizeArg::Word).is_none());
        assert!(check_alignment(0x00000004, SizeArg::Word).is_none());
        assert!(check_alignment(0x00000001, SizeArg::Word).is_some());
        assert!(check_alignment(0x00000002, SizeArg::Word).is_some());
        assert!(check_alignment(0x00000003, SizeArg::Word).is_some());
    }

    #[test]
    fn test_check_data_size_byte() {
        assert!(check_data_size(0x00, SizeArg::Byte).is_none());
        assert!(check_data_size(0xFF, SizeArg::Byte).is_none());
        assert!(check_data_size(0x100, SizeArg::Byte).is_some());
        assert!(check_data_size(0xDEADBEEF, SizeArg::Byte).is_some());
    }

    #[test]
    fn test_check_data_size_halfword() {
        assert!(check_data_size(0x0000, SizeArg::Halfword).is_none());
        assert!(check_data_size(0xFFFF, SizeArg::Halfword).is_none());
        assert!(check_data_size(0x10000, SizeArg::Halfword).is_some());
        assert!(check_data_size(0xDEADBEEF, SizeArg::Halfword).is_some());
    }

    #[test]
    fn test_check_data_size_word() {
        // Word can hold any u32 value
        assert!(check_data_size(0x00000000, SizeArg::Word).is_none());
        assert!(check_data_size(0xFFFFFFFF, SizeArg::Word).is_none());
        assert!(check_data_size(0xDEADBEEF, SizeArg::Word).is_none());
    }

    #[test]
    fn test_parse_help_returns_help_text() {
        // "help" should return Ok(HelpText), not an error
        let result = ShellCommand::parse("help");
        assert!(
            matches!(result, Ok(ParseResult::HelpText(_))),
            "Expected Ok(HelpText) for 'help' command, got {:?}",
            result
        );
    }

    #[test]
    fn test_parse_help_for_subcommand() {
        // "help connect" should return Ok(HelpText)
        let result = ShellCommand::parse("help connect");
        assert!(
            matches!(result, Ok(ParseResult::HelpText(_))),
            "Expected Ok(HelpText) for 'help connect' command, got {:?}",
            result
        );
    }

    #[test]
    fn test_parse_reset() {
        assert!(matches!(
            ShellCommand::parse("reset"),
            Ok(ParseResult::Command(ShellCommand::Reset { hard: false }))
        ));
        assert!(matches!(
            ShellCommand::parse("reset --hard"),
            Ok(ParseResult::Command(ShellCommand::Reset { hard: true }))
        ));
    }

    #[test]
    fn test_parse_boot_with_address() {
        let result = ShellCommand::parse("boot 0x80000000");
        assert!(matches!(
            result,
            Ok(ParseResult::Command(ShellCommand::Boot {
                address: Some(0x80000000)
            }))
        ));

        let result = ShellCommand::parse("boot 0x100");
        assert!(matches!(
            result,
            Ok(ParseResult::Command(ShellCommand::Boot {
                address: Some(0x100)
            }))
        ));
    }

    #[test]
    fn test_parse_boot_no_address() {
        let result = ShellCommand::parse("boot");
        assert!(matches!(
            result,
            Ok(ParseResult::Command(ShellCommand::Boot { address: None }))
        ));
    }
}
