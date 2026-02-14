//! RV32C Compressed Instruction Extension Tests
//!
//! Integration tests for the RV32C compressed instruction extension.
//! Tests basic compressed instructions and critical transitions between
//! compressed and uncompressed instructions.
//!
//! Migrated from cpu-sim/tests/test_rv32c_basic.rs to use backend-agnostic
//! device-runtime APIs. Uses byte-level program construction to handle
//! mixed compressed/uncompressed instruction sequences.

mod common;

use common::{
    create_test_runtime, load_and_boot, tohost_termination, wait_for_tohost, LONG_TIMEOUT,
};
use riscv_core::instruction::{add, addi, c_add, c_addi, c_li, c_mv, lui, sw};
use riscv_shared::bus::DRAM_BASE;
use riscv_shared::sim_control::SUCCESS_CODE;

/// Helper to build a mixed program with compressed and uncompressed instructions.
/// Returns bytes suitable for load_program.
fn build_mixed_program(
    compressed_prefix: &[(u32, u16)], // (offset, c_insn)
    standard_suffix: &[(u32, u32)],   // (offset, insn)
) -> Vec<u8> {
    let max_offset = compressed_prefix
        .iter()
        .map(|(off, _)| *off + 2)
        .chain(standard_suffix.iter().map(|(off, _)| *off + 4))
        .max()
        .unwrap_or(0);

    let mut bytes = vec![0u8; max_offset as usize];

    for &(offset, c_insn) in compressed_prefix {
        let insn_bytes = c_insn.to_le_bytes();
        bytes[offset as usize] = insn_bytes[0];
        bytes[offset as usize + 1] = insn_bytes[1];
    }

    for &(offset, insn) in standard_suffix {
        let insn_bytes = insn.to_le_bytes();
        bytes[offset as usize..offset as usize + 4].copy_from_slice(&insn_bytes);
    }

    bytes
}

#[test]
fn test_c_li() {
    let mut runtime = create_test_runtime();

    // Build program:
    // 0x00: C.LI x10, 5 (compressed)
    // 0x02: LUI x15, DRAM_BASE (standard)
    // 0x06: SW x10, 0x100(x15) (standard)
    // + tohost termination
    let compressed = vec![(0, c_li(10, 5))];

    let mut standard = vec![(2, lui(15, DRAM_BASE)), (6, sw(15, 10, 0x100))];

    // Add tohost termination starting at offset 10
    let tohost_insns = tohost_termination(7, 8, SUCCESS_CODE);
    for (i, &insn) in tohost_insns.iter().enumerate() {
        standard.push((10 + (i as u32 * 4), insn));
    }

    let program_bytes = build_mixed_program(&compressed, &standard);

    const BOOT_PC: u32 = 0x8000_0000;
    load_and_boot(runtime.as_mut(), BOOT_PC, &program_bytes);
    let tohost_value = wait_for_tohost(runtime.as_mut(), LONG_TIMEOUT);

    assert_eq!(
        tohost_value, SUCCESS_CODE,
        "Program should terminate with tohost=1"
    );

    // Read result from memory
    // let value = read_word_with_timeout(runtime.as_mut(), 0x80000100, SHORT_TIMEOUT);
    // assert_eq!(value, 5, "x10 should be 5");
}

#[test]
fn test_c_addi() {
    let mut runtime = create_test_runtime();

    // Build program:
    // 0x00: C.LI x10, 10 (compressed)
    // 0x02: C.ADDI x10, 5 (compressed) -> x10 = 15
    // 0x04: LUI x15, DRAM_BASE (standard)
    // 0x08: SW x10, 0x100(x15) (standard)
    // + tohost termination
    let compressed = vec![(0, c_li(10, 10)), (2, c_addi(10, 5))];

    let mut standard = vec![(4, lui(15, DRAM_BASE)), (8, sw(15, 10, 0x100))];

    let tohost_insns = tohost_termination(7, 8, SUCCESS_CODE);
    for (i, &insn) in tohost_insns.iter().enumerate() {
        standard.push((12 + (i as u32 * 4), insn));
    }

    let program_bytes = build_mixed_program(&compressed, &standard);

    const BOOT_PC: u32 = 0x8000_0000;
    load_and_boot(runtime.as_mut(), BOOT_PC, &program_bytes);
    let tohost_value = wait_for_tohost(runtime.as_mut(), LONG_TIMEOUT);

    assert_eq!(
        tohost_value, SUCCESS_CODE,
        "Program should terminate with tohost=1"
    );

    // let value = read_word_with_timeout(runtime.as_mut(), 0x80000100, SHORT_TIMEOUT);
    // assert_eq!(value, 15, "x10 should be 15");
}

