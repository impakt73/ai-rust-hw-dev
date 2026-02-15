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
    create_test_runtime, instructions_to_bytes, load_and_boot, read_word_with_timeout,
    tohost_termination, wait_for_cpu_halt, LONG_TIMEOUT, SHORT_TIMEOUT, TEST_BOOT_PC,
};
use riscv_core::instruction::*;
use riscv_shared::bus::{DRAM_BASE, SIM_CONTROL_BASE};
use riscv_shared::sim_control::{FAILURE_CODE, SUCCESS_CODE};
const SRAM_BASE_ADDR: u32 = 0x5200_0000;

// ============================================================================
// Basic Execution Tests
// ============================================================================

#[test]
fn test_cpu_basic_execution() {
    let mut runtime = create_test_runtime();

    // Program: Simple arithmetic operations
    let mut instructions = vec![addi(1, 0, 5), addi(2, 0, 3), add(3, 1, 2)];
    instructions.extend(tohost_termination(30, 31, SUCCESS_CODE));

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
    instructions.extend(tohost_termination(30, 31, SUCCESS_CODE));

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
    instructions.extend(tohost_termination(30, 31, SUCCESS_CODE));

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
    instructions.extend(tohost_termination(30, 31, SUCCESS_CODE));

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
        addi(3, 0, 0),
        beq(1, 2, 8),
        addi(3, 0, 99),
        addi(4, 0, 5),
        addi(5, 0, 0),
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
        addi(3, 0, 0),
        blt(1, 2, 8),
        addi(3, 0, 99),
        addi(4, 0, 0),
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
        addi(3, 0, 0),
        bltu(1, 2, 8),
        addi(3, 0, 99),
        addi(4, 0, 0),
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
    instructions.extend(tohost_termination(30, 31, SUCCESS_CODE));

    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program_bytes);
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
}

#[test]
fn test_cpu_store_byte() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![
        lui(1, DRAM_BASE),
        addi(2, 0, 0x12),
        addi(3, 0, 0x34),
        addi(4, 0, 0x56),
        addi(5, 0, 0x78),
        sb(1, 2, 0),
        sb(1, 3, 1),
        sb(1, 4, 2),
        sb(1, 5, 3),
        lw(6, 1, 0),
    ];
    instructions.extend(tohost_termination(30, 31, SUCCESS_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), 0x8000_0000, SHORT_TIMEOUT),
        0x7856_3412
    );
}

#[test]
fn test_cpu_store_halfword() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![
        lui(1, DRAM_BASE),
        addi(2, 0, 0x234),
        addi(3, 0, 0x678),
        sh(1, 2, 0),
        sh(1, 3, 2),
        lw(4, 1, 0),
    ];
    instructions.extend(tohost_termination(30, 31, SUCCESS_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), 0x8000_0000, SHORT_TIMEOUT),
        0x0678_0234
    );
}

#[test]
fn test_cpu_byte_halfword_mixed() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![
        lui(1, DRAM_BASE),
        addi(2, 0, -128),
        sb(1, 2, 0),
        lb(3, 1, 0),
        lbu(4, 1, 0),
        addi(5, 0, -1),
        sh(1, 5, 4),
        lh(6, 1, 4),
        lhu(7, 1, 4),
        sw(1, 3, 0x10),
        sw(1, 4, 0x14),
        sw(1, 6, 0x18),
        sw(1, 7, 0x1C),
    ];
    instructions.extend(tohost_termination(30, 31, SUCCESS_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), 0x8000_0010, SHORT_TIMEOUT),
        0xFFFF_FF80
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), 0x8000_0014, SHORT_TIMEOUT),
        0x0000_0080
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), 0x8000_0018, SHORT_TIMEOUT),
        0xFFFF_FFFF
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), 0x8000_001C, SHORT_TIMEOUT),
        0x0000_FFFF
    );
}

#[test]
fn test_cpu_auipc() {
    let mut runtime = create_test_runtime();
    let mut instructions = vec![
        auipc(1, 0x12345000),
        auipc(2, 0x00001000),
        lui(9, DRAM_BASE),
        sw(9, 1, 0),
        sw(9, 2, 4),
    ];
    instructions.extend(tohost_termination(30, 31, SUCCESS_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), 0x8000_0000, SHORT_TIMEOUT),
        0x9234_5000
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), 0x8000_0004, SHORT_TIMEOUT),
        0x8000_1004
    );
}

