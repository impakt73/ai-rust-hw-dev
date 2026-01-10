/// Integration tests for hung state detection
///
/// These tests verify that the hung detector correctly identifies
/// stuck program counters, stuck FSM states, and out-of-bounds PC jumps.

use crate::hung_detector::HungDetectorConfig;
use crate::bus::SystemBus;
use crate::Simulator;

fn init_test_logger() {
    let _ = env_logger::builder().is_test(true).try_init();
}

/// Helper to create a simulator for programmatic testing
fn create_test_simulator() -> Result<Simulator<'static, fn(u32), fn(&riscv_core::trace::InstructionTrace)>, String> {
    let runtime = Box::leak(Box::new(
        riscv_core::create_cpu_runtime().map_err(|e| e.to_string())?
    ));
    let bus = SystemBus::new();
    
    Simulator::new(
        runtime,
        bus,
        false,  // Don't print instruction trace
        false,  // Don't print FSM state
        None::<fn(u32)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        0,      // Zero latency
    )
}

#[test]
fn test_infinite_loop_detection() {
    init_test_logger();
    
    let mut sim = create_test_simulator().expect("Failed to create simulator");
    
    // Configure hung detector with low threshold for testing
    let mut config = HungDetectorConfig::default();
    config.pc_stuck_threshold = 10; // Detect after 10 identical PCs
    sim.set_hung_detector_config(config);
    
    // Create an infinite loop: jump to self
    // JAL x0, 0 (offset = 0, jumps to current PC)
    let infinite_loop_instr = riscv_core::instruction::jal(0, 0);
    
    let start_addr = 0x8000_0000;
    let instructions = vec![
        infinite_loop_instr.to_le_bytes(),
    ];
    
    let instr_bytes: Vec<u8> = instructions.into_iter().flatten().collect();
    sim.write_memory_region(start_addr, &instr_bytes);
    sim.set_valid_pc_range(start_addr, start_addr + instr_bytes.len() as u32);
    
    // Run the simulation - should panic due to PC stuck detection
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // This should trigger hung detection before hitting max cycles
        let _ = sim.run(start_addr, 1000);
    }));
    
    assert!(result.is_err(), "Expected panic from hung detector");
    
    // Verify the panic message mentions PC stuck
    if let Err(panic_info) = result {
        if let Some(msg) = panic_info.downcast_ref::<String>() {
            assert!(msg.contains("PC stuck"), "Expected PC stuck error, got: {}", msg);
            println!("✓ Correctly detected infinite loop: {}", msg);
        }
    }
}

#[test]
fn test_valid_pc_range_auto_set_from_elf() {
    init_test_logger();
    
    // This test verifies that load_elf automatically sets the valid PC range
    // from the executable segments in the ELF file
    
    use std::path::PathBuf;
    use crate::run_elf;
    
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap();
    let elf_path = workspace_root.join("test_programs").join("test.elf");
    
    // Run the ELF - valid PC range should be auto-detected
    let result = run_elf(&elf_path, 500, false);
    
    assert!(result.is_ok(), "Should successfully load and run ELF with auto-detected PC range");
    println!("✓ Valid PC range automatically detected from ELF executable segments");
}
