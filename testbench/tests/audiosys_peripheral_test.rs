use riscv_core::AsDynamicVerilatedModel;
use riscv_core::{create_audiosys_peripheral_runtime, AudiosysPeripheralTestWrapper};
use std::sync::{Mutex, MutexGuard, OnceLock};

const AUDIOSYS_BASE_ADDR: u32 = 0x6000_0000;
const AUDIOSYS_MODE_ADDR: u32 = AUDIOSYS_BASE_ADDR;
const AUDIOSYS_TUNING_WORD_ADDR: u32 = AUDIOSYS_BASE_ADDR + 4;
const AUDIOSYS_FIFO_SAMPLE_ADDR: u32 = AUDIOSYS_BASE_ADDR + 8;
const AUDIOSYS_FIFO_SPACE_ADDR: u32 = AUDIOSYS_BASE_ADDR + 12;

const AUDIOSYS_MODE_OFF: u32 = 0;
const AUDIOSYS_MODE_TONE: u32 = 1;
const AUDIOSYS_MODE_FIFO: u32 = 2;

const MEM_SIZE_WORD: u8 = 2;
const TEST_TUNING_WORD: u32 = 0x1000_0000;
const RESET_SETTLE_CYCLES: usize = 6;
const TEST_FIFO_DEPTH: u32 = 8;
const LOW_WATER_THRESHOLD: u32 = TEST_FIFO_DEPTH / 2;

fn audiosys_test_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("audiosys test lock poisoned")
}

macro_rules! clock_cycle {
    ($dut:expr) => {
        $dut.sys_clk = 0;
        $dut.audio_clk = 0;
        $dut.eval();
        $dut.sys_clk = 1;
        $dut.audio_clk = 1;
        $dut.eval();
        $dut.sys_clk = 0;
        $dut.audio_clk = 0;
        $dut.eval();
    };
}

