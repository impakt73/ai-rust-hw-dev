//! RV32F Extension Device-Runtime Integration Tests
//!
//! Tests that verify the floating-point extension works correctly in the device runtime,
//! including FP load/store, register interactions, FCSR management, and multi-cycle execution.

mod common;

use riscv_core::instruction::*;
use riscv_shared::bus::SIM_CONTROL_BASE;
use riscv_shared::sim_control::{FAILURE_CODE, SUCCESS_CODE};

/// Helper to build a 32-bit expected value using LUI+ADDI and branch to success/failure paths.
/// This checks if the result register matches the expected value and branches accordingly.
///
/// # Arguments
/// * `instructions` - The instruction vector to append to
/// * `result_reg` - Register containing the value to check
/// * `expected` - Expected 32-bit value
fn append_value_check(instructions: &mut Vec<u32>, result_reg: u32, expected: u32) {
    let upper = (expected >> 12) & 0xFFFFF;
    let lower = (expected & 0xFFF) as i32;
    let lower_adjusted = if lower > 0x7FF { lower - 0x1000 } else { lower };

    instructions.extend([
        lui(29, upper << 12),
        addi(29, 29, (lower_adjusted as i16) as i32),
        beq(result_reg, 29, 20),
        lui(28, SIM_CONTROL_BASE),
        addi(27, 0, FAILURE_CODE as i32),
        sw(28, 27, 0),
        jal(0, 0),
    ]);
}

// ============================================================================
// FP Load/Store Tests
// ============================================================================

#[test]
fn test_cpu_flw_fsw_basic() {
    let mut instructions = vec![
        lui(1, 0x80001000), // x1 = 0x80001000 (data address)
        lui(2, 0x3F800000), // x2 = 0x3F800000 (1.0 in FP)
        sw(1, 2, 0),        // Store integer representation to memory
        flw(1, 1, 0),       // f1 = load FP value from memory
        fsw(1, 1, 4),       // Store f1 to memory[0x80001004]
        lw(3, 1, 4),        // x3 = load from memory[0x80001004]
    ];
    append_value_check(&mut instructions, 3, 0x3F800000);

    common::append_tohost_termination(&mut instructions, 28, 27, SUCCESS_CODE);

    let program = common::instructions_to_bytes(&instructions);
    let mut runtime = common::create_test_runtime();
    common::load_and_boot(runtime.as_mut(), common::DEFAULT_BOOT_PC, &program);
    assert_eq!(
        common::wait_for_tohost(runtime.as_mut(), common::MEDIUM_TIMEOUT),
        SUCCESS_CODE
    );
}

#[test]
fn test_cpu_flw_multiple_registers() {
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
    ];
    append_value_check(&mut instructions, 5, 0x3F800000);
    append_value_check(&mut instructions, 6, 0x40000000);
    append_value_check(&mut instructions, 7, 0x40400000);

    common::append_tohost_termination(&mut instructions, 28, 27, SUCCESS_CODE);

    let program = common::instructions_to_bytes(&instructions);
    let mut runtime = common::create_test_runtime();
    common::load_and_boot(runtime.as_mut(), common::DEFAULT_BOOT_PC, &program);
    assert_eq!(
        common::wait_for_tohost(runtime.as_mut(), common::MEDIUM_TIMEOUT),
        SUCCESS_CODE
    );
}

// ============================================================================
// FP Arithmetic in CPU Context
// ============================================================================

#[test]
fn test_cpu_fadd_basic() {
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
    ];
    append_value_check(&mut instructions, 4, 0x40400000);

    common::append_tohost_termination(&mut instructions, 28, 27, SUCCESS_CODE);

    let program = common::instructions_to_bytes(&instructions);
    let mut runtime = common::create_test_runtime();
    common::load_and_boot(runtime.as_mut(), common::DEFAULT_BOOT_PC, &program);
    assert_eq!(
        common::wait_for_tohost(runtime.as_mut(), common::MEDIUM_TIMEOUT),
        SUCCESS_CODE
    );
}

#[test]
fn test_cpu_fmul_basic() {
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
    ];
    append_value_check(&mut instructions, 4, 0x40C00000);

    common::append_tohost_termination(&mut instructions, 28, 27, SUCCESS_CODE);

    let program = common::instructions_to_bytes(&instructions);
    let mut runtime = common::create_test_runtime();
    common::load_and_boot(runtime.as_mut(), common::DEFAULT_BOOT_PC, &program);
    assert_eq!(
        common::wait_for_tohost(runtime.as_mut(), common::MEDIUM_TIMEOUT),
        SUCCESS_CODE
    );
}

// ============================================================================
// FP/Integer Conversion Tests
// ============================================================================

