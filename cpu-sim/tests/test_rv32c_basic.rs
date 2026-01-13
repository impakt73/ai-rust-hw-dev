//! RV32C Compressed Instruction Extension Tests
//!
//! Integration tests for the RV32C compressed instruction extension.
//! Tests basic compressed instructions, memory operations, control flow,
//! and critical transitions between compressed and uncompressed instructions.

use cpu_sim::*;
use riscv_core::instruction::*;

/// Helper function to initialize test logger (idempotent)
fn init_test_logger() {
    let _ = env_logger::builder().is_test(true).try_init();
}

/// Generate tohost termination sequence using standard (32-bit) instructions
fn tohost_termination(addr_reg: u32, value_reg: u32) -> Vec<u32> {
    vec![
        addi(addr_reg, 0, -16),     // Load -16 (0xFFFF_FFF0) into addr_reg
        addi(value_reg, 0, 1),      // Load success code (1)
        sw(addr_reg, value_reg, 0), // Store value to tohost address
        jal(0, 0),                  // Infinite loop (jump to self)
    ]
}

/// Helper to write compressed instruction bytes at a specific address
/// Compressed instructions are 16 bits (2 bytes)
fn write_compressed_instruction<'a, F, T>(sim: &mut Simulator<'a, F, T>, addr: u32, c_insn: u16)
where
    F: FnMut(&mut cpu_sim::Fifo),
    T: FnMut(&riscv_core::trace::InstructionTrace),
{
    let bytes = c_insn.to_le_bytes();
    sim.write_memory_region(addr, &bytes, true);
}

/// Helper to write a standard 32-bit instruction
fn write_standard_instruction<'a, F, T>(sim: &mut Simulator<'a, F, T>, addr: u32, insn: u32)
where
    F: FnMut(&mut cpu_sim::Fifo),
    T: FnMut(&riscv_core::trace::InstructionTrace),
{
    let bytes = insn.to_le_bytes();
    sim.write_memory_region(addr, &bytes, true);
}

// ============================================================================
// Basic Compressed Instruction Tests
// ============================================================================

#[test]
fn test_c_li() {
    init_test_logger();

    const START_ADDR: u32 = 0x8000_0000;

    let result = run_program(
        100,
        false,
        false,
        None::<fn(&mut Fifo)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            // C.LI x10, 5 (compressed)
            write_compressed_instruction(sim, START_ADDR, c_li(10, 5));

            // Write result to memory
            let mut offset = 2;
            write_standard_instruction(sim, START_ADDR + offset, addi(15, 0, 0x100));
            offset += 4;
            write_standard_instruction(sim, START_ADDR + offset, sw(15, 10, 0));
            offset += 4;

            // Add tohost termination
            for &insn in &tohost_termination(7, 8) {
                write_standard_instruction(sim, START_ADDR + offset, insn);
                offset += 4;
            }

            Ok(START_ADDR)
        },
        |sim, result| {
            assert!(
                result.tohost_value == Some(1),
                "Program should terminate with tohost=1"
            );
            let value = sim.bus.read_word(0x100);
            assert_eq!(value, 5, "x10 should be 5");
        },
    );

    result.expect("C.LI test should run successfully");
}

#[test]
fn test_c_addi() {
    init_test_logger();

    const START_ADDR: u32 = 0x8000_0000;

    let result = run_program(
        100,
        false,
        false,
        None::<fn(&mut Fifo)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            // Setup: x10 = 10
            write_compressed_instruction(sim, START_ADDR, c_li(10, 10));
            // C.ADDI x10, 5 -> x10 = x10 + 5 = 15
            write_compressed_instruction(sim, START_ADDR + 2, c_addi(10, 5));

            // Write result to memory
            let mut offset = 4;
            write_standard_instruction(sim, START_ADDR + offset, addi(15, 0, 0x100));
            offset += 4;
            write_standard_instruction(sim, START_ADDR + offset, sw(15, 10, 0));
            offset += 4;

            // Add tohost termination
            for &insn in &tohost_termination(7, 8) {
                write_standard_instruction(sim, START_ADDR + offset, insn);
                offset += 4;
            }

            Ok(START_ADDR)
        },
        |sim, result| {
            assert!(
                result.tohost_value == Some(1),
                "Program should terminate with tohost=1"
            );
            let value = sim.bus.read_word(0x100);
            assert_eq!(value, 15, "x10 should be 15");
        },
    );

    result.expect("C.ADDI test should run successfully");
}

