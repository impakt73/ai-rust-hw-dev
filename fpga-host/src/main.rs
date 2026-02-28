//! FPGA Host Interface
//!
//! This binary provides a host interface for communicating with a RISC-V CPU
//! via a device runtime. It features an interactive command shell powered by
//! rustyline with history and dynamic device connection management.

mod app;
mod shell;

use app::{create_fifo_device, App};
use clap::{Parser, Subcommand};
use device_runtime::{create_device_runtime, DeviceRuntimeType, SimDeviceRuntimeArgs};
use rustyline::{error::ReadlineError, DefaultEditor};
use std::io;
use std::io::ErrorKind;
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about = "FPGA Host Interface for RISC-V CPU")]
struct Args {
    /// Device runtime configuration (auto-connect on startup)
    #[command(subcommand)]
    runtime: Option<RuntimeArgs>,

    /// Path to the RISC-V ELF executable to load
    /// If provided, auto-load on startup
    #[arg(short, long)]
    elf: Option<PathBuf>,

    /// Enable verbose logging (debug level)
    #[arg(short, long)]
    verbose: bool,
}

const SIM_TRACE_CALLBACK: device_runtime::SimInstructionTraceCallback =
    |trace| log::info!("SIM TRACE: {}", trace);
const HISTORY_FILE_NAME: &str = ".fpga-host-history";

#[derive(Subcommand)]
enum RuntimeArgs {
    /// Connect to an FPGA over a serial link
    Fpga {
        /// Path to the device (e.g., /dev/ttyUSB0)
        #[arg(short = 'd', long)]
        device: PathBuf,

        /// Baud rate for device communication
        #[arg(short, long, default_value_t = 1_000_000)]
        baud: u32,
    },
    /// Use the software simulator
    Sim {
        /// Enable instruction trace callback logging.
        #[arg(long)]
        trace: bool,
        /// Optional VCD output path.
        #[arg(long)]
        vcd: Option<PathBuf>,
        /// Fixed simulator memory latency in cycles.
        #[arg(long, default_value_t = 0)]
        memory_latency_cycles: u32,
    },
}

fn main() -> io::Result<()> {
    // Parse CLI arguments
    let args = Args::parse();
    run_app(args)
}

/// Main application loop
fn run_app(args: Args) -> io::Result<()> {
    let mut app = App::new();
    let mut editor = DefaultEditor::new()
        .map_err(|e| io::Error::other(format!("Failed to initialize rustyline editor: {e}")))?;
    let history_path = history_file_path();

    // Apply verbose mode from CLI args
    app.set_verbose(args.verbose);

    // Welcome messages
    app.add_log(log::Level::Info, "FPGA Host Interface started".to_string());
    app.add_log(
        log::Level::Info,
        "Type 'help' for available commands".to_string(),
    );

    // Handle CLI-provided device connection
    if let Some(runtime_args) = args.runtime {
        let runtime_type = match runtime_args {
            RuntimeArgs::Fpga { device, baud } => DeviceRuntimeType::Fpga {
                device: device.to_string_lossy().to_string(),
                baud,
                startup_reset: device_runtime::StartupReset::None,
            },
            RuntimeArgs::Sim {
                trace,
                vcd,
                memory_latency_cycles,
            } => DeviceRuntimeType::Sim {
                args: SimDeviceRuntimeArgs {
                    vcd_path: vcd.map(|path| path.to_string_lossy().to_string()),
                    instruction_trace_callback: if trace {
                        Some(SIM_TRACE_CALLBACK)
                    } else {
                        None
                    },
                    memory_latency_cycles,
                },
            },
        };
        let (fifo_reg, fifo_rx) = create_fifo_device();
        match create_device_runtime(runtime_type, Some(vec![fifo_reg])) {
            Ok(runtime) => {
                let description = runtime.to_string();
                app.add_log(log::Level::Info, format!("Connected to {}", description));
                app.fifo_line_rx = Some(fifo_rx);
                app.device_runtime = Some(runtime);
            }
            Err(e) => {
                app.add_log(log::Level::Error, format!("Failed to connect: {}", e));
            }
        }
    }

    // Handle CLI-provided ELF file (requires an active device connection)
    if let Some(ref elf_path) = args.elf {
        if let Some(ref mut runtime) = app.device_runtime {
            match runtime.load_elf(elf_path) {
                Ok(entry) => {
                    app.last_entry_point = Some(entry);
                    app.add_log(
                        log::Level::Info,
                        format!(
                            "Loaded ELF: {} (entry: 0x{:08x})",
                            elf_path.display(),
                            entry
                        ),
                    );
                }
                Err(e) => {
                    app.add_log(log::Level::Error, format!("Failed to load ELF: {}", e));
                }
            }
        } else {
            app.add_log(
                log::Level::Error,
                "Cannot load ELF: no device connected. Connect first, then load ELF.".to_string(),
            );
        }
    }

    if let Some(path) = history_path.as_ref() {
        match editor.load_history(path) {
            Ok(()) => {}
            Err(ReadlineError::Io(io_error)) if io_error.kind() == ErrorKind::NotFound => {}
            Err(e) => app.add_log(
                log::Level::Warn,
                format!("Failed to load history file {}: {}", path.display(), e),
            ),
        }
    }

    print_logs(&mut app);

    // Main event loop
    loop {
        poll_and_print(&mut app);

        let prompt = if app.is_connected() {
            "[CONNECTED] > "
        } else {
            "[DISCONNECTED] > "
        };

        match editor.readline(prompt) {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                editor
                    .add_history_entry(line)
                    .map_err(|e| io::Error::other(format!("Failed to add history entry: {e}")))?;
                if let Some(path) = history_path.as_ref() {
                    if let Err(e) = editor.save_history(path) {
                        app.add_log(
                            log::Level::Warn,
                            format!("Failed to save history file {}: {}", path.display(), e),
                        );
                    }
                }
                app.execute_command_line(line);
            }
            Err(ReadlineError::Interrupted) => {
                app.add_log(
                    log::Level::Info,
                    "Interrupted (Ctrl+C). Use 'exit' to quit.".to_string(),
                );
            }
            Err(ReadlineError::Eof) => {
                app.add_log(log::Level::Info, "Received EOF. Exiting...".to_string());
                app.should_quit = true;
            }
            Err(e) => {
                return Err(io::Error::other(format!("Unexpected readline error: {e}")));
            }
        }
        poll_and_print(&mut app);

        // Check exit condition
        if app.should_quit {
            break;
        }
    }

    Ok(())
}

fn history_file_path() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|path| path.join(HISTORY_FILE_NAME))
}

fn print_logs(app: &mut App) {
    for line in app.take_logs() {
        println!("[{:5}] {}", line.level, line.message);
    }
}

fn poll_and_print(app: &mut App) {
    app.poll_runtime();
    app.poll_fifo();
    print_logs(app);
}
