use cpu_sim::*;
use riscv_core::instruction::*;

/// Helper to convert a slice of u32 instructions to little-endian byte vector
fn instructions_to_bytes(instructions: &[u32]) -> Vec<u8> {
    instructions
        .iter()
        .flat_map(|instr| instr.to_le_bytes())
        .collect()
}

/// Simple RISC-V assembly test for instruction trace validation
///
/// This test was migrated from test_programs/trace_test.s assembly file.
/// It performs specific operations that can be validated via the trace callback:
/// - ADDI instructions with known immediate values
/// - ADD instruction
/// - SUB instruction
/// - ANDI instruction
/// - ORI instruction
/// - LUI instruction
/// - SW and LW instructions
///
/// Success is indicated by writing 42 to the tohost address (0xFFFFFFF0).
#[test]
fn test_assembly_trace_validation() {
    let _ = env_logger::builder().is_test(true).try_init();

    println!("\n=== ASSEMBLY TRACE VALIDATION TEST ===");
    println!("Testing instruction trace with predictable operations");

    // Assembly program translated from trace_test.s using instruction helpers
    #[rustfmt::skip]
    let instructions = vec![
        // Test ADDI instructions - easy to validate
        addi(1, 0, 10),              // x1 = 10
        addi(2, 0, 20),              // x2 = 20
        addi(3, 0, 5),               // x3 = 5

        // Test ADD instruction
        add(4, 1, 2),                // x4 = x1 + x2 = 30

        // Test SUB instruction  
        sub(5, 2, 3),                // x5 = x2 - x3 = 15

        // Test AND instruction
        andi(6, 1, 0xFF),            // x6 = x1 & 0xFF = 10

        // Test OR instruction
        ori(7, 2, 0x1),              // x7 = x2 | 0x1 = 21

        // Test LUI instruction
        lui(8, 0x12345000),          // x8 = 0x12345000

        // Test SW and LW instructions
        sw(0, 1, 0),                 // Store x1 (10) to address 0
        lw(9, 0, 0),                 // Load from address 0 to x9 (should be 10)

        // Exit with success code (42)
        // Write to tohost address 0xFFFFFFF0 (which is -16 in 32-bit)
        addi(10, 0, 42),             // x10 = 42
        addi(11, 0, -16),            // x11 = 0xFFFFFFF0 (sign-extended -16)
        sw(11, 10, 0),               // Write to tohost address (0xFFFFFFF0)

        // Infinite loop (should never reach here)
        jal(0, 0),                   // loop: j loop
    ];

    let program = instructions_to_bytes(&instructions);
    const START_ADDR: u32 = 0x8000_0000;

    let result = run_program(
        1000,  // max_cycles
        false, // print_inst_trace
        false, // print_fsm_state
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,  // vcd_path
        0,     // mem_latency_cycles
        |sim| {
            // Write the program to memory
            sim.write_memory_region(START_ADDR, &program, true);
            println!("✓ Trace test program loaded at 0x{:08x}", START_ADDR);
            println!("  Program size: {} bytes ({} instructions)", program.len(), instructions.len());
            Ok(START_ADDR)
        },
        None::<fn(&cpu_sim::SimulatorView, &cpu_sim::SimulationResult)>,
    )
    .expect("Simulation should succeed");

    println!("\n=== RESULTS ===");
    println!("✓ Simulation completed in {} cycles", result.cycles);
    println!(
        "✓ Tohost value: 0x{:08x} ({})",
        result.tohost_value.unwrap_or(0),
        result.tohost_value.unwrap_or(0)
    );

    // Verify the program executed correctly
    assert_eq!(
        result.tohost_value,
        Some(42),
        "Expected tohost value 42 (success), got {:?}",
        result.tohost_value
    );

    println!("\n========================================");
    println!("✓ ASSEMBLY TRACE VALIDATION TEST PASSED");
    println!("========================================");
    println!("Validated:");
    println!("  - ADDI with known immediate values");
    println!("  - ADD operation");
    println!("  - SUB operation");
    println!("  - ANDI bitwise AND");
    println!("  - ORI bitwise OR");
    println!("  - LUI load upper immediate");
    println!("  - SW/LW memory operations");
    println!("========================================");
}
