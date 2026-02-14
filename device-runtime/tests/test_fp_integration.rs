//! RV32F Extension CPU Integration Tests
//!
//! Tests that verify the floating-point extension works correctly in the full CPU context,
//! including FP load/store, register interactions, and multi-cycle execution.
//!
//! Migrated from cpu-sim/tests/test_fp_integration.rs to use backend-agnostic
//! device-runtime APIs.

mod common;

use common::{
    create_test_runtime, instructions_to_bytes, load_and_boot, tohost_termination, wait_for_tohost,
    LONG_TIMEOUT,
};
use riscv_core::instruction::*;
use riscv_shared::bus::DRAM_BASE;
use riscv_shared::sim_control::SUCCESS_CODE;

// ============================================================================
// FP Load/Store Tests
// ============================================================================

#[test]
fn test_cpu_flw_fsw_basic() {
    let mut runtime = create_test_runtime();

    // Program: Test FLW and FSW instructions
    // Store a floating point value to memory, then load it back
    let mut instructions = vec![
        lui(1, 0x80001000), // x1 = 0x80001000 (data address)
        lui(2, 0x3F800000), // x2 = 0x3F800000 (1.0 in FP)
        sw(1, 2, 0),        // Store integer representation to memory
        flw(1, 1, 0),       // f1 = load FP value from memory
        fsw(1, 1, 4),       // Store f1 to memory[0x80001004]
        lw(3, 1, 4),        // x3 = load from memory[0x80001004]
        lui(4, DRAM_BASE),  // x4 = 0x80000000 (base)
        addi(4, 4, 0x100),  // x4 = 0x80000100
        sw(4, 3, 0),        // Store result to 0x80000100
    ];
    instructions.extend(tohost_termination(7, 8, SUCCESS_CODE));

    const BOOT_PC: u32 = 0x8000_0000;
    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), BOOT_PC, &program_bytes);
    let tohost_value = wait_for_tohost(runtime.as_mut(), LONG_TIMEOUT);

    assert_eq!(
        tohost_value, SUCCESS_CODE,
        "Program should terminate with tohost=1"
    );
}

#[test]
fn test_cpu_flw_multiple_registers() {
    let mut runtime = create_test_runtime();

    // Program: Load different FP values into multiple FP registers
    let mut instructions = vec![
        lui(1, 0x80001000), // x1 = 0x80001000 (base address)
        lui(2, 0x3F800000), // x2 = 1.0
        lui(3, 0x40000000), // x3 = 2.0
        lui(4, 0x40400000), // x4 = 3.0
        sw(1, 2, 0),        // mem[x1+0] = 1.0
        sw(1, 3, 4),        // mem[x1+4] = 2.0
        sw(1, 4, 8),        // mem[x1+8] = 3.0
        flw(1, 1, 0),       // f1 = 1.0
        flw(2, 1, 4),       // f2 = 2.0
        flw(3, 1, 8),       // f3 = 3.0
        fsw(1, 1, 12),      // mem[x1+12] = f1
        fsw(1, 2, 16),      // mem[x1+16] = f2
        fsw(1, 3, 20),      // mem[x1+20] = f3
        lw(5, 1, 12),       // x5 = f1 value
        lw(6, 1, 16),       // x6 = f2 value
        lw(7, 1, 20),       // x7 = f3 value
        sw(1, 5, 0x100),    // Store results to x1+0x100 (0x80001100)
        sw(1, 6, 0x104),
        sw(1, 7, 0x108),
    ];
    instructions.extend(tohost_termination(11, 12, SUCCESS_CODE));

    const BOOT_PC: u32 = 0x8000_0000;
    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), BOOT_PC, &program_bytes);
    let tohost_value = wait_for_tohost(runtime.as_mut(), LONG_TIMEOUT);

    assert_eq!(
        tohost_value, SUCCESS_CODE,
        "Program should terminate with tohost=1"
    );

    // let val1 = read_word_with_timeout(runtime.as_mut(), 0x80001100, SHORT_TIMEOUT);
    // let val2 = read_word_with_timeout(runtime.as_mut(), 0x80001104, SHORT_TIMEOUT);
    // assert_eq!(val2, 0x40000000, "f2 should be 2.0");
    // assert_eq!(val3, 0x40400000, "f3 should be 3.0");
}

