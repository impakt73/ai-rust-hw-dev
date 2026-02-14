//! UART Controller RTL Peripheral Tests (CPU-Level)

mod common;

use riscv_core::instruction::*;
use riscv_shared::bus::{
    uart_rxdata_addr, uart_status_addr, uart_txdata_addr, UART_BASE, UART_RXDATA_OFFSET,
    UART_STATUS_OFFSET, UART_STATUS_RX_EMPTY, UART_STATUS_TX_EMPTY, UART_STATUS_TX_FULL,
    UART_TXDATA_OFFSET,
};
use riscv_shared::sim_control::{FAILURE_CODE, SUCCESS_CODE};
use std::time::Duration;

fn run_program_and_wait(instructions: &[u32], timeout: Duration) -> u32 {
    let mut runtime = common::create_test_runtime();
    let program_bytes = common::instructions_to_bytes(instructions);
    common::load_and_boot(runtime.as_mut(), common::DEFAULT_BOOT_PC, &program_bytes);
    common::wait_for_tohost(runtime.as_mut(), timeout)
}

#[test]
fn test_uart_constants() {
    assert_eq!(UART_BASE, 0x52000000, "UART base address");
    assert_eq!(UART_TXDATA_OFFSET, 0x00, "UART_TXDATA register offset");
    assert_eq!(UART_RXDATA_OFFSET, 0x04, "UART_RXDATA register offset");
    assert_eq!(UART_STATUS_OFFSET, 0x08, "UART_STATUS register offset");
    assert_eq!(uart_txdata_addr(), 0x52000000, "UART TXDATA address");
    assert_eq!(uart_rxdata_addr(), 0x52000004, "UART RXDATA address");
    assert_eq!(uart_status_addr(), 0x52000008, "UART STATUS address");
    assert_eq!(UART_STATUS_TX_FULL, 1 << 0, "TX_FULL bit mask");
    assert_eq!(UART_STATUS_TX_EMPTY, 1 << 1, "TX_EMPTY bit mask");
    assert_eq!(UART_STATUS_RX_EMPTY, 1 << 5, "RX_EMPTY bit mask");
}

#[test]
fn test_uart_tx_write_byte() {
    let mut instructions = vec![
        lui(15, UART_BASE),
        addi(14, 0, 0x42),
        sw(15, 14, UART_TXDATA_OFFSET as i32),
    ];
    instructions.extend(common::tohost_termination(10, 9, SUCCESS_CODE));
    assert_eq!(
        run_program_and_wait(&instructions, common::MEDIUM_TIMEOUT),
        SUCCESS_CODE
    );
}

#[test]
fn test_uart_status_initial_state() {
    let mut instructions = vec![
        lui(15, UART_BASE),
        lw(13, 15, UART_STATUS_OFFSET as i32),
        andi(12, 13, UART_STATUS_TX_EMPTY as i32),
        beq(12, 0, 20),
        andi(12, 13, UART_STATUS_RX_EMPTY as i32),
        beq(12, 0, 12),
    ];
    instructions.extend(common::tohost_termination(10, 9, SUCCESS_CODE));
    instructions.extend(common::tohost_termination(10, 9, FAILURE_CODE));
    assert_eq!(
        run_program_and_wait(&instructions, common::MEDIUM_TIMEOUT),
        SUCCESS_CODE
    );
}

#[test]
fn test_uart_loopback_single_byte() {
    let instructions = vec![
        lui(15, UART_BASE),
        addi(14, 0, 0xA5),
        sw(15, 14, UART_TXDATA_OFFSET as i32),
        lw(13, 15, UART_STATUS_OFFSET as i32),
        andi(12, 13, UART_STATUS_TX_EMPTY as i32),
        beq(12, 0, -8),
        lw(13, 15, UART_STATUS_OFFSET as i32),
        andi(12, 13, UART_STATUS_RX_EMPTY as i32),
        bne(12, 0, -8),
        lw(11, 15, UART_RXDATA_OFFSET as i32),
        lui(10, riscv_shared::bus::SIM_CONTROL_BASE),
        bne(11, 14, 16),
        addi(9, 0, SUCCESS_CODE as i32),
        sw(10, 9, 0),
        jal(0, 12),
        addi(9, 0, FAILURE_CODE as i32),
        sw(10, 9, 0),
        jal(0, 0),
    ];
    assert_eq!(
        run_program_and_wait(&instructions, common::LONG_TIMEOUT),
        SUCCESS_CODE
    );
}

#[test]
fn test_uart_tx_fifo_full() {
    let mut instructions = vec![lui(15, UART_BASE)];
    for i in 0..8 {
        instructions.push(addi(14, 0, i));
        instructions.push(sw(15, 14, UART_TXDATA_OFFSET as i32));
    }
    instructions.push(lw(13, 15, UART_STATUS_OFFSET as i32));
    instructions.push(andi(12, 13, UART_STATUS_TX_EMPTY as i32));
    instructions.push(bne(12, 0, 12));
    instructions.extend(common::tohost_termination(10, 9, SUCCESS_CODE));
    instructions.extend(common::tohost_termination(10, 9, FAILURE_CODE));
    assert_eq!(
        run_program_and_wait(&instructions, common::MEDIUM_TIMEOUT),
        SUCCESS_CODE
    );
}

