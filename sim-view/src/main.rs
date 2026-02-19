// Import modules from lib
use sim_view::{gui_backends, headless_backends, viewer};

use clap::Parser;
use device_runtime::{DeviceRuntimeType, StartupReset};
use gui_backends::{GuiAudioBackend, GuiEventSource, GuiVideoBackend};
use headless_backends::{HeadlessAudioBackend, HeadlessEventSource, HeadlessVideoBackend};
use std::path::PathBuf;

// Type aliases for convenience
type GuiSimViewer = viewer::SimViewer<GuiVideoBackend, GuiAudioBackend, GuiEventSource>;
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

    /// Run in headless mode (no GUI, for testing)
    #[arg(long)]
    headless: bool,

    /// Runtime backend to use
    #[arg(long, value_enum, default_value_t = RuntimeBackend::Sim)]
    runtime: RuntimeBackend,

    /// FPGA serial device path (required when --runtime fpga)
    #[arg(long)]
    fpga_device: Option<PathBuf>,

    /// FPGA serial baud rate
    #[arg(long, default_value_t = 115200)]
    fpga_baud: u32,

    /// Startup reset mode for FPGA runtime
    #[arg(long, value_enum, default_value_t = StartupResetArg::Cpu)]
    fpga_startup_reset: StartupResetArg,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum RuntimeBackend {
    Sim,
    Fpga,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum StartupResetArg {
    None,
    Cpu,
    System,
}

impl From<StartupResetArg> for StartupReset {
    fn from(value: StartupResetArg) -> Self {
        match value {
            StartupResetArg::None => StartupReset::None,
            StartupResetArg::Cpu => StartupReset::Cpu,
            StartupResetArg::System => StartupReset::System,
        }
    }
}

fn runtime_type_from_args(args: &Args) -> Result<DeviceRuntimeType, String> {
    match args.runtime {
        RuntimeBackend::Sim => Ok(DeviceRuntimeType::Sim),
        RuntimeBackend::Fpga => {
            let device = args
                .fpga_device
                .as_ref()
                .ok_or_else(|| "--fpga-device is required when --runtime fpga".to_string())?;
            Ok(DeviceRuntimeType::Fpga {
                device: device.to_string_lossy().to_string(),
                baud: args.fpga_baud,
                startup_reset: args.fpga_startup_reset.into(),
            })
        }
    }
}

fn main() {
    let args = Args::parse();
    let runtime_type = match runtime_type_from_args(&args) {
        Ok(runtime_type) => runtime_type,
        Err(e) => {
            eprintln!("✗ Error: {}", e);
            std::process::exit(1);
        }
    };

    // Initialize logger
    let log_level = if args.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    log::info!("sim-view: RISC-V CPU Simulator Viewer");

    // Run in headless or GUI mode
    let result = if args.headless {
        log::info!("Running in headless mode (no GUI)");
        run_headless_mode(args, runtime_type)
    } else {
        log::info!("Controls:");
        log::info!("  - Ctrl+R: Reload last ELF file");
        log::info!("  - Escape: Exit");
        run_gui_mode(args, runtime_type)
    };

    // Handle errors
    if let Err(e) = result {
        eprintln!("✗ Error: {}", e);
        std::process::exit(1);
    }
}

fn run_gui_mode(args: Args, runtime_type: DeviceRuntimeType) -> Result<(), String> {
    // Create viewer configuration
    let config = viewer::ViewerConfig {
        initial_width: args.width,
        initial_height: args.height,
        max_cycles: args.max_cycles,
        print_inst_trace: args.print_inst_trace,
        runtime_type,
    };

    // Create GUI backends
    let video = GuiVideoBackend::new(args.width, args.height)?;
    let window_handle = video.get_window_handle();
    let active_handle = video.get_active_handle();
    let audio = GuiAudioBackend::new()?;
    let events = GuiEventSource::new(window_handle, active_handle);

    // Create viewer
    let mut viewer = GuiSimViewer::new(config, video, audio, events)?;

    // Load initial ELF if provided
    if let Some(elf_path) = args.elf {
        viewer.load_elf(&elf_path)?;
    }

    // Run main loop
    viewer.run()
}

fn run_headless_mode(args: Args, runtime_type: DeviceRuntimeType) -> Result<(), String> {
    // Create viewer configuration
    let config = viewer::ViewerConfig {
        initial_width: args.width,
        initial_height: args.height,
        max_cycles: args.max_cycles,
        print_inst_trace: args.print_inst_trace,
        runtime_type,
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
        return Err("Headless mode requires an ELF file".to_string());
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
