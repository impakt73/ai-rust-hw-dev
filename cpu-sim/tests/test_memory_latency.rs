/// Test memory latency functionality
mod common;

use common::{create_test_program, run_program, write_memory_region};
use cpu_sim::*;
use riscv_core::instruction::*;

/// Helper function to initialize test logger (idempotent)
fn init_test_logger() {
    let _ = env_logger::builder().is_test(true).try_init();
}

/// Test that the simulator works with zero latency (default)
#[test]
fn test_zero_latency_default() {
    init_test_logger();

    // Load a simple program:
    // lui x2, SIM_CONTROL_BASE - load tohost address
    // addi x1, x0, 42      - load value 42
    // sw x1, 0(x2)         - write to tohost
    // ebreak               - halt CPU
    // jal x0, 0            - fallback infinite loop
    let instructions: Vec<u32> = vec![
        lui(2, SIM_CONTROL_BASE), // x2 = SIM_CONTROL_BASE
        addi(1, 0, 42),           // addi x1, x0, 42
        sw(2, 1, 0),              // sw x1, 0(x2)
        ebreak(),
        jal(0, 0), // jal x0, 0
    ];
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0, // Zero latency
        |sim| {
            write_memory_region(sim, 0x8000_0000, &program_bytes);
            Ok(0x8000_0000)
        },
        None::<fn(&cpu_sim::SimulationResult)>,
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(42),
        "Should halt with tohost value 42"
    );

    // With serialized bus protocol, each memory access takes multiple cycles
    // (5 bytes TX for read, 1-4 bytes RX for response)
    // So we expect more cycles than with direct memory interface
    println!("✓ Zero latency test completed in {} cycles", result.cycles);
    assert!(
        result.cycles < 120,
        "Should complete in fewer than 120 cycles with zero latency"
    );
}

/// Test that the simulator works with multi-cycle memory latency
#[test]
fn test_multi_cycle_memory_latency() {
    init_test_logger();

    // Load a simple program:
    // lui x2, SIM_CONTROL_BASE - load tohost address
    // addi x1, x0, 42      - load value 42
    // sw x1, 0(x2)         - write to tohost
    // ebreak               - halt CPU
    // jal x0, 0            - fallback infinite loop
    let instructions: Vec<u32> = vec![
        lui(2, SIM_CONTROL_BASE), // x2 = SIM_CONTROL_BASE
        addi(1, 0, 42),           // addi x1, x0, 42
        sw(2, 1, 0),              // sw x1, 0(x2)
        ebreak(),
        jal(0, 0), // jal x0, 0
    ];
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        3, // 3-cycle latency
        |sim| {
            write_memory_region(sim, 0x8000_0000, &program_bytes);
            Ok(0x8000_0000)
        },
        None::<fn(&cpu_sim::SimulationResult)>,
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(42),
        "Should halt with tohost value 42 even with latency"
    );

    // With 3-cycle latency, this should take more cycles
    // Each instruction fetch takes 3 cycles, store takes 3 cycles
    println!(
        "✓ Multi-cycle latency test completed in {} cycles",
        result.cycles
    );
    assert!(
        result.cycles > 15,
        "Should take more cycles with 3-cycle latency"
    );
}

/// Test load/store operations with variable latency
#[test]
fn test_load_store_with_latency() {
    init_test_logger();

    // Load a program that does:
    // 1. lui x4, DRAM_BASE  (x4 = DRAM base address)
    // 2. addi x1, x0, 100    (x1 = 100)
    // 3. sw x1, 0(x4)        (store 100 to DRAM address)
    // 4. lw x2, 0(x4)        (load from DRAM into x2)
    // 5. lui x3, SIM_CONTROL_BASE  (load tohost address)
    // 6. sw x2, 0(x3)        (write to tohost to halt)
    // 7. ebreak              (halt CPU)
    // 8. jal x0, 0           (fallback infinite loop)
    let instructions: Vec<u32> = vec![
        lui(4, DRAM_BASE),        // x4 = DRAM_BASE
        addi(1, 0, 100),          // addi x1, x0, 100
        sw(4, 1, 0),              // sw x1, 0(x4)
        lw(2, 4, 0),              // lw x2, 0(x4)
        lui(3, SIM_CONTROL_BASE), // x3 = SIM_CONTROL_BASE
        sw(3, 2, 0),              // sw x2, 0(x3)
        ebreak(),
        jal(0, 0), // jal x0, 0
    ];
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        2, // 2-cycle latency
        |sim| {
            write_memory_region(sim, 0x8000_0000, &program_bytes);
            Ok(0x8000_0000)
        },
        None::<fn(&cpu_sim::SimulationResult)>,
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(100),
        "Should halt with tohost value 100"
    );

    println!(
        "✓ Load/store with latency test completed in {} cycles",
        result.cycles
    );
    assert!(
        result.cycles > 20,
        "Should take more cycles with memory latency"
    );
}

/// Test that existing ELF programs still work with variable latency
#[test]
fn test_comprehensive_elf_with_latency() {
    init_test_logger();

    let program = create_test_program();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        2, // 2-cycle latency
        |sim| {
            write_memory_region(sim, 0x8000_0000, &program);
            Ok(0x8000_0000)
        },
        None::<fn(&cpu_sim::SimulationResult)>,
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(0x2a),
        "Should complete with expected tohost value even with memory latency"
    );

    println!(
        "✓ Comprehensive program with latency completed in {} cycles",
        result.cycles
    );
}
