mod common;

use common::{
    assert_tohost, create_loop_program, create_test_program, init_test_logger, run_program,
    write_memory_region,
};
use cpu_sim::*;

/// Comprehensive test that runs a full in-memory program for basic simulation verification.
#[allow(dead_code)]
#[test]
fn test_comprehensive_elf() {
    init_test_logger();

    let program = create_test_program();
    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false, // print_inst_trace
        false, // print_fsm_state
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        None, // vcd_path
        0,    // mem_latency_cycles
        |sim| {
            write_memory_region(sim, 0x8000_0000, &program);
            Ok(0x8000_0000)
        },
        None::<fn(&cpu_sim::SimulationResult)>,
    )
    .expect("Simulation should succeed");

    assert_tohost(&result, 0x2a, "comprehensive test");
    println!(
        "✓ Comprehensive test executed successfully in {} cycles",
        result.cycles
    );
}

#[test]
fn test_global_max_cycles_safety_margin() {
    init_test_logger();

    println!("\n========================================");
    println!("GLOBAL_MAX_CYCLES SAFETY VERIFICATION");
    println!("========================================");

    // Test 1: Simple comprehensive test
    let program = create_test_program();
    let result1 = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        None,
        0,
        |sim| {
            write_memory_region(sim, 0x8000_0000, &program);
            Ok(0x8000_0000)
        },
        None::<fn(&cpu_sim::SimulationResult)>,
    )
    .expect("Simple test should succeed");

    println!(
        "  Simple test cycles:        {:6} / {} ({:.1}%)",
        result1.cycles,
        GLOBAL_MAX_CYCLES,
        (result1.cycles as f64 / GLOBAL_MAX_CYCLES as f64) * 100.0
    );

    // Test 2: Run a loop-based program that exercises more cycles than the simple test
    let loop_program = create_loop_program(500);
    let result2 = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        None,
        0,
        |sim| {
            write_memory_region(sim, 0x8000_0000, &loop_program);
            Ok(0x8000_0000)
        },
        None::<fn(&cpu_sim::SimulationResult)>,
    )
    .expect("Loop program test should succeed");

    println!(
        "  Loop program cycles:        {:6} / {} ({:.1}%)",
        result2.cycles,
        GLOBAL_MAX_CYCLES,
        (result2.cycles as f64 / GLOBAL_MAX_CYCLES as f64) * 100.0
    );

    // Test 3: Memory latency test
    let instructions: Vec<u8> = vec![
        0x37, 0x01, 0x00, 0x40, // lui x2, 0x40000 (x2 = 0x40000000 = tohost address)
        0x93, 0x00, 0xa0, 0x02, // addi x1, x0, 42
        0x23, 0x20, 0x11, 0x00, // sw x1, 0(x2)
        0x73, 0x00, 0x10, 0x00, // ebreak
        0x6f, 0x00, 0x00, 0x00, // jal x0, 0
    ];

    let result3 = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        None,
        3, // 3-cycle latency
        |sim| {
            write_memory_region(sim, 0x8000_0000, &instructions);
            Ok(0x8000_0000)
        },
        None::<fn(&cpu_sim::SimulationResult)>,
    )
    .expect("Latency test should succeed");

    println!(
        "  Latency test cycles:       {:6} / {} ({:.1}%)",
        result3.cycles,
        GLOBAL_MAX_CYCLES,
        (result3.cycles as f64 / GLOBAL_MAX_CYCLES as f64) * 100.0
    );

    // Assert safety margins
    let max_observed = result1.cycles.max(result2.cycles).max(result3.cycles);
    let safety_factor = GLOBAL_MAX_CYCLES as f64 / max_observed as f64;

    println!("\n  Maximum observed:          {:6} cycles", max_observed);
    println!(
        "  GLOBAL_MAX_CYCLES:         {:6} cycles",
        GLOBAL_MAX_CYCLES
    );
    println!("  Safety factor:             {:.1}×", safety_factor);

    // Assert we have at least 2× headroom
    assert!(
        safety_factor >= 2.0,
        "GLOBAL_MAX_CYCLES should provide at least 2× safety margin. \
         Current factor: {:.1}×",
        safety_factor
    );

    // Assert no test uses more than 50% of the limit
    assert!(
        max_observed < GLOBAL_MAX_CYCLES / 2,
        "No test should use more than 50% of GLOBAL_MAX_CYCLES. \
         Max observed: {} ({}%)",
        max_observed,
        (max_observed as f64 / GLOBAL_MAX_CYCLES as f64) * 100.0
    );

    println!("\n✓ All tests well within GLOBAL_MAX_CYCLES");
    println!("✓ Safety margin verified (>2× headroom)");
    println!("========================================\n");
}
