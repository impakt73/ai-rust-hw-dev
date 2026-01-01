use cpu_sim::run_elf_with_vcd;
use std::path::PathBuf;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let elf_path = manifest_dir.join("../test_programs/hello_world.elf");
    let vcd_path = manifest_dir.join("../target/hello_world_debug.vcd");
    
    println!("Running hello_world.elf with VCD dumping...");
    println!("VCD output: {}", vcd_path.display());
    
    let result = run_elf_with_vcd(
        &elf_path,
        100_000,  // max cycles
        true,     // print instruction trace
        vcd_path.to_str().unwrap(),
    );
    
    match result {
        Ok(sim_result) => {
            println!("\n=== Simulation Complete ===");
            println!("Cycles: {}", sim_result.cycles);
            if let Some(tohost) = sim_result.tohost_value {
                println!("Tohost value: 0x{:08x}", tohost);
            }
            println!("VCD file written to: {}", vcd_path.display());
            println!("\nYou can view the waveform with:");
            println!("  gtkwave {}", vcd_path.display());
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