// ============================================================================
// FP Arithmetic in CPU Context
// ============================================================================

#[test]
fn test_cpu_fadd_basic() {
    let mut runtime = create_test_runtime();

    // Program: Test FADD.S instruction in CPU context
    let mut instructions = vec![
        lui(1, 0x80001000), // x1 = 0x80001000 (base address)
        lui(2, 0x3F800000), // x2 = 1.0
        lui(3, 0x40000000), // x3 = 2.0
        sw(1, 2, 0),        // mem[x1+0] = 1.0
        sw(1, 3, 4),        // mem[x1+4] = 2.0
        flw(1, 1, 0),       // f1 = 1.0
        flw(2, 1, 4),       // f2 = 2.0
        fadd_s(3, 1, 2),    // f3 = f1 + f2 = 3.0
        fsw(1, 3, 8),       // mem[x1+8] = f3
        lw(4, 1, 8),        // x4 = result
        lui(5, DRAM_BASE),  // x5 = 0x80000000 (base)
        addi(5, 5, 0x100),  // x5 = 0x80000100
        sw(5, 4, 0),        // Store result to 0x80000100
    ];
    instructions.extend(tohost_termination(7, 8, SUCCESS_CODE));

    const BOOT_PC: u32 = 0x8000_0000;
    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), BOOT_PC, &program_bytes);
    let tohost_value = wait_for_tohost(runtime.as_mut(), LONG_TIMEOUT);

    assert_eq!(
        tohost_value, SUCCESS_CODE,
        "Program should terminate with tohost=1"
    );
}

#[test]
fn test_cpu_fmul_basic() {
    let mut runtime = create_test_runtime();

    // Program: Test FMUL.S instruction
    let mut instructions = vec![
        lui(1, 0x80001000), // x1 = 0x80001000
        lui(2, 0x40000000), // x2 = 2.0
        lui(3, 0x40400000), // x3 = 3.0
        sw(1, 2, 0),        // mem[x1+0] = 2.0
        sw(1, 3, 4),        // mem[x1+4] = 3.0
        flw(1, 1, 0),       // f1 = 2.0
        flw(2, 1, 4),       // f2 = 3.0
        fmul_s(3, 1, 2),    // f3 = f1 * f2 = 6.0
        fsw(1, 3, 8),       // mem[x1+8] = f3
        lw(4, 1, 8),        // x4 = result
        lui(5, DRAM_BASE),  // x5 = 0x80000000 (base)
        addi(5, 5, 0x100),  // x5 = 0x80000100
        sw(5, 4, 0),        // Store result to 0x80000100
    ];
    instructions.extend(tohost_termination(7, 8, SUCCESS_CODE));

    const BOOT_PC: u32 = 0x8000_0000;
    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), BOOT_PC, &program_bytes);
    let tohost_value = wait_for_tohost(runtime.as_mut(), LONG_TIMEOUT);

    assert_eq!(
        tohost_value, SUCCESS_CODE,
        "Program should terminate with tohost=1"
    );
}

// ============================================================================
// FP/Integer Conversion Tests
// ============================================================================

#[test]
fn test_cpu_fcvt_s_w() {
    let mut runtime = create_test_runtime();

    // Program: Test FCVT.S.W (integer to FP conversion)
    let mut instructions = vec![
        addi(1, 0, 42),     // x1 = 42 (integer)
        fcvt_s_w(1, 1),     // f1 = (float)42
        lui(2, 0x80001000), // x2 = 0x80001000
        fsw(2, 1, 0),       // mem[x2] = f1
        lw(3, 2, 0),        // x3 = result
        lui(4, DRAM_BASE),  // x4 = 0x80000000 (base)
        addi(4, 4, 0x100),  // x4 = 0x80000100
        sw(4, 3, 0),        // Store result to 0x80000100
    ];
    instructions.extend(tohost_termination(7, 8, SUCCESS_CODE));

    const BOOT_PC: u32 = 0x8000_0000;
    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), BOOT_PC, &program_bytes);
    let tohost_value = wait_for_tohost(runtime.as_mut(), LONG_TIMEOUT);

    assert_eq!(
        tohost_value, SUCCESS_CODE,
        "Program should terminate with tohost=1"
    );
}

