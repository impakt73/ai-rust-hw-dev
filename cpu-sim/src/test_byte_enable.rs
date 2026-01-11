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
    fn test_byte_enable_heap_directly() {
        let _ = env_logger::builder().is_test(true).try_init();

        let elf_path = test_program_path("test_heap_directly.elf");

        let fifo_data = Arc::new(Mutex::new(Vec::new()));
        let fifo_data_clone = fifo_data.clone();
        let fifo_callback = move |word: u32| {
            fifo_data_clone.lock().unwrap().push(word);
        };

        let bus = SystemBus::new();
        let runtime = riscv_core::create_cpu_runtime().expect("Failed to create CPU runtime");

        let mut sim = Simulator::new(
            &runtime,
            bus,
            false,
            false, // Don't print FSM state,
            Some(fifo_callback),
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            0, // Zero latency
            Some(HungDetectorConfig::default()),
        )
        .expect("Failed to create simulator");

        let entry_point = load_elf(&mut sim, &elf_path).expect("Failed to load ELF");
        sim.reset(entry_point).expect("Failed to reset simulator");

        let mut result = None;
        for _ in 0..10000 {
            let step_result = sim.step();
            if let Some(tohost) = step_result.expect("Step failed").tohost_value {
                result = Some(tohost);
                break;
            }
        }

        assert_eq!(result, Some(42), "Program should exit with code 42");

        let words = fifo_data.lock().unwrap();
        println!("\n=== BYTE ENABLE TEST: HEAP DIRECT ACCESS ===");
        println!("Total words received: {}", words.len());

        // Print all words for debugging
        println!("\nAll FIFO words:");
        for (i, &word) in words.iter().enumerate() {
            println!("  [{}]: 0x{:08x} ({})", i, word, word);
        }

        // Find markers
        let marker_a = words.iter().position(|&w| w == 0xAAAAAAAA);
        let marker_b = words.iter().position(|&w| w == 0xBBBBBBBB);

        if let (Some(a_pos), Some(b_pos)) = (marker_a, marker_b) {
            println!("\nTest: Direct heap allocation + ptr::write + ptr::read");
            println!(
                "Marker A at position {}, Marker B at position {}",
                a_pos, b_pos
            );
            println!("Bytes between markers (expected [12, 34, 56, 78, 9a, bc, de, f0]):");

            let expected = [0x12u8, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];
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
                println!("\n✓ HEAP TEST PASSED! Byte enable fix is working correctly!");
            } else {
                panic!("✗ HEAP TEST FAILED! Byte corruption still present.");
            }
        } else {
            panic!("Could not find markers in FIFO data");
        }
    }

    #[test]
    fn test_byte_enable_stack_memory() {
        let _ = env_logger::builder().is_test(true).try_init();

        let elf_path = test_program_path("test_stack_memory.elf");

        let fifo_data = Arc::new(Mutex::new(Vec::new()));
        let fifo_data_clone = fifo_data.clone();
        let fifo_callback = move |word: u32| {
            fifo_data_clone.lock().unwrap().push(word);
        };

        let bus = SystemBus::new();
        let runtime = riscv_core::create_cpu_runtime().expect("Failed to create CPU runtime");

        let mut sim = Simulator::new(
            &runtime,
            bus,
            false,
            false, // Don't print FSM state,
            Some(fifo_callback),
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            0, // Zero latency
            Some(HungDetectorConfig::default()),
        )
        .expect("Failed to create simulator");

        let entry_point = load_elf(&mut sim, &elf_path).expect("Failed to load ELF");
        sim.reset(entry_point).expect("Failed to reset simulator");

        let mut result = None;
        for _ in 0..10000 {
            let step_result = sim.step();
            if let Some(tohost) = step_result.expect("Step failed").tohost_value {
                result = Some(tohost);
                break;
            }
        }

        assert_eq!(result, Some(42), "Program should exit with code 42");

        let words = fifo_data.lock().unwrap();
        println!("\n=== BYTE ENABLE TEST: STACK MEMORY ===");
        println!("Stack test should still work (uses SW instructions)");
        println!("Total words received: {}", words.len());
        println!("✓ Stack test completed successfully");
    }
}
