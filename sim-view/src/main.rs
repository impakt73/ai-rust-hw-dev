mod audio_stream;
mod simulator_controller;
mod video_window;
mod viewer;

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "sim-view",
    about = "RISC-V CPU Simulator with Real-time Video and Audio Output",
    long_about = "Interactive viewer for running ELF programs on a simulated RISC-V CPU with live video and audio output"
)]
struct Args {
    /// Path to the RISC-V ELF executable to run on startup (optional)
    #[arg(value_name = "ELF_FILE")]
    elf: Option<PathBuf>,

    /// Maximum cycles to run before auto-terminating (0 = unlimited)
    #[arg(short, long, default_value_t = 0)]
    max_cycles: u64,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Print instruction trace (prints every instruction executed)
    #[arg(long)]
    print_inst_trace: bool,

    /// Initial window width (default: 320)
    #[arg(long, default_value_t = 320)]
    width: u32,

    /// Initial window height (default: 240)
    #[arg(long, default_value_t = 240)]
    height: u32,
}

fn main() {
    let args = Args::parse();

    // Initialize logger
    let log_level = if args.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    log::info!("sim-view: RISC-V CPU Simulator Viewer");
    log::info!("Controls:");
    log::info!("  - Drag & Drop ELF file to load (not yet implemented)");
    log::info!("  - Ctrl+R: Reload last ELF file");
    log::info!("  - Space: Pause/Resume simulation");
    log::info!("  - Escape: Exit");

    // Create viewer configuration
    let config = viewer::ViewerConfig {
        initial_width: args.width,
        initial_height: args.height,
        max_cycles: args.max_cycles,
        print_inst_trace: args.print_inst_trace,
    };

    // Create and run viewer
    match viewer::SimViewer::new(config) {
        Ok(mut viewer) => {
            // Load initial ELF if provided
            if let Some(elf_path) = args.elf {
                if let Err(e) = viewer.load_elf(&elf_path) {
                    eprintln!("✗ Failed to load ELF: {}", e);
                    std::process::exit(1);
                }
            }

            // Run main loop
            if let Err(e) = viewer.run() {
                eprintln!("✗ Viewer error: {}", e);
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("✗ Failed to create viewer: {}", e);
            std::process::exit(1);
        }
    }
}
