use riscv_core::AsDynamicVerilatedModel;
use riscv_core::{
    create_external_interrupt_controller_runtime, ExternalInterruptControllerTestWrapper,
};
use riscv_shared::bus::{
    INTERRUPT_CTRL_BASE, INTERRUPT_CTRL_CLAIM_OFFSET, INTERRUPT_CTRL_COMPLETE_OFFSET,
    INTERRUPT_CTRL_ENABLE_OFFSET, INTERRUPT_CTRL_PENDING_CLEAR_OFFSET,
    INTERRUPT_CTRL_PENDING_OFFSET, INTERRUPT_CTRL_PENDING_SET_OFFSET,
    INTERRUPT_CTRL_RAW_STATUS_OFFSET, INTERRUPT_CTRL_SOURCE_COUNT_OFFSET,
    INTERRUPT_CTRL_SOURCE_AUDIOSYS_FIFO_LOW_WATER,
    INTERRUPT_CTRL_SOURCE_TEST0, INTERRUPT_CTRL_SOURCE_TEST1, INTERRUPT_CTRL_SOURCE_TEST2,
};

const MEM_SIZE_WORD: u8 = 2;
const RESET_SETTLE_CYCLES: usize = 4;

macro_rules! clock_cycle {
    ($dut:expr) => {
        $dut.clk = 0;
        $dut.eval();
        $dut.clk = 1;
        $dut.eval();
        $dut.clk = 0;
        $dut.eval();
    };
}

fn reset(dut: &mut ExternalInterruptControllerTestWrapper) {
    dut.rst = 1;
    dut.irq_sources = 0;
    dut.mem_a_addr = 0;
    dut.mem_a_wdata = 0;
    dut.mem_a_we = 0;
    dut.mem_a_size = 0;
    dut.mem_a_valid = 0;
    dut.mem_d_ready = 0;

    for _ in 0..RESET_SETTLE_CYCLES {
        clock_cycle!(dut);
    }

    dut.rst = 0;
    for _ in 0..RESET_SETTLE_CYCLES {
        clock_cycle!(dut);
    }
}

fn read_access(dut: &mut ExternalInterruptControllerTestWrapper, addr: u32) -> u32 {
    dut.mem_a_addr = addr;
    dut.mem_a_wdata = 0;
    dut.mem_a_we = 0;
    dut.mem_a_size = MEM_SIZE_WORD;
    dut.mem_a_valid = 1;
    dut.eval();
    assert_eq!(
        dut.mem_a_ready, 1,
        "controller must accept the read request"
    );

    clock_cycle!(dut);
    dut.mem_a_valid = 0;
    dut.eval();

    for _ in 0..8 {
        if dut.mem_d_valid != 0 {
            break;
        }
        clock_cycle!(dut);
    }
    assert_eq!(
        dut.mem_d_valid, 1,
        "timed out waiting for controller read response"
    );

    let data = dut.mem_d_rdata;
    dut.mem_d_ready = 1;
    clock_cycle!(dut);
    dut.mem_d_ready = 0;
    dut.eval();
    data
}

fn write_access(dut: &mut ExternalInterruptControllerTestWrapper, addr: u32, value: u32) {
    dut.mem_a_addr = addr;
    dut.mem_a_wdata = value;
    dut.mem_a_we = 1;
    dut.mem_a_size = MEM_SIZE_WORD;
    dut.mem_a_valid = 1;
    dut.eval();
    assert_eq!(
        dut.mem_a_ready, 1,
        "controller must accept the write request"
    );

    clock_cycle!(dut);
    dut.mem_a_valid = 0;
    dut.mem_a_we = 0;
    dut.eval();

    for _ in 0..8 {
        if dut.mem_d_valid != 0 {
            break;
        }
        clock_cycle!(dut);
    }
    assert_eq!(
        dut.mem_d_valid, 1,
        "timed out waiting for controller write response"
    );
    assert_eq!(dut.mem_d_rdata, 0, "write ack payload must be zero");

    dut.mem_d_ready = 1;
    clock_cycle!(dut);
    dut.mem_d_ready = 0;
    dut.eval();
}