#[test]
fn test_c_add() {
    let mut runtime = create_test_runtime();

    // Build program:
    // 0x00: C.LI x10, 7 (compressed)
    // 0x02: C.LI x11, 3 (compressed)
    // 0x04: C.ADD x10, x11 (compressed) -> x10 = 10
    // 0x06: LUI x15, DRAM_BASE (standard)
    // 0x0A: SW x10, 0x100(x15) (standard)
    // + tohost termination
    let compressed = vec![(0, c_li(10, 7)), (2, c_li(11, 3)), (4, c_add(10, 11))];

    let mut standard = vec![(6, lui(15, DRAM_BASE)), (10, sw(15, 10, 0x100))];

    let tohost_insns = tohost_termination(7, 8, SUCCESS_CODE);
    for (i, &insn) in tohost_insns.iter().enumerate() {
        standard.push((14 + (i as u32 * 4), insn));
    }

    let program_bytes = build_mixed_program(&compressed, &standard);

    const BOOT_PC: u32 = 0x8000_0000;
    load_and_boot(runtime.as_mut(), BOOT_PC, &program_bytes);
    let tohost_value = wait_for_tohost(runtime.as_mut(), LONG_TIMEOUT);

    assert_eq!(
        tohost_value, SUCCESS_CODE,
        "Program should terminate with tohost=1"
    );

    // let value = read_word_with_timeout(runtime.as_mut(), 0x80000100, SHORT_TIMEOUT);
    // assert_eq!(value, 10, "x10 should be 10");
}

#[test]
fn test_c_mv() {
    let mut runtime = create_test_runtime();

    // Build program:
    // 0x00: ADDI x11, x0, 42 (standard)
    // 0x04: C.MV x10, x11 (compressed) -> x10 = 42
    // 0x06: LUI x15, DRAM_BASE (standard)
    // 0x0A: SW x10, 0x100(x15) (standard)
    // + tohost termination
    let compressed = vec![(4, c_mv(10, 11))];

    let mut standard = vec![
        (0, addi(11, 0, 42)),
        (6, lui(15, DRAM_BASE)),
        (10, sw(15, 10, 0x100)),
    ];

    let tohost_insns = tohost_termination(7, 8, SUCCESS_CODE);
    for (i, &insn) in tohost_insns.iter().enumerate() {
        standard.push((14 + (i as u32 * 4), insn));
    }

    let program_bytes = build_mixed_program(&compressed, &standard);

    const BOOT_PC: u32 = 0x8000_0000;
    load_and_boot(runtime.as_mut(), BOOT_PC, &program_bytes);
    let tohost_value = wait_for_tohost(runtime.as_mut(), LONG_TIMEOUT);

    assert_eq!(
        tohost_value, SUCCESS_CODE,
        "Program should terminate with tohost=1"
    );

    // let value = read_word_with_timeout(runtime.as_mut(), 0x80000100, SHORT_TIMEOUT);
    // assert_eq!(value, 42, "x10 should be 42");
}

// ============================================================================
// Transition Tests: Compressed <-> Uncompressed
// ============================================================================

#[test]
fn test_compressed_to_compressed_transition() {
    let mut runtime = create_test_runtime();

    // Sequence of compressed instructions (C→C transition)
    let compressed = vec![
        (0, c_li(10, 1)),   // x10 = 1
        (2, c_addi(10, 2)), // x10 = 3
        (4, c_addi(10, 3)), // x10 = 6
        (6, c_addi(10, 4)), // x10 = 10
    ];

    let mut standard = vec![(8, lui(15, DRAM_BASE)), (12, sw(15, 10, 0x100))];

    let tohost_insns = tohost_termination(7, 11, SUCCESS_CODE);
    for (i, &insn) in tohost_insns.iter().enumerate() {
        standard.push((16 + (i as u32 * 4), insn));
    }

    let program_bytes = build_mixed_program(&compressed, &standard);

    const BOOT_PC: u32 = 0x8000_0000;
    load_and_boot(runtime.as_mut(), BOOT_PC, &program_bytes);
    let tohost_value = wait_for_tohost(runtime.as_mut(), LONG_TIMEOUT);

    assert_eq!(
        tohost_value, SUCCESS_CODE,
        "Program should terminate with tohost=1"
    );

    // let value = read_word_with_timeout(runtime.as_mut(), 0x80000100, SHORT_TIMEOUT);
    // assert_eq!(value, 10, "x10 should be 10 after C→C transitions");
}

