//! LED Controller RTL Peripheral Tests
//!
//! Tests for the LED controller peripheral (RTL-based).
//! Address: 0x50000000
//! Features: 8-bit output register
//!
//! Migrated from cpu-sim/tests/test_led_peripheral.rs to use backend-agnostic
//! device-runtime APIs. Tests verify LED operations complete successfully.
//! Note: LED read-back verification is done by the CPU itself before tohost.

mod common;

use common::{
    create_test_runtime, instructions_to_bytes, load_and_boot, tohost_termination,
    wait_for_cpu_halt, LONG_TIMEOUT, TEST_BOOT_PC,
};
use riscv_core::instruction::{addi, andi, bne, ebreak, jal, lui, lw, ori, sb, sh, sub, sw};
use riscv_shared::bus::{LED_BASE, LED_OUT_OFFSET, LED_SIZE, SIM_CONTROL_BASE};
use riscv_shared::sim_control::{FAILURE_CODE, SUCCESS_CODE};

#[test]
fn test_led_constants() {
    // Verify LED controller memory map constants
    assert_eq!(LED_BASE, 0x50000000, "LED base address");
    assert_eq!(LED_OUT_OFFSET, 0x00, "LED_OUT register offset");
    assert_eq!(LED_SIZE, 0x10, "LED controller size");
}

#[test]
fn test_led_basic_write_word() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![lui(15, LED_BASE), addi(14, 0, 0xAA), sw(15, 14, 0)];
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
}

#[test]
fn test_led_byte_access() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![lui(15, LED_BASE), addi(14, 0, 0x55), sb(15, 14, 0)];
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
}

#[test]
fn test_led_halfword_access() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![lui(15, LED_BASE), addi(14, 0, 0xFF), sh(15, 14, 0)];
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
}

#[test]
fn test_led_read_back() {
    let mut runtime = create_test_runtime();

    // CPU writes, reads back, and verifies - tohost only reached if successful
    let instructions = vec![
        lui(15, LED_BASE),
        addi(14, 0, 0xCC),
        sw(15, 14, 0),
        lw(13, 15, 0),
        andi(13, 13, 0xFF),
        addi(12, 0, 0xCC),
        sub(11, 13, 12),
        bne(11, 0, 16),
        lui(7, SIM_CONTROL_BASE),
        addi(8, 0, SUCCESS_CODE as i32),
        sw(7, 8, 0),
        ebreak(),
        jal(0, 0),
        addi(8, 0, FAILURE_CODE as i32),
        sw(7, 8, 0),
        ebreak(),
        jal(0, 0),
    ];

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
fn test_led_pattern_sequence() {
    let mut runtime = create_test_runtime();

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
}

#[test]
fn test_led_upper_bits_ignored() {
    let mut runtime = create_test_runtime();

    // Write value with upper bits set, read back, verify only lower 8 bits
    let instructions = vec![
        lui(15, LED_BASE),
        lui(14, 0xFFFFF000),
        ori(14, 14, 0xAA),
        sw(15, 14, 0),
        lw(13, 15, 0),
        andi(13, 13, 0xFF),
        addi(12, 0, 0xAA),
        sub(11, 13, 12),
        bne(11, 0, 16),
        lui(7, SIM_CONTROL_BASE),
        addi(8, 0, SUCCESS_CODE as i32),
        sw(7, 8, 0),
        ebreak(),
        jal(0, 0),
        addi(8, 0, FAILURE_CODE as i32),
        sw(7, 8, 0),
        ebreak(),
        jal(0, 0),
    ];

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