#[test]
fn test_uart_rx_read_empty() {
    let instructions = vec![
        lui(15, UART_BASE),
        lw(13, 15, UART_RXDATA_OFFSET as i32),
        bne(13, 0, 12),
        lui(10, riscv_shared::bus::SIM_CONTROL_BASE),
        addi(9, 0, SUCCESS_CODE as i32),
        sw(10, 9, 0),
        jal(0, 12),
        lui(10, riscv_shared::bus::SIM_CONTROL_BASE),
        addi(9, 0, FAILURE_CODE as i32),
        sw(10, 9, 0),
        jal(0, 0),
    ];
    assert_eq!(
        run_program_and_wait(&instructions, common::MEDIUM_TIMEOUT),
        SUCCESS_CODE
    );
}

#[test]
fn test_uart_loopback_pattern() {
    let instructions = vec![
        lui(15, UART_BASE),
        lui(10, riscv_shared::bus::SIM_CONTROL_BASE),
        addi(14, 0, 0x00),
        sw(15, 14, UART_TXDATA_OFFSET as i32),
        lw(13, 15, UART_STATUS_OFFSET as i32),
        andi(12, 13, UART_STATUS_TX_EMPTY as i32),
        beq(12, 0, -8),
        lw(13, 15, UART_STATUS_OFFSET as i32),
        andi(12, 13, UART_STATUS_RX_EMPTY as i32),
        bne(12, 0, -8),
        lw(11, 15, UART_RXDATA_OFFSET as i32),
        bne(11, 14, 116),
        addi(14, 0, 0xFF),
        sw(15, 14, UART_TXDATA_OFFSET as i32),
        lw(13, 15, UART_STATUS_OFFSET as i32),
        andi(12, 13, UART_STATUS_TX_EMPTY as i32),
        beq(12, 0, -8),
        lw(13, 15, UART_STATUS_OFFSET as i32),
        andi(12, 13, UART_STATUS_RX_EMPTY as i32),
        bne(12, 0, -8),
        lw(11, 15, UART_RXDATA_OFFSET as i32),
        andi(11, 11, 0xFF),
        bne(11, 14, 72),
        addi(14, 0, 0xAA),
        sw(15, 14, UART_TXDATA_OFFSET as i32),
        lw(13, 15, UART_STATUS_OFFSET as i32),
        andi(12, 13, UART_STATUS_TX_EMPTY as i32),
        beq(12, 0, -8),
        lw(13, 15, UART_STATUS_OFFSET as i32),
        andi(12, 13, UART_STATUS_RX_EMPTY as i32),
        bne(12, 0, -8),
        lw(11, 15, UART_RXDATA_OFFSET as i32),
        bne(11, 14, 32),
        addi(14, 0, 0x55),
        sw(15, 14, UART_TXDATA_OFFSET as i32),
        lw(13, 15, UART_STATUS_OFFSET as i32),
        andi(12, 13, UART_STATUS_TX_EMPTY as i32),
        beq(12, 0, -8),
        lw(13, 15, UART_STATUS_OFFSET as i32),
        andi(12, 13, UART_STATUS_RX_EMPTY as i32),
        bne(12, 0, -8),
        lw(11, 15, UART_RXDATA_OFFSET as i32),
        bne(11, 14, 12),
        addi(9, 0, SUCCESS_CODE as i32),
        sw(10, 9, 0),
        jal(0, 8),
        addi(9, 0, FAILURE_CODE as i32),
        sw(10, 9, 0),
        jal(0, 0),
    ];
    assert_eq!(
        run_program_and_wait(&instructions, common::LONG_TIMEOUT),
        SUCCESS_CODE
    );
}

#[test]
fn test_uart_loopback_multi_byte() {
    let mut instructions = vec![
        lui(15, UART_BASE),
        lui(10, riscv_shared::bus::SIM_CONTROL_BASE),
    ];

    for i in 1..=8u8 {
        instructions.push(addi(14, 0, i as i32));
        instructions.push(sw(15, 14, UART_TXDATA_OFFSET as i32));
        instructions.push(lw(13, 15, UART_STATUS_OFFSET as i32));
        instructions.push(andi(12, 13, UART_STATUS_TX_EMPTY as i32));
        instructions.push(beq(12, 0, -8));
        instructions.push(lw(13, 15, UART_STATUS_OFFSET as i32));
        instructions.push(andi(12, 13, UART_STATUS_RX_EMPTY as i32));
        instructions.push(bne(12, 0, -8));
        instructions.push(lw(11, 15, UART_RXDATA_OFFSET as i32));
        let remaining_iterations = 8 - i as usize;
        let instructions_per_iteration = 9;
        let instructions_after_this = remaining_iterations * instructions_per_iteration + 3;
        let failure_offset = (instructions_after_this * 4) as i32;
        instructions.push(bne(11, 14, failure_offset));
    }

    instructions.push(addi(9, 0, SUCCESS_CODE as i32));
    instructions.push(sw(10, 9, 0));
    instructions.push(jal(0, 8));
    instructions.push(addi(9, 0, FAILURE_CODE as i32));
    instructions.push(sw(10, 9, 0));
    instructions.push(jal(0, 0));

    assert_eq!(
        run_program_and_wait(&instructions, common::LONG_TIMEOUT),
        SUCCESS_CODE
    );
}
