use cpu_sim::*;
use std::path::PathBuf;

fn main() {
    env_logger::builder().filter_level(log::LevelFilter::Info).init();
    
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap();
    let test_program = workspace_root.join("test_programs/simple_test.elf");
    
    let result = run_elf(
        &test_program,
        100,   // max_cycles (limited for debugging)
        true,  // print_inst_trace = true
    );
    
    match result {
        Ok(sim_result) => {
            println!("\n=== Simulation completed ===");
            println!("Cycles: {}", sim_result.cycles);
            println!("Tohost: {:?}", sim_result.tohost_value);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }
}
