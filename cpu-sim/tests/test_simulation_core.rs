mod common;

use common::create_test_program;
use cpu_sim::*;

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

/// Helper function to run program with FSM state printing enabled for debugging
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
            sim.write_memory_region(0x8000_0000, &program);
            Ok(0x8000_0000)
        },
        None::<fn(&cpu_sim::SimulatorView, &cpu_sim::SimulationResult)>,
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
            sim.write_memory_region(0x8000_0000, &program);
            Ok(0x8000_0000)
        },
        None::<fn(&cpu_sim::SimulatorView, &cpu_sim::SimulationResult)>,
    )
    .expect("Simple test should succeed");

    println!(
        "  Simple test cycles:        {:6} / {} ({:.1}%)",
        result1.cycles,
        GLOBAL_MAX_CYCLES,
        (result1.cycles as f64 / GLOBAL_MAX_CYCLES as f64) * 100.0
    );

    // Test 2: Run an ELF file (println_macro is typically the highest)
    let elf_path = sim_tests::test_program_path("hello_world").expect("Failed to find hello_world");
    let result2 = run_elf(
        &elf_path,
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        None,
        0,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&cpu_sim::SimulatorView, &cpu_sim::SimulationResult)>,
    )
    .expect("ELF test should succeed");

    println!(
        "  Hello ELF cycles:          {:6} / {} ({:.1}%)",
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
            sim.write_memory_region(0x8000_0000, &instructions);
            Ok(0x8000_0000)
        },
        None::<fn(&cpu_sim::SimulatorView, &cpu_sim::SimulationResult)>,
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
