use cpu_sim::{run_elf_with_all_callbacks, InstructionTrace};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let elf_path = manifest_dir.join("../test_programs/hello_world.elf");
    let vcd_path = manifest_dir.join("../target/hello_world_detailed.vcd");
    
    // Collect FIFO data
    let fifo_data = Arc::new(Mutex::new(Vec::new()));
    let fifo_data_clone = Arc::clone(&fifo_data);
    let fifo_callback = move |word: u32| {
        fifo_data_clone.lock().unwrap().push(word);
        println!("FIFO TX: 0x{:08x} ('{}')", word, (word as u8) as char);
    };
    
    // Collect instruction trace
    let trace_count = Arc::new(Mutex::new(0u64));
    let trace_count_clone = Arc::clone(&trace_count);
    let trace_callback = move |_trace: &InstructionTrace| {
        let mut count = trace_count_clone.lock().unwrap();
        *count += 1;
    };
    
    println!("Running hello_world.elf with detailed tracing...");
    println!("VCD output: {}", vcd_path.display());
    
    let test_string = "Qu1ck_Br0wn-F0x!Jump5*0v3r@Lazy#D0g$2024%";
    
    let result = run_elf_with_all_callbacks(
        &elf_path,
        100_000,
        true,  // print trace
        Some(fifo_callback),
        Some(test_string),
        Some(trace_callback),
        Some(vcd_path.to_str().unwrap()),
    );
    
    match result {
        Ok(sim_result) => {
            println!("\n=== Simulation Complete ===");
            println!("Cycles: {}", sim_result.cycles);
            if let Some(tohost) = sim_result.tohost_value {
                println!("Tohost value: 0x{:08x}", tohost);
            }
            
            let fifo = fifo_data.lock().unwrap();
            println!("\n=== FIFO Data Received ===");
            println!("Words received: {}", fifo.len());
            
            let received_string: String = fifo.iter()
                .map(|&w| (w as u8) as char)
                .collect();
            println!("String: '{}'", received_string);
            println!("Expected: '{}'", test_string);
            
            if received_string.is_empty() {
                println!("\n⚠️  NO DATA RECEIVED! This is the bug.");
            } else if received_string != test_string {
                println!("\n⚠️  Data mismatch!");
            } else {
                println!("\n✓ Data matches!");
            }
            
            println!("\nVCD file written to: {}", vcd_path.display());
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