#[test]
fn test_external_interrupt_controller_reports_source_count_and_reset_state() {
    let runtime = create_external_interrupt_controller_runtime()
        .expect("Failed to create interrupt controller runtime");
    let mut dut = runtime
        .create_model_simple::<ExternalInterruptControllerTestWrapper>()
        .expect("Failed to create interrupt controller model");

    reset(&mut dut);

    assert_eq!(
        read_access(
            &mut dut,
            INTERRUPT_CTRL_BASE + INTERRUPT_CTRL_SOURCE_COUNT_OFFSET
        ),
        INTERRUPT_CTRL_SOURCE_AUDIOSYS_FIFO_LOW_WATER
    );
    assert_eq!(
        read_access(
            &mut dut,
            INTERRUPT_CTRL_BASE + INTERRUPT_CTRL_PENDING_OFFSET
        ),
        0
    );
    assert_eq!(
        read_access(&mut dut, INTERRUPT_CTRL_BASE + INTERRUPT_CTRL_CLAIM_OFFSET),
        0
    );
    assert_eq!(dut.meip, 0, "meip must be deasserted after reset");
}

#[test]
fn test_external_interrupt_controller_latches_raw_sources_and_masks_until_enabled() {
    let runtime = create_external_interrupt_controller_runtime()
        .expect("Failed to create interrupt controller runtime");
    let mut dut = runtime
        .create_model_simple::<ExternalInterruptControllerTestWrapper>()
        .expect("Failed to create interrupt controller model");

    reset(&mut dut);

    dut.irq_sources = 1 << (INTERRUPT_CTRL_SOURCE_TEST1 - 1);
    clock_cycle!(dut);
    dut.irq_sources = 0;
    dut.eval();

    assert_eq!(
        read_access(
            &mut dut,
            INTERRUPT_CTRL_BASE + INTERRUPT_CTRL_RAW_STATUS_OFFSET
        ),
        0,
        "RAW_STATUS should drop back to zero after the source pulse"
    );
    assert_eq!(
        read_access(
            &mut dut,
            INTERRUPT_CTRL_BASE + INTERRUPT_CTRL_PENDING_OFFSET
        ),
        1 << (INTERRUPT_CTRL_SOURCE_TEST1 - 1),
        "Pending register must latch the source pulse"
    );
    assert_eq!(dut.meip, 0, "Disabled pending sources must not assert meip");

    write_access(
        &mut dut,
        INTERRUPT_CTRL_BASE + INTERRUPT_CTRL_ENABLE_OFFSET,
        1 << (INTERRUPT_CTRL_SOURCE_TEST1 - 1),
    );
    assert_eq!(dut.meip, 1, "Enabling a pending source must assert meip");
    assert_eq!(
        read_access(&mut dut, INTERRUPT_CTRL_BASE + INTERRUPT_CTRL_CLAIM_OFFSET),
        INTERRUPT_CTRL_SOURCE_TEST1,
        "Claim must return the enabled pending source ID"
    );
}

#[test]
fn test_external_interrupt_controller_claim_priority_and_completion_flow() {
    let runtime = create_external_interrupt_controller_runtime()
        .expect("Failed to create interrupt controller runtime");
    let mut dut = runtime
        .create_model_simple::<ExternalInterruptControllerTestWrapper>()
        .expect("Failed to create interrupt controller model");

    reset(&mut dut);

    write_access(
        &mut dut,
        INTERRUPT_CTRL_BASE + INTERRUPT_CTRL_ENABLE_OFFSET,
        0b0111,
    );
    write_access(
        &mut dut,
        INTERRUPT_CTRL_BASE + INTERRUPT_CTRL_PENDING_SET_OFFSET,
        0b0111,
    );

    assert_eq!(
        read_access(&mut dut, INTERRUPT_CTRL_BASE + INTERRUPT_CTRL_CLAIM_OFFSET),
        INTERRUPT_CTRL_SOURCE_TEST0,
        "Lowest source ID must win fixed-priority arbitration"
    );

    write_access(
        &mut dut,
        INTERRUPT_CTRL_BASE + INTERRUPT_CTRL_COMPLETE_OFFSET,
        INTERRUPT_CTRL_SOURCE_TEST0,
    );
    assert_eq!(
        read_access(&mut dut, INTERRUPT_CTRL_BASE + INTERRUPT_CTRL_CLAIM_OFFSET),
        INTERRUPT_CTRL_SOURCE_TEST1,
        "Completion must reveal the next pending source"
    );

    write_access(
        &mut dut,
        INTERRUPT_CTRL_BASE + INTERRUPT_CTRL_COMPLETE_OFFSET,
        INTERRUPT_CTRL_SOURCE_TEST1,
    );
    assert_eq!(
        read_access(&mut dut, INTERRUPT_CTRL_BASE + INTERRUPT_CTRL_CLAIM_OFFSET),
        INTERRUPT_CTRL_SOURCE_TEST2,
        "Claim must continue walking remaining pending sources by priority"
    );
}

