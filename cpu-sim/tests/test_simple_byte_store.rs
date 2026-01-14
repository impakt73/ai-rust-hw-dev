use cpu_sim::*;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn test_program_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../test_programs")
        .join(name)
}

#[test]
fn test_simple_byte_store() {
    let _ = env_logger::builder().is_test(true).try_init();

    let elf_path = test_program_path("test_byte_store_simple.elf");

    let fifo_data = Arc::new(Mutex::new(Vec::new()));
    let fifo_data_clone = fifo_data.clone();
    let inst_complete_callback = move |view: &mut SimulatorView| {
        // Drain all words from TX FIFO and collect them
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
        None::<fn(&mut SimulatorView)>, // setup_callback
        None::<fn(&cpu_sim::SimulatorView, &cpu_sim::SimulationResult)>,
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(42),
        "Program should exit with code 42"
    );

    let words = fifo_data.lock().unwrap();
    println!("\n=== SIMPLE BYTE STORE TEST ===");
    println!("Total words received: {}", words.len());

    // Find markers
    let marker_a = words.iter().position(|&w| w == 0xAAAAAAAA);
    let marker_b = words.iter().position(|&w| w == 0xBBBBBBBB);

    if let (Some(a_pos), Some(_b_pos)) = (marker_a, marker_b) {
        println!("\nBytes between markers (expected [11, 22, 33, 44, 55, 66, 77, 88]):");

        let expected = [0x11u8, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
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
            println!("\n✓ SIMPLE BYTE STORE TEST PASSED!");
        } else {
            panic!("✗ SIMPLE BYTE STORE TEST FAILED!");
        }
    } else {
        panic!("Could not find markers in FIFO data");
    }
}
