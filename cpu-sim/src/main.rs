mod memory;
mod sim;

use clap::Parser;
use memory::Memory;
use sim::Simulator;
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
}

fn main() {
    let args = Args::parse();

    // Initialize logger
    let log_level = if args.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    log::info!("RISC-V CPU Simulator");
    log::info!("Loading ELF: {}", args.elf.display());

    // Initialize Memory and load ELF
    let mut mem = Memory::new();
    let entry_point = match mem.load_elf(&args.elf) {
        Ok(entry_point) => {
            log::info!("ELF loaded successfully");
            log::info!("Entry point: 0x{:08x}", entry_point);
            entry_point
        }
        Err(e) => {
            eprintln!("Error loading ELF: {}", e);
            std::process::exit(1);
        }
    };

    // Initialize CPU Simulator
    let runtime = match riscv_core::create_cpu_runtime() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("Error creating CPU runtime: {}", e);
            std::process::exit(1);
        }
    };
    let mut sim = match Simulator::new(&runtime, mem, entry_point, args.print_inst_trace) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error creating simulator: {}", e);
            std::process::exit(1);
        }
    };

    // Run simulation
    match sim.run(args.max_cycles) {
        Ok(result) => {
            if let Some(tohost_value) = result.tohost_value {
                println!(
                    "✓ Simulation completed in {} cycles (tohost value: 0x{:08x})",
                    result.cycles, tohost_value
                );
            } else {
                println!("✓ Simulation completed in {} cycles", result.cycles);
            }
            log::info!("Program finished successfully");
        }
        Err(e) => {
            eprintln!("✗ Simulation error: {}", e);
            std::process::exit(1);
        }
    }
}