#[test]
fn test_c_add() {
    init_test_logger();

    const START_ADDR: u32 = 0x8000_0000;

    let result = run_program(
        100,
        false,
        false,
        None::<fn(&mut Fifo)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            // Setup: x10 = 7, x11 = 3
            write_compressed_instruction(sim, START_ADDR, c_li(10, 7));
            write_compressed_instruction(sim, START_ADDR + 2, c_li(11, 3));
            // C.ADD x10, x11 -> x10 = x10 + x11 = 10
            write_compressed_instruction(sim, START_ADDR + 4, c_add(10, 11));

            // Write result to memory
            let mut offset = 6;
            write_standard_instruction(sim, START_ADDR + offset, addi(15, 0, 0x100));
            offset += 4;
            write_standard_instruction(sim, START_ADDR + offset, sw(15, 10, 0));
            offset += 4;

            // Add tohost termination
            for &insn in &tohost_termination(7, 8) {
                write_standard_instruction(sim, START_ADDR + offset, insn);
                offset += 4;
            }

            Ok(START_ADDR)
        },
        |sim, result| {
            assert!(
                result.tohost_value == Some(1),
                "Program should terminate with tohost=1"
            );
            let value = sim.bus.read_word(0x100);
            assert_eq!(value, 10, "x10 should be 10");
        },
    );

    result.expect("C.ADD test should run successfully");
}

#[test]
fn test_c_mv() {
    init_test_logger();

    const START_ADDR: u32 = 0x8000_0000;

    let result = run_program(
        100,
        false,
        false,
        None::<fn(&mut Fifo)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            // Setup: x11 = 42
            write_standard_instruction(sim, START_ADDR, addi(11, 0, 42));
            // C.MV x10, x11 -> x10 = x11 = 42
            write_compressed_instruction(sim, START_ADDR + 4, c_mv(10, 11));

            // Write result to memory
            let mut offset = 6;
            write_standard_instruction(sim, START_ADDR + offset, addi(15, 0, 0x100));
            offset += 4;
            write_standard_instruction(sim, START_ADDR + offset, sw(15, 10, 0));
            offset += 4;

            // Add tohost termination
            for &insn in &tohost_termination(7, 8) {
                write_standard_instruction(sim, START_ADDR + offset, insn);
                offset += 4;
            }

            Ok(START_ADDR)
        },
        |sim, result| {
            assert!(
                result.tohost_value == Some(1),
                "Program should terminate with tohost=1"
            );
            let value = sim.bus.read_word(0x100);
            assert_eq!(value, 42, "x10 should be 42");
        },
    );

    result.expect("C.MV test should run successfully");
}

// ============================================================================
// Transition Tests: Compressed <-> Uncompressed
// ============================================================================

#[test]
fn test_compressed_to_compressed_transition() {
    init_test_logger();

    const START_ADDR: u32 = 0x8000_0000;

    let result = run_program(
        100,
        false,
        false,
        None::<fn(&mut Fifo)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            // Sequence of compressed instructions (C→C transition)
            // C.LI x10, 1
            write_compressed_instruction(sim, START_ADDR, c_li(10, 1));
            // C.ADDI x10, 2 -> x10 = 3
            write_compressed_instruction(sim, START_ADDR + 2, c_addi(10, 2));
            // C.ADDI x10, 3 -> x10 = 6
            write_compressed_instruction(sim, START_ADDR + 4, c_addi(10, 3));
            // C.ADDI x10, 4 -> x10 = 10
            write_compressed_instruction(sim, START_ADDR + 6, c_addi(10, 4));

            // Write result to memory
            let mut offset = 8;
            write_standard_instruction(sim, START_ADDR + offset, addi(15, 0, 0x100));
            offset += 4;
            write_standard_instruction(sim, START_ADDR + offset, sw(15, 10, 0));
            offset += 4;

            // Add tohost termination
            for &insn in &tohost_termination(7, 11) {
                write_standard_instruction(sim, START_ADDR + offset, insn);
                offset += 4;
            }

            Ok(START_ADDR)
        },
        |sim, result| {
            assert!(
                result.tohost_value == Some(1),
                "Program should terminate with tohost=1"
            );
            let value = sim.bus.read_word(0x100);
            assert_eq!(value, 10, "x10 should be 10 after C→C transitions");
        },
    );

    result.expect("C→C transition test should run successfully");
}

#[test]
fn test_compressed_to_uncompressed_transition() {
    init_test_logger();

    const START_ADDR: u32 = 0x8000_0000;

    let result = run_program(
        100,
        false,
        false,
        None::<fn(&mut Fifo)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            // C.LI x10, 5 (compressed)
            write_compressed_instruction(sim, START_ADDR, c_li(10, 5));

            // ADDI x10, x10, 10 (standard 32-bit) -> x10 = 15
            write_standard_instruction(sim, START_ADDR + 2, addi(10, 10, 10));

            // Write result to memory
            let mut offset = 6; // 2 bytes (C.LI) + 4 bytes (ADDI)
            write_standard_instruction(sim, START_ADDR + offset, addi(15, 0, 0x100));
            offset += 4;
            write_standard_instruction(sim, START_ADDR + offset, sw(15, 10, 0));
            offset += 4;

            // Add tohost termination
            for &insn in &tohost_termination(7, 11) {
                write_standard_instruction(sim, START_ADDR + offset, insn);
                offset += 4;
            }

            Ok(START_ADDR)
        },
        |sim, result| {
            assert!(
                result.tohost_value == Some(1),
                "Program should terminate with tohost=1"
            );
            let value = sim.bus.read_word(0x100);
            assert_eq!(value, 15, "x10 should be 15 after C→U transition");
        },
    );

    result.expect("C→U transition test should run successfully");
}

