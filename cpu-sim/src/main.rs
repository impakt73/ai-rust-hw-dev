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
}

fn main() {
    let args = Args::parse();

    // Initialize logger
    let log_level = if args.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    log::info!("RISC-V CPU Simulator");
    log::info!("Loading ELF: {}", args.elf.display());

    // Run simulation using the library
    match cpu_sim::run_elf(&args.elf, args.max_cycles, args.print_inst_trace) {
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
