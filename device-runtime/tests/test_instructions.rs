//! CPU Instruction Verification Tests
//!
//! Comprehensive instruction verification tests for the RISC-V CPU implementation.
//! Tests cover basic execution, arithmetic/logic operations, branches, loads/stores,
//! CSR operations, M extension (multiply/divide), and SRAM access patterns.
//!
//! These tests use programmatic instruction sequences (not ELF binaries) to verify
//! individual instruction behavior and common instruction patterns.

mod common;

use common::{
    create_test_runtime, instructions_to_bytes, load_and_boot, read_word_with_timeout,
    tohost_termination, wait_for_cpu_halt, write_word_with_timeout, LONG_TIMEOUT, SHORT_TIMEOUT,
    TEST_BOOT_PC,
};
use riscv_core::instruction::*;
use riscv_shared::bus::{DRAM_BASE, SIM_CONTROL_BASE, SRAM_BASE};
use riscv_shared::sim_control::{FAILURE_CODE, SUCCESS_CODE};
const SRAM_BASE_ADDR: u32 = SRAM_BASE;

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
fn test_cpu_jal_jalr_return_addresses() {
    let mut runtime = create_test_runtime();
    const RESULT_BASE_OFFSET: i32 = 0x100;
    const RESULT_BASE_ADDR: u32 = DRAM_BASE + RESULT_BASE_OFFSET as u32;
    const SKIPPED_PATH_SENTINEL: u32 = 0xA5A5_5A5A;
    let mut instructions = vec![
        lui(9, DRAM_BASE),
        auipc(5, 0),
        addi(5, 5, 31),
        jal(1, 8),
        addi(6, 0, 99),
        sw(9, 1, RESULT_BASE_OFFSET),
        jalr(2, 5, 1),
        addi(6, 0, 77),
        sw(9, 6, RESULT_BASE_OFFSET + 12),
        sw(9, 2, RESULT_BASE_OFFSET + 4),
        sw(9, 5, RESULT_BASE_OFFSET + 8),
    ];
    instructions.extend(tohost_termination(30, 31, SUCCESS_CODE));

    write_word_with_timeout(
        runtime.as_mut(),
        RESULT_BASE_ADDR + 12,
        SKIPPED_PATH_SENTINEL,
        SHORT_TIMEOUT,
    );
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
        read_word_with_timeout(runtime.as_mut(), RESULT_BASE_ADDR, SHORT_TIMEOUT),
        TEST_BOOT_PC + 16
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), RESULT_BASE_ADDR + 4, SHORT_TIMEOUT),
        TEST_BOOT_PC + 28
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), RESULT_BASE_ADDR + 8, SHORT_TIMEOUT),
        TEST_BOOT_PC + 35
    );
    // JALR should add the odd base plus immediate, clear bit 0, and jump to TEST_BOOT_PC + 36,
    // leaving this skipped-path sentinel untouched.
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), RESULT_BASE_ADDR + 12, SHORT_TIMEOUT),
        SKIPPED_PATH_SENTINEL
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

    let mut instructions = vec![
        lui(1, SRAM_BASE_ADDR),
        lui(2, 0x1234_5000),
        ori(2, 2, 0x678),
        sw(1, 2, 0),
        lw(3, 1, 0),
        lui(9, DRAM_BASE),
        sw(9, 3, 0),
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
        addi(2, 2, 0x400),
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
        read_word_with_timeout(runtime.as_mut(), SRAM_BASE_ADDR + 0x2FFC, SHORT_TIMEOUT),
        0x0000_0055
    );
}

#[test]
fn test_sram_peripheral_unaligned_load_store_cross_boundary() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![
        lui(1, SRAM_BASE_ADDR),
        lui(2, 0x1122_3000),
        ori(2, 2, 0x344),
        sw(1, 2, 0),
        lui(2, 0x5566_7000),
        ori(2, 2, 0x788),
        sw(1, 2, 4),
        lui(2, 0x1234_5000),
        ori(2, 2, 0x678),
        sw(1, 2, 1),
        addi(2, 0, 0x234),
        sh(1, 2, 3),
        lw(3, 1, 0),
        lw(4, 1, 4),
        lw(5, 1, 1),
        lh(6, 1, 3),
        lhu(7, 1, 3),
        lui(9, DRAM_BASE),
        sw(9, 3, 0),
        sw(9, 4, 4),
        sw(9, 5, 8),
        sw(9, 6, 12),
        sw(9, 7, 16),
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
        0x3456_7844
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 4, SHORT_TIMEOUT),
        0x5566_7702
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 8, SHORT_TIMEOUT),
        0x0234_5678
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 12, SHORT_TIMEOUT),
        0x0000_0234
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 16, SHORT_TIMEOUT),
        0x0000_0234
    );
}

