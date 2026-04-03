use riscv_core::AsDynamicVerilatedModel;
use riscv_core::{create_audiosys_peripheral_runtime, AudiosysPeripheralTestWrapper};

const AUDIOSYS_BASE_ADDR: u32 = 0x6000_0000;
const AUDIOSYS_CONTROL_ADDR: u32 = AUDIOSYS_BASE_ADDR;
const AUDIOSYS_TUNING_WORD_ADDR: u32 = AUDIOSYS_BASE_ADDR + 4;
const AUDIOSYS_ENABLE_BIT: u32 = 1;
const MEM_SIZE_WORD: u8 = 2;
const TEST_TUNING_WORD: u32 = 0x1000_0000;
const RESET_SETTLE_CYCLES: usize = 6;

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

    wait_for_response(dut, 32);
    assert_eq!(
        dut.mem_d_rdata, 0,
        "writes should acknowledge with zero data"
    );

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

    wait_for_response(dut, 32);
    let rdata = dut.mem_d_rdata;

    dut.mem_d_ready = 1;
    clock_cycle!(dut);
    dut.mem_d_ready = 0;
    dut.eval();

    rdata
}

fn audio_dac_stays_low_for_cycles(dut: &mut AudiosysPeripheralTestWrapper, cycles: usize) {
    for _ in 0..cycles {
        assert_eq!(dut.audio_dac, 0, "expected muted audio output");
        clock_cycle!(dut);
    }
}

fn wait_for_muted_audio_window(
    dut: &mut AudiosysPeripheralTestWrapper,
    consecutive_low_cycles: usize,
    max_cycles: usize,
) {
    let mut observed_low_cycles = 0usize;

    for _ in 0..max_cycles {
        if dut.audio_dac == 0 {
            observed_low_cycles += 1;
            if observed_low_cycles == consecutive_low_cycles {
                return;
            }
        } else {
            observed_low_cycles = 0;
        }
        clock_cycle!(dut);
    }

    panic!("timed out waiting for audiosys output to mute");
}

#[test]
fn test_audiosys_registers_reset_low_and_mask_reserved_control_bits() {
    let runtime =
        create_audiosys_peripheral_runtime().expect("Failed to create audiosys peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<AudiosysPeripheralTestWrapper>()
        .expect("Failed to create audiosys peripheral model");

    reset(&mut dut);

    assert_eq!(read_access(&mut dut, AUDIOSYS_TUNING_WORD_ADDR), 0);
    assert_eq!(read_access(&mut dut, AUDIOSYS_CONTROL_ADDR), 0);

    write_access(&mut dut, AUDIOSYS_CONTROL_ADDR, u32::MAX);
    assert_eq!(
        read_access(&mut dut, AUDIOSYS_CONTROL_ADDR),
        AUDIOSYS_ENABLE_BIT,
        "reserved control bits must read back as zero"
    );
}

#[test]
fn test_audiosys_control_register_is_at_base_addr() {
    let runtime =
        create_audiosys_peripheral_runtime().expect("Failed to create audiosys peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<AudiosysPeripheralTestWrapper>()
        .expect("Failed to create audiosys peripheral model");

    reset(&mut dut);

    write_access(&mut dut, AUDIOSYS_TUNING_WORD_ADDR, TEST_TUNING_WORD);
    assert_eq!(
        read_access(&mut dut, AUDIOSYS_TUNING_WORD_ADDR),
        TEST_TUNING_WORD
    );

    write_access(&mut dut, AUDIOSYS_CONTROL_ADDR, AUDIOSYS_ENABLE_BIT);
    assert_eq!(
        read_access(&mut dut, AUDIOSYS_CONTROL_ADDR),
        AUDIOSYS_ENABLE_BIT
    );
}

#[test]
fn test_audiosys_disable_mutes_output() {
    let runtime =
        create_audiosys_peripheral_runtime().expect("Failed to create audiosys peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<AudiosysPeripheralTestWrapper>()
        .expect("Failed to create audiosys peripheral model");

    reset(&mut dut);

    write_access(&mut dut, AUDIOSYS_TUNING_WORD_ADDR, TEST_TUNING_WORD);
    write_access(&mut dut, AUDIOSYS_CONTROL_ADDR, AUDIOSYS_ENABLE_BIT);

    write_access(&mut dut, AUDIOSYS_CONTROL_ADDR, 0);
    assert_eq!(read_access(&mut dut, AUDIOSYS_CONTROL_ADDR), 0);
    audio_dac_stays_low_for_cycles(&mut dut, 256);
    wait_for_muted_audio_window(&mut dut, 128, 2048);
}