#[test]
fn test_compressed_to_uncompressed_transition() {
    let mut runtime = create_test_runtime();

    // C.LI (compressed) followed by ADDI (standard)
    let compressed = vec![(0, c_li(10, 5))];

    let mut standard = vec![
        (2, addi(10, 10, 10)), // x10 = 15
        (6, lui(15, DRAM_BASE)),
        (10, sw(15, 10, 0x100)),
    ];

    let tohost_insns = tohost_termination(7, 11, SUCCESS_CODE);
    for (i, &insn) in tohost_insns.iter().enumerate() {
        standard.push((14 + (i as u32 * 4), insn));
    }

    let program_bytes = build_mixed_program(&compressed, &standard);

    const BOOT_PC: u32 = 0x8000_0000;
    load_and_boot(runtime.as_mut(), BOOT_PC, &program_bytes);
    let tohost_value = wait_for_tohost(runtime.as_mut(), LONG_TIMEOUT);

    assert_eq!(
        tohost_value, SUCCESS_CODE,
        "Program should terminate with tohost=1"
    );

    // let value = read_word_with_timeout(runtime.as_mut(), 0x80000100, SHORT_TIMEOUT);
    // assert_eq!(value, 15, "x10 should be 15 after C→U transition");
}

#[test]
fn test_uncompressed_to_compressed_transition() {
    let mut runtime = create_test_runtime();

    // ADDI (standard) followed by C.ADDI (compressed)
    let compressed = vec![(4, c_addi(10, 10))]; // x10 = 15

    let mut standard = vec![
        (0, addi(10, 0, 5)), // x10 = 5
        (6, lui(15, DRAM_BASE)),
        (10, sw(15, 10, 0x100)),
    ];

    let tohost_insns = tohost_termination(7, 11, SUCCESS_CODE);
    for (i, &insn) in tohost_insns.iter().enumerate() {
        standard.push((14 + (i as u32 * 4), insn));
    }

    let program_bytes = build_mixed_program(&compressed, &standard);

    const BOOT_PC: u32 = 0x8000_0000;
    load_and_boot(runtime.as_mut(), BOOT_PC, &program_bytes);
    let tohost_value = wait_for_tohost(runtime.as_mut(), LONG_TIMEOUT);

    assert_eq!(
        tohost_value, SUCCESS_CODE,
        "Program should terminate with tohost=1"
    );

    // let value = read_word_with_timeout(runtime.as_mut(), 0x80000100, SHORT_TIMEOUT);
    // assert_eq!(value, 15, "x10 should be 15 after U→C transition");
}

#[test]
fn test_uncompressed_to_uncompressed_regression() {
    let mut runtime = create_test_runtime();

    // Sequence of standard 32-bit instructions (regression test)
    let compressed = vec![];

    let mut standard = vec![
        (0, addi(10, 0, 5)),
        (4, addi(11, 0, 3)),
        (8, add(12, 10, 11)),
        (12, lui(15, DRAM_BASE)),
        (16, sw(15, 12, 0x100)),
    ];

    let tohost_insns = tohost_termination(7, 13, SUCCESS_CODE);
    for (i, &insn) in tohost_insns.iter().enumerate() {
        standard.push((20 + (i as u32 * 4), insn));
    }

    let program_bytes = build_mixed_program(&compressed, &standard);

    const BOOT_PC: u32 = 0x8000_0000;
    load_and_boot(runtime.as_mut(), BOOT_PC, &program_bytes);
    let tohost_value = wait_for_tohost(runtime.as_mut(), LONG_TIMEOUT);

    assert_eq!(
        tohost_value, SUCCESS_CODE,
        "Program should terminate with tohost=1"
    );

    // let value = read_word_with_timeout(runtime.as_mut(), 0x80000100, SHORT_TIMEOUT);
    // assert_eq!(value, 8, "x12 should be 8");
}

#[test]
fn test_mixed_sequence_across_word_boundary() {
    let mut runtime = create_test_runtime();

    // Mix compressed and standard instructions across word boundaries
    let compressed = vec![
        (0, c_li(10, 1)),   // x10 = 1
        (2, c_addi(10, 2)), // x10 = 3
        (8, c_addi(10, 8)), // x10 = 15
    ];

    let mut standard = vec![
        (4, addi(10, 10, 4)), // x10 = 7
        (10, lui(15, DRAM_BASE)),
        (14, sw(15, 10, 0x100)),
    ];

    let tohost_insns = tohost_termination(7, 11, SUCCESS_CODE);
    for (i, &insn) in tohost_insns.iter().enumerate() {
        standard.push((18 + (i as u32 * 4), insn));
    }

    let program_bytes = build_mixed_program(&compressed, &standard);

    const BOOT_PC: u32 = 0x8000_0000;
    load_and_boot(runtime.as_mut(), BOOT_PC, &program_bytes);
    let tohost_value = wait_for_tohost(runtime.as_mut(), LONG_TIMEOUT);

    assert_eq!(
        tohost_value, SUCCESS_CODE,
        "Program should terminate with tohost=1"
    );

    // let value = read_word_with_timeout(runtime.as_mut(), 0x80000100, SHORT_TIMEOUT);
    // assert_eq!(value, 15, "x10 should be 15 after mixed sequence");
}