// ============================================================================
// RV32A Atomic Extension Tests
// ============================================================================

#[test]
fn test_cpu_lr_sc_success() {
    let mut runtime = create_test_runtime();

    // Program: Successful LR/SC sequence
    // Memory location: DRAM_BASE (0x80000000)
    // 1. Store initial value 100 to DRAM_BASE
    // 2. Load-Reserved from DRAM_BASE into x2
    // 3. Add 5 to the loaded value (x2 = 100 + 5 = 105)
    // 4. Store-Conditional the new value back to DRAM_BASE
    // 5. Store SC result (x4 = 0 on success) and final memory value for verification

    let mut instructions = vec![
        // Setup: x1 = DRAM_BASE (memory address)
        lui(1, DRAM_BASE),
        // Store initial value
        addi(2, 0, 100),
        sw(1, 2, 0), // mem[x1+0] = 100
        // LR/SC sequence
        lr_w(2, 1),    // x2 = mem[x1] (100), set reservation
        addi(3, 2, 5), // x3 = x2 + 5 = 105
        sc_w(4, 1, 3), // mem[x1] = x3 (105), x4 = 0 on success
        // Store results for host verification
        sw(1, 4, 0x100), // mem[x1+0x100] = x4 (SC status, 0 = success)
        lw(5, 1, 0),     // x5 = mem[x1+0] (should be 105)
        sw(1, 5, 0x104), // mem[x1+0x104] = x5 (final memory value)
    ];
    instructions.extend(tohost_termination(7, 8, SUCCESS_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
    // SC must have succeeded (status = 0)
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 0x100, SHORT_TIMEOUT),
        0,
        "SC status should be 0 (success)"
    );
    // Final memory value must be the SC-written value (105)
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 0x104, SHORT_TIMEOUT),
        105,
        "Memory should contain 105 after LR/SC"
    );
}

#[test]
fn test_cpu_amoswap() {
    let mut runtime = create_test_runtime();

    // Program: Atomic swap operation
    // 1. Store initial value 42 to DRAM_BASE
    // 2. Atomic swap with value 100: returns old value (42), writes new value (100)
    // 3. Store returned old value and final memory value for verification

    let mut instructions = vec![
        // Setup: x1 = DRAM_BASE (memory address)
        lui(1, DRAM_BASE),
        // Store initial value
        addi(2, 0, 42),
        sw(1, 2, 0), // mem[x1+0] = 42
        // Atomic swap
        addi(3, 0, 100),    // x3 = 100 (new value)
        amoswap_w(4, 1, 3), // x4 = mem[x1] (42), mem[x1] = 100
        // Store results for host verification
        sw(1, 4, 0x100), // mem[x1+0x100] = x4 (old value, should be 42)
        lw(5, 1, 0),     // x5 = mem[x1+0] (should be 100)
        sw(1, 5, 0x104), // mem[x1+0x104] = x5 (new value)
    ];
    instructions.extend(tohost_termination(7, 8, SUCCESS_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
    // AMOSWAP must return the original value (42)
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 0x100, SHORT_TIMEOUT),
        42,
        "AMOSWAP should return old value 42"
    );
    // Memory must contain the new value (100)
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 0x104, SHORT_TIMEOUT),
        100,
        "Memory should contain new value 100 after AMOSWAP"
    );
}

