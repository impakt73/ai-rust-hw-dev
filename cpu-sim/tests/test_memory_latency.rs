/// Test memory latency functionality
mod common;

use common::create_test_program;
use cpu_sim::*;

/// Helper function to initialize test logger (idempotent)
fn init_test_logger() {
    let _ = env_logger::builder().is_test(true).try_init();
}

/// Test that the simulator works with zero latency (default)
#[test]
fn test_zero_latency_default() {
    init_test_logger();

    // Load a simple program: addi x1, x0, 42 then sw x1, -16(x0) to write to 0xFFFFFFF0, then infinite loop
    let instructions: Vec<u8> = vec![
        0x93, 0x00, 0xa0, 0x02, // addi x1, x0, 42 (0x02a00093)
        0x23, 0x28, 0x10, 0xfe, // sw x1, -16(x0) (0xfe102823) - writes to 0xFFFFFFF0
        0x6f, 0x00, 0x00, 0x00, // jal x0, 0 (0x0000006f) - infinite loop
    ];

    let result = run_program(
        100,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0, // Zero latency
        |sim| {
            sim.write_memory_region(0x8000_0000, &instructions, true);
            Ok(0x8000_0000)
        },
        None::<fn(&cpu_sim::SimulatorView, &cpu_sim::SimulationResult)>,
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(42),
        "Should halt with tohost value 42"
    );

    // With zero latency, this should complete quickly
    println!("✓ Zero latency test completed in {} cycles", result.cycles);
    assert!(
        result.cycles < 20,
        "Should complete in fewer than 20 cycles with zero latency"
    );
}

/// Test that the simulator works with multi-cycle memory latency
#[test]
fn test_multi_cycle_memory_latency() {
    init_test_logger();

    // Load a simple program: addi x1, x0, 42 then sw x1, -16(x0) to write to 0xFFFFFFF0, then infinite loop
    let instructions: Vec<u8> = vec![
        0x93, 0x00, 0xa0, 0x02, // addi x1, x0, 42
        0x23, 0x28, 0x10, 0xfe, // sw x1, -16(x0) - writes to 0xFFFFFFF0
        0x6f, 0x00, 0x00, 0x00, // jal x0, 0 - infinite loop
    ];

    let result = run_program(
        100,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        3, // 3-cycle latency
        |sim| {
            sim.write_memory_region(0x8000_0000, &instructions, true);
            Ok(0x8000_0000)
        },
        None::<fn(&cpu_sim::SimulatorView, &cpu_sim::SimulationResult)>,
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
        result.cycles > 10,
        "Should take more cycles with 3-cycle latency"
    );
}

/// Test load/store operations with variable latency
#[test]
fn test_load_store_with_latency() {
    init_test_logger();

    // Load a program that does:
    // 1. addi x1, x0, 100  (x1 = 100)
    // 2. sw x1, 0(x0)      (store 100 to address 0)
    // 3. lw x2, 0(x0)      (load from address 0 into x2)
    // 4. sw x2, -16(x0)    (write to tohost 0xFFFFFFF0 to halt)
    // 5. jal x0, 0         (infinite loop)
    let instructions: Vec<u8> = vec![
        0x93, 0x00, 0x40, 0x06, // addi x1, x0, 100
        0x23, 0x20, 0x10, 0x00, // sw x1, 0(x0)
        0x03, 0x21, 0x00, 0x00, // lw x2, 0(x0)
        0x23, 0x28, 0x20, 0xfe, // sw x2, -16(x0)
        0x6f, 0x00, 0x00, 0x00, // jal x0, 0
    ];

    let result = run_program(
        200,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        2, // 2-cycle latency
        |sim| {
            sim.write_memory_region(0x8000_0000, &instructions, true);
            Ok(0x8000_0000)
        },
        None::<fn(&cpu_sim::SimulatorView, &cpu_sim::SimulationResult)>,
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
        result.cycles > 15,
        "Should take more cycles with memory latency"
    );
}

/// Test that existing ELF programs still work with variable latency
#[test]
fn test_comprehensive_elf_with_latency() {
    init_test_logger();

    let program = create_test_program();

    let result = run_program(
        1000,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        2,  // 2-cycle latency
        |sim| {
            sim.write_memory_region(0x8000_0000, &program, true);
            Ok(0x8000_0000)
        },
        None::<fn(&cpu_sim::SimulatorView, &cpu_sim::SimulationResult)>,
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
