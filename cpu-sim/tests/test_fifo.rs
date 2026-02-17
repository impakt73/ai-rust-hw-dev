mod common;

use cpu_sim::*;
use std::sync::{Arc, Mutex};

/// Helper function to initialize test logger (idempotent)
fn init_test_logger() {
    let _ = env_logger::builder().is_test(true).try_init();
}

/// Helper function to assert tohost value matches expected
fn assert_tohost(result: &SimulationResult, expected: u32, test_name: &str) {
    assert_eq!(
        result.tohost_value,
        Some(expected),
        "Expected tohost value 0x{:x} ({}) from {}",
        expected,
        expected,
        test_name
    );
}

/// Helper function to create a FIFO data collector
fn create_fifo_collector() -> (Arc<Mutex<Vec<u8>>>, impl FnMut(&mut SimulatorView)) {
    let fifo_data = Arc::new(Mutex::new(Vec::new()));
    let fifo_data_clone = Arc::clone(&fifo_data);

    let callback = move |view: &mut SimulatorView| {
        // Drain all words from TX FIFO and collect them as bytes
        while let Some(word) = view.fifo_read_tx() {
            // Convert u32 word to bytes (little-endian)
            let bytes = [
                (word & 0xFF) as u8,
                ((word >> 8) & 0xFF) as u8,
                ((word >> 16) & 0xFF) as u8,
                ((word >> 24) & 0xFF) as u8,
            ];
            let mut data = fifo_data_clone
                .lock()
                .expect("Failed to lock FIFO data mutex in create_fifo_collector callback");
            data.extend_from_slice(&bytes);
        }
    };

    (fifo_data, callback)
}

/// Helper function to extract string from FIFO data (removes trailing nulls)
fn fifo_data_to_string(data: &[u8]) -> String {
    let end_index = data.iter().rposition(|&b| b != 0);
    let trimmed_data = match end_index {
        Some(idx) => &data[..=idx],
        None => &[],
    };
    String::from_utf8(trimmed_data.to_vec()).expect("FIFO data should be valid UTF-8")
}

#[test]
fn test_fifo_hello_world() {
    init_test_logger();

    let elf_path = sim_tests::test_program_path("hello_world").expect("Failed to find hello_world");
    let (fifo_data, callback) = create_fifo_collector();

    let test_string = "Qu1ck_Br0wn-F0x!Jump5*0v3r@Lazy#D0g$2024%";
    let result = run_elf(
        &elf_path,
        GLOBAL_MAX_CYCLES,
        false, // print_inst_trace
        false, // print_fsm_state
        Some(callback),
        None::<fn(&InstructionTrace)>,
        None, // vcd_path
        0,    // mem_latency_cycles
        Some(|sim: &mut SimulatorView| {
            // Write test string to FIFO RX after ELF is loaded
            sim.fifo_write_rx_string(test_string);
        }),
        None::<fn(&cpu_sim::SimulatorView, &cpu_sim::SimulationResult)>,
    )
    .expect("FIFO hello world simulation should succeed");

    assert_tohost(&result, 0x2a, "hello_world program");

    let received_data = fifo_data.lock().unwrap();
    let received_string = fifo_data_to_string(&received_data);

    assert_eq!(
        received_string, test_string,
        "Expected to receive echoed test string via FIFO"
    );

    println!("✓ FIFO echo test passed in {} cycles", result.cycles);
    println!("✓ Echoed data via FIFO: '{}'", received_string);
}