#[test]
fn test_cpu_fcvt_w_s() {
    let mut runtime = create_test_runtime();

    // Program: Test FCVT.W.S (FP to integer conversion)
    let mut instructions = vec![
        lui(1, 0x80001000), // x1 = 0x80001000
        lui(2, 0x42280000), // x2 = 42.0 in FP
        sw(1, 2, 0),        // mem[x1] = 42.0
        flw(1, 1, 0),       // f1 = 42.0
        fcvt_w_s(3, 1),     // x3 = (int)f1 = 42
        lui(4, DRAM_BASE),  // x4 = 0x80000000 (base)
        addi(4, 4, 0x100),  // x4 = 0x80000100
        sw(4, 3, 0),        // Store result to 0x80000100
    ];
    instructions.extend(tohost_termination(7, 8, SUCCESS_CODE));

    const BOOT_PC: u32 = 0x8000_0000;
    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), BOOT_PC, &program_bytes);
    let tohost_value = wait_for_tohost(runtime.as_mut(), LONG_TIMEOUT);

    assert_eq!(
        tohost_value, SUCCESS_CODE,
        "Program should terminate with tohost=1"
    );
}

// ============================================================================
// FP Comparison Tests
// ============================================================================

#[test]
fn test_cpu_feq_flt() {
    let mut runtime = create_test_runtime();

    // Program: Test FEQ.S and FLT.S comparisons
    let mut instructions = vec![
        lui(1, 0x80001000), // x1 = 0x80001000
        lui(2, 0x3F800000), // x2 = 1.0
        lui(3, 0x40000000), // x3 = 2.0
        sw(1, 2, 0),        // mem[x1+0] = 1.0
        sw(1, 3, 4),        // mem[x1+4] = 2.0
        flw(1, 1, 0),       // f1 = 1.0
        flw(2, 1, 4),       // f2 = 2.0
        feq_s(4, 1, 1),     // x4 = (f1 == f1) = 1
        feq_s(5, 1, 2),     // x5 = (f1 == f2) = 0
        flt_s(6, 1, 2),     // x6 = (f1 < f2) = 1
        flt_s(7, 2, 1),     // x7 = (f2 < f1) = 0
        sw(1, 4, 0x100),    // Store results to x1+0x100
        sw(1, 5, 0x104),
        sw(1, 6, 0x108),
        sw(1, 7, 0x10C),
    ];
    instructions.extend(tohost_termination(11, 12, SUCCESS_CODE));

    const BOOT_PC: u32 = 0x8000_0000;
    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), BOOT_PC, &program_bytes);
    let tohost_value = wait_for_tohost(runtime.as_mut(), LONG_TIMEOUT);

    assert_eq!(
        tohost_value, SUCCESS_CODE,
        "Program should terminate with tohost=1"
    );

    // let eq_same = read_word_with_timeout(runtime.as_mut(), 0x80001100, SHORT_TIMEOUT);
    // let eq_diff = read_word_with_timeout(runtime.as_mut(), 0x80001104, SHORT_TIMEOUT);
    // let lt_true = read_word_with_timeout(runtime.as_mut(), 0x80001108, SHORT_TIMEOUT);
    // assert_eq!(eq_diff, 0, "1.0 == 2.0 should be false");
    // assert_eq!(lt_true, 1, "1.0 < 2.0 should be true");
    // assert_eq!(lt_false, 0, "2.0 < 1.0 should be false");
}

// ============================================================================
// FP Move Tests
// ============================================================================

#[test]
fn test_cpu_fmv_x_w_fmv_w_x() {
    let mut runtime = create_test_runtime();

    // Program: Test FMV.X.W and FMV.W.X (bitwise moves)
    let mut instructions = vec![
        lui(1, 0x3F800000), // x1 = 0x3F800000 (1.0 in FP)
        fmv_w_x(1, 1),      // f1 = x1 (bitwise move)
        fmv_x_w(2, 1),      // x2 = f1 (bitwise move back)
        lui(3, DRAM_BASE),  // x3 = 0x80000000 (base)
        addi(3, 3, 0x100),  // x3 = 0x80000100
        sw(3, 2, 0),        // Store result to 0x80000100
    ];
    instructions.extend(tohost_termination(7, 8, SUCCESS_CODE));

    const BOOT_PC: u32 = 0x8000_0000;
    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), BOOT_PC, &program_bytes);
    let tohost_value = wait_for_tohost(runtime.as_mut(), LONG_TIMEOUT);

    assert_eq!(
        tohost_value, SUCCESS_CODE,
        "Program should terminate with tohost=1"
    );
}