#[test]
fn test_external_interrupt_controller_pending_clear_deasserts_meip() {
    let runtime = create_external_interrupt_controller_runtime()
        .expect("Failed to create interrupt controller runtime");
    let mut dut = runtime
        .create_model_simple::<ExternalInterruptControllerTestWrapper>()
        .expect("Failed to create interrupt controller model");

    reset(&mut dut);

    write_access(
        &mut dut,
        INTERRUPT_CTRL_BASE + INTERRUPT_CTRL_ENABLE_OFFSET,
        0b0010,
    );
    write_access(
        &mut dut,
        INTERRUPT_CTRL_BASE + INTERRUPT_CTRL_PENDING_SET_OFFSET,
        0b0010,
    );
    assert_eq!(dut.meip, 1, "Pending enabled source must assert meip");

    write_access(
        &mut dut,
        INTERRUPT_CTRL_BASE + INTERRUPT_CTRL_PENDING_CLEAR_OFFSET,
        0b0010,
    );
    assert_eq!(
        read_access(
            &mut dut,
            INTERRUPT_CTRL_BASE + INTERRUPT_CTRL_PENDING_OFFSET
        ),
        0,
        "Pending clear register must clear latched sources"
    );
    assert_eq!(
        dut.meip, 0,
        "Clearing the last pending source must drop meip"
    );
}

#[test]
fn test_external_interrupt_controller_supports_audiosys_source_id() {
    let runtime = create_external_interrupt_controller_runtime()
        .expect("Failed to create interrupt controller runtime");
    let mut dut = runtime
        .create_model_simple::<ExternalInterruptControllerTestWrapper>()
        .expect("Failed to create interrupt controller model");

    reset(&mut dut);

    let audiosys_mask = 1 << (INTERRUPT_CTRL_SOURCE_AUDIOSYS_FIFO_LOW_WATER - 1);

    write_access(
        &mut dut,
        INTERRUPT_CTRL_BASE + INTERRUPT_CTRL_ENABLE_OFFSET,
        audiosys_mask,
    );
    write_access(
        &mut dut,
        INTERRUPT_CTRL_BASE + INTERRUPT_CTRL_PENDING_SET_OFFSET,
        audiosys_mask,
    );

    assert_eq!(dut.meip, 1, "enabled audiosys source must assert meip");
    assert_eq!(
        read_access(&mut dut, INTERRUPT_CTRL_BASE + INTERRUPT_CTRL_CLAIM_OFFSET),
        INTERRUPT_CTRL_SOURCE_AUDIOSYS_FIFO_LOW_WATER,
        "controller must claim the audiosys fifo low-water source ID"
    );

    write_access(
        &mut dut,
        INTERRUPT_CTRL_BASE + INTERRUPT_CTRL_COMPLETE_OFFSET,
        INTERRUPT_CTRL_SOURCE_AUDIOSYS_FIFO_LOW_WATER,
    );
    assert_eq!(
        read_access(&mut dut, INTERRUPT_CTRL_BASE + INTERRUPT_CTRL_PENDING_OFFSET),
        0,
        "completing the audiosys source should clear its pending bit"
    );
}
