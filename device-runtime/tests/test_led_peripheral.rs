//! LED Controller RTL Peripheral Tests

mod common;

use riscv_core::instruction::*;
use riscv_shared::bus::{LED_BASE, LED_OUT_OFFSET, LED_SIZE, SIM_CONTROL_BASE};
use riscv_shared::sim_control::{FAILURE_CODE, SUCCESS_CODE};

fn run_program(instructions: &[u32]) -> u32 {
    let mut runtime = common::create_test_runtime();
    let program_bytes = common::instructions_to_bytes(instructions);
    common::load_and_boot(runtime.as_mut(), common::DEFAULT_BOOT_PC, &program_bytes);
    common::wait_for_tohost(runtime.as_mut(), common::MEDIUM_TIMEOUT)
}

fn append_led_check(instructions: &mut Vec<u32>, expected: i32) {
    instructions.extend([
        lw(13, 15, 0),
        andi(13, 13, 0xFF),
        addi(12, 0, expected),
        bne(13, 12, 16),
        lui(11, SIM_CONTROL_BASE),
        addi(10, 0, SUCCESS_CODE as i32),
        sw(11, 10, 0),
        jal(0, 12),
        lui(11, SIM_CONTROL_BASE),
        addi(10, 0, FAILURE_CODE as i32),
        sw(11, 10, 0),
        jal(0, 0),
    ]);
}

#[test]
fn test_led_constants() {
    assert_eq!(LED_BASE, 0x50000000, "LED base address");
    assert_eq!(LED_OUT_OFFSET, 0x00, "LED_OUT register offset");
    assert_eq!(LED_SIZE, 0x10, "LED controller size");
}

#[test]
fn test_led_basic_write_word() {
    let mut instructions = vec![lui(15, LED_BASE), addi(14, 0, 0xAA), sw(15, 14, 0)];
    append_led_check(&mut instructions, 0xAA);
    assert_eq!(run_program(&instructions), SUCCESS_CODE);
}

#[test]
fn test_led_byte_access() {
    let mut instructions = vec![lui(15, LED_BASE), addi(14, 0, 0x55), sb(15, 14, 0)];
    append_led_check(&mut instructions, 0x55);
    assert_eq!(run_program(&instructions), SUCCESS_CODE);
}

#[test]
fn test_led_halfword_access() {
    let mut instructions = vec![lui(15, LED_BASE), addi(14, 0, 0xFF), sh(15, 14, 0)];
    append_led_check(&mut instructions, 0xFF);
    assert_eq!(run_program(&instructions), SUCCESS_CODE);
}

#[test]
fn test_led_read_back() {
    let mut instructions = vec![lui(15, LED_BASE), addi(14, 0, 0xCC), sw(15, 14, 0)];
    append_led_check(&mut instructions, 0xCC);
    assert_eq!(run_program(&instructions), SUCCESS_CODE);
}

#[test]
fn test_led_pattern_sequence() {
    let mut instructions = vec![
        lui(15, LED_BASE),
        addi(14, 0, 0x00),
        sw(15, 14, 0),
        addi(14, 0, 0xFF),
        sw(15, 14, 0),
        addi(14, 0, 0xAA),
        sw(15, 14, 0),
        addi(14, 0, 0x55),
        sw(15, 14, 0),
    ];
    append_led_check(&mut instructions, 0x55);
    assert_eq!(run_program(&instructions), SUCCESS_CODE);
}

#[test]
fn test_led_upper_bits_ignored() {
    let mut instructions = vec![
        lui(15, LED_BASE),
        lui(14, 0xFFFFF000),
        ori(14, 14, 0xAA),
        sw(15, 14, 0),
        lw(13, 15, 0),
        andi(13, 13, 0xFF),
        addi(12, 0, 0xAA),
    ];
    append_led_check(&mut instructions, 0xAA);
    assert_eq!(run_program(&instructions), SUCCESS_CODE);
}
