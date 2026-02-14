//! RV32C Compressed Instruction Extension Tests

mod common;

use riscv_core::instruction::*;
use riscv_shared::sim_control::{FAILURE_CODE, SUCCESS_CODE};

fn emit_compressed(program: &mut Vec<u8>, insn: u16) {
    program.extend(insn.to_le_bytes());
}

fn emit_standard(program: &mut Vec<u8>, insn: u32) {
    program.extend(insn.to_le_bytes());
}

fn append_tohost(program: &mut Vec<u8>, value: u32) {
    for insn in common::tohost_termination(31, 30, value) {
        emit_standard(program, insn);
    }
}

fn append_result_check(program: &mut Vec<u8>, result_reg: u32, expected: i32) {
    emit_standard(program, addi(29, 0, expected));
    emit_standard(program, bne(result_reg, 29, 16));
    append_tohost(program, SUCCESS_CODE);
    append_tohost(program, FAILURE_CODE);
}

fn run_program(program: Vec<u8>) {
    let mut runtime = common::create_test_runtime();
    common::load_and_boot(runtime.as_mut(), common::DEFAULT_BOOT_PC, &program);
    assert_eq!(
        common::wait_for_tohost(runtime.as_mut(), common::MEDIUM_TIMEOUT),
        SUCCESS_CODE
    );
}

#[test]
fn test_c_li() {
    let mut program = Vec::new();
    emit_compressed(&mut program, c_li(10, 5));
    append_result_check(&mut program, 10, 5);
    run_program(program);
}

#[test]
fn test_c_addi() {
    let mut program = Vec::new();
    emit_compressed(&mut program, c_li(10, 10));
    emit_compressed(&mut program, c_addi(10, 5));
    append_result_check(&mut program, 10, 15);
    run_program(program);
}

#[test]
fn test_c_add() {
    let mut program = Vec::new();
    emit_compressed(&mut program, c_li(10, 7));
    emit_compressed(&mut program, c_li(11, 3));
    emit_compressed(&mut program, c_add(10, 11));
    append_result_check(&mut program, 10, 10);
    run_program(program);
}

#[test]
fn test_c_mv() {
    let mut program = Vec::new();
    emit_standard(&mut program, addi(11, 0, 42));
    emit_compressed(&mut program, c_mv(10, 11));
    append_result_check(&mut program, 10, 42);
    run_program(program);
}

#[test]
fn test_compressed_to_compressed_transition() {
    let mut program = Vec::new();
    emit_compressed(&mut program, c_li(10, 1));
    emit_compressed(&mut program, c_addi(10, 2));
    emit_compressed(&mut program, c_addi(10, 3));
    emit_compressed(&mut program, c_addi(10, 4));
    append_result_check(&mut program, 10, 10);
    run_program(program);
}

#[test]
fn test_compressed_to_uncompressed_transition() {
    let mut program = Vec::new();
    emit_compressed(&mut program, c_li(10, 5));
    emit_standard(&mut program, addi(10, 10, 10));
    append_result_check(&mut program, 10, 15);
    run_program(program);
}

#[test]
fn test_uncompressed_to_compressed_transition() {
    let mut program = Vec::new();
    emit_standard(&mut program, addi(10, 0, 5));
    emit_compressed(&mut program, c_addi(10, 10));
    append_result_check(&mut program, 10, 15);
    run_program(program);
}

#[test]
fn test_uncompressed_to_uncompressed_regression() {
    let mut program = Vec::new();
    emit_standard(&mut program, addi(10, 0, 5));
    emit_standard(&mut program, addi(11, 0, 3));
    emit_standard(&mut program, add(12, 10, 11));
    append_result_check(&mut program, 12, 8);
    run_program(program);
}

#[test]
fn test_mixed_sequence_across_word_boundary() {
    let mut program = Vec::new();
    emit_compressed(&mut program, c_li(10, 1));
    emit_compressed(&mut program, c_addi(10, 2));
    emit_standard(&mut program, addi(10, 10, 4));
    emit_compressed(&mut program, c_addi(10, 8));
    append_result_check(&mut program, 10, 15);
    run_program(program);
}
