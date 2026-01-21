// Import modules from lib
use sim_view::{gui_backends, headless_backends, viewer, ThreadedSimViewer, ThreadedViewerConfig};

use clap::Parser;
use gui_backends::{GuiAudioBackend, GuiEventSource, GuiVideoBackend};
use headless_backends::{HeadlessAudioBackend, HeadlessEventSource, HeadlessVideoBackend};
use std::path::PathBuf;

// Type aliases for convenience
type ThreadedGuiViewer = ThreadedSimViewer<GuiVideoBackend, GuiAudioBackend, GuiEventSource>;
type HeadlessSimViewer =
    viewer::SimViewer<HeadlessVideoBackend, HeadlessAudioBackend, HeadlessEventSource>;

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

    /// Maximum cycles to run before auto-terminating (0 = unlimited, headless mode only)
    #[arg(short, long, default_value_t = 0)]
    max_cycles: u64,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Print instruction trace (prints every instruction executed, headless mode only)
    #[arg(long)]
    print_inst_trace: bool,

    /// Initial window width (default: 320)
    #[arg(long, default_value_t = 320)]
    width: u32,

    /// Initial window height (default: 240)
    #[arg(long, default_value_t = 240)]
    height: u32,

    /// Run in headless mode (no GUI, for testing)
    #[arg(long)]
    headless: bool,
}

fn main() {
    let args = Args::parse();

    // Initialize logger
    let log_level = if args.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    log::info!("sim-view: RISC-V CPU Simulator Viewer");

    // Run in headless or GUI mode
    let result = if args.headless {
        log::info!("Running in headless mode (no GUI)");
        run_headless_mode(args)
    } else {
        log::info!("Running in threaded GUI mode");
        log::info!("Controls:");
        log::info!("  - Ctrl+R: Reload last ELF file");
        log::info!("  - Space: Pause/Resume simulation");
        log::info!("  - Escape: Exit");
        run_gui_mode(args)
    };

    // Handle errors
    if let Err(e) = result {
        eprintln!("✗ Error: {}", e);
        std::process::exit(1);
    }
}

fn run_gui_mode(args: Args) -> Result<(), String> {
    // Warn if print_inst_trace is enabled - not supported in threaded mode
    if args.print_inst_trace {
        log::warn!(
            "--print-inst-trace is not supported in GUI mode (threaded execution). \
             Use --headless mode for instruction tracing."
        );
    }

    // Create threaded viewer configuration
    let config = ThreadedViewerConfig {
        initial_width: args.width,
        initial_height: args.height,
        print_inst_trace: args.print_inst_trace,
    };

    // Create GUI backends
    let video = GuiVideoBackend::new(args.width, args.height)?;
    let window_handle = video.get_window_handle();
    let active_handle = video.get_active_handle();
    let audio = GuiAudioBackend::new()?;
    let events = GuiEventSource::new(window_handle, active_handle);

    // Create threaded viewer
    let mut viewer = ThreadedGuiViewer::new(config, video, audio, events)?;

    // Load initial ELF if provided
    if let Some(elf_path) = args.elf {
        viewer.load_elf(&elf_path)?;
    }

    // Run main loop
    viewer.run()
}

fn run_headless_mode(args: Args) -> Result<(), String> {
    // Create viewer configuration (original SimViewer for headless mode)
    let config = viewer::ViewerConfig {
        initial_width: args.width,
        initial_height: args.height,
        max_cycles: args.max_cycles,
        print_inst_trace: args.print_inst_trace,
    };

    // Create headless backends
    let video = HeadlessVideoBackend::new();
    let audio = HeadlessAudioBackend::new();
    let events = HeadlessEventSource::new();

    // Create viewer
    let mut viewer = HeadlessSimViewer::new(config, video, audio, events)?;

    // Load initial ELF if provided
    if let Some(elf_path) = args.elf {
        viewer.load_elf(&elf_path)?;
    } else {
        log::warn!("No ELF file specified for headless mode");
    }

    // Run main loop
    viewer.run()?;

    // Print summary
    let frames = viewer.get_video_frames();
    let frame_count = frames.len();

    println!();
    println!("Headless mode completed:");
    println!("  Frames captured: {}", frame_count);

    Ok(())
}
