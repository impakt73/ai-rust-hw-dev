use cpu_sim::*;
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
    let inst_complete_callback = move |view: &mut SimulatorView| {
        while let Some(word) = view.fifo_read_tx() {
            fifo_data_clone.lock().unwrap().push(word);
        }
    };

    let result = run_elf(
        &elf_path,
        10000,
        false, // print_inst_trace
        false, // print_fsm_state
        Some(inst_complete_callback),
        None::<fn(&InstructionTrace)>,
        None,                           // vcd_path
        0,                              // mem_latency_cycles
        None::<fn(&mut SimulatorView)>, // prep_callback
        |_sim, _result| {},
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(42),
        "Program should exit with code 42"
    );

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
    let inst_complete_callback = move |view: &mut SimulatorView| {
        while let Some(word) = view.fifo_read_tx() {
            fifo_data_clone.lock().unwrap().push(word);
        }
    };

    let result = run_elf(
        &elf_path,
        10000,
        false, // print_inst_trace
        false, // print_fsm_state
        Some(inst_complete_callback),
        None::<fn(&InstructionTrace)>,
        None,                           // vcd_path
        0,                              // mem_latency_cycles
        None::<fn(&mut SimulatorView)>, // prep_callback
        |_sim, _result| {},
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(42),
        "Program should exit with code 42"
    );

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
    let simple = SimpleStruct {
        a: 0x12345678,
        b: 0xABCDEF00,
    };
    let expected_bytes = postcard::to_allocvec(&simple).unwrap();
    println!("\nExpected postcard serialization: {:02x?}", expected_bytes);
    println!(
        "Received first {} bytes:          {:02x?}",
        expected_bytes.len(),
        &bytes[..bytes.len().min(expected_bytes.len())]
    );

    if bytes[..bytes.len().saturating_sub(4).min(expected_bytes.len())] == expected_bytes[..] {
        println!("✓ Bytes match perfectly!");
    } else {
        println!("✗ Bytes DO NOT match!");
        for i in 0..bytes.len().min(expected_bytes.len()) {
            if bytes[i] != expected_bytes[i] {
                println!(
                    "  Mismatch at byte {}: received 0x{:02x}, expected 0x{:02x}",
                    i, bytes[i], expected_bytes[i]
                );
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
    let inst_complete_callback = move |view: &mut SimulatorView| {
        while let Some(word) = view.fifo_read_tx() {
            fifo_data_clone.lock().unwrap().push(word);
        }
    };

    let result = run_elf(
        &elf_path,
        10000,
        false, // print_inst_trace
        false, // print_fsm_state
        Some(inst_complete_callback),
        None::<fn(&InstructionTrace)>,
        None,                           // vcd_path
        0,                              // mem_latency_cycles
        None::<fn(&mut SimulatorView)>, // prep_callback
        |_sim, _result| {},
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(42),
        "Program should exit with code 42"
    );

    let words = fifo_data.lock().unwrap();
    println!("\n=== TEST 3: Debug - byte-by-byte then chunked ===");
    println!("Total words: {}", words.len());
    println!("All words: {:08x?}", &words);

    // First word should be the length
    let expected_len = words[0] as usize;
    println!("\nExpected serialized length: {} bytes", expected_len);

    // Next words should be individual bytes
    println!("Individual byte writes (words 1-{}):", expected_len);
    for i in 1..=expected_len.min(words.len() - 1) {
        println!(
            "  Word {}: 0x{:08x} = byte 0x{:02x}",
            i, words[i], words[i] as u8
        );
    }

    // Find the marker 0xAAAAAAAA
    if let Some(marker_pos) = words.iter().position(|&w| w == 0xAAAAAAAA) {
        println!("\nMarker found at position {}", marker_pos);
        println!("Chunked writes start at position {}", marker_pos + 1);
        println!("Chunked words: {:08x?}", &words[marker_pos + 1..]);
    }

    // Create expected serialization
    #[derive(serde::Serialize)]
    struct SimpleStruct {
        a: u32,
        b: u32,
    }
    let simple = SimpleStruct {
        a: 0x12345678,
        b: 0xABCDEF00,
    };
    let expected_bytes = postcard::to_allocvec(&simple).unwrap();
    println!("\nExpected postcard bytes: {:02x?}", expected_bytes);
}

#[test]
fn test_allocator() {
    let _ = env_logger::builder().is_test(true).try_init();

    let elf_path = test_program_path("test_allocator.elf");

    let fifo_data = Arc::new(Mutex::new(Vec::new()));
    let fifo_data_clone = fifo_data.clone();
    let inst_complete_callback = move |view: &mut SimulatorView| {
        while let Some(word) = view.fifo_read_tx() {
            fifo_data_clone.lock().unwrap().push(word);
        }
    };

    let result = run_elf(
        &elf_path,
        10000,
        false, // print_inst_trace
        false, // print_fsm_state
        Some(inst_complete_callback),
        None::<fn(&InstructionTrace)>,
        None,                           // vcd_path
        0,                              // mem_latency_cycles
        None::<fn(&mut SimulatorView)>, // prep_callback
        |_sim, _result| {},
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(42),
        "Program should exit with code 42"
    );

    let words = fifo_data.lock().unwrap();
    println!("\n=== ALLOCATOR TEST ===");
    println!("Total words: {}", words.len());
    println!("All words: {:08x?}", &words);

    let expected_len = words[0] as usize;
    println!("\nVec length: {}", expected_len);
    println!("Expected bytes: [12, 34, 56, 78, 9a, bc, de, f0]");
    println!("Received bytes:");
    for i in 1..=expected_len.min(words.len() - 1) {
        println!("  Byte {}: 0x{:02x}", i - 1, words[i] as u8);
    }

    // Check if the bytes match
    let expected = [0x12u8, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];
    let mut all_match = true;
    for i in 0..expected.len() {
        let received = words[i + 1] as u8;
        if received != expected[i] {
            println!(
                "✗ Mismatch at byte {}: received 0x{:02x}, expected 0x{:02x}",
                i, received, expected[i]
            );
            all_match = false;
        }
    }

    if all_match {
        println!("✓ ALL BYTES MATCH - Allocator is working correctly!");
    } else {
        println!("✗ ALLOCATOR OR VEC IS CORRUPTING DATA!");
    }
}

#[test]
fn test_heap_directly() {
    let _ = env_logger::builder().is_test(true).try_init();

    let elf_path = test_program_path("test_heap_directly.elf");

    let fifo_data = Arc::new(Mutex::new(Vec::new()));
    let fifo_data_clone = fifo_data.clone();
    let inst_complete_callback = move |view: &mut SimulatorView| {
        while let Some(word) = view.fifo_read_tx() {
            fifo_data_clone.lock().unwrap().push(word);
        }
    };

    let result = run_elf(
        &elf_path,
        10000,
        false, // print_inst_trace
        false, // print_fsm_state
        Some(inst_complete_callback),
        None::<fn(&InstructionTrace)>,
        None,                           // vcd_path
        0,                              // mem_latency_cycles
        None::<fn(&mut SimulatorView)>, // prep_callback
        |_sim, _result| {},
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(42),
        "Program should exit with code 42"
    );

    let words = fifo_data.lock().unwrap();
    println!("\n=== HEAP DIRECT ACCESS TEST ===");
    println!("All words: {:08x?}", &words);

    // Find markers
    let marker_a = words.iter().position(|&w| w == 0xAAAAAAAA);
    let marker_b = words.iter().position(|&w| w == 0xBBBBBBBB);
    let marker_c = words.iter().position(|&w| w == 0xCCCCCCCC);

    if let Some(a_pos) = marker_a {
        println!("\nTest 1: Direct heap allocation + ptr::write + ptr::read");
        println!("Marker A at position {}", a_pos);
        if let Some(b_pos) = marker_b {
            println!("Marker B at position {}", b_pos);
            println!("Bytes between markers (expected [12, 34, 56, 78, 9a, bc, de, f0]):");
            for i in (a_pos + 1)..b_pos {
                println!("  Byte {}: 0x{:02x}", i - a_pos - 1, words[i] as u8);
            }
        }
    }

    if let Some(c_pos) = marker_c {
        println!("\nTest 2: Vec::with_capacity + set_len");
        println!("Marker C at position {}", c_pos);
        println!("Bytes after marker (expected [aa, bb, cc, dd]):");
        for i in (c_pos + 1)..words.len().min(c_pos + 5) {
            println!("  Byte {}: 0x{:02x}", i - c_pos - 1, words[i] as u8);
        }
    }
}

#[test]
fn test_stack_memory() {
    let _ = env_logger::builder().is_test(true).try_init();

    let elf_path = test_program_path("test_stack_memory.elf");

    let fifo_data = Arc::new(Mutex::new(Vec::new()));
    let fifo_data_clone = fifo_data.clone();
    let inst_complete_callback = move |view: &mut SimulatorView| {
        while let Some(word) = view.fifo_read_tx() {
            fifo_data_clone.lock().unwrap().push(word);
        }
    };

    let result = run_elf(
        &elf_path,
        10000,
        false, // print_inst_trace
        false, // print_fsm_state
        Some(inst_complete_callback),
        None::<fn(&InstructionTrace)>,
        None,                           // vcd_path
        0,                              // mem_latency_cycles
        None::<fn(&mut SimulatorView)>, // prep_callback
        |_sim, _result| {},
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(42),
        "Program should exit with code 42"
    );

    let words = fifo_data.lock().unwrap();
    println!("\n=== STACK MEMORY TEST ===");
    println!("All words: {:08x?}", &words);

    let marker_a = words.iter().position(|&w| w == 0xAAAAAAAA);
    let marker_b = words.iter().position(|&w| w == 0xBBBBBBBB);
    let marker_c = words.iter().position(|&w| w == 0xCCCCCCCC);

    if let Some(a_pos) = marker_a {
        println!("\nTest 1: Stack array + ptr::write");
        if let Some(b_pos) = marker_b {
            println!("Bytes between markers A and B (expected [12, 34, 56, 78, 9a, bc, de, f0]):");
            for i in (a_pos + 1)..b_pos {
                println!("  Byte {}: 0x{:02x}", i - a_pos - 1, words[i] as u8);
            }

            let expected = [0x12u8, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];
            let mut all_match = true;
            for (idx, exp) in expected.iter().enumerate() {
                let actual = words[a_pos + 1 + idx] as u8;
                if actual != *exp {
                    println!(
                        "  ✗ Mismatch at byte {}: got 0x{:02x}, expected 0x{:02x}",
                        idx, actual, exp
                    );
                    all_match = false;
                }
            }
            if all_match {
                println!("  ✓ Stack test 1 PASSED!");
            }
        }
    }

    if let Some(c_pos) = marker_c {
        println!("\nTest 2: Stack array with direct initialization");
        println!("Bytes after marker C (expected [11, 22, 33, 44, 55, 66, 77, 88]):");
        for i in (c_pos + 1)..words.len().min(c_pos + 9) {
            println!("  Byte {}: 0x{:02x}", i - c_pos - 1, words[i] as u8);
        }

        let expected = [0x11u8, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
        let mut all_match = true;
        for (idx, exp) in expected.iter().enumerate() {
            let actual = words[c_pos + 1 + idx] as u8;
            if actual != *exp {
                println!(
                    "  ✗ Mismatch at byte {}: got 0x{:02x}, expected 0x{:02x}",
                    idx, actual, exp
                );
                all_match = false;
            }
        }
        if all_match {
            println!("  ✓ Stack test 2 PASSED!");
        }
    }
}

#[test]
fn test_static_heap() {
    let _ = env_logger::builder().is_test(true).try_init();

    let elf_path = test_program_path("test_static_heap.elf");

    let fifo_data = Arc::new(Mutex::new(Vec::new()));
    let fifo_data_clone = fifo_data.clone();
    let inst_complete_callback = move |view: &mut SimulatorView| {
        while let Some(word) = view.fifo_read_tx() {
            fifo_data_clone.lock().unwrap().push(word);
        }
    };

    let result = run_elf(
        &elf_path,
        10000,
        false, // print_inst_trace
        false, // print_fsm_state
        Some(inst_complete_callback),
        None::<fn(&InstructionTrace)>,
        None,                           // vcd_path
        0,                              // mem_latency_cycles
        None::<fn(&mut SimulatorView)>, // prep_callback
        |_sim, _result| {},
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(42),
        "Program should exit with code 42"
    );

    let words = fifo_data.lock().unwrap();
    println!("\n=== STATIC HEAP TEST ===");
    println!("All words: {:08x?}", &words);

    let marker_a = words.iter().position(|&w| w == 0xAAAAAAAA);
    let marker_b = words.iter().position(|&w| w == 0xBBBBBBBB);

    if let Some(a_pos) = marker_a {
        println!("\nTest 1: static mut HEAP with ptr::write/read");
        if let Some(b_pos) = marker_b {
            println!("Bytes between markers A and B (expected [12, 34, 56, 78, 9a, bc, de, f0]):");
            for i in (a_pos + 1)..b_pos {
                println!("  Byte {}: 0x{:02x}", i - a_pos - 1, words[i] as u8);
            }

            let expected = [0x12u8, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];
            let mut all_match = true;
            for (idx, exp) in expected.iter().enumerate() {
                let actual = words[a_pos + 1 + idx] as u8;
                if actual != *exp {
                    println!(
                        "  ✗ Mismatch at byte {}: got 0x{:02x}, expected 0x{:02x}",
                        idx, actual, exp
                    );
                    all_match = false;
                }
            }
            if all_match {
                println!("  ✓ HEAP test 1 PASSED!");
            } else {
                println!("  ✗ HEAP test 1 FAILED - corruption detected!");
            }
        }
    }

    if let Some(b_pos) = marker_b {
        println!("\nTest 2: static mut HEAP with array index notation");
        println!("Bytes after marker B (expected [aa, bb, cc, dd]):");
        for i in (b_pos + 1)..words.len().min(b_pos + 5) {
            println!("  Byte {}: 0x{:02x}", i - b_pos - 1, words[i] as u8);
        }

        let expected = [0xAAu8, 0xBB, 0xCC, 0xDD];
        let mut all_match = true;
        for (idx, exp) in expected.iter().enumerate() {
            let actual = words[b_pos + 1 + idx] as u8;
            if actual != *exp {
                println!(
                    "  ✗ Mismatch at byte {}: got 0x{:02x}, expected 0x{:02x}",
                    idx, actual, exp
                );
                all_match = false;
            }
        }
        if all_match {
            println!("  ✓ HEAP test 2 PASSED!");
        } else {
            println!("  ✗ HEAP test 2 FAILED - corruption detected!");
        }
    }
}
