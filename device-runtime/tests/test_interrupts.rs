mod common;

use common::{
    create_test_runtime, instructions_to_bytes, load_and_boot, read_word_with_timeout,
    wait_for_cpu_halt, write_word_with_timeout, LONG_TIMEOUT, MEDIUM_TIMEOUT, TEST_BOOT_PC,
};
use riscv_core::instruction::{addi, csrrs, csrrsi, csrrw, lui, lw, mret, slli, sw, wfi};
use riscv_shared::bus::{
    interrupt_ctrl_claim_addr, interrupt_ctrl_complete_addr, interrupt_ctrl_enable_addr,
    interrupt_ctrl_pending_addr, interrupt_ctrl_pending_set_addr, sysctrl_cpu_pc_addr, DRAM_BASE,
    INTERRUPT_CTRL_CLAIM_OFFSET, INTERRUPT_CTRL_COMPLETE_OFFSET, INTERRUPT_CTRL_ENABLE_OFFSET,
    INTERRUPT_CTRL_SOURCE_TEST1,
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
