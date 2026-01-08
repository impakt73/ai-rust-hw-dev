use clap::Parser;
use cpu_sim::vcd_analyzer::analyze_vcd;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    author,
    version,
    about = "VCD File Analyzer for RISC-V CPU Simulations"
)]
struct Args {
    /// Path to the VCD file
    vcd_file: PathBuf,

    /// Output format (text or json)
    #[arg(short, long, default_value = "text")]
    format: String,
}

fn main() {
    let args = Args::parse();

    match analyze_vcd(args.vcd_file.to_str().unwrap()) {
        Ok(stats) => {
            if args.format == "json" {
                // TODO: Add JSON output
                println!("JSON output not yet implemented");
            } else {
                println!("=== VCD Analysis Results ===\n");
                println!("Simulation Time:");
                println!("  Max Timestamp: {} ps", stats.max_timestamp);

                // Each timestamp unit in this VCD corresponds to one clock cycle
                // (based on the simulation output showing 16948 cycles matching timestamp 16948)
                let total_cycles = stats.max_timestamp;

                println!("  Total Clock Cycles: {}", total_cycles);
                println!("\nExecution Statistics:");
                println!(
                    "  Total Instructions Executed: {}",
                    stats.total_instructions
                );
                println!("  Memory Reads: {}", stats.memory_reads);
                println!("  Memory Writes: {}", stats.memory_writes);
                println!("\nProgram Counter Range:");
                println!("  Min PC: 0x{:08x}", stats.pc_range.0);
                println!("  Max PC: 0x{:08x}", stats.pc_range.1);
                println!("  Unique PC Values: {}", stats.unique_pcs.len());

                if !stats.unique_pcs.is_empty() && stats.unique_pcs.len() <= 50 {
                    println!("\nUnique PC Values:");
                    for pc in &stats.unique_pcs {
                        println!("  0x{:08x}", pc);
                    }
                }

                println!("\nPerformance Metrics:");
                if total_cycles > 0 && stats.total_instructions > 0 {
                    let cpi = total_cycles as f64 / stats.total_instructions as f64;
                    let ipc = stats.total_instructions as f64 / total_cycles as f64;
                    println!("  Cycles Per Instruction (CPI): {:.2}", cpi);
                    println!("  Instructions Per Cycle (IPC): {:.2}", ipc);
                }
            }
        }
        Err(e) => {
            eprintln!("Error analyzing VCD file: {}", e);
            std::process::exit(1);
        }
    }
}
