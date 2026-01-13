use cpu_sim::*;
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
        None, // vcd_path
        0,    // mem_latency_cycles
        |_sim, _result| {},
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(42),
        "Program should exit with code 42"
    );

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
        None, // vcd_path
        0,    // mem_latency_cycles
        |_sim, _result| {},
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(42),
        "Program should exit with code 42"
    );

    let words = fifo_data.lock().unwrap();
    println!("\n=== BYTE ENABLE TEST: STACK MEMORY ===");
    println!("Stack test should still work (uses SW instructions)");
    println!("Total words received: {}", words.len());
    println!("✓ Stack test completed successfully");
}