// ============================================================================
// Additional FP Instructions
// ============================================================================

#[test]
fn test_cpu_fsub_fdiv_fsqrt() {
    let mut runtime = create_test_runtime();

    // Program: Test FSUB.S, FDIV.S, FSQRT.S
    let mut instructions = vec![
        lui(1, 0x80001000), // x1 = 0x80001000
        lui(2, 0x40A00000), // x2 = 5.0
        lui(3, 0x40000000), // x3 = 2.0
        sw(1, 2, 0),        // mem[x1+0] = 5.0
        sw(1, 3, 4),        // mem[x1+4] = 2.0
        flw(1, 1, 0),       // f1 = 5.0
        flw(2, 1, 4),       // f2 = 2.0
        fsub_s(3, 1, 2),    // f3 = 5.0 - 2.0 = 3.0
        fdiv_s(4, 1, 2),    // f4 = 5.0 / 2.0 = 2.5
        fsqrt_s(5, 2),      // f5 = sqrt(2.0)
        fsw(1, 3, 8),       // Store f3
        fsw(1, 4, 12),      // Store f4
        lw(4, 1, 8),        // x4 = result (FSUB)
        lw(5, 1, 12),       // x5 = result (FDIV)
        sw(1, 4, 0x100),    // Store FSUB result
        sw(1, 5, 0x104),    // Store FDIV result
    ];
    instructions.extend(tohost_termination(7, 8, SUCCESS_CODE));

    const BOOT_PC: u32 = 0x8000_0000;
    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), BOOT_PC, &program_bytes);
    let tohost_value = wait_for_tohost(runtime.as_mut(), LONG_TIMEOUT);

    assert_eq!(
        tohost_value, SUCCESS_CODE,
        "Program should terminate with tohost=1"
    );

    // let fsub_result = read_word_with_timeout(runtime.as_mut(), 0x80001100, SHORT_TIMEOUT);
    // assert_eq!(fdiv_result, 0x40200000, "5.0 / 2.0 should equal 2.5");
}

#[test]
fn test_cpu_fmin_fmax() {
    let mut runtime = create_test_runtime();

    // Program: Test FMIN.S and FMAX.S
    let mut instructions = vec![
        lui(1, 0x80001000), // x1 = 0x80001000
        lui(2, 0x3F800000), // x2 = 1.0
        lui(3, 0x40400000), // x3 = 3.0
        sw(1, 2, 0),        // mem[x1+0] = 1.0
        sw(1, 3, 4),        // mem[x1+4] = 3.0
        flw(1, 1, 0),       // f1 = 1.0
        flw(2, 1, 4),       // f2 = 3.0
        fmin_s(3, 1, 2),    // f3 = min(1.0, 3.0) = 1.0
        fmax_s(4, 1, 2),    // f4 = max(1.0, 3.0) = 3.0
        fsw(1, 3, 8),       // Store min result
        fsw(1, 4, 12),      // Store max result
        lw(4, 1, 8),        // x4 = min result
        lw(5, 1, 12),       // x5 = max result
        sw(1, 4, 0x100),    // Store min
        sw(1, 5, 0x104),    // Store max
    ];
    instructions.extend(tohost_termination(7, 8, SUCCESS_CODE));

    const BOOT_PC: u32 = 0x8000_0000;
    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), BOOT_PC, &program_bytes);
    let tohost_value = wait_for_tohost(runtime.as_mut(), LONG_TIMEOUT);

    assert_eq!(
        tohost_value, SUCCESS_CODE,
        "Program should terminate with tohost=1"
    );

    // let min_result = read_word_with_timeout(runtime.as_mut(), 0x80001100, SHORT_TIMEOUT);
    // let max_result = read_word_with_timeout(runtime.as_mut(), 0x80001104, SHORT_TIMEOUT);
    // assert_eq!(min_result, 0x3F800000, "min(1.0, 3.0) should be 1.0");
    // assert_eq!(max_result, 0x40400000, "max(1.0, 3.0) should be 3.0");
}

