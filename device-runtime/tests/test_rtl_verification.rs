//! RTL Verification Tests (Subset)
//!
//! Basic instruction verification tests migrated from cpu-sim/tests/test_rtl_verification.rs
//! to use backend-agnostic device-runtime APIs.
//!
//! This file contains the first 10 tests covering basic execution, LUI, logic operations,
//! branches, and load/store operations. Additional RTL verification tests remain in cpu-sim
//! for simulator-specific validation.

mod common;

use common::{
    create_test_runtime, instructions_to_bytes, load_and_boot, tohost_termination,
    wait_for_cpu_halt, LONG_TIMEOUT, TEST_BOOT_PC,
};
use riscv_core::instruction::*;
use riscv_shared::bus::{DRAM_BASE, SIM_CONTROL_BASE};
use riscv_shared::sim_control::{FAILURE_CODE, SUCCESS_CODE};

// ============================================================================
// Basic Execution Tests
// ============================================================================

#[test]
fn test_cpu_basic_execution() {
    let mut runtime = create_test_runtime();

    // Program: Simple arithmetic operations
    let mut instructions = vec![addi(1, 0, 5), addi(2, 0, 3), add(3, 1, 2)];
    instructions.extend(tohost_termination(7, 8, SUCCESS_CODE));

    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program_bytes);
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
}

#[test]
fn test_cpu_three_instructions() {
    let mut runtime = create_test_runtime();

    // Program: Execute exactly 3 instructions
    let mut instructions = vec![addi(1, 0, 10), add(2, 1, 1), sub(3, 2, 1)];
    instructions.extend(tohost_termination(7, 8, SUCCESS_CODE));

    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program_bytes);
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
}

#[test]
fn test_cpu_lui_instruction() {
    let mut runtime = create_test_runtime();

    // Program: Test LUI instruction
    let mut instructions = vec![lui(1, 0x12345000), addi(2, 1, 0x678)];
    instructions.extend(tohost_termination(7, 8, SUCCESS_CODE));

    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program_bytes);
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
}

#[test]
fn test_cpu_logic_operations() {
    let mut runtime = create_test_runtime();

    // Program: Test logic operations
    let mut instructions = vec![
        addi(1, 0, 0xFF),
        addi(2, 0, 0x0F),
        and(3, 1, 2),
        or(4, 1, 2),
        xor(5, 1, 2),
    ];
    instructions.extend(tohost_termination(7, 8, SUCCESS_CODE));

    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program_bytes);
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
}

// ============================================================================
// Branch Tests
// ============================================================================

#[test]
fn test_cpu_branch_beq_bne() {
    let mut runtime = create_test_runtime();

    // Program: Test BEQ and BNE instructions
    let instructions = vec![
        addi(1, 0, 10),
        addi(2, 0, 10),
        beq(1, 2, 8),
        addi(3, 0, 99),
        addi(4, 0, 5),
        bne(1, 4, 8),
        addi(5, 0, 99),
        addi(6, 0, 1),
        lui(9, DRAM_BASE),
        sw(9, 3, 0),
        sw(9, 5, 4),
        or(10, 3, 5),
        bne(10, 0, 20),
        lui(7, SIM_CONTROL_BASE),
        addi(8, 0, SUCCESS_CODE as i32),
        sw(7, 8, 0),
        ebreak(),
        jal(0, 0),
        lui(7, SIM_CONTROL_BASE),
        addi(8, 0, FAILURE_CODE as i32),
        sw(7, 8, 0),
        ebreak(),
        jal(0, 0),
    ];

    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program_bytes);
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
}

#[test]
fn test_cpu_branch_blt_bge() {
    let mut runtime = create_test_runtime();

    // Program: Test BLT and BGE instructions
    let instructions = vec![
        addi(1, 0, 5),
        addi(2, 0, 10),
        blt(1, 2, 8),
        addi(3, 0, 99),
        bge(2, 1, 8),
        addi(4, 0, 99),
        addi(5, 0, 1),
        lui(9, DRAM_BASE),
        sw(9, 3, 0),
        sw(9, 4, 4),
        or(10, 3, 4),
        bne(10, 0, 20),
        lui(7, SIM_CONTROL_BASE),
        addi(8, 0, SUCCESS_CODE as i32),
        sw(7, 8, 0),
        ebreak(),
        jal(0, 0),
        lui(7, SIM_CONTROL_BASE),
        addi(8, 0, FAILURE_CODE as i32),
        sw(7, 8, 0),
        ebreak(),
        jal(0, 0),
    ];

    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program_bytes);
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
}