#[test]
fn test_cpu_tohost_halt() {
    let mut runtime = create_test_runtime();
    let mut instructions = vec![addi(1, 0, 10), addi(2, 1, 5), add(3, 1, 2)];
    instructions.extend(tohost_termination(30, 31, SUCCESS_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
}

#[test]
fn test_cpu_fence_instruction() {
    let mut runtime = create_test_runtime();
    let mut instructions = vec![addi(1, 0, 10), fence(), addi(2, 1, 5), addi(0, 0, 0)];
    instructions.extend(tohost_termination(30, 31, SUCCESS_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
}

#[test]
fn test_cpu_ecall_instruction() {
    let mut runtime = create_test_runtime();
    let mut instructions = vec![addi(1, 0, 42)];
    instructions.extend(tohost_termination(30, 31, SUCCESS_CODE));
    instructions.push(ecall());
    instructions.push(addi(2, 0, 99));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
}

#[test]
fn test_cpu_ebreak_instruction() {
    let mut runtime = create_test_runtime();
    let mut instructions = vec![addi(1, 0, 100)];
    instructions.extend(tohost_termination(30, 31, SUCCESS_CODE));
    instructions.push(ebreak());
    instructions.push(addi(2, 0, 200));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
}

#[test]
fn test_cpu_csr_read_write() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![
        csrrw(0, 0, 0x300),
        addi(1, 0, 100),
        csrrw(2, 1, 0x300),
        lui(8, DRAM_BASE),
        sw(8, 2, 0),
        csrrw(3, 0, 0x300),
        sw(8, 3, 4),
        csrrw(4, 0, 0x300),
        sw(8, 4, 8),
    ];
    instructions.extend(tohost_termination(30, 31, SUCCESS_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), 0x8000_0000, SHORT_TIMEOUT),
        0
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), 0x8000_0004, SHORT_TIMEOUT),
        100
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), 0x8000_0008, SHORT_TIMEOUT),
        0
    );
}

#[test]
fn test_cpu_csr_set_clear() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![
        addi(1, 0, 0b1010),
        csrrw(0, 1, 0x301),
        addi(2, 0, 0b0101),
        csrrs(3, 2, 0x301),
        lui(8, DRAM_BASE),
        sw(8, 3, 0),
        addi(4, 0, 0b1000),
        csrrc(5, 4, 0x301),
        sw(8, 5, 4),
        csrrw(6, 0, 0x301),
        sw(8, 6, 8),
    ];
    instructions.extend(tohost_termination(30, 31, SUCCESS_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), 0x8000_0000, SHORT_TIMEOUT),
        0b1010
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), 0x8000_0004, SHORT_TIMEOUT),
        0b1111
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), 0x8000_0008, SHORT_TIMEOUT),
        0b0111
    );
}

#[test]
fn test_cpu_csr_immediate() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![
        csrrwi(0, 0, 0x302),
        csrrwi(1, 15, 0x302),
        lui(8, DRAM_BASE),
        sw(8, 1, 0),
        csrrsi(2, 8, 0x302),
        sw(8, 2, 4),
        csrrci(3, 4, 0x302),
        sw(8, 3, 8),
        csrrw(4, 0, 0x302),
        sw(8, 4, 12),
    ];
    instructions.extend(tohost_termination(30, 31, SUCCESS_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), 0x8000_0000, SHORT_TIMEOUT),
        0
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), 0x8000_0004, SHORT_TIMEOUT),
        15
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), 0x8000_0008, SHORT_TIMEOUT),
        15
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), 0x8000_000C, SHORT_TIMEOUT),
        11
    );
}

#[test]
fn test_cpu_csr_instret() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![
        addi(1, 0, 0),
        addi(2, 0, 0),
        addi(3, 0, 0),
        csrrs(4, 0, 0xC02),
        lui(8, DRAM_BASE),
        sw(8, 4, 0),
    ];
    instructions.extend(tohost_termination(30, 31, SUCCESS_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), 0x8000_0000, SHORT_TIMEOUT),
        3
    );
}

#[test]
fn test_cpu_mul_instruction() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![
        addi(1, 0, 10),
        addi(2, 0, 20),
        mul(3, 1, 2),
        lui(8, DRAM_BASE),
        sw(8, 3, 0),
    ];
    instructions.extend(tohost_termination(30, 31, SUCCESS_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), 0x8000_0000, SHORT_TIMEOUT),
        200
    );
}

#[test]
fn test_cpu_mulh_instruction() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![
        lui(1, 0x10000),
        lui(2, 0x10000),
        mulh(3, 1, 2),
        lui(8, DRAM_BASE),
        sw(8, 3, 0),
    ];
    instructions.extend(tohost_termination(30, 31, SUCCESS_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), 0x8000_0000, SHORT_TIMEOUT),
        0x0000_0001
    );
}

