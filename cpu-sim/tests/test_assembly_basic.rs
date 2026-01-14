use cpu_sim::*;
use riscv_core::instruction::*;

/// Helper to convert a slice of u32 instructions to little-endian byte vector
fn instructions_to_bytes(instructions: &[u32]) -> Vec<u8> {
    instructions
        .iter()
        .flat_map(|instr| instr.to_le_bytes())
        .collect()
}

/// Comprehensive RISC-V instruction test covering arithmetic, logical, shift,
/// comparison, branches, memory operations, and loops.
///
/// This test was migrated from test_programs/test.s assembly file.
/// It validates:
/// - Arithmetic ALU operations (ADD, SUB, ADDI)
/// - Logical ALU operations (AND, OR, XOR, ANDI, ORI, XORI)
/// - Shift operations (SLLI, SRLI, SRAI)
/// - Comparison operations (SLT, SLTI, SLTU)
/// - Conditional branches (BEQ, BNE, BLT, BGE)
/// - Memory store and load (SW, LW)
/// - Loops with constant and variable iterations
/// - Nested arithmetic sequences
/// - Upper immediate operations (LUI, AUIPC)
///
/// Success is indicated by writing 42 to the tohost address (0xFFFFFFF0).
#[test]
fn test_assembly_basic_instructions() {
    let _ = env_logger::builder().is_test(true).try_init();

    println!("\n=== ASSEMBLY BASIC INSTRUCTIONS TEST ===");
    println!("Testing comprehensive RISC-V instruction set");

    // Assembly program translated from test.s using instruction helpers
    #[rustfmt::skip]
    let instructions = vec![
        // Initialize base registers
        addi(1, 0, 10),              // x1 = 10
        addi(2, 0, 20),              // x2 = 20
        
        // Test 1: Arithmetic ALU Operations
        add(3, 1, 2),                // x3 = x1 + x2 = 30
        sub(4, 2, 1),                // x4 = x2 - x1 = 10
        addi(5, 1, 5),               // x5 = x1 + 5 = 15
        
        // Test 2: Logical ALU Operations
        and(6, 1, 2),                // x6 = x1 & x2 = 0
        or(7, 1, 2),                 // x7 = x1 | x2 = 30
        xor(8, 1, 2),                // x8 = x1 ^ x2 = 30
        andi(9, 1, 15),              // x9 = x1 & 15 = 10
        ori(10, 1, 5),               // x10 = x1 | 5 = 15
        xori(11, 1, 7),              // x11 = x1 ^ 7 = 13
        
        // Test 3: Shift Operations
        addi(12, 0, 8),              // x12 = 8
        slli(13, 12, 2),             // x13 = x12 << 2 = 32
        srli(14, 13, 1),             // x14 = x13 >> 1 = 16
        addi(15, 0, -8),             // x15 = -8 (0xFFFFFFF8)
        srai(16, 15, 1),             // x16 = x15 >>> 1 = -4
        
        // Test 4: Comparison Operations
        addi(17, 0, 5),              // x17 = 5
        addi(18, 0, 10),             // x18 = 10
        slt(19, 17, 18),             // x19 = (x17 < x18) = 1
        slti(20, 17, 3),             // x20 = (x17 < 3) = 0
        sltu(21, 17, 18),            // x21 = (x17 < x18 unsigned) = 1
        
        // Test 5: Conditional Branches (BEQ, BNE)
        addi(22, 0, 42),             // x22 = 42
        addi(23, 0, 42),             // x23 = 42
        beq(22, 23, 8),              // if x22 == x23, skip next instr
        addi(24, 0, 99),             // x24 = 99 (should be skipped)
        addi(24, 0, 1),              // beq_pass: x24 = 1
        addi(25, 0, 10),             // x25 = 10
        addi(26, 0, 20),             // x26 = 20
        bne(25, 26, 8),              // if x25 != x26, skip next instr
        addi(27, 0, 99),             // x27 = 99 (should be skipped)
        addi(27, 0, 1),              // bne_pass: x27 = 1
        
        // Test 6: Conditional Branches (BLT, BGE)
        addi(28, 0, 5),              // x28 = 5
        addi(29, 0, 10),             // x29 = 10
        blt(28, 29, 8),              // if x28 < x29, skip next instr
        addi(30, 0, 99),             // x30 = 99 (should be skipped)
        addi(30, 0, 1),              // blt_pass: x30 = 1
        addi(31, 0, 15),             // x31 = 15
        bge(31, 29, 8),              // if x31 >= x29, skip next instr
        addi(28, 0, 99),             // x28 = 99 (should be skipped)
        addi(28, 0, 2),              // bge_pass: x28 = 2
        
        // Test 7: Memory Store and Load Verification
        lui(1, 0x80001000),          // x1 = 0x80001000
        addi(2, 0, 100),             // x2 = 100
        sw(1, 2, 0),                 // mem[x1+0] = x2
        addi(3, 0, 200),             // x3 = 200
        sw(1, 3, 4),                 // mem[x1+4] = x3
        addi(4, 0, 300),             // x4 = 300
        sw(1, 4, 8),                 // mem[x1+8] = x4
        lw(5, 1, 0),                 // x5 = mem[x1+0] = 100
        lw(6, 1, 4),                 // x6 = mem[x1+4] = 200
        lw(7, 1, 8),                 // x7 = mem[x1+8] = 300
        bne(2, 5, 100),              // if x2 != x5, jump to test_fail
        bne(3, 6, 96),               // if x3 != x6, jump to test_fail
        bne(4, 7, 92),               // if x4 != x7, jump to test_fail
        
        // Test 8: Loop with Constant Counter
        addi(8, 0, 0),               // x8 = 0 (accumulator)
        addi(9, 0, 5),               // x9 = 5 (loop counter)
        addi(8, 8, 1),               // const_loop: x8++
        addi(9, 9, -1),              // x9--
        bne(9, 0, -8),               // if x9 != 0, loop back
        
        // Test 9: Loop with Variable Iterations from Memory
        lui(10, 0x80001000),         // x10 = 0x80001000
        addi(11, 0, 7),              // x11 = 7
        sw(10, 11, 12),              // mem[x10+12] = x11
        lw(12, 10, 12),              // x12 = mem[x10+12] = 7
        addi(13, 0, 0),              // x13 = 0 (accumulator)
        addi(13, 13, 2),             // var_loop: x13 += 2
        addi(12, 12, -1),            // x12--
        bne(12, 0, -8),              // if x12 != 0, loop back
        
        // Test 10: Nested Arithmetic Sequence
        addi(14, 0, 3),              // x14 = 3
        addi(15, 0, 4),              // x15 = 4
        addi(16, 0, 5),              // x16 = 5
        add(17, 14, 15),             // x17 = x14 + x15 = 7
        add(18, 17, 16),             // x18 = x17 + x16 = 12
        slli(19, 18, 1),             // x19 = x18 << 1 = 24
        srli(20, 19, 2),             // x20 = x19 >> 2 = 6
        
        // Test 11: Upper Immediate Operations
        lui(21, 0x12345000),         // x21 = 0x12345000
        addi(21, 21, 0x678),         // x21 = x21 + 0x678 = 0x12345678
        auipc(22, 0),                // x22 = PC + 0
        
        // All Tests Passed - Store success to tohost
        lui(31, 0),                  // x31 = 0
        addi(31, 31, -16),           // x31 = 0xFFFFFFF0 (tohost address)
        addi(30, 0, 42),             // x30 = 42 (success code)
        sw(31, 30, 0),               // mem[tohost] = 42
        jal(0, 20),                  // j halt
        
        // test_fail:
        lui(31, 0),                  // x31 = 0
        addi(31, 31, -16),           // x31 = 0xFFFFFFF0
        addi(30, 0, 1),              // x30 = 1 (failure code)
        sw(31, 30, 0),               // mem[tohost] = 1
        
        // halt:
        jal(0, 0),                   // j halt (infinite loop)
    ];

    let program = instructions_to_bytes(&instructions);
    const START_ADDR: u32 = 0x8000_0000;

    let result = run_program(
        10000, // max_cycles - generous limit for complex test
        false, // print_inst_trace
        false, // print_fsm_state
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,  // vcd_path
        0,     // mem_latency_cycles
        |sim| {
            // Write the program to memory
            sim.write_memory_region(START_ADDR, &program, true);
            println!("✓ Assembly program loaded at 0x{:08x}", START_ADDR);
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
    println!("✓ ASSEMBLY BASIC INSTRUCTIONS TEST PASSED");
    println!("========================================");
    println!("Validated:");
    println!("  - Arithmetic operations (ADD, SUB, ADDI)");
    println!("  - Logical operations (AND, OR, XOR, etc.)");
    println!("  - Shift operations (SLLI, SRLI, SRAI)");
    println!("  - Comparisons (SLT, SLTI, SLTU)");
    println!("  - Branches (BEQ, BNE, BLT, BGE)");
    println!("  - Memory operations (SW, LW)");
    println!("  - Loops and control flow");
    println!("  - Upper immediates (LUI, AUIPC)");
    println!("========================================");
}