#[test]
fn test_cpu_branch_bltu_bgeu() {
    let mut runtime = create_test_runtime();

    // Program: Test BLTU and BGEU instructions (unsigned comparison)
    let instructions = vec![
        addi(1, 0, 5),
        addi(2, 0, 10),
        bltu(1, 2, 8),
        addi(3, 0, 99),
        bgeu(2, 1, 8),
        addi(4, 0, 99),
        addi(5, 0, 1),
        lui(9, DRAM_BASE),
        sw(9, 3, 0),
        sw(9, 4, 4),
        or(10, 3, 4),
        bne(10, 0, 20),
        lui(7, SIM_CONTROL_BASE),
        addi(8, 0, SUCCESS_CODE as i32),
        sw(7, 8, 0),
        ebreak(),
        jal(0, 0),
        lui(7, SIM_CONTROL_BASE),
        addi(8, 0, FAILURE_CODE as i32),
        sw(7, 8, 0),
        ebreak(),
        jal(0, 0),
    ];

    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program_bytes);
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
}

// ============================================================================
// Load/Store Tests
// ============================================================================

#[test]
fn test_cpu_load_store() {
    let mut runtime = create_test_runtime();

    // Program: Test basic load and store
    let instructions = vec![
        lui(1, DRAM_BASE),
        addi(2, 0, 42),
        sw(1, 2, 0x100), // Store 42 to 0x80000100
        lw(3, 1, 0x100), // Load back into x3
        lui(4, DRAM_BASE),
        sw(4, 3, 0x200), // Store x3 to 0x80000200 to verify
        sub(10, 3, 2),
        bne(10, 0, 20),
        lui(7, SIM_CONTROL_BASE),
        addi(8, 0, SUCCESS_CODE as i32),
        sw(7, 8, 0),
        ebreak(),
        jal(0, 0),
        lui(7, SIM_CONTROL_BASE),
        addi(8, 0, FAILURE_CODE as i32),
        sw(7, 8, 0),
        ebreak(),
        jal(0, 0),
    ];

    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program_bytes);
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
}

#[test]
fn test_cpu_load_byte() {
    let mut runtime = create_test_runtime();

    // Program: Test byte load (LB, LBU)
    let instructions = vec![
        lui(1, DRAM_BASE),
        addi(2, 0, 0xFF),
        sb(1, 2, 0x100),  // Store byte 0xFF
        lb(3, 1, 0x100),  // Load signed byte (should sign-extend to 0xFFFFFFFF)
        lbu(4, 1, 0x100), // Load unsigned byte (should be 0x000000FF)
        sw(1, 3, 0x200),  // Store signed result
        sw(1, 4, 0x204),  // Store unsigned result
        addi(12, 0, -1),
        sub(10, 3, 12),
        bne(10, 0, 40),
        addi(12, 0, 0xFF),
        sub(10, 4, 12),
        bne(10, 0, 20),
        lui(7, SIM_CONTROL_BASE),
        addi(8, 0, SUCCESS_CODE as i32),
        sw(7, 8, 0),
        ebreak(),
        jal(0, 0),
        lui(7, SIM_CONTROL_BASE),
        addi(8, 0, FAILURE_CODE as i32),
        sw(7, 8, 0),
        ebreak(),
        jal(0, 0),
    ];

    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program_bytes);
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
}

#[test]
fn test_cpu_load_halfword() {
    let mut runtime = create_test_runtime();

    // Program: Test halfword load (LH, LHU)
    let mut instructions = vec![
        lui(1, DRAM_BASE),
        lui(2, 0x0000F000),
        ori(2, 2, 0xFFF),
        sh(1, 2, 0x100),  // Store halfword 0xFFFF
        lh(3, 1, 0x100),  // Load signed halfword (should sign-extend to 0xFFFFFFFF)
        lhu(4, 1, 0x100), // Load unsigned halfword (should be 0x0000FFFF)
        sw(1, 3, 0x200),  // Store signed result
        sw(1, 4, 0x204),  // Store unsigned result
    ];
    instructions.extend(tohost_termination(7, 8, SUCCESS_CODE));

    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program_bytes);
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
}