#[test]
fn test_cpu_amoadd() {
    let mut runtime = create_test_runtime();

    // Program: Atomic add operation (atomic counter)
    // 1. Store initial counter value 10 to DRAM_BASE
    // 2. Atomic add 5: returns old value (10), writes new value (15)
    // 3. Store returned old value and final memory value for verification

    let mut instructions = vec![
        // Setup: x1 = DRAM_BASE (memory address)
        lui(1, DRAM_BASE),
        // Store initial value
        addi(2, 0, 10),
        sw(1, 2, 0), // mem[x1+0] = 10
        // Atomic add
        addi(3, 0, 5),     // x3 = 5
        amoadd_w(4, 1, 3), // x4 = mem[x1] (10), mem[x1] = 10 + 5 = 15
        // Store results for host verification
        sw(1, 4, 0x100), // mem[x1+0x100] = x4 (old value, should be 10)
        lw(5, 1, 0),     // x5 = mem[x1+0] (should be 15)
        sw(1, 5, 0x104), // mem[x1+0x104] = x5 (new value)
    ];
    instructions.extend(tohost_termination(7, 8, SUCCESS_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
    // AMOADD must return the original value (10)
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 0x100, SHORT_TIMEOUT),
        10,
        "AMOADD should return old value 10"
    );
    // Memory must contain the sum (15)
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 0x104, SHORT_TIMEOUT),
        15,
        "Memory should contain 15 after AMOADD"
    );
}

#[test]
fn test_cpu_amo_logical() {
    let mut runtime = create_test_runtime();

    // Program: Test AMOXOR, AMOAND, AMOOR
    // All operate on the same memory location with different values.
    // Store each returned old value and final result for verification.

    let mut instructions = vec![
        // Setup: x1 = DRAM_BASE (memory address)
        lui(1, DRAM_BASE),
        // Test AMOXOR: mem = 0xFF, xor with 0x0F -> old=0xFF, mem = 0xF0
        addi(2, 0, 0xFF),
        sw(1, 2, 0), // mem[x1+0] = 0xFF
        addi(3, 0, 0x0F),
        amoxor_w(4, 1, 3), // x4 = 0xFF (old), mem[x1] = 0xF0
        sw(1, 4, 0x100),   // store old value of AMOXOR
        // Test AMOAND: mem = 0xF0, and with 0x3C -> old=0xF0, mem = 0x30
        addi(5, 0, 0x3C),
        amoand_w(6, 1, 5), // x6 = 0xF0 (old), mem[x1] = 0x30
        sw(1, 6, 0x104),   // store old value of AMOAND
        // Test AMOOR: mem = 0x30, or with 0x0F -> old=0x30, mem = 0x3F
        addi(7, 0, 0x0F),
        amoor_w(8, 1, 7), // x8 = 0x30 (old), mem[x1] = 0x3F
        sw(1, 8, 0x108),  // store old value of AMOOR
        // Load final value
        lw(9, 1, 0),     // x9 = mem[x1] (should be 0x3F)
        sw(1, 9, 0x10C), // store final value
    ];
    instructions.extend(tohost_termination(10, 11, SUCCESS_CODE));

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
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 0x100, SHORT_TIMEOUT),
        0xFF,
        "AMOXOR should return old value 0xFF"
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 0x104, SHORT_TIMEOUT),
        0xF0,
        "AMOAND should return old value 0xF0"
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 0x108, SHORT_TIMEOUT),
        0x30,
        "AMOOR should return old value 0x30"
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 0x10C, SHORT_TIMEOUT),
        0x3F,
        "Final memory value should be 0x3F after AMOXOR+AMOAND+AMOOR"
    );
}

#[test]
fn test_cpu_amo_min_max() {
    let mut runtime = create_test_runtime();

    // Program: Test AMOMIN, AMOMAX (signed)
    // Store returned old values and final result for verification.

    let mut instructions = vec![
        // Setup: x1 = DRAM_BASE (memory address)
        lui(1, DRAM_BASE),
        // Test AMOMIN: mem = 20, min with 15 -> old=20, mem = 15
        addi(2, 0, 20),
        sw(1, 2, 0), // mem[x1+0] = 20
        addi(3, 0, 15),
        amomin_w(4, 1, 3), // x4 = 20 (old), mem[x1] = 15
        sw(1, 4, 0x100),   // store old value of AMOMIN
        // Test AMOMAX: mem = 15, max with 25 -> old=15, mem = 25
        addi(5, 0, 25),
        amomax_w(6, 1, 5), // x6 = 15 (old), mem[x1] = 25
        sw(1, 6, 0x104),   // store old value of AMOMAX
        // Load final value
        lw(7, 1, 0),     // x7 = mem[x1] (should be 25)
        sw(1, 7, 0x108), // store final value
    ];
    instructions.extend(tohost_termination(10, 11, SUCCESS_CODE));

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
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 0x100, SHORT_TIMEOUT),
        20,
        "AMOMIN should return old value 20"
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 0x104, SHORT_TIMEOUT),
        15,
        "AMOMAX should return old value 15"
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 0x108, SHORT_TIMEOUT),
        25,
        "Final memory value should be 25 after AMOMIN+AMOMAX"
    );
}

