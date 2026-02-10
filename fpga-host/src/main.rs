//! FPGA Host Interface
//!
//! This binary provides a host interface for communicating with a RISC-V CPU
//! running on an FPGA over a serial connection. It features an interactive
//! TUI with a scrolling log view, command shell, and dynamic serial port
//! connection management.

mod app;
mod elf_loader;
mod memory;
mod serial;
mod shell;
mod ui;

use app::App;
use clap::Parser;
use crossterm::event::{self, Event};
use ratatui::DefaultTerminal;
use riscv_shared::bus::{sysctrl_boot_addr, SYSCTRL_STATUS_CPU_BOOTING};
use serial::{BusEvent, SerialConnection};
use std::io;
use std::panic;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[derive(Parser)]
#[command(author, version, about = "FPGA Host Interface for RISC-V CPU")]
struct Args {
    /// Path to the serial device (e.g., /dev/ttyUSB0)
    /// If provided, auto-connect on startup
    #[arg(short, long)]
    serial: Option<PathBuf>,

    /// Baud rate for serial communication
    #[arg(short, long, default_value_t = 115200)]
    baud: u32,

    /// Path to the RISC-V ELF executable to load
    /// If provided, auto-load on startup
    #[arg(short, long)]
    elf: Option<PathBuf>,

    /// Enable verbose logging (debug level)
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> io::Result<()> {
    // Parse CLI arguments
    let args = Args::parse();

    // Set up panic hook to restore terminal on panic
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // Attempt to restore terminal before displaying panic
        ratatui::restore();
        original_hook(panic_info);
    }));

    // Initialize terminal (switches to alternate screen)
    let terminal = ratatui::init();

    // Run the application
    let result = run_app(terminal, args);

    // Restore terminal to normal state
    ratatui::restore();

    result
}