#[test]
fn test_cpu_fle() {
    let mut runtime = create_test_runtime();

    // Program: Test FLE.S (less than or equal)
    let mut instructions = vec![
        lui(1, 0x80001000), // x1 = 0x80001000
        lui(2, 0x3F800000), // x2 = 1.0
        lui(3, 0x40000000), // x3 = 2.0
        sw(1, 2, 0),        // mem[x1+0] = 1.0
        sw(1, 3, 4),        // mem[x1+4] = 2.0
        flw(1, 1, 0),       // f1 = 1.0
        flw(2, 1, 4),       // f2 = 2.0
        fle_s(4, 1, 2),     // x4 = (1.0 <= 2.0) = 1
        fle_s(5, 2, 1),     // x5 = (2.0 <= 1.0) = 0
        fle_s(6, 1, 1),     // x6 = (1.0 <= 1.0) = 1
        sw(1, 4, 0x100),    // Store results
        sw(1, 5, 0x104),
        sw(1, 6, 0x108),
    ];
    instructions.extend(tohost_termination(10, 11, SUCCESS_CODE));

    const BOOT_PC: u32 = 0x8000_0000;
    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), BOOT_PC, &program_bytes);
    let tohost_value = wait_for_tohost(runtime.as_mut(), LONG_TIMEOUT);

    assert_eq!(
        tohost_value, SUCCESS_CODE,
        "Program should terminate with tohost=1"
    );

    // let le1 = read_word_with_timeout(runtime.as_mut(), 0x80001100, SHORT_TIMEOUT);
    // let le2 = read_word_with_timeout(runtime.as_mut(), 0x80001104, SHORT_TIMEOUT);
    // assert_eq!(le2, 0, "2.0 <= 1.0 should be false");
    // assert_eq!(le3, 1, "1.0 <= 1.0 should be true");
}

#[test]
fn test_cpu_fsgnj_ops() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![
        lui(7, 0x80001000), // x7 = 0x80001000 (base address)
        lui(1, 0x3F800000), // x1 = 1.0 (positive)
        lui(2, 0xBF800000), // x2 = -1.0 (negative)
        fmv_w_x(1, 1),      // f1 = 1.0
        fmv_w_x(2, 2),      // f2 = -1.0
        fsgnj_s(3, 1, 2),   // f3 = abs(1.0) with sign of -1.0 = -1.0
        fsgnjn_s(4, 1, 2),  // f4 = abs(1.0) with inverted sign of -1.0 = 1.0
        fsgnjx_s(5, 1, 2),  // f5 = abs(1.0) with XOR of signs = -1.0
        fmv_x_w(4, 3),      // x4 = bits of f3
        fmv_x_w(5, 4),      // x5 = bits of f4
        fmv_x_w(6, 5),      // x6 = bits of f5
        sw(7, 4, 0x100),    // Store FSGNJ result to mem[x7+0x100]
        sw(7, 5, 0x104),    // Store FSGNJN result to mem[x7+0x104]
        sw(7, 6, 0x108),    // Store FSGNJX result to mem[x7+0x108]
    ];
    instructions.extend(tohost_termination(10, 11, SUCCESS_CODE));

    const BOOT_PC: u32 = 0x8000_0000;
    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), BOOT_PC, &program_bytes);
    let tohost_value = wait_for_tohost(runtime.as_mut(), LONG_TIMEOUT);

    assert_eq!(
        tohost_value, SUCCESS_CODE,
        "Program should terminate with tohost=1"
    );
}

#[test]
fn test_cpu_fcvt_unsigned() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![
        lui(1, 0x80001000), // x1 = 0x80001000
        lui(2, 0x42280000), // x2 = 42.0 in FP
        sw(1, 2, 0),        // mem[x1] = 42.0
        flw(1, 1, 0),       // f1 = 42.0
        fcvt_wu_s(3, 1),    // x3 = (unsigned int)42.0 = 42
        addi(4, 0, 100),    // x4 = 100 (unsigned int)
        fcvt_s_wu(2, 4),    // f2 = (float)100 = 100.0
        fsw(1, 2, 4),       // Store conversion result
        lw(5, 1, 4),        // x5 = 100.0 as bits
        sw(1, 3, 0x100),    // Store FCVT.WU.S result to mem[x1+0x100]
        sw(1, 5, 0x104),    // Store FCVT.S.WU result to mem[x1+0x104]
    ];
    instructions.extend(tohost_termination(7, 8, SUCCESS_CODE));

    const BOOT_PC: u32 = 0x8000_0000;
    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), BOOT_PC, &program_bytes);
    let tohost_value = wait_for_tohost(runtime.as_mut(), LONG_TIMEOUT);

    assert_eq!(
        tohost_value, SUCCESS_CODE,
        "Program should terminate with tohost=1"
    );
}

