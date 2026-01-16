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

    // Load a simple program:
    // lui x2, 0x10000000   - load tohost address
    // addi x1, x0, 42      - load value 42
    // sw x1, 0(x2)         - write to tohost (0x10000000)
    // jal x0, 0            - infinite loop
    let instructions: Vec<u8> = vec![
        0x37, 0x01, 0x00, 0x10, // lui x2, 0x10000 (0x10000137) - loads 0x10000000
        0x93, 0x00, 0xa0, 0x02, // addi x1, x0, 42 (0x02a00093)
        0x23, 0x20, 0x11, 0x00, // sw x1, 0(x2) (0x00112023) - writes to 0x10000000
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
        result.cycles < 30,
        "Should complete in fewer than 30 cycles with zero latency"
    );
}

/// Test that the simulator works with multi-cycle memory latency
#[test]
fn test_multi_cycle_memory_latency() {
    init_test_logger();

    // Load a simple program:
    // lui x2, 0x10000000   - load tohost address
    // addi x1, x0, 42      - load value 42
    // sw x1, 0(x2)         - write to tohost
    // jal x0, 0            - infinite loop
    let instructions: Vec<u8> = vec![
        0x37, 0x01, 0x00, 0x10, // lui x2, 0x10000 - loads 0x10000000
        0x93, 0x00, 0xa0, 0x02, // addi x1, x0, 42
        0x23, 0x20, 0x11, 0x00, // sw x1, 0(x2) - writes to 0x10000000
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
        result.cycles > 15,
        "Should take more cycles with 3-cycle latency"
    );
}

/// Test load/store operations with variable latency
#[test]
fn test_load_store_with_latency() {
    init_test_logger();

    // Load a program that does:
    // 1. lui x4, 0x80000000  (x4 = DRAM base address)
    // 2. addi x1, x0, 100    (x1 = 100)
    // 3. sw x1, 0(x4)        (store 100 to DRAM address)
    // 4. lw x2, 0(x4)        (load from DRAM into x2)
    // 5. lui x3, 0x10000000  (load tohost address)
    // 6. sw x2, 0(x3)        (write to tohost 0x10000000 to halt)
    // 7. jal x0, 0           (infinite loop)
    let instructions: Vec<u8> = vec![
        0x37, 0x02, 0x00, 0x80, // lui x4, 0x80000 (x4 = 0x80000000)
        0x93, 0x00, 0x40, 0x06, // addi x1, x0, 100
        0x23, 0x20, 0x12, 0x00, // sw x1, 0(x4)
        0x03, 0x21, 0x02, 0x00, // lw x2, 0(x4)
        0xb7, 0x01, 0x00, 0x10, // lui x3, 0x10000 (x3 = 0x10000000)
        0x23, 0xa0, 0x21, 0x00, // sw x2, 0(x3) - writes to tohost
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
        1000,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        2, // 2-cycle latency
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
