mod common;

use common::{
    create_test_runtime, instructions_to_bytes, load_and_boot, read_word_with_timeout,
    wait_for_cpu_halt, write_word_with_timeout, LONG_TIMEOUT, MEDIUM_TIMEOUT, TEST_BOOT_PC,
};
use riscv_core::instruction::{addi, blt, bne, csrrs, csrrsi, csrrw, lui, lw, mret, slli, sw, wfi};
use riscv_shared::bus::{
    audiosys_fifo_space_addr, audiosys_mode_addr, AUDIOSYS_FIFO_SAMPLE_OFFSET,
    AUDIOSYS_FIFO_SPACE_OFFSET, AUDIOSYS_MODE_FIFO, AUDIOSYS_MODE_OFF,
    interrupt_ctrl_claim_addr, interrupt_ctrl_complete_addr, interrupt_ctrl_enable_addr,
    interrupt_ctrl_pending_addr, interrupt_ctrl_pending_set_addr, sysctrl_cpu_pc_addr, DRAM_BASE,
    INTERRUPT_CTRL_CLAIM_OFFSET, INTERRUPT_CTRL_COMPLETE_OFFSET, INTERRUPT_CTRL_ENABLE_OFFSET,
    INTERRUPT_CTRL_SOURCE_AUDIOSYS_FIFO_LOW_WATER, INTERRUPT_CTRL_SOURCE_TEST1,
};
use riscv_shared::sim_control::SUCCESS_CODE;
use std::time::{Duration, Instant};

const CSR_MSTATUS: u32 = 0x300;
const CSR_MIE: u32 = 0x304;
const CSR_MTVEC: u32 = 0x305;
const CSR_MEPC: u32 = 0x341;
const CSR_MCAUSE: u32 = 0x342;
const MSTATUS_MIE_ZIMM: u32 = 1 << 3;
const INTERRUPT_CAUSE_MEI: u32 = 0x8000_000B;
const AUDIOSYS_FIFO_DEPTH: u32 = 1024;

#[test]
fn test_host_injected_external_interrupt_claim_complete_flow() {
    let mut runtime = create_test_runtime();

    let handler_addr = TEST_BOOT_PC + 0x60;
    let handler_offset = i32::try_from(handler_addr & 0xFFF).expect("handler offset must fit");

    let mut instructions = vec![
        lui(5, TEST_BOOT_PC),
        addi(5, 5, handler_offset),
        csrrw(0, 5, CSR_MTVEC),
        addi(6, 0, INTERRUPT_CTRL_SOURCE_TEST1 as i32),
        lui(15, interrupt_ctrl_enable_addr() & 0xFFFF_F000),
        sw(
            15,
            6,
            i32::try_from(INTERRUPT_CTRL_ENABLE_OFFSET).expect("enable offset must fit"),
        ),
        addi(7, 0, 1),
        slli(7, 7, 11),
        csrrw(0, 7, CSR_MIE),
        csrrsi(0, MSTATUS_MIE_ZIMM, CSR_MSTATUS),
        wfi(),
    ];
    instructions.extend(common::tohost_termination(13, 14, SUCCESS_CODE));

    instructions.resize(0x60 / 4, addi(0, 0, 0));
    instructions.extend([
        lui(14, DRAM_BASE),
        lw(
            10,
            15,
            i32::try_from(INTERRUPT_CTRL_CLAIM_OFFSET).expect("claim offset must fit"),
        ),
        sw(14, 10, 0),
        csrrs(11, 0, CSR_MCAUSE),
        sw(14, 11, 4),
        csrrs(12, 0, CSR_MEPC),
        sw(14, 12, 8),
        sw(
            15,
            10,
            i32::try_from(INTERRUPT_CTRL_COMPLETE_OFFSET).expect("complete offset must fit"),
        ),
        mret(),
    ]);

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );

    let wait_start = Instant::now();
    loop {
        let current_pc =
            read_word_with_timeout(runtime.as_mut(), sysctrl_cpu_pc_addr(), MEDIUM_TIMEOUT);
        if current_pc == TEST_BOOT_PC + 0x2c {
            break;
        }
        assert!(
            wait_start.elapsed() < MEDIUM_TIMEOUT,
            "CPU did not reach the WFI resume PC before interrupt injection"
        );
        std::thread::sleep(Duration::from_millis(1));
    }

    write_word_with_timeout(
        runtime.as_mut(),
        interrupt_ctrl_pending_set_addr(),
        1 << (INTERRUPT_CTRL_SOURCE_TEST1 - 1),
        MEDIUM_TIMEOUT,
    );

    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );

    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE, MEDIUM_TIMEOUT),
        INTERRUPT_CTRL_SOURCE_TEST1,
        "Handler must claim the injected interrupt source"
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 4, MEDIUM_TIMEOUT),
        INTERRUPT_CAUSE_MEI,
        "Handler must observe machine external interrupt cause"
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 8, MEDIUM_TIMEOUT),
        TEST_BOOT_PC + 0x2c,
        "MEPC must point at the post-WFI resume PC"
    );
    assert_eq!(
        read_word_with_timeout(
            runtime.as_mut(),
            interrupt_ctrl_pending_addr(),
            MEDIUM_TIMEOUT
        ),
        0,
        "Complete write must clear the pending bit"
    );
    assert_eq!(
        read_word_with_timeout(
            runtime.as_mut(),
            interrupt_ctrl_claim_addr(),
            MEDIUM_TIMEOUT
        ),
        0,
        "No interrupt should remain claimable after completion"
    );
    assert_eq!(
        read_word_with_timeout(
            runtime.as_mut(),
            interrupt_ctrl_complete_addr(),
            MEDIUM_TIMEOUT
        ),
        0,
        "Writes to COMPLETE must acknowledge with zero data"
    );
}