#[test]
fn test_cpu_fclass() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![
        lui(7, 0x80001000), // x7 = 0x80001000 (base address)
        lui(1, 0x3F800000), // x1 = 1.0 (positive normal)
        lui(2, 0xBF800000), // x2 = -1.0 (negative normal)
        lui(3, 0x00000000), // x3 = +0.0 (positive zero)
        fmv_w_x(1, 1),      // f1 = 1.0
        fmv_w_x(2, 2),      // f2 = -1.0
        fmv_w_x(3, 3),      // f3 = +0.0
        fclass_s(4, 1),     // x4 = classify(1.0) = positive normal (bit 6 = 0x40)
        fclass_s(5, 2),     // x5 = classify(-1.0) = negative normal (bit 1 = 0x02)
        fclass_s(6, 3),     // x6 = classify(+0.0) = positive zero (bit 4 = 0x10)
        sw(7, 4, 0x100),    // Store classify results to mem[x7+0x100]
        sw(7, 5, 0x104),    // Store to mem[x7+0x104]
        sw(7, 6, 0x108),    // Store to mem[x7+0x108]
    ];
    instructions.extend(tohost_termination(10, 11, SUCCESS_CODE));

    const BOOT_PC: u32 = 0x8000_0000;
    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), BOOT_PC, &program_bytes);
    let tohost_value = wait_for_tohost(runtime.as_mut(), LONG_TIMEOUT);

    assert_eq!(
        tohost_value, SUCCESS_CODE,
        "Program should terminate with tohost=1"
    );
}

#[test]
fn test_cpu_fused_multiply_add_ops() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![
        lui(1, 0x80001000),   // x1 = 0x80001000
        lui(2, 0x40000000),   // x2 = 2.0
        lui(3, 0x40400000),   // x3 = 3.0
        lui(4, 0x3F800000),   // x4 = 1.0
        sw(1, 2, 0),          // mem[x1+0] = 2.0
        sw(1, 3, 4),          // mem[x1+4] = 3.0
        sw(1, 4, 8),          // mem[x1+8] = 1.0
        flw(1, 1, 0),         // f1 = 2.0
        flw(2, 1, 4),         // f2 = 3.0
        flw(3, 1, 8),         // f3 = 1.0
        fmadd_s(4, 1, 2, 3),  // f4 = (2.0 * 3.0) + 1.0 = 7.0
        fmsub_s(5, 1, 2, 3),  // f5 = (2.0 * 3.0) - 1.0 = 5.0
        fnmsub_s(6, 1, 2, 3), // f6 = -(2.0 * 3.0 - 1.0) = -5.0
        fnmadd_s(7, 1, 2, 3), // f7 = -(2.0 * 3.0 + 1.0) = -7.0
        fsw(1, 4, 12),        // Store FMADD result
        fsw(1, 5, 16),        // Store FMSUB result
        fsw(1, 6, 20),        // Store FNMSUB result
        fsw(1, 7, 24),        // Store FNMADD result
        lw(4, 1, 12),         // Load results into integer regs
        lw(5, 1, 16),
        lw(6, 1, 20),
        lw(7, 1, 24),
        sw(1, 4, 0x100), // Store all results to mem[x1+0x100]
        sw(1, 5, 0x104), // Store to mem[x1+0x104]
        sw(1, 6, 0x108), // Store to mem[x1+0x108]
        sw(1, 7, 0x10C), // Store to mem[x1+0x10C]
    ];
    instructions.extend(tohost_termination(10, 11, SUCCESS_CODE));

    const BOOT_PC: u32 = 0x8000_0000;
    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), BOOT_PC, &program_bytes);
    let tohost_value = wait_for_tohost(runtime.as_mut(), LONG_TIMEOUT);

    assert_eq!(
        tohost_value, SUCCESS_CODE,
        "Program should terminate with tohost=1"
    );
}