fn reset(dut: &mut AudiosysPeripheralTestWrapper) {
    dut.rst = 1;
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

fn wait_for_response(dut: &mut AudiosysPeripheralTestWrapper, max_cycles: usize) {
    for _ in 0..max_cycles {
        if dut.mem_d_valid != 0 {
            return;
        }
        clock_cycle!(dut);
    }

    panic!("timed out waiting for audiosys peripheral response");
}

fn write_access(dut: &mut AudiosysPeripheralTestWrapper, addr: u32, wdata: u32) {
    dut.mem_a_addr = addr;
    dut.mem_a_wdata = wdata;
    dut.mem_a_we = 1;
    dut.mem_a_size = MEM_SIZE_WORD;
    dut.mem_a_valid = 1;
    dut.eval();

    assert_eq!(
        dut.mem_a_ready, 1,
        "expected audiosys MMIO request to be accepted"
    );

    clock_cycle!(dut);
    dut.mem_a_valid = 0;
    dut.mem_a_we = 0;
    dut.eval();

    wait_for_response(dut, 64);
    assert_eq!(dut.mem_d_rdata, 0, "writes should acknowledge with zero data");

    dut.mem_d_ready = 1;
    clock_cycle!(dut);
    dut.mem_d_ready = 0;
    dut.eval();
}

fn read_access(dut: &mut AudiosysPeripheralTestWrapper, addr: u32) -> u32 {
    dut.mem_a_addr = addr;
    dut.mem_a_wdata = 0;
    dut.mem_a_we = 0;
    dut.mem_a_size = MEM_SIZE_WORD;
    dut.mem_a_valid = 1;
    dut.eval();

    assert_eq!(
        dut.mem_a_ready, 1,
        "expected audiosys MMIO request to be accepted"
    );

    clock_cycle!(dut);
    dut.mem_a_valid = 0;
    dut.eval();

    wait_for_response(dut, 64);
    let rdata = dut.mem_d_rdata;

    dut.mem_d_ready = 1;
    clock_cycle!(dut);
    dut.mem_d_ready = 0;
    dut.eval();

    rdata
}

fn begin_write_access(dut: &mut AudiosysPeripheralTestWrapper, addr: u32, wdata: u32) {
    dut.mem_a_addr = addr;
    dut.mem_a_wdata = wdata;
    dut.mem_a_we = 1;
    dut.mem_a_size = MEM_SIZE_WORD;
    dut.mem_a_valid = 1;
    dut.eval();
}

fn finish_pending_write(dut: &mut AudiosysPeripheralTestWrapper, max_cycles: usize) {
    for _ in 0..max_cycles {
        if dut.mem_a_ready != 0 {
            break;
        }
        clock_cycle!(dut);
    }

    assert_eq!(dut.mem_a_ready, 1, "pending write never became ready");

    clock_cycle!(dut);
    dut.mem_a_valid = 0;
    dut.mem_a_we = 0;
    dut.eval();

    wait_for_response(dut, 64);
    assert_eq!(dut.mem_d_rdata, 0, "write ack payload must stay zero");

    dut.mem_d_ready = 1;
    clock_cycle!(dut);
    dut.mem_d_ready = 0;
    dut.eval();
}

fn wait_for_mode_active(dut: &mut AudiosysPeripheralTestWrapper, expected_mode: u32, max_cycles: usize) {
    for _ in 0..max_cycles {
        if u32::from(dut.debug_audio_mode_active) == expected_mode {
            return;
        }
        clock_cycle!(dut);
    }

    panic!("timed out waiting for active audio mode {expected_mode}");
}

fn wait_for_fifo_space(dut: &mut AudiosysPeripheralTestWrapper, expected_space: u32, max_cycles: usize) {
    for _ in 0..max_cycles {
        if read_access(dut, AUDIOSYS_FIFO_SPACE_ADDR) == expected_space {
            return;
        }
        clock_cycle!(dut);
    }

    panic!("timed out waiting for fifo space {expected_space}");
}

fn wait_for_fifo_space_at_least(
    dut: &mut AudiosysPeripheralTestWrapper,
    minimum_space: u32,
    max_cycles: usize,
) -> u32 {
    for _ in 0..max_cycles {
        let space = read_access(dut, AUDIOSYS_FIFO_SPACE_ADDR);
        if space >= minimum_space {
            return space;
        }
        clock_cycle!(dut);
    }

    panic!("timed out waiting for fifo space >= {minimum_space}");
}

fn wait_for_irq_level(dut: &mut AudiosysPeripheralTestWrapper, asserted: bool, max_cycles: usize) {
    let expected = u8::from(asserted);
    for _ in 0..max_cycles {
        if dut.fifo_low_water_irq == expected {
            return;
        }
        clock_cycle!(dut);
    }

    panic!("timed out waiting for fifo_low_water_irq={expected}");
}

fn next_sample_ready_value(dut: &mut AudiosysPeripheralTestWrapper, max_cycles: usize) -> u16 {
    for _ in 0..max_cycles {
        if dut.debug_i2s_sample_ready != 0 {
            let value = dut.debug_i2s_sample_data as u16;
            clock_cycle!(dut);
            return value;
        }
        clock_cycle!(dut);
    }

    panic!("timed out waiting for sample_ready");
}

fn collect_sample_ready_values(
    dut: &mut AudiosysPeripheralTestWrapper,
    count: usize,
    max_cycles_per_value: usize,
) -> Vec<u16> {
    (0..count)
        .map(|_| next_sample_ready_value(dut, max_cycles_per_value))
        .collect()
}

fn contains_subsequence(values: &[u16], expected: &[u16]) -> bool {
    values.windows(expected.len()).any(|window| window == expected)
}

fn pack_stereo_sample(left: u16, right: u16) -> u32 {
    (u32::from(left) << 16) | u32::from(right)
}

#[test]
fn test_audiosys_registers_reset_low_and_mask_reserved_mode_bits() {
    let _guard = audiosys_test_lock();
    let runtime =
        create_audiosys_peripheral_runtime().expect("Failed to create audiosys peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<AudiosysPeripheralTestWrapper>()
        .expect("Failed to create audiosys peripheral model");

    reset(&mut dut);

    assert_eq!(read_access(&mut dut, AUDIOSYS_TUNING_WORD_ADDR), 0);
    assert_eq!(read_access(&mut dut, AUDIOSYS_MODE_ADDR), AUDIOSYS_MODE_OFF);
    assert_eq!(read_access(&mut dut, AUDIOSYS_FIFO_SPACE_ADDR), TEST_FIFO_DEPTH);
    assert_eq!(dut.fifo_low_water_irq, 0, "irq must be low while mode is off");

    write_access(&mut dut, AUDIOSYS_MODE_ADDR, u32::MAX);
    assert_eq!(
        read_access(&mut dut, AUDIOSYS_MODE_ADDR),
        AUDIOSYS_MODE_OFF,
        "reserved mode values must read back as off"
    );
}

#[test]
fn test_audiosys_tone_mode_still_updates_registers_and_can_be_muted() {
    let _guard = audiosys_test_lock();
    let runtime =
        create_audiosys_peripheral_runtime().expect("Failed to create audiosys peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<AudiosysPeripheralTestWrapper>()
        .expect("Failed to create audiosys peripheral model");

    reset(&mut dut);

    write_access(&mut dut, AUDIOSYS_TUNING_WORD_ADDR, TEST_TUNING_WORD);
    write_access(&mut dut, AUDIOSYS_MODE_ADDR, AUDIOSYS_MODE_TONE);

    assert_eq!(
        read_access(&mut dut, AUDIOSYS_TUNING_WORD_ADDR),
        TEST_TUNING_WORD
    );
    assert_eq!(read_access(&mut dut, AUDIOSYS_MODE_ADDR), AUDIOSYS_MODE_TONE);

    wait_for_mode_active(&mut dut, AUDIOSYS_MODE_TONE, 4096);

    let mut observed_nonzero = false;
    for _ in 0..4096 {
        if dut.audio_dac != 0 {
            observed_nonzero = true;
            break;
        }
        clock_cycle!(dut);
    }
    assert!(observed_nonzero, "tone mode should eventually drive non-zero serial data");

    write_access(&mut dut, AUDIOSYS_MODE_ADDR, AUDIOSYS_MODE_OFF);
    wait_for_mode_active(&mut dut, AUDIOSYS_MODE_OFF, 4096);
    for _ in 0..256 {
        assert_eq!(dut.audio_dac, 0, "off mode must mute audio output");
        clock_cycle!(dut);
    }
}

#[test]
fn test_audiosys_fifo_space_tracks_writes_and_playback() {
    let _guard = audiosys_test_lock();
    let runtime =
        create_audiosys_peripheral_runtime().expect("Failed to create audiosys peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<AudiosysPeripheralTestWrapper>()
        .expect("Failed to create audiosys peripheral model");

    reset(&mut dut);

    write_access(&mut dut, AUDIOSYS_MODE_ADDR, AUDIOSYS_MODE_FIFO);
    wait_for_mode_active(&mut dut, AUDIOSYS_MODE_FIFO, 4096);
    wait_for_irq_level(&mut dut, true, 64);

    for index in 0..6 {
        write_access(
            &mut dut,
            AUDIOSYS_FIFO_SAMPLE_ADDR,
            pack_stereo_sample(0x1000 + index, 0x2000 + index),
        );
    }

    assert_eq!(read_access(&mut dut, AUDIOSYS_FIFO_SPACE_ADDR), 2);
    wait_for_irq_level(&mut dut, false, 256);

    let space_after_drain = wait_for_fifo_space_at_least(&mut dut, 3, 4096);
    assert!(
        space_after_drain >= 3,
        "playback should consume fifo entries and free space"
    );
}

#[test]
fn test_audiosys_fifo_full_write_waits_for_available_space() {
    let _guard = audiosys_test_lock();
    let runtime =
        create_audiosys_peripheral_runtime().expect("Failed to create audiosys peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<AudiosysPeripheralTestWrapper>()
        .expect("Failed to create audiosys peripheral model");

    reset(&mut dut);

    write_access(&mut dut, AUDIOSYS_MODE_ADDR, AUDIOSYS_MODE_FIFO);
    wait_for_mode_active(&mut dut, AUDIOSYS_MODE_FIFO, 4096);

    for index in 0..TEST_FIFO_DEPTH {
        write_access(
            &mut dut,
            AUDIOSYS_FIFO_SAMPLE_ADDR,
            pack_stereo_sample(0x3000 + index as u16, 0x4000 + index as u16),
        );
    }
    assert_eq!(read_access(&mut dut, AUDIOSYS_FIFO_SPACE_ADDR), 0);

    begin_write_access(
        &mut dut,
        AUDIOSYS_FIFO_SAMPLE_ADDR,
        pack_stereo_sample(0x5555, 0xAAAA),
    );
    assert_eq!(
        dut.mem_a_ready, 0,
        "fifo sample writes must stall while the buffer is full"
    );

    finish_pending_write(&mut dut, 4096);
    assert_eq!(
        read_access(&mut dut, AUDIOSYS_FIFO_SPACE_ADDR),
        0,
        "once a sample drains, the pending write should refill the freed slot"
    );
}

#[test]
fn test_audiosys_fifo_low_water_irq_asserts_and_clears_on_refill() {
    let _guard = audiosys_test_lock();
    let runtime =
        create_audiosys_peripheral_runtime().expect("Failed to create audiosys peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<AudiosysPeripheralTestWrapper>()
        .expect("Failed to create audiosys peripheral model");

    reset(&mut dut);

    write_access(&mut dut, AUDIOSYS_MODE_ADDR, AUDIOSYS_MODE_FIFO);
    wait_for_mode_active(&mut dut, AUDIOSYS_MODE_FIFO, 4096);
    wait_for_irq_level(&mut dut, true, 64);

    for index in 0..LOW_WATER_THRESHOLD {
        write_access(
            &mut dut,
            AUDIOSYS_FIFO_SAMPLE_ADDR,
            pack_stereo_sample(0x0100 + index as u16, 0x0200 + index as u16),
        );
    }
    wait_for_irq_level(&mut dut, false, 256);

    wait_for_fifo_space(&mut dut, TEST_FIFO_DEPTH - (LOW_WATER_THRESHOLD - 1), 4096);
    wait_for_irq_level(&mut dut, true, 256);

    write_access(
        &mut dut,
        AUDIOSYS_FIFO_SAMPLE_ADDR,
        pack_stereo_sample(0x0333, 0x0444),
    );
    write_access(
        &mut dut,
        AUDIOSYS_FIFO_SAMPLE_ADDR,
        pack_stereo_sample(0x0555, 0x0666),
    );
    wait_for_irq_level(&mut dut, false, 256);
}

#[test]
fn test_audiosys_fifo_playback_uses_written_stereo_samples() {
    let _guard = audiosys_test_lock();
    let runtime =
        create_audiosys_peripheral_runtime().expect("Failed to create audiosys peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<AudiosysPeripheralTestWrapper>()
        .expect("Failed to create audiosys peripheral model");

    reset(&mut dut);

    write_access(&mut dut, AUDIOSYS_MODE_ADDR, AUDIOSYS_MODE_FIFO);
    wait_for_mode_active(&mut dut, AUDIOSYS_MODE_FIFO, 4096);

    let expected = [0x1234u16, 0x5678, 0x9ABCu16, 0xDEF0];
    write_access(
        &mut dut,
        AUDIOSYS_FIFO_SAMPLE_ADDR,
        pack_stereo_sample(expected[0], expected[1]),
    );
    write_access(
        &mut dut,
        AUDIOSYS_FIFO_SAMPLE_ADDR,
        pack_stereo_sample(expected[2], expected[3]),
    );

    let observed = collect_sample_ready_values(&mut dut, 8, 4096);
    assert!(
        contains_subsequence(&observed, &expected),
        "fifo playback should present left/right samples in order; observed={observed:X?}"
    );
}

#[test]
fn test_audiosys_fifo_underrun_outputs_zero_and_keeps_irq_asserted() {
    let _guard = audiosys_test_lock();
    let runtime =
        create_audiosys_peripheral_runtime().expect("Failed to create audiosys peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<AudiosysPeripheralTestWrapper>()
        .expect("Failed to create audiosys peripheral model");

    reset(&mut dut);

    write_access(&mut dut, AUDIOSYS_MODE_ADDR, AUDIOSYS_MODE_FIFO);
    wait_for_mode_active(&mut dut, AUDIOSYS_MODE_FIFO, 4096);

    write_access(
        &mut dut,
        AUDIOSYS_FIFO_SAMPLE_ADDR,
        pack_stereo_sample(0x1111, 0x2222),
    );

    wait_for_fifo_space(&mut dut, TEST_FIFO_DEPTH, 4096);
    wait_for_irq_level(&mut dut, true, 256);

    let observed = collect_sample_ready_values(&mut dut, 4, 4096);
    assert!(
        observed.iter().all(|&value| value == 0),
        "underrun should drive zero-valued sample words, observed={observed:X?}"
    );
}