#[test]
fn test_uncompressed_to_compressed_transition() {
    init_test_logger();

    const START_ADDR: u32 = 0x8000_0000;

    let result = run_program(
        100,
        false,
        false,
        None::<fn(&mut Fifo)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            // ADDI x10, x0, 5 (standard 32-bit)
            write_standard_instruction(sim, START_ADDR, addi(10, 0, 5));

            // C.ADDI x10, 10 (compressed) -> x10 = 15
            write_compressed_instruction(sim, START_ADDR + 4, c_addi(10, 10));

            // Write result to memory
            let mut offset = 6; // 4 bytes (ADDI) + 2 bytes (C.ADDI)
            write_standard_instruction(sim, START_ADDR + offset, addi(15, 0, 0x100));
            offset += 4;
            write_standard_instruction(sim, START_ADDR + offset, sw(15, 10, 0));
            offset += 4;

            // Add tohost termination
            for &insn in &tohost_termination(7, 11) {
                write_standard_instruction(sim, START_ADDR + offset, insn);
                offset += 4;
            }

            Ok(START_ADDR)
        },
        |sim, result| {
            assert!(
                result.tohost_value == Some(1),
                "Program should terminate with tohost=1"
            );
            let value = sim.bus.read_word(0x100);
            assert_eq!(value, 15, "x10 should be 15 after U→C transition");
        },
    );

    result.expect("U→C transition test should run successfully");
}

#[test]
fn test_uncompressed_to_uncompressed_regression() {
    init_test_logger();

    const START_ADDR: u32 = 0x8000_0000;

    let result = run_program(
        100,
        false,
        false,
        None::<fn(&mut Fifo)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            // Sequence of standard 32-bit instructions (regression test)
            let mut offset = 0;
            write_standard_instruction(sim, START_ADDR + offset, addi(10, 0, 5));
            offset += 4;
            write_standard_instruction(sim, START_ADDR + offset, addi(11, 0, 3));
            offset += 4;
            write_standard_instruction(sim, START_ADDR + offset, add(12, 10, 11));
            offset += 4;

            // Write results to memory
            write_standard_instruction(sim, START_ADDR + offset, addi(15, 0, 0x100));
            offset += 4;
            write_standard_instruction(sim, START_ADDR + offset, sw(15, 12, 0));
            offset += 4;

            // Add tohost termination
            for &insn in &tohost_termination(7, 13) {
                write_standard_instruction(sim, START_ADDR + offset, insn);
                offset += 4;
            }

            Ok(START_ADDR)
        },
        |sim, result| {
            assert!(
                result.tohost_value == Some(1),
                "Program should terminate with tohost=1"
            );
            let value = sim.bus.read_word(0x100);
            assert_eq!(value, 8, "x12 should be 8");
        },
    );

    result.expect("U→U regression test should run successfully");
}

#[test]
fn test_mixed_sequence_across_word_boundary() {
    init_test_logger();

    const START_ADDR: u32 = 0x8000_0000;

    let result = run_program(
        100,
        false,
        false,
        None::<fn(&mut Fifo)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            // Mix compressed and standard instructions across word boundaries
            // Addr 0x00: C.LI x10, 1 (2 bytes)
            write_compressed_instruction(sim, START_ADDR, c_li(10, 1));

            // Addr 0x02: C.ADDI x10, 2 (2 bytes) -> x10 = 3
            write_compressed_instruction(sim, START_ADDR + 2, c_addi(10, 2));

            // Addr 0x04: ADDI x10, x10, 4 (4 bytes, word-aligned) -> x10 = 7
            write_standard_instruction(sim, START_ADDR + 4, addi(10, 10, 4));

            // Addr 0x08: C.ADDI x10, 8 (2 bytes) -> x10 = 15
            write_compressed_instruction(sim, START_ADDR + 8, c_addi(10, 8));

            // Write result to memory
            let mut offset = 10;
            write_standard_instruction(sim, START_ADDR + offset, addi(15, 0, 0x100));
            offset += 4;
            write_standard_instruction(sim, START_ADDR + offset, sw(15, 10, 0));
            offset += 4;

            // Add tohost termination
            for &insn in &tohost_termination(7, 11) {
                write_standard_instruction(sim, START_ADDR + offset, insn);
                offset += 4;
            }

            Ok(START_ADDR)
        },
        |sim, result| {
            assert!(
                result.tohost_value == Some(1),
                "Program should terminate with tohost=1"
            );
            let value = sim.bus.read_word(0x100);
            assert_eq!(value, 15, "x10 should be 15 after mixed sequence");
        },
    );

    result.expect("Mixed sequence test should run successfully");
}