#[test]
fn test_cpu_amo_unsigned_min_max() {
    let mut runtime = create_test_runtime();

    // Program: Test AMOMINU, AMOMAXU (unsigned)
    // Store returned old values and final result for verification.

    let mut instructions = vec![
        // Setup: x1 = DRAM_BASE (memory address)
        lui(1, DRAM_BASE),
        // Test AMOMINU: mem = 100, minu with 50 -> old=100, mem = 50
        addi(2, 0, 100),
        sw(1, 2, 0), // mem[x1+0] = 100
        addi(3, 0, 50),
        amominu_w(4, 1, 3), // x4 = 100 (old), mem[x1] = 50
        sw(1, 4, 0x100),    // store old value of AMOMINU
        // Test AMOMAXU: mem = 50, maxu with 75 -> old=50, mem = 75
        addi(5, 0, 75),
        amomaxu_w(6, 1, 5), // x6 = 50 (old), mem[x1] = 75
        sw(1, 6, 0x104),    // store old value of AMOMAXU
        // Load final value
        lw(7, 1, 0),     // x7 = mem[x1] (should be 75)
        sw(1, 7, 0x108), // store final value
    ];
    instructions.extend(tohost_termination(10, 11, SUCCESS_CODE));

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
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 0x100, SHORT_TIMEOUT),
        100,
        "AMOMINU should return old value 100"
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 0x104, SHORT_TIMEOUT),
        50,
        "AMOMAXU should return old value 50"
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 0x108, SHORT_TIMEOUT),
        75,
        "Final memory value should be 75 after AMOMINU+AMOMAXU"
    );
}

#[test]
fn test_cpu_amo_min_max_signed_negative_values() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![
        lui(1, DRAM_BASE),
        addi(2, 0, -5),
        sw(1, 2, 0), // mem[x1] = -5
        addi(3, 0, 3),
        amomin_w(4, 1, 3), // x4 = -5, mem[x1] stays -5
        sw(1, 4, 0x100),
        amomax_w(5, 1, 3), // x5 = -5, mem[x1] becomes 3
        sw(1, 5, 0x104),
        lw(6, 1, 0),
        sw(1, 6, 0x108),
    ];
    instructions.extend(tohost_termination(10, 11, SUCCESS_CODE));

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
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 0x100, SHORT_TIMEOUT),
        0xFFFF_FFFB,
        "AMOMIN should return the original negative value"
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 0x104, SHORT_TIMEOUT),
        0xFFFF_FFFB,
        "AMOMAX should return the original negative value before replacing it"
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 0x108, SHORT_TIMEOUT),
        3,
        "AMOMAX should replace the negative value with the larger positive operand"
    );
}

// ============================================================================
// Invalid Instruction Tests
// ============================================================================

/// Test that CPU halts when fetching an instruction value of 0
///
/// When memory returns 0x0000, the decompressor identifies this as an invalid
/// compressed instruction (C.ADDI4SPN with nzuimm=0), sets is_valid=0.
/// The CPU should transition to S_HALT state when it detects this.
#[test]
fn test_cpu_halts_on_zero_instruction() {
    let mut runtime = create_test_runtime();

    // Load 16 zero bytes (four zero words = four invalid compressed instructions)
    // The CPU should halt when it fetches 0x0000
    let program_bytes: Vec<u8> = vec![0u8; 16];

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program_bytes);

    // The CPU enters S_HALT on the invalid instruction.
    // wait_for_cpu_halt returns None because the program never writes to tohost.
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        None,
        "Expected CPU to halt without writing to tohost on zero instruction"
    );
}
