#[cfg(test)]
mod tests {
    use crate::{Dram, SystemBus, Simulator};
    use std::sync::{Arc, Mutex};

    fn test_program_path(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("test_programs")
            .join(name)
    }

    #[test]
    fn test_minimal_postcard_byte_by_byte() {
        let _ = env_logger::builder().is_test(true).try_init();
        
        let elf_path = test_program_path("minimal_postcard_test.elf");
        
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
        println!("\n=== TEST 1: Byte-by-byte writes ===");
        println!("Total words: {}", words.len());
        println!("First 10 words: {:08x?}", &words[..words.len().min(10)]);
        
        let mut bytes = Vec::new();
        for &word in words.iter() {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        println!("First 20 bytes: {:02x?}", &bytes[..bytes.len().min(20)]);
    }

    #[test]
    fn test_minimal_postcard_word_packing() {
        let _ = env_logger::builder().is_test(true).try_init();
        
        let elf_path = test_program_path("minimal_postcard_test2.elf");
        
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
        println!("\n=== TEST 2: Word-packed writes (like packet_test.rs) ===");
        println!("Total words: {}", words.len());
        println!("Words (hex): {:08x?}", &words);
        
        let mut bytes = Vec::new();
        for &word in words.iter() {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        println!("Bytes (hex): {:02x?}", &bytes);
        
        // Simulate what should have been serialized
        #[derive(serde::Serialize)]
        struct SimpleStruct {
            a: u32,
            b: u32,
        }
        let simple = SimpleStruct { a: 0x12345678, b: 0xABCDEF00 };
        let expected_bytes = postcard::to_allocvec(&simple).unwrap();
        println!("\nExpected postcard serialization: {:02x?}", expected_bytes);
        println!("Received first {} bytes:          {:02x?}", expected_bytes.len(), &bytes[..bytes.len().min(expected_bytes.len())]);
        
        if bytes[..bytes.len().saturating_sub(4).min(expected_bytes.len())] == expected_bytes[..] {
            println!("✓ Bytes match perfectly!");
        } else {
            println!("✗ Bytes DO NOT match!");
            for i in 0..bytes.len().min(expected_bytes.len()) {
                if bytes[i] != expected_bytes[i] {
                    println!("  Mismatch at byte {}: received 0x{:02x}, expected 0x{:02x}", 
                            i, bytes[i], expected_bytes[i]);
                }
            }
        }
    }

    #[test]
    fn test_minimal_debug_double_write() {
        let _ = env_logger::builder().is_test(true).try_init();
        
        let elf_path = test_program_path("minimal_debug_test.elf");
        
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
        println!("\n=== TEST 3: Debug - byte-by-byte then chunked ===");
        println!("Total words: {}", words.len());
        println!("All words: {:08x?}", &words);
        
        // First word should be the length
        let expected_len = words[0] as usize;
        println!("\nExpected serialized length: {} bytes", expected_len);
        
        // Next words should be individual bytes
        println!("Individual byte writes (words 1-{}):", expected_len);
        for i in 1..=expected_len.min(words.len()-1) {
            println!("  Word {}: 0x{:08x} = byte 0x{:02x}", i, words[i], words[i] as u8);
        }
        
        // Find the marker 0xAAAAAAAA
        if let Some(marker_pos) = words.iter().position(|&w| w == 0xAAAAAAAA) {
            println!("\nMarker found at position {}", marker_pos);
            println!("Chunked writes start at position {}", marker_pos + 1);
            println!("Chunked words: {:08x?}", &words[marker_pos+1..]);
        }
        
        // Create expected serialization
        #[derive(serde::Serialize)]
        struct SimpleStruct {
            a: u32,
            b: u32,
        }
        let simple = SimpleStruct { a: 0x12345678, b: 0xABCDEF00 };
        let expected_bytes = postcard::to_allocvec(&simple).unwrap();
        println!("\nExpected postcard bytes: {:02x?}", expected_bytes);
    }

    #[test]
    fn test_allocator() {
        let _ = env_logger::builder().is_test(true).try_init();
        
        let elf_path = test_program_path("test_allocator.elf");
        
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
        println!("\n=== ALLOCATOR TEST ===");
        println!("Total words: {}", words.len());
        println!("All words: {:08x?}", &words);
        
        let expected_len = words[0] as usize;
        println!("\nVec length: {}", expected_len);
        println!("Expected bytes: [12, 34, 56, 78, 9a, bc, de, f0]");
        println!("Received bytes:");
        for i in 1..=expected_len.min(words.len()-1) {
            println!("  Byte {}: 0x{:02x}", i-1, words[i] as u8);
        }
        
        // Check if the bytes match
        let expected = vec![0x12u8, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];
        let mut all_match = true;
        for i in 0..expected.len() {
            let received = words[i+1] as u8;
            if received != expected[i] {
                println!("✗ Mismatch at byte {}: received 0x{:02x}, expected 0x{:02x}", i, received, expected[i]);
                all_match = false;
            }
        }
        
        if all_match {
            println!("✓ ALL BYTES MATCH - Allocator is working correctly!");
        } else {
            println!("✗ ALLOCATOR OR VEC IS CORRUPTING DATA!");
        }
    }
}