/// Main application loop
fn run_app(mut terminal: DefaultTerminal, args: Args) -> io::Result<()> {
    let mut app = App::new();

    // Apply verbose mode from CLI args
    app.set_verbose(args.verbose);

    // Welcome messages
    app.add_log(log::Level::Info, "FPGA Host Interface started".to_string());
    app.add_log(
        log::Level::Info,
        "Type 'help' for available commands".to_string(),
    );

    // Handle CLI-provided ELF file
    if let Some(ref elf_path) = args.elf {
        let result = elf_loader::load_elf(&mut app.memory.lock().unwrap(), elf_path);
        match result {
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
    }

    // Handle CLI-provided serial connection
    if let Some(ref serial_path) = args.serial {
        let path_str = serial_path.to_string_lossy();
        match SerialConnection::connect(&path_str, args.baud, Arc::clone(&app.memory)) {
            Ok(serial) => {
                app.add_log(
                    log::Level::Info,
                    format!("Connected to {} at {} baud", path_str, args.baud),
                );
                app.serial = Some(serial);
            }
            Err(e) => {
                app.add_log(log::Level::Error, format!("Failed to connect: {}", e));
            }
        }
    }

    // Main event loop
    loop {
        // Draw UI
        terminal.draw(|frame| ui::render(frame, &app))?;

        // Handle input events (with timeout for serial polling)
        if event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = event::read()? {
                app.handle_key_event(key);
            }
        }

        // Poll serial connection if connected
        let mut should_disconnect = false;
        let mut pending_boot_request: Option<(u32, u32)> = None; // (boot_addr, status_val)

        if let Some(ref mut serial) = app.serial {
            // Get pending request info before polling
            let pending_request = serial.pending_host_request();

            match serial.poll() {
                Ok(Some(event)) => {
                    match &event {
                        BusEvent::Read { .. } | BusEvent::Write { .. } => {
                            // CPU-initiated transaction
                            app.log_bus_event(&event);
                            app.request_count += 1;
                        }
                        BusEvent::HostReadResponse { data, size } => {
                            // Host-initiated read response - check for pending boot
                            if let Some(pending_boot) = app.pending_boot.take() {
                                // This is the STATUS register read response for boot command
                                let status_val = *data;
                                let req_addr =
                                    pending_request.as_ref().map(|r| r.addr).unwrap_or(0);

                                // Verify this is the expected STATUS read
                                if req_addr != pending_boot.expected_status_addr {
                                    // Mismatch - log error and don't write BOOT
                                    app.add_log(
                                        log::Level::Error,
                                        format!(
                                            "Boot flow error: Expected STATUS read at 0x{:08x}, got response for 0x{:08x}",
                                            pending_boot.expected_status_addr, req_addr
                                        )
                                    );
                                    // Still log the read response
                                    let width = size.byte_count() as usize * 2;
                                    let msg = format!(
                                        "HOST READ {} @ 0x{:08x} => 0x{:0width$x}",
                                        serial::access_size_name(*size),
                                        req_addr,
                                        status_val,
                                        width = width
                                    );
                                    app.add_log(log::Level::Info, msg);
                                } else {
                                    // Log the STATUS read first
                                    let width = size.byte_count() as usize * 2;
                                    let msg = format!(
                                        "HOST READ {} @ 0x{:08x} => 0x{:0width$x}",
                                        serial::access_size_name(*size),
                                        req_addr,
                                        status_val,
                                        width = width
                                    );
                                    app.add_log(log::Level::Info, msg);

                                    // Verify cpu_booting bit (bit 0) is set
                                    if (status_val & SYSCTRL_STATUS_CPU_BOOTING) == 0 {
                                        app.add_log(
                                            log::Level::Error,
                                            format!("Boot failed: cpu_booting bit not set (STATUS=0x{:08x})", status_val)
                                        );
                                    } else {
                                        // Schedule the BOOT write to happen after we finish processing this event
                                        pending_boot_request =
                                            Some((pending_boot.boot_addr, status_val));
                                    }
                                }
                            } else {
                                // Normal read response - log with request details
                                if let Some(req) = &pending_request {
                                    app.log_host_read_response(req.addr, *data, *size);
                                } else {
                                    app.log_bus_event(&event);
                                }
                            }
                        }
                        BusEvent::HostWriteResponse { size } => {
                            // Host-initiated write response - log with request details
                            if let Some(req) = &pending_request {
                                app.log_host_write_response(req.addr, req.wdata, *size);
                            } else {
                                app.log_bus_event(&event);
                            }
                        }
                        BusEvent::HostRequestTimeout { addr } => {
                            // Host request timed out - clear pending boot and emit warning in TUI
                            app.pending_boot = None;
                            app.add_log(
                                log::Level::Warn,
                                format!(
                                    "Host request timeout (1s) for address 0x{:08x}. Resetting host bus handler.",
                                    addr
                                )
                            );
                        }
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    // Check if this is a fatal error (e.g., device disconnected)
                    if e.is_fatal() {
                        app.add_log(log::Level::Error, format!("Serial connection lost: {}", e));
                        should_disconnect = true;
                    } else {
                        app.add_log(log::Level::Error, format!("Serial error: {}", e));
                    }
                }
            }
        }

        // Handle pending boot request (after serial borrow has ended)
        if let Some((boot_addr, status_val)) = pending_boot_request {
            if let Some(ref mut serial) = app.serial {
                let boot_reg_addr = sysctrl_boot_addr();
                let request = host_bus_handler::BusRequest::write(
                    boot_reg_addr,
                    boot_addr,
                    host_bus_handler::AccessSize::Word,
                );

                match serial.send_host_request(request) {
                    Ok(()) => {
                        app.add_log(
                            log::Level::Info,
                            format!(
                                "STATUS verified (0x{:08x}), sending boot address 0x{:08x} to 0x{:08x}",
                                status_val, boot_addr, boot_reg_addr
                            )
                        );
                    }
                    Err(e) => {
                        app.add_log(
                            log::Level::Error,
                            format!("Failed to send boot write request: {}", e),
                        );
                    }
                }
            }
        }

        // Handle fatal serial errors by disconnecting (outside the borrow)
        if should_disconnect {
            if let Some(serial) = app.serial.take() {
                let device = serial.device_path().to_string();
                drop(serial);
                app.add_log(
                    log::Level::Warn,
                    format!("Disconnected from {} due to serial error", device),
                );
            }
        }

        // Check exit condition
        if app.should_quit {
            break;
        }
    }

    Ok(())
}