#[test]
fn test_cpu_div_instruction() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![
        addi(1, 0, 100),
        addi(2, 0, 7),
        div(3, 1, 2),
        lui(8, DRAM_BASE),
        sw(8, 3, 0),
    ];
    instructions.extend(tohost_termination(30, 31, SUCCESS_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), 0x8000_0000, SHORT_TIMEOUT),
        14
    );
}

#[test]
fn test_cpu_div_by_zero() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![
        addi(1, 0, 100),
        addi(2, 0, 0),
        div(3, 1, 2),
        lui(8, DRAM_BASE),
        sw(8, 3, 0),
    ];
    instructions.extend(tohost_termination(30, 31, SUCCESS_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), 0x8000_0000, SHORT_TIMEOUT),
        0xFFFF_FFFF
    );
}

#[test]
fn test_cpu_rem_instruction() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![
        addi(1, 0, 100),
        addi(2, 0, 7),
        rem(3, 1, 2),
        lui(8, DRAM_BASE),
        sw(8, 3, 0),
    ];
    instructions.extend(tohost_termination(30, 31, SUCCESS_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), 0x8000_0000, SHORT_TIMEOUT),
        2
    );
}

#[test]
fn test_cpu_divu_remu_unsigned() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![
        addi(1, 0, -1),
        addi(2, 0, 2),
        divu(3, 1, 2),
        remu(4, 1, 2),
        lui(8, DRAM_BASE),
        sw(8, 3, 0),
        sw(8, 4, 4),
    ];
    instructions.extend(tohost_termination(30, 31, SUCCESS_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), 0x8000_0000, SHORT_TIMEOUT),
        0x7FFF_FFFF
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), 0x8000_0004, SHORT_TIMEOUT),
        1
    );
}

#[test]
fn test_cpu_m_extension_program() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![
        addi(1, 0, 12),
        addi(2, 0, 5),
        addi(3, 0, 3),
        addi(4, 0, 17),
        addi(5, 0, 5),
        mul(6, 1, 2),
        div(7, 6, 3),
        rem(8, 4, 5),
        add(9, 7, 8),
        lui(10, DRAM_BASE),
        sw(10, 9, 0),
    ];
    instructions.extend(tohost_termination(30, 31, SUCCESS_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), 0x8000_0000, SHORT_TIMEOUT),
        22
    );
}

// ============================================================================
// SRAM Peripheral Tests
// ============================================================================

#[test]
fn test_sram_peripheral_word_read_write() {
    let mut runtime = create_test_runtime();

    let instructions = vec![
        lui(1, SRAM_BASE_ADDR),
        lui(2, 0x1234_5000),
        ori(2, 2, 0x678),
        sw(1, 2, 0),
        lw(3, 1, 0),
        lui(9, DRAM_BASE),
        sw(9, 3, 0),
    ];
    let mut instructions = instructions;
    instructions.extend(tohost_termination(30, 31, SUCCESS_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), SRAM_BASE_ADDR, SHORT_TIMEOUT),
        0x1234_5678
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE, SHORT_TIMEOUT),
        0x1234_5678
    );
}

#[test]
fn test_sram_peripheral_subword_masking_and_alignment() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![
        lui(1, SRAM_BASE_ADDR),
        addi(2, 0, 0x12),
        sb(1, 2, 0),
        addi(2, 0, 0x34),
        sb(1, 2, 1),
        addi(2, 0, 0x56),
        sb(1, 2, 2),
        addi(2, 0, 0x78),
        sb(1, 2, 3),
        lw(3, 1, 0),
        addi(4, 0, 0x78),
        sh(1, 4, 2),
        lw(5, 1, 0),
        lui(9, DRAM_BASE),
        sw(9, 3, 0),
        sw(9, 5, 4),
    ];
    instructions.extend(tohost_termination(30, 31, SUCCESS_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE, SHORT_TIMEOUT),
        0x7856_3412
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 4, SHORT_TIMEOUT),
        0x0078_3412
    );
}

#[test]
fn test_sram_peripheral_boundary_word_access() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![
        lui(1, SRAM_BASE_ADDR),
        addi(2, 0, 0x7FF),
        slli(2, 2, 2),
        add(3, 1, 2),
        addi(4, 0, 0x55),
        sw(3, 4, 0),
        lw(5, 3, 0),
        lui(9, DRAM_BASE),
        sw(9, 5, 0),
    ];
    instructions.extend(tohost_termination(30, 31, SUCCESS_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE, SHORT_TIMEOUT),
        0x0000_0055
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), SRAM_BASE_ADDR + 0x1FFC, SHORT_TIMEOUT),
        0x0000_0055
    );
}