#[test]
fn test_cpu_fcvt_s_w() {
    let mut instructions = vec![
        addi(1, 0, 42),     // x1 = 42 (integer)
        fcvt_s_w(1, 1),     // f1 = (float)42
        lui(2, 0x80001000), // x2 = 0x80001000
        fsw(2, 1, 0),       // mem[x2] = f1
        lw(3, 2, 0),        // x3 = result
    ];
    append_value_check(&mut instructions, 3, 0x42280000);

    common::append_tohost_termination(&mut instructions, 28, 27, SUCCESS_CODE);

    let program = common::instructions_to_bytes(&instructions);
    let mut runtime = common::create_test_runtime();
    common::load_and_boot(runtime.as_mut(), common::DEFAULT_BOOT_PC, &program);
    assert_eq!(
        common::wait_for_tohost(runtime.as_mut(), common::MEDIUM_TIMEOUT),
        SUCCESS_CODE
    );
}

#[test]
fn test_cpu_fcvt_w_s() {
    let mut instructions = vec![
        lui(1, 0x80001000), // x1 = 0x80001000
        lui(2, 0x42280000), // x2 = 42.0 in FP (0x42280000)
        sw(1, 2, 0),        // mem[x1] = 42.0
        flw(1, 1, 0),       // f1 = 42.0
        fcvt_w_s(3, 1),     // x3 = (int)f1 = 42
    ];
    append_value_check(&mut instructions, 3, 42);

    common::append_tohost_termination(&mut instructions, 28, 27, SUCCESS_CODE);

    let program = common::instructions_to_bytes(&instructions);
    let mut runtime = common::create_test_runtime();
    common::load_and_boot(runtime.as_mut(), common::DEFAULT_BOOT_PC, &program);
    assert_eq!(
        common::wait_for_tohost(runtime.as_mut(), common::MEDIUM_TIMEOUT),
        SUCCESS_CODE
    );
}

// ============================================================================
// FP Comparison Tests
// ============================================================================

#[test]
fn test_cpu_feq_flt() {
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
    ];
    append_value_check(&mut instructions, 4, 1);
    append_value_check(&mut instructions, 5, 0);
    append_value_check(&mut instructions, 6, 1);
    append_value_check(&mut instructions, 7, 0);

    common::append_tohost_termination(&mut instructions, 28, 27, SUCCESS_CODE);

    let program = common::instructions_to_bytes(&instructions);
    let mut runtime = common::create_test_runtime();
    common::load_and_boot(runtime.as_mut(), common::DEFAULT_BOOT_PC, &program);
    assert_eq!(
        common::wait_for_tohost(runtime.as_mut(), common::MEDIUM_TIMEOUT),
        SUCCESS_CODE
    );
}

// ============================================================================
// FP Move Tests
// ============================================================================

#[test]
fn test_cpu_fmv_x_w_fmv_w_x() {
    let mut instructions = vec![
        lui(1, 0x3F800000), // x1 = 0x3F800000 (1.0 in FP)
        fmv_w_x(1, 1),      // f1 = x1 (bitwise move)
        fmv_x_w(2, 1),      // x2 = f1 (bitwise move back)
    ];
    append_value_check(&mut instructions, 2, 0x3F800000);

    common::append_tohost_termination(&mut instructions, 28, 27, SUCCESS_CODE);

    let program = common::instructions_to_bytes(&instructions);
    let mut runtime = common::create_test_runtime();
    common::load_and_boot(runtime.as_mut(), common::DEFAULT_BOOT_PC, &program);
    assert_eq!(
        common::wait_for_tohost(runtime.as_mut(), common::MEDIUM_TIMEOUT),
        SUCCESS_CODE
    );
}

// ============================================================================
// Comprehensive FP Instruction Coverage Tests
// ============================================================================

#[test]
fn test_cpu_fsub_fdiv_fsqrt() {
    let mut instructions = vec![
        lui(1, 0x80001000), // x1 = 0x80001000
        lui(2, 0x40A00000), // x2 = 5.0 (0x40A00000)
        lui(3, 0x40000000), // x3 = 2.0 (0x40000000)
        sw(1, 2, 0),        // mem[x1+0] = 5.0
        sw(1, 3, 4),        // mem[x1+4] = 2.0
        flw(1, 1, 0),       // f1 = 5.0
        flw(2, 1, 4),       // f2 = 2.0
        fsub_s(3, 1, 2),    // f3 = 5.0 - 2.0 = 3.0
        fdiv_s(4, 1, 2),    // f4 = 5.0 / 2.0 = 2.5
        fsqrt_s(5, 2),      // f5 = sqrt(2.0) ≈ 1.414...
        fsw(1, 3, 8),       // Store f3 to mem[x1+8]
        fsw(1, 4, 12),      // Store f4 to mem[x1+12]
        lw(4, 1, 8),        // x4 = result (FSUB)
        lw(5, 1, 12),       // x5 = result (FDIV)
    ];
    append_value_check(&mut instructions, 4, 0x40400000);
    append_value_check(&mut instructions, 5, 0x40200000);

    common::append_tohost_termination(&mut instructions, 28, 27, SUCCESS_CODE);

    let program = common::instructions_to_bytes(&instructions);
    let mut runtime = common::create_test_runtime();
    common::load_and_boot(runtime.as_mut(), common::DEFAULT_BOOT_PC, &program);
    assert_eq!(
        common::wait_for_tohost(runtime.as_mut(), common::MEDIUM_TIMEOUT),
        SUCCESS_CODE
    );
}

