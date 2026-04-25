mod common;

use common::{
    create_test_runtime, instructions_to_bytes, load_and_boot, read_word_with_timeout,
    write_word_with_timeout, LONG_TIMEOUT, MEDIUM_TIMEOUT, TEST_BOOT_PC,
};
use riscv_core::instruction::{addi, csrrs, csrrsi, csrrw, jal, lui, lw, mret, slli, sw, wfi};
use riscv_shared::bus::{
    audiosys_fifo_sample_addr, audiosys_fifo_space_addr, audiosys_mode_addr,
    interrupt_ctrl_claim_addr, interrupt_ctrl_complete_addr, interrupt_ctrl_enable_addr,
    interrupt_ctrl_pending_addr, interrupt_ctrl_pending_set_addr, sysctrl_cpu_pc_addr,
    AUDIOSYS_MODE_FIFO, DRAM_BASE, INTERRUPT_CTRL_CLAIM_OFFSET, INTERRUPT_CTRL_COMPLETE_OFFSET,
    INTERRUPT_CTRL_ENABLE_OFFSET, INTERRUPT_CTRL_SOURCE_AUDIOSYS_FIFO_LOW_WATER,
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
// Must match rtl/common/wrappers/top_sim_test_wrapper.sv.
const AUDIOSYS_FIFO_DEPTH: u32 = 2;

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

    let wait_start = Instant::now();
    loop {
        if read_word_with_timeout(runtime.as_mut(), DRAM_BASE, MEDIUM_TIMEOUT) == 2 {
            break;
        }
        assert!(
            wait_start.elapsed() < LONG_TIMEOUT,
            "external interrupt handler for injected INTERRUPT_CTRL_SOURCE_TEST1 did not reach the expected count before timeout"
        );
        std::thread::sleep(Duration::from_millis(1));
    }

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
    let audiosys_source_mask = 1 << (INTERRUPT_CTRL_SOURCE_AUDIOSYS_FIFO_LOW_WATER - 1);

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&[addi(1, 1, 1), jal(0, -4)]),
    );

    write_word_with_timeout(
        runtime.as_mut(),
        audiosys_mode_addr(),
        AUDIOSYS_MODE_FIFO,
        MEDIUM_TIMEOUT,
    );

    let fill_start = Instant::now();
    let mut current_space =
        read_word_with_timeout(runtime.as_mut(), audiosys_fifo_space_addr(), MEDIUM_TIMEOUT);
    while current_space >= (AUDIOSYS_FIFO_DEPTH / 2) {
        for _ in 0..current_space {
            write_word_with_timeout(
                runtime.as_mut(),
                audiosys_fifo_sample_addr(),
                0,
                MEDIUM_TIMEOUT,
            );
        }
        current_space =
            read_word_with_timeout(runtime.as_mut(), audiosys_fifo_space_addr(), MEDIUM_TIMEOUT);
        assert!(
            fill_start.elapsed() < LONG_TIMEOUT,
            "host setup failed to fill the audiosys fifo above the half-full threshold"
        );
    }

    write_word_with_timeout(
        runtime.as_mut(),
        interrupt_ctrl_enable_addr(),
        audiosys_source_mask,
        MEDIUM_TIMEOUT,
    );

    let pending_start = Instant::now();
    let first_space = loop {
        let pending = read_word_with_timeout(
            runtime.as_mut(),
            interrupt_ctrl_pending_addr(),
            MEDIUM_TIMEOUT,
        );
        let space =
            read_word_with_timeout(runtime.as_mut(), audiosys_fifo_space_addr(), MEDIUM_TIMEOUT);
        if (pending & audiosys_source_mask) != 0 {
            break space;
        }
        assert!(
            pending_start.elapsed() < LONG_TIMEOUT,
            "audiosys low-water interrupt did not assert after the fifo drained below half full"
        );
    };

    assert!(
        first_space > (AUDIOSYS_FIFO_DEPTH / 2),
        "first interrupt should only happen once fifo space exceeds half depth"
    );

    write_word_with_timeout(
        runtime.as_mut(),
        interrupt_ctrl_complete_addr(),
        INTERRUPT_CTRL_SOURCE_AUDIOSYS_FIFO_LOW_WATER,
        MEDIUM_TIMEOUT,
    );

    let repend_start = Instant::now();
    let second_space_before = loop {
        let pending = read_word_with_timeout(
            runtime.as_mut(),
            interrupt_ctrl_pending_addr(),
            MEDIUM_TIMEOUT,
        );
        let space =
            read_word_with_timeout(runtime.as_mut(), audiosys_fifo_space_addr(), MEDIUM_TIMEOUT);
        if (pending & audiosys_source_mask) != 0 {
            break space;
        }
        assert!(
            repend_start.elapsed() < LONG_TIMEOUT,
            "audiosys low-water interrupt did not re-pend after completion without refill"
        );
    };

    assert!(
        second_space_before > (AUDIOSYS_FIFO_DEPTH / 2),
        "second interrupt should still observe low-water space before refill"
    );

    let refill_start = Instant::now();
    let mut second_space_after = second_space_before;
    while second_space_after >= (AUDIOSYS_FIFO_DEPTH / 2) {
        for _ in 0..second_space_after {
            write_word_with_timeout(
                runtime.as_mut(),
                audiosys_fifo_sample_addr(),
                0,
                MEDIUM_TIMEOUT,
            );
        }
        second_space_after =
            read_word_with_timeout(runtime.as_mut(), audiosys_fifo_space_addr(), MEDIUM_TIMEOUT);
        assert!(
            refill_start.elapsed() < LONG_TIMEOUT,
            "host refill failed to restore the audiosys fifo above half full"
        );
    }

    write_word_with_timeout(
        runtime.as_mut(),
        interrupt_ctrl_complete_addr(),
        INTERRUPT_CTRL_SOURCE_AUDIOSYS_FIFO_LOW_WATER,
        MEDIUM_TIMEOUT,
    );

    for _ in 0..2 {
        assert_eq!(
            read_word_with_timeout(
                runtime.as_mut(),
                interrupt_ctrl_pending_addr(),
                MEDIUM_TIMEOUT
            ) & audiosys_source_mask,
            0,
            "pending bit should clear immediately after refill above half full"
        );
        let _ =
            read_word_with_timeout(runtime.as_mut(), audiosys_fifo_space_addr(), MEDIUM_TIMEOUT);
    }

    assert!(
        second_space_after < (AUDIOSYS_FIFO_DEPTH / 2),
        "refill should push occupancy back above half full"
    );
}