#[test]
fn test_audiosys_fifo_low_water_interrupt_refill_flow() {
    let mut runtime = create_test_runtime();

    let handler_addr = TEST_BOOT_PC + 0xA0;
    let handler_offset = i32::try_from(handler_addr & 0xFFF).expect("handler offset must fit");
    let audiosys_source_mask = 1 << (INTERRUPT_CTRL_SOURCE_AUDIOSYS_FIFO_LOW_WATER - 1);

    let mut instructions = vec![
        lui(5, TEST_BOOT_PC),
        addi(5, 5, handler_offset),
        csrrw(0, 5, CSR_MTVEC),
        lui(13, DRAM_BASE),
        lui(14, audiosys_mode_addr() & 0xFFFF_F000),
        lui(15, interrupt_ctrl_enable_addr() & 0xFFFF_F000),
        addi(6, 0, AUDIOSYS_MODE_FIFO as i32),
        sw(14, 6, 0),
        addi(9, 0, 3),
        slli(9, 9, 8),
        addi(10, 0, 0),
        sw(
            14,
            10,
            i32::try_from(AUDIOSYS_FIFO_SAMPLE_OFFSET).expect("fifo sample offset must fit"),
        ),
        addi(9, 9, -1),
        bne(9, 0, -8),
        addi(6, 0, audiosys_source_mask as i32),
        sw(
            15,
            6,
            i32::try_from(INTERRUPT_CTRL_ENABLE_OFFSET).expect("enable offset must fit"),
        ),
        addi(7, 0, 1),
        slli(7, 7, 11),
        csrrw(0, 7, CSR_MIE),
        csrrsi(0, MSTATUS_MIE_ZIMM, CSR_MSTATUS),
        wfi(),
        lw(12, 13, 0),
        addi(11, 0, 2),
        blt(12, 11, -8),
        addi(6, 0, AUDIOSYS_MODE_OFF as i32),
        sw(14, 6, 0),
    ];
    instructions.extend(common::tohost_termination(20, 21, SUCCESS_CODE));

    instructions.resize(0xA0 / 4, addi(0, 0, 0));
    instructions.extend([
        lui(13, DRAM_BASE),
        lui(14, audiosys_mode_addr() & 0xFFFF_F000),
        lui(15, interrupt_ctrl_enable_addr() & 0xFFFF_F000),
        lw(16, 13, 0),
        lw(
            10,
            15,
            i32::try_from(INTERRUPT_CTRL_CLAIM_OFFSET).expect("claim offset must fit"),
        ),
        bne(16, 0, 28),
        sw(13, 10, 4),
        lw(
            11,
            14,
            i32::try_from(AUDIOSYS_FIFO_SPACE_OFFSET).expect("fifo space offset must fit"),
        ),
        sw(13, 11, 12),
        sw(
            15,
            10,
            i32::try_from(INTERRUPT_CTRL_COMPLETE_OFFSET).expect("complete offset must fit"),
        ),
        addi(16, 0, 1),
        sw(13, 16, 0),
        mret(),
        sw(13, 10, 8),
        lw(
            11,
            14,
            i32::try_from(AUDIOSYS_FIFO_SPACE_OFFSET).expect("fifo space offset must fit"),
        ),
        sw(13, 11, 16),
        sw(
            15,
            10,
            i32::try_from(INTERRUPT_CTRL_COMPLETE_OFFSET).expect("complete offset must fit"),
        ),
        addi(16, 0, 2),
        sw(13, 16, 0),
        addi(17, 0, 4),
        addi(12, 0, 0),
        sw(
            14,
            12,
            i32::try_from(AUDIOSYS_FIFO_SAMPLE_OFFSET).expect("fifo sample offset must fit"),
        ),
        addi(17, 17, -1),
        bne(17, 0, -8),
        lw(
            11,
            14,
            i32::try_from(AUDIOSYS_FIFO_SPACE_OFFSET).expect("fifo space offset must fit"),
        ),
        sw(13, 11, 20),
        mret(),
    ]);

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
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 4, MEDIUM_TIMEOUT),
        INTERRUPT_CTRL_SOURCE_AUDIOSYS_FIFO_LOW_WATER,
        "first handler must claim the audiosys fifo low-water source"
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 8, MEDIUM_TIMEOUT),
        INTERRUPT_CTRL_SOURCE_AUDIOSYS_FIFO_LOW_WATER,
        "level-sensitive audiosys source must re-pend after completion without refill"
    );

    let first_space = read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 12, MEDIUM_TIMEOUT);
    let second_space_before =
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 16, MEDIUM_TIMEOUT);
    let second_space_after =
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 20, MEDIUM_TIMEOUT);

    assert!(
        first_space > (AUDIOSYS_FIFO_DEPTH / 2),
        "first interrupt should only happen once fifo space exceeds half depth"
    );
    assert!(
        second_space_before > (AUDIOSYS_FIFO_DEPTH / 2),
        "second interrupt should still observe low-water space before refill"
    );
    assert!(
        second_space_after < (AUDIOSYS_FIFO_DEPTH / 2),
        "refill should push occupancy back above half full"
    );
    assert_eq!(
        read_word_with_timeout(
            runtime.as_mut(),
            interrupt_ctrl_pending_addr(),
            MEDIUM_TIMEOUT
        ),
        0,
        "pending register should be clear after completion and mode-off shutdown"
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), audiosys_fifo_space_addr(), MEDIUM_TIMEOUT),
        second_space_after,
        "host-visible fifo space should match the value captured after refill"
    );
}
