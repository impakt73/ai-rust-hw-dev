#[cfg(test)]
mod tests {
    use crate::*;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    fn test_program_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../test_programs")
            .join(name)
    }

    #[test]
    fn test_alloc_only() {
        let _ = env_logger::builder().is_test(true).try_init();
        
        let elf_path = test_program_path("test_alloc_only.elf");
        
        let fifo_data = Arc::new(Mutex::new(Vec::new()));
        let fifo_data_clone = fifo_data.clone();
        let fifo_callback = move |word: u32| {
            fifo_data_clone.lock().unwrap().push(word);
        };
        
        let mut dram = Dram::new();
        let entry_point = dram.load_elf(&elf_path).expect("Failed to load ELF");
        let bus = SystemBus::new(dram);
        let runtime = riscv_core::create_cpu_runtime().expect("Failed to create CPU runtime");
        
        let mut sim = Simulator::new(
            &runtime,
            bus,
            entry_point,
            false,
            Some(fifo_callback),
            None::<fn(&riscv_core::trace::InstructionTrace)>,
        ).expect("Failed to create simulator");
        
        sim.reset();
        
        let mut result = None;
        for _ in 0..10000 {
            if let Some(tohost) = sim.step() {
                result = Some(tohost);
                break;
            }
        }
        
        assert_eq!(result, Some(42), "Program should exit with code 42");
        
        let words = fifo_data.lock().unwrap();
        println!("\n=== ALLOC ONLY TEST ===");
        println!("Total words received: {}", words.len());
        
        // Find markers
        let marker_a = words.iter().position(|&w| w == 0xAAAAAAAA);
        let marker_b = words.iter().position(|&w| w == 0xBBBBBBBB);
        
        if let (Some(a_pos), Some(_b_pos)) = (marker_a, marker_b) {
            println!("\nBytes between markers (expected [12, 34, 56, 78, 9a, bc, de, f0]):");
            
            let expected = vec![0x12u8, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];
            let mut all_match = true;
            
            for (i, &exp) in expected.iter().enumerate() {
                if a_pos + 1 + i < words.len() {
                    let actual = words[a_pos + 1 + i] as u8;
                    println!("  Byte {}: 0x{:02x} (expected 0x{:02x})", i, actual, exp);
                    if actual != exp {
                        println!("    ✗ MISMATCH!");
                        all_match = false;
                    }
                } else {
                    println!("  Byte {}: MISSING", i);
                    all_match = false;
                }
            }
            
            if all_match {
                println!("\n✓ ALLOC ONLY TEST PASSED!");
            } else {
                panic!("✗ ALLOC ONLY TEST FAILED!");
            }
        } else {
            panic!("Could not find markers in FIFO data");
        }
    }
}
