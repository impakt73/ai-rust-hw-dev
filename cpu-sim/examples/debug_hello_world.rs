use cpu_sim::{run_elf_with_all_callbacks, InstructionTrace};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    
    let elf_path = PathBuf::from("test_programs/hello_world.elf");
    
    // Create a FIFO data collector
    let fifo_data: Arc<Mutex<Vec<u32>>> = Arc::new(Mutex::new(Vec::new()));
    let fifo_data_clone = Arc::clone(&fifo_data);
    
    let fifo_callback = move |word: u32| {
        let mut data = fifo_data_clone.lock().unwrap();
        data.push(word);
        println!("FIFO TX: 0x{:08x} ({} bytes collected)", word, data.len() * 4);
    };
    
    // Create instruction trace callback
    let inst_count = Arc::new(Mutex::new(0usize));
    let inst_count_clone = Arc::clone(&inst_count);
    
    let trace_callback = move |trace: &InstructionTrace| {
        let mut count = inst_count_clone.lock().unwrap();
        *count += 1;
        
        if *count <= 100 {
            println!("Cycle {}: PC=0x{:08x}, Inst=0x{:08x}, Type={:?}", 
                     *count, trace.pc, trace.instruction, trace.inst_type);
        }
    };
    
    let test_string = "Qu1ck_Br0wn-F0x!Jump5*0v3r@Lazy#D0g$2024%";
    println!("Running hello_world.elf with test string: '{}'", test_string);
    println!("Test string length: {} bytes", test_string.len());
    
    let result = run_elf_with_all_callbacks(
        &elf_path,
        10000,      // max cycles
        false,       // don't print all instructions
        Some(fifo_callback),
        Some(test_string),
        Some(trace_callback),
    );
    
    match result {
        Ok(sim_result) => {
            println!("\n✓ Simulation completed in {} cycles", sim_result.cycles);
            println!("  tohost value: {:?}", sim_result.tohost_value);
            
            let received_data = fifo_data.lock().unwrap();
            println!("  FIFO TX words received: {}", received_data.len());
            
            // Convert to string
            let mut bytes = Vec::new();
            for word in received_data.iter() {
                bytes.push((*word & 0xFF) as u8);
                bytes.push(((*word >> 8) & 0xFF) as u8);
                bytes.push(((*word >> 16) & 0xFF) as u8);
                bytes.push(((*word >> 24) & 0xFF) as u8);
            }
            
            // Remove trailing zeros
            while bytes.last() == Some(&0) {
                bytes.pop();
            }
            
            let received_string = String::from_utf8_lossy(&bytes);
            println!("  Received string: '{}'", received_string);
            println!("  Expected string: '{}'", test_string);
            
            if received_string == test_string {
                println!("\n✓✓ FIFO echo test PASSED!");
            } else {
                println!("\n✗✗ FIFO echo test FAILED!");
                println!("  Mismatch detected");
            }
        }
        Err(e) => {
            eprintln!("✗ Simulation failed: {}", e);
            std::process::exit(1);
        }
    }
}
