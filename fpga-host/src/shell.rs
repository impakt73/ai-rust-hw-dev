//! Shell command parsing and execution
//!
//! This module provides the interactive command shell functionality.

use crate::app::{create_fifo_device, App};
use bus_shared::{AccessSize, BusRequest};
use clap::{error::ErrorKind as ClapErrorKind, Parser, Subcommand, ValueEnum};
use device_runtime::{
    access_size_name, create_device_runtime, DeviceRuntimeType, ResetKind, SimDeviceRuntimeArgs,
};
use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

/// Default baud rate for device connections
const DEFAULT_BAUD_RATE: u32 = 1_000_000;
const MEMTEST_WORD_BYTES: u32 = 4;
const MEMTEST_MISMATCH_PREVIEW_LIMIT: usize = 8;

const SIM_TRACE_CALLBACK: device_runtime::SimInstructionTraceCallback =
    |trace| log::info!("SIM TRACE: {}", trace);

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
    /// Load a raw file into memory at the specified address
    #[command(name = "loadmem")]
    LoadMem {
        /// Memory address to start writing to (hex with 0x prefix or decimal)
        #[arg(value_parser = parse_hex_or_decimal)]
        address: u32,
        /// Path to the file to load
        path: String,
    },
    /// Dump a memory region to a file on disk
    #[command(name = "dumpmem")]
    DumpMem {
        /// Memory address to start reading from (hex with 0x prefix or decimal)
        #[arg(value_parser = parse_hex_or_decimal)]
        address: u32,
        /// Number of bytes to dump (hex with 0x prefix or decimal)
        #[arg(value_parser = parse_hex_or_decimal)]
        size: u32,
        /// Path to the output file
        path: String,
    },
    /// Write then verify an address-based memory test pattern
    #[command(name = "memtest")]
    MemTest {
        /// Memory address to start testing from (hex with 0x prefix or decimal)
        #[arg(value_parser = parse_hex_or_decimal)]
        address: u32,
        /// Number of bytes to test (must be a non-zero multiple of 4)
        #[arg(value_parser = parse_hex_or_decimal)]
        size: u32,
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
        /// Baud rate (default: 1000000)
        #[arg(default_value_t = DEFAULT_BAUD_RATE, value_parser = clap::value_parser!(u32).range(1..))]
        baud: u32,
    },
    /// Connect to the software simulator
    Sim {
        /// Enable instruction trace callback logging.
        #[arg(long)]
        trace: bool,
        /// Optional VCD output path.
        #[arg(long)]
        vcd: Option<String>,
        /// Fixed simulator memory latency in cycles.
        #[arg(long, default_value_t = 0)]
        memory_latency_cycles: u32,
    },
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
                ConnectRuntime::Sim {
                    trace,
                    vcd,
                    memory_latency_cycles,
                } => execute_connect_sim(app, trace, vcd, memory_latency_cycles),
            },

            ShellCommand::Disconnect => execute_disconnect(app),

            ShellCommand::LoadElf { path } => execute_loadelf(app, &path),

            ShellCommand::LoadMem { address, path } => execute_loadmem(app, address, &path),

            ShellCommand::DumpMem {
                address,
                size,
                path,
            } => execute_dumpmem(app, address, size, &path),

            ShellCommand::MemTest { address, size } => execute_memtest(app, address, size),

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
fn execute_connect_sim(
    app: &mut App,
    trace: bool,
    vcd: Option<String>,
    memory_latency_cycles: u32,
) -> CommandResult {
    if app.device_runtime.is_some() {
        return CommandResult::error("Already connected. Disconnect first.");
    }

    let (fifo_reg, fifo_rx) = create_fifo_device();
    match create_device_runtime(
        DeviceRuntimeType::Sim {
            args: SimDeviceRuntimeArgs {
                vcd_path: vcd,
                instruction_trace_callback: if trace {
                    Some(SIM_TRACE_CALLBACK)
                } else {
                    None
                },
                memory_latency_cycles,
            },
        },
        Some(vec![fifo_reg]),
    ) {
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

/// Execute the loadmem command
fn execute_loadmem(app: &mut App, address: u32, path: &str) -> CommandResult {
    let runtime = match app.device_runtime.as_mut() {
        Some(r) => r,
        None => return CommandResult::error("Not connected. Use 'connect' first."),
    };

    if runtime.has_pending_host_request() {
        return CommandResult::error("A host request is already pending. Wait for response.");
    }

    let path = Path::new(path);

    if !path.exists() {
        return CommandResult::error(format!("File not found: {}", path.display()));
    }

    let data = match fs::read(path) {
        Ok(data) => data,
        Err(e) => {
            return CommandResult::error(format!("Failed to read file {}: {}", path.display(), e));
        }
    };

    match runtime.write_memory_region(address, &data, None) {
        Ok(()) => CommandResult::ok(format!(
            "Loaded {} bytes from {} to 0x{:08x}",
            data.len(),
            path.display(),
            address
        )),
        Err(e) => CommandResult::error(format!("Failed to load memory: {}", e)),
    }
}

/// Execute the dumpmem command
fn execute_dumpmem(app: &mut App, address: u32, size: u32, path: &str) -> CommandResult {
    let runtime = match app.device_runtime.as_mut() {
        Some(r) => r,
        None => return CommandResult::error("Not connected. Use 'connect' first."),
    };

    if runtime.has_pending_host_request() {
        return CommandResult::error("A host request is already pending. Wait for response.");
    }

    let data = match runtime.read_memory_region(address, size, None) {
        Ok(data) => data,
        Err(e) => return CommandResult::error(format!("Failed to dump memory: {}", e)),
    };

    let path = Path::new(path);
    match fs::write(path, &data) {
        Ok(()) => CommandResult::ok(format!(
            "Dumped {} bytes from 0x{:08x} to {}",
            data.len(),
            address,
            path.display()
        )),
        Err(e) => CommandResult::error(format!("Failed to write file {}: {}", path.display(), e)),
    }
}

fn validate_memtest_range(address: u32, size: u32) -> Option<String> {
    if size == 0 {
        return Some("Memtest size must be greater than zero.".to_string());
    }

    if let Some(err) = check_alignment(address, SizeArg::Word) {
        return Some(err);
    }

    if !size.is_multiple_of(MEMTEST_WORD_BYTES) {
        return Some(format!(
            "Memtest size 0x{size:x} must be a multiple of {MEMTEST_WORD_BYTES} bytes."
        ));
    }

    let last_offset = size.checked_sub(1)?;
    if address.checked_add(last_offset).is_none() {
        return Some(format!(
            "Memtest range 0x{address:08x}..0x{address:08x}+0x{size:x} overflows 32-bit address space."
        ));
    }

    None
}

fn generate_memtest_offset() -> u32 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let folded_secs = (secs ^ (secs >> 32)) as u32;
    let mixed = folded_secs.rotate_left(13) ^ now.subsec_nanos().rotate_left(7);
    if mixed == 0 {
        0xA5A5_5A5A
    } else {
        mixed
    }
}

fn generate_memtest_pattern(start_addr: u32, size: u32, offset: u32) -> Result<Vec<u8>, String> {
    let byte_len = usize::try_from(size)
        .map_err(|_| format!("Memtest size 0x{size:x} does not fit in usize."))?;
    let word_count = size / MEMTEST_WORD_BYTES;
    let mut pattern = Vec::with_capacity(byte_len);

    for word_index in 0..word_count {
        let byte_offset = word_index
            .checked_mul(MEMTEST_WORD_BYTES)
            .ok_or_else(|| format!("Memtest offset overflow for size 0x{size:x}."))?;
        let addr = start_addr.checked_add(byte_offset).ok_or_else(|| {
            format!(
                "Memtest range overflow at word index {} for start address 0x{start_addr:08x}.",
                word_index
            )
        })?;
        pattern.extend_from_slice(&addr.wrapping_add(offset).to_le_bytes());
    }

    Ok(pattern)
}

fn decode_memtest_word(chunk: &[u8]) -> u32 {
    let mut word = [0u8; MEMTEST_WORD_BYTES as usize];
    word.copy_from_slice(chunk);
    u32::from_le_bytes(word)
}

fn validate_memtest_data(start_addr: u32, offset: u32, actual: &[u8]) -> Result<(), String> {
    if !actual.len().is_multiple_of(MEMTEST_WORD_BYTES as usize) {
        return Err(format!(
            "Memtest read-back size {} is not a multiple of {} bytes.",
            actual.len(),
            MEMTEST_WORD_BYTES
        ));
    }

    let mut mismatch_count = 0usize;
    let mut mismatch_details = Vec::new();

    for (index, chunk) in actual.chunks_exact(MEMTEST_WORD_BYTES as usize).enumerate() {
        let word_index = u32::try_from(index)
            .map_err(|_| format!("Memtest index {} does not fit in u32.", index))?;
        let byte_offset = word_index
            .checked_mul(MEMTEST_WORD_BYTES)
            .ok_or_else(|| format!("Memtest byte offset overflow at word index {}.", index))?;
        let addr = start_addr.checked_add(byte_offset).ok_or_else(|| {
            format!(
                "Memtest address overflow while validating word index {} from 0x{start_addr:08x}.",
                index
            )
        })?;
        let expected = addr.wrapping_add(offset);
        let observed = decode_memtest_word(chunk);

        if observed != expected {
            mismatch_count += 1;
            if mismatch_details.len() < MEMTEST_MISMATCH_PREVIEW_LIMIT {
                mismatch_details.push(format!(
                    "0x{addr:08x}: expected 0x{expected:08x}, got 0x{observed:08x}"
                ));
            }
        }
    }

    if mismatch_count == 0 {
        Ok(())
    } else {
        let mut message = format!("Memtest failed with {mismatch_count} mismatched word(s).");
        for detail in mismatch_details {
            message.push_str("\n  ");
            message.push_str(&detail);
        }
        if mismatch_count > MEMTEST_MISMATCH_PREVIEW_LIMIT {
            message.push_str(&format!(
                "\n  ... {} additional mismatch(es) not shown",
                mismatch_count - MEMTEST_MISMATCH_PREVIEW_LIMIT
            ));
        }
        Err(message)
    }
}

/// Execute the memtest command
fn execute_memtest(app: &mut App, address: u32, size: u32) -> CommandResult {
    let runtime = match app.device_runtime.as_mut() {
        Some(r) => r,
        None => return CommandResult::error("Not connected. Use 'connect' first."),
    };

    if runtime.has_pending_host_request() {
        return CommandResult::error("A host request is already pending. Wait for response.");
    }

    if let Some(err) = validate_memtest_range(address, size) {
        return CommandResult::error(err);
    }

    let offset = generate_memtest_offset();
    let pattern = match generate_memtest_pattern(address, size, offset) {
        Ok(pattern) => pattern,
        Err(err) => return CommandResult::error(err),
    };

    if let Err(e) = runtime.write_memory_region(address, &pattern, None) {
        return CommandResult::error(format!("Memtest write pass failed: {}", e));
    }

    let read_back = match runtime.read_memory_region(address, size, None) {
        Ok(data) => data,
        Err(e) => return CommandResult::error(format!("Memtest read pass failed: {}", e)),
    };

    if read_back.len() != pattern.len() {
        return CommandResult::error(format!(
            "Memtest read pass returned {} bytes, expected {} bytes.",
            read_back.len(),
            pattern.len()
        ));
    }

    if let Err(err) = validate_memtest_data(address, offset, &read_back) {
        return CommandResult::error(err);
    }

    let end_addr = address + size - 1;
    CommandResult::ok(format!(
        "Memtest passed for 0x{address:08x}-0x{end_addr:08x} ({} bytes, offset 0x{offset:08x}).",
        size
    ))
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
                runtime: ConnectRuntime::Fpga { ref device, baud: 1_000_000 }
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
                runtime: ConnectRuntime::Sim {
                    trace: false,
                    vcd: None,
                    memory_latency_cycles: 0
                }
            }))
        ));
    }

    #[test]
    fn test_parse_connect_sim_with_trace_and_vcd() {
        let result = ShellCommand::parse("connect sim --trace --vcd sim.vcd");
        assert!(matches!(
            result,
            Ok(ParseResult::Command(ShellCommand::Connect {
                runtime: ConnectRuntime::Sim {
                    trace: true,
                    vcd: Some(ref path),
                    memory_latency_cycles: 0
                }
            })) if path == "sim.vcd"
        ));
    }

    #[test]
    fn test_parse_connect_sim_with_memory_latency() {
        let result = ShellCommand::parse("connect sim --memory-latency-cycles 5");
        assert!(matches!(
            result,
            Ok(ParseResult::Command(ShellCommand::Connect {
                runtime: ConnectRuntime::Sim {
                    trace: false,
                    vcd: None,
                    memory_latency_cycles: 5
                }
            }))
        ));
    }

    #[test]
    fn test_parse_connect_invalid_baud() {
        assert!(ShellCommand::parse("connect fpga /dev/ttyUSB0 abc").is_err());
    }

    #[test]
    fn test_parse_connect_zero_baud() {
        assert!(ShellCommand::parse("connect fpga /dev/ttyUSB0 0").is_err());
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
    fn test_parse_loadmem() {
        let result = ShellCommand::parse("loadmem 0x80000000 test.bin");
        assert!(matches!(
            result,
            Ok(ParseResult::Command(ShellCommand::LoadMem {
                address: 0x80000000,
                ref path
            })) if path == "test.bin"
        ));
    }

    #[test]
    fn test_parse_loadmem_with_quoted_path() {
        let result = ShellCommand::parse("loadmem 256 \"test data.bin\"");
        assert!(matches!(
            result,
            Ok(ParseResult::Command(ShellCommand::LoadMem {
                address: 256,
                ref path
            })) if path == "test data.bin"
        ));
    }

    #[test]
    fn test_parse_loadmem_missing_args() {
        assert!(ShellCommand::parse("loadmem").is_err());
        assert!(ShellCommand::parse("loadmem 0x80000000").is_err());
    }

    #[test]
    fn test_parse_dumpmem() {
        let result = ShellCommand::parse("dumpmem 0x80000000 0x100 out.bin");
        assert!(matches!(
            result,
            Ok(ParseResult::Command(ShellCommand::DumpMem {
                address: 0x80000000,
                size: 0x100,
                ref path
            })) if path == "out.bin"
        ));
    }

    #[test]
    fn test_parse_dumpmem_decimal_size() {
        let result = ShellCommand::parse("dumpmem 2147483648 256 dump.bin");
        assert!(matches!(
            result,
            Ok(ParseResult::Command(ShellCommand::DumpMem {
                address: 2147483648,
                size: 256,
                ref path
            })) if path == "dump.bin"
        ));
    }

    #[test]
    fn test_parse_dumpmem_missing_args() {
        assert!(ShellCommand::parse("dumpmem").is_err());
        assert!(ShellCommand::parse("dumpmem 0x80000000").is_err());
        assert!(ShellCommand::parse("dumpmem 0x80000000 0x100").is_err());
    }

    #[test]
    fn test_parse_memtest() {
        let result = ShellCommand::parse("memtest 0x80000000 0x40");
        assert!(matches!(
            result,
            Ok(ParseResult::Command(ShellCommand::MemTest {
                address: 0x80000000,
                size: 0x40
            }))
        ));
    }

    #[test]
    fn test_parse_memtest_missing_args() {
        assert!(ShellCommand::parse("memtest").is_err());
        assert!(ShellCommand::parse("memtest 0x80000000").is_err());
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
        let result = ShellCommand::parse("read 0x20000010");
        assert!(matches!(
            result,
            Ok(ParseResult::Command(ShellCommand::Read {
                address: 0x20000010,
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
        let result = ShellCommand::parse("read 0x20000010 byte");
        assert!(matches!(
            result,
            Ok(ParseResult::Command(ShellCommand::Read {
                address: 0x20000010,
                size: SizeArg::Byte
            }))
        ));

        let result = ShellCommand::parse("read 0x20000010 halfword");
        assert!(matches!(
            result,
            Ok(ParseResult::Command(ShellCommand::Read {
                address: 0x20000010,
                size: SizeArg::Halfword
            }))
        ));

        let result = ShellCommand::parse("read 0x20000010 half");
        assert!(matches!(
            result,
            Ok(ParseResult::Command(ShellCommand::Read {
                address: 0x20000010,
                size: SizeArg::Halfword
            }))
        ));
    }

    #[test]
    fn test_parse_write_hex() {
        let result = ShellCommand::parse("write 0x20000010 0xDEADBEEF");
        assert!(matches!(
            result,
            Ok(ParseResult::Command(ShellCommand::Write {
                address: 0x20000010,
                data: 0xDEADBEEF,
                size: SizeArg::Word
            }))
        ));
    }

    #[test]
    fn test_parse_write_with_size() {
        let result = ShellCommand::parse("write 0x20000010 0xAB byte");
        assert!(matches!(
            result,
            Ok(ParseResult::Command(ShellCommand::Write {
                address: 0x20000010,
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
        assert!(ShellCommand::parse("write 0x20000010").is_err());
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
    fn test_validate_memtest_range() {
        assert!(validate_memtest_range(0x80000000, 0x40).is_none());
        assert!(validate_memtest_range(0x80000002, 0x40).is_some());
        assert!(validate_memtest_range(0x80000000, 0).is_some());
        assert!(validate_memtest_range(0x80000000, 6).is_some());
        assert!(validate_memtest_range(0xFFFF_FFFC, 8).is_some());
    }

    #[test]
    fn test_generate_memtest_pattern() {
        let pattern = generate_memtest_pattern(0x8000_0000, MEMTEST_WORD_BYTES * 2, 0x10).unwrap();
        assert_eq!(
            pattern,
            [
                0x10, 0x00, 0x00, 0x80, //
                0x14, 0x00, 0x00, 0x80
            ]
        );
    }

    #[test]
    fn test_validate_memtest_data_accepts_expected_pattern() {
        let data = generate_memtest_pattern(0x8000_0000, MEMTEST_WORD_BYTES * 2, 0x20).unwrap();
        assert!(validate_memtest_data(0x8000_0000, 0x20, &data).is_ok());
    }

    #[test]
    fn test_validate_memtest_data_reports_mismatch() {
        let mut data = generate_memtest_pattern(0x8000_0000, MEMTEST_WORD_BYTES * 2, 0x20).unwrap();
        data[4] ^= 0xFF;
        let result = validate_memtest_data(0x8000_0000, 0x20, &data);
        assert!(matches!(result, Err(ref err) if err.contains("0x80000004")));
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
