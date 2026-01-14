use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about = "RISC-V CPU Simulator")]
struct Args {
    /// Path to the RISC-V ELF executable
    elf: PathBuf,

    /// Maximum cycles to run (default: 10000)
    #[arg(short, long, default_value_t = 10000)]
    max_cycles: u64,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Print instruction trace (prints every instruction executed)
    #[arg(long)]
    print_inst_trace: bool,

    /// Enable VCD waveform dumping and specify output file path
    #[arg(long)]
    vcd: Option<String>,

    /// Dump memory region after execution: --dump-memory <addr> <size> <output_file>
    #[arg(long, num_args = 3, value_names = ["ADDR", "SIZE", "OUTPUT"])]
    dump_memory: Option<Vec<String>>,

    /// Dump memory region as RGBA8 image: --dump-image <addr> <width> <height> <output_file>
    #[arg(long, num_args = 4, value_names = ["ADDR", "WIDTH", "HEIGHT", "OUTPUT"])]
    dump_image: Option<Vec<String>>,
}

fn main() {
    let args = Args::parse();

    // Initialize logger
    let log_level = if args.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    log::info!("RISC-V CPU Simulator");
    log::info!("Loading ELF: {}", args.elf.display());

    // Single unified execution path using callback-based approach
    run_simulation(&args);
}

/// Unified simulation runner that handles all options through a single code path
fn run_simulation(args: &Args) {
    // Define the termination callback
    let termination_callback = |sim: &cpu_sim::SimulatorView,
                                result: &cpu_sim::SimulationResult| {
        // Print simulation result
        print_simulation_result(result);

        // Handle memory dump if requested
        if let Some(params) = &args.dump_memory {
            handle_memory_dump(sim, params);
        }

        // Handle image dump if requested
        if let Some(params) = &args.dump_image {
            handle_image_dump(sim, params);
        }

        // Print VCD path if enabled
        if let Some(vcd_path) = &args.vcd {
            println!("✓ VCD waveform written to: {}", vcd_path);
        }
    };

    // Always use the callback-based approach for unified handling
    let result = cpu_sim::run_elf(
        &args.elf,
        args.max_cycles,
        args.print_inst_trace,
        false,                                   // print_fsm_state
        None::<fn(&mut cpu_sim::SimulatorView)>, // inst_complete_callback
        None::<fn(&cpu_sim::InstructionTrace)>,  // trace_callback
        args.vcd.as_deref(),
        0,                                       // mem_latency_cycles
        None::<fn(&mut cpu_sim::SimulatorView)>, // setup_callback
        Some(termination_callback),
    );

    match result {
        Ok(_) => {
            log::info!("Program finished successfully");
        }
        Err(e) => {
            eprintln!("✗ Simulation error: {}", e);
            std::process::exit(1);
        }
    }
}

fn print_simulation_result(result: &cpu_sim::SimulationResult) {
    // Convert microseconds to milliseconds as a float with 2 decimal places
    let elapsed_ms = result.elapsed_cpu_time_us as f64 / 1000.0;

    if let Some(tohost_value) = result.tohost_value {
        println!(
            "✓ Simulation completed in {} cycles (tohost value: 0x{:08x}), CPU time: {:.2} ms",
            result.cycles, tohost_value, elapsed_ms
        );
    } else {
        println!(
            "✓ Simulation completed in {} cycles, CPU time: {:.2} ms",
            result.cycles, elapsed_ms
        );
    }
}

fn handle_memory_dump(sim: &cpu_sim::SimulatorView, params: &[String]) {
    if params.len() != 3 {
        eprintln!("✗ Invalid --dump-memory arguments. Expected: <addr> <size> <output>");
        std::process::exit(1);
    }

    let addr = parse_address(&params[0]);
    let size = parse_size(&params[1]);
    let output = &params[2];

    log::info!(
        "Dumping memory: addr=0x{:08x}, size=0x{:x} bytes to {}",
        addr,
        size,
        output
    );

    let bytes: Vec<u8> = sim.dump_memory_region(addr, size).collect();
    if let Err(e) = std::fs::write(output, &bytes) {
        eprintln!("✗ Failed to write memory dump: {}", e);
        std::process::exit(1);
    }

    println!(
        "✓ Memory dump written to: {} ({} bytes)",
        output,
        bytes.len()
    );
}

fn handle_image_dump(sim: &cpu_sim::SimulatorView, params: &[String]) {
    if params.len() != 4 {
        eprintln!("✗ Invalid --dump-image arguments. Expected: <addr> <width> <height> <output>");
        std::process::exit(1);
    }

    let addr = parse_address(&params[0]);
    let width = parse_size(&params[1]);
    let height = parse_size(&params[2]);
    let output = &params[3];

    log::info!(
        "Dumping image: addr=0x{:08x}, size={}x{} to {}",
        addr,
        width,
        height,
        output
    );

    if let Err(e) = sim.dump_memory_region_as_image(addr, width, height, output) {
        eprintln!("✗ Failed to dump image: {}", e);
        std::process::exit(1);
    }

    println!(
        "✓ Image written to: {} ({}x{} RGBA8)",
        output, width, height
    );
}

/// Parse an address from a string (supports hex with 0x prefix or decimal)
fn parse_address(s: &str) -> u32 {
    if let Some(hex_str) = s.strip_prefix("0x") {
        u32::from_str_radix(hex_str, 16).unwrap_or_else(|_| {
            eprintln!("✗ Invalid hex address: {}", s);
            std::process::exit(1);
        })
    } else {
        s.parse::<u32>().unwrap_or_else(|_| {
            eprintln!("✗ Invalid address: {}", s);
            std::process::exit(1);
        })
    }
}

/// Parse a size value from a string (supports hex with 0x prefix or decimal)
fn parse_size(s: &str) -> u32 {
    if let Some(hex_str) = s.strip_prefix("0x") {
        u32::from_str_radix(hex_str, 16).unwrap_or_else(|_| {
            eprintln!("✗ Invalid hex size: {}", s);
            std::process::exit(1);
        })
    } else {
        s.parse::<u32>().unwrap_or_else(|_| {
            eprintln!("✗ Invalid size: {}", s);
            std::process::exit(1);
        })
    }
}
