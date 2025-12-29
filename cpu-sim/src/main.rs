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
    match mem.load_elf(&args.elf) {
        Ok(_entry_point) => {
            log::info!("ELF loaded successfully");
            log::debug!("Note: CPU starts at 0x00000000, ELF entry point is ignored");
        }
        Err(e) => {
            eprintln!("Error loading ELF: {}", e);
            std::process::exit(1);
        }
    }

    // Initialize CPU Simulator
    let runtime = riscv_core::create_cpu_runtime();
    let mut sim = Simulator::new(&runtime, mem);

    // Run simulation
    match sim.run(args.max_cycles) {
        Ok(cycles) => {
            println!("✓ Simulation completed in {} cycles", cycles);
            log::info!("Program finished successfully");
        }
        Err(e) => {
            eprintln!("✗ Simulation error: {}", e);
            std::process::exit(1);
        }
    }
}