#[test]
fn test_cpu_fmin_fmax() {
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
    ];
    append_value_check(&mut instructions, 4, 0x3F800000);
    append_value_check(&mut instructions, 5, 0x40400000);

    common::append_tohost_termination(&mut instructions, 28, 27, SUCCESS_CODE);

    let program = common::instructions_to_bytes(&instructions);
    let mut runtime = common::create_test_runtime();
    common::load_and_boot(runtime.as_mut(), common::DEFAULT_BOOT_PC, &program);
    assert_eq!(
        common::wait_for_tohost(runtime.as_mut(), common::MEDIUM_TIMEOUT),
        SUCCESS_CODE
    );
}

#[test]
fn test_cpu_fsgnj_ops() {
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
    ];
    append_value_check(&mut instructions, 4, 0xBF800000);
    append_value_check(&mut instructions, 5, 0x3F800000);
    append_value_check(&mut instructions, 6, 0xBF800000);

    common::append_tohost_termination(&mut instructions, 28, 27, SUCCESS_CODE);

    let program = common::instructions_to_bytes(&instructions);
    let mut runtime = common::create_test_runtime();
    common::load_and_boot(runtime.as_mut(), common::DEFAULT_BOOT_PC, &program);
    assert_eq!(
        common::wait_for_tohost(runtime.as_mut(), common::MEDIUM_TIMEOUT),
        SUCCESS_CODE
    );
}

#[test]
fn test_cpu_fle() {
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
    ];
    append_value_check(&mut instructions, 4, 1);
    append_value_check(&mut instructions, 5, 0);
    append_value_check(&mut instructions, 6, 1);

    common::append_tohost_termination(&mut instructions, 28, 27, SUCCESS_CODE);

    let program = common::instructions_to_bytes(&instructions);
    let mut runtime = common::create_test_runtime();
    common::load_and_boot(runtime.as_mut(), common::DEFAULT_BOOT_PC, &program);
    assert_eq!(
        common::wait_for_tohost(runtime.as_mut(), common::MEDIUM_TIMEOUT),
        SUCCESS_CODE
    );
}

#[test]
fn test_cpu_fcvt_unsigned() {
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
    ];
    append_value_check(&mut instructions, 3, 42);
    append_value_check(&mut instructions, 5, 0x42C80000);

    common::append_tohost_termination(&mut instructions, 28, 27, SUCCESS_CODE);

    let program = common::instructions_to_bytes(&instructions);
    let mut runtime = common::create_test_runtime();
    common::load_and_boot(runtime.as_mut(), common::DEFAULT_BOOT_PC, &program);
    assert_eq!(
        common::wait_for_tohost(runtime.as_mut(), common::MEDIUM_TIMEOUT),
        SUCCESS_CODE
    );
}

#[test]
fn test_cpu_fclass() {
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
    ];
    append_value_check(&mut instructions, 4, 0x40);
    append_value_check(&mut instructions, 5, 0x02);
    append_value_check(&mut instructions, 6, 0x10);

    common::append_tohost_termination(&mut instructions, 28, 27, SUCCESS_CODE);

    let program = common::instructions_to_bytes(&instructions);
    let mut runtime = common::create_test_runtime();
    common::load_and_boot(runtime.as_mut(), common::DEFAULT_BOOT_PC, &program);
    assert_eq!(
        common::wait_for_tohost(runtime.as_mut(), common::MEDIUM_TIMEOUT),
        SUCCESS_CODE
    );
}

#[test]
fn test_cpu_fused_multiply_add_ops() {
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
    ];
    append_value_check(&mut instructions, 4, 0x40E00000);
    append_value_check(&mut instructions, 5, 0x40A00000);
    append_value_check(&mut instructions, 6, 0xC0A00000);
    append_value_check(&mut instructions, 7, 0xC0E00000);

    common::append_tohost_termination(&mut instructions, 28, 27, SUCCESS_CODE);

    let program = common::instructions_to_bytes(&instructions);
    let mut runtime = common::create_test_runtime();
    common::load_and_boot(runtime.as_mut(), common::DEFAULT_BOOT_PC, &program);
    assert_eq!(
        common::wait_for_tohost(runtime.as_mut(), common::MEDIUM_TIMEOUT),
        SUCCESS_CODE
    );
}
