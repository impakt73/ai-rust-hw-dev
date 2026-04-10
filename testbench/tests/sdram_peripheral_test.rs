use riscv_core::AsDynamicVerilatedModel;
use riscv_core::{create_sdram_peripheral_runtime, SdramPeripheralTestWrapper};

const SDRAM_BASE_ADDR: u32 = 0x1000_0000;
const SDRAM_ADDR_SIZE: u32 = 0x0000_0100;
const SIZE_BYTE: u8 = 0b00;
const SIZE_HALFWORD: u8 = 0b01;
const SIZE_WORD: u8 = 0b10;
const RESPONSE_TIMEOUT_CYCLES: usize = 64;
const RESET_SETTLE_CYCLES: usize = 6;

macro_rules! clock_cycle {
    ($dut:expr) => {
        $dut.sys_clk = 0;
        $dut.sdram_clk = 0;
        $dut.eval();
        $dut.sys_clk = 1;
        $dut.sdram_clk = 1;
        $dut.eval();
        $dut.sys_clk = 0;
        $dut.sdram_clk = 0;
        $dut.eval();
    };
}

fn reset(dut: &mut SdramPeripheralTestWrapper) {
    dut.rst = 1;
    dut.mem_a_addr = 0;
    dut.mem_a_wdata = 0;
    dut.mem_a_we = 0;
    dut.mem_a_size = SIZE_WORD;
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

fn wait_for_response(dut: &mut SdramPeripheralTestWrapper, max_cycles: usize) {
    for _ in 0..max_cycles {
        if dut.mem_d_valid != 0 {
            return;
        }
        clock_cycle!(dut);
    }

    panic!("timed out waiting for sdram peripheral response");
}

fn write_access_with_size(dut: &mut SdramPeripheralTestWrapper, addr: u32, wdata: u32, size: u8) {
    dut.mem_a_addr = addr;
    dut.mem_a_wdata = wdata;
    dut.mem_a_we = 1;
    dut.mem_a_size = size;
    dut.mem_a_valid = 1;
    dut.eval();

    assert_eq!(dut.mem_a_ready, 1, "expected SDRAM request to be accepted");

    clock_cycle!(dut);
    dut.mem_a_valid = 0;
    dut.mem_a_we = 0;
    dut.eval();

    wait_for_response(dut, RESPONSE_TIMEOUT_CYCLES);
    assert_eq!(dut.mem_d_rdata, 0, "writes should respond with zero data");

    dut.mem_d_ready = 1;
    clock_cycle!(dut);
    dut.mem_d_ready = 0;
    dut.eval();
}

fn write_access(dut: &mut SdramPeripheralTestWrapper, addr: u32, wdata: u32) {
    write_access_with_size(dut, addr, wdata, SIZE_WORD);
}

fn read_access_with_size(dut: &mut SdramPeripheralTestWrapper, addr: u32, size: u8) -> u32 {
    dut.mem_a_addr = addr;
    dut.mem_a_wdata = 0;
    dut.mem_a_we = 0;
    dut.mem_a_size = size;
    dut.mem_a_valid = 1;
    dut.eval();

    assert_eq!(dut.mem_a_ready, 1, "expected SDRAM request to be accepted");

    clock_cycle!(dut);
    dut.mem_a_valid = 0;
    dut.eval();

    wait_for_response(dut, RESPONSE_TIMEOUT_CYCLES);
    let rdata = dut.mem_d_rdata;

    dut.mem_d_ready = 1;
    clock_cycle!(dut);
    dut.mem_d_ready = 0;
    dut.eval();

    rdata
}

fn read_access(dut: &mut SdramPeripheralTestWrapper, addr: u32) -> u32 {
    read_access_with_size(dut, addr, SIZE_WORD)
}

#[test]
fn test_sdram_peripheral_aligned_word_access_round_trips_through_stub_memory() {
    let runtime =
        create_sdram_peripheral_runtime().expect("Failed to create SDRAM peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<SdramPeripheralTestWrapper>()
        .expect("Failed to create SDRAM peripheral model");

    reset(&mut dut);

    write_access(&mut dut, SDRAM_BASE_ADDR, 0xDEAD_BEEF);

    assert_eq!(
        dut.burst_rd_count, 0,
        "aligned word writes should use the direct burst-write path"
    );
    assert_eq!(
        dut.burstwr_strobe_count, 2,
        "aligned word writes should emit two 16-bit burst write strobes"
    );
    assert_eq!(read_access(&mut dut, SDRAM_BASE_ADDR), 0xDEAD_BEEF);
}

#[test]
fn test_sdram_peripheral_split_word_write_updates_both_words() {
    let runtime =
        create_sdram_peripheral_runtime().expect("Failed to create SDRAM peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<SdramPeripheralTestWrapper>()
        .expect("Failed to create SDRAM peripheral model");

    reset(&mut dut);

    write_access(&mut dut, SDRAM_BASE_ADDR, 0x1122_3344);
    write_access(&mut dut, SDRAM_BASE_ADDR + 4, 0x5566_7788);

    let burst_rd_before = dut.burst_rd_count;
    let burstwr_before = dut.burstwr_strobe_count;

    write_access_with_size(&mut dut, SDRAM_BASE_ADDR + 1, 0xAABB_CCDD, SIZE_WORD);

    assert_eq!(
        dut.burst_rd_count - burst_rd_before,
        1,
        "split word writes should perform one read-modify-write read burst"
    );
    assert_eq!(
        dut.burstwr_strobe_count - burstwr_before,
        4,
        "split word writes should rewrite both affected 32-bit words"
    );
    assert_eq!(read_access(&mut dut, SDRAM_BASE_ADDR), 0xBBCC_DD44);
    assert_eq!(read_access(&mut dut, SDRAM_BASE_ADDR + 4), 0x5566_77AA);
    assert_eq!(
        read_access_with_size(&mut dut, SDRAM_BASE_ADDR + 1, SIZE_WORD),
        0xAABB_CCDD
    );
}

#[test]
fn test_sdram_peripheral_split_halfword_write_updates_both_words() {
    let runtime =
        create_sdram_peripheral_runtime().expect("Failed to create SDRAM peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<SdramPeripheralTestWrapper>()
        .expect("Failed to create SDRAM peripheral model");

    reset(&mut dut);

    write_access(&mut dut, SDRAM_BASE_ADDR, 0x1122_3344);
    write_access(&mut dut, SDRAM_BASE_ADDR + 4, 0x5566_7788);

    let burst_rd_before = dut.burst_rd_count;
    let burstwr_before = dut.burstwr_strobe_count;

    write_access_with_size(&mut dut, SDRAM_BASE_ADDR + 3, 0x0000_ABCD, SIZE_HALFWORD);

    assert_eq!(
        dut.burst_rd_count - burst_rd_before,
        1,
        "cross-word halfword writes should perform one read burst"
    );
    assert_eq!(
        dut.burstwr_strobe_count - burstwr_before,
        4,
        "cross-word halfword writes should rewrite both affected words"
    );
    assert_eq!(read_access(&mut dut, SDRAM_BASE_ADDR), 0xCD22_3344);
    assert_eq!(read_access(&mut dut, SDRAM_BASE_ADDR + 4), 0x5566_77AB);
    assert_eq!(
        read_access_with_size(&mut dut, SDRAM_BASE_ADDR + 3, SIZE_HALFWORD),
        0x0000_ABCD
    );
}

#[test]
fn test_sdram_peripheral_out_of_range_requests_return_zero_without_bursts() {
    let runtime =
        create_sdram_peripheral_runtime().expect("Failed to create SDRAM peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<SdramPeripheralTestWrapper>()
        .expect("Failed to create SDRAM peripheral model");

    reset(&mut dut);

    let burst_rd_before = dut.burst_rd_count;
    let burstwr_before = dut.burstwr_strobe_count;
    let out_of_range_addr = SDRAM_BASE_ADDR + SDRAM_ADDR_SIZE;

    assert_eq!(read_access(&mut dut, out_of_range_addr), 0);
    write_access(&mut dut, out_of_range_addr, 0xCAFE_BABE);

    assert_eq!(
        dut.burst_rd_count, burst_rd_before,
        "out-of-range accesses should not issue burst reads"
    );
    assert_eq!(
        dut.burstwr_strobe_count, burstwr_before,
        "out-of-range accesses should not issue burst writes"
    );
}

#[test]
fn test_sdram_peripheral_response_holds_under_backpressure() {
    let runtime =
        create_sdram_peripheral_runtime().expect("Failed to create SDRAM peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<SdramPeripheralTestWrapper>()
        .expect("Failed to create SDRAM peripheral model");

    reset(&mut dut);
    write_access(&mut dut, SDRAM_BASE_ADDR, 0xCAFE_BABE);

    dut.mem_a_addr = SDRAM_BASE_ADDR;
    dut.mem_a_wdata = 0;
    dut.mem_a_we = 0;
    dut.mem_a_size = SIZE_WORD;
    dut.mem_a_valid = 1;
    dut.mem_d_ready = 0;
    dut.eval();

    assert_eq!(dut.mem_a_ready, 1, "expected read request to be accepted");

    clock_cycle!(dut);
    dut.mem_a_valid = 0;
    dut.eval();

    wait_for_response(&mut dut, RESPONSE_TIMEOUT_CYCLES);
    let held_rdata = dut.mem_d_rdata;
    assert_eq!(held_rdata, 0xCAFE_BABE);

    for _ in 0..3 {
        clock_cycle!(dut);
        assert_eq!(
            dut.mem_d_valid, 1,
            "D valid should remain asserted under backpressure"
        );
        assert_eq!(
            dut.mem_d_rdata, held_rdata,
            "D data should remain stable under backpressure"
        );
    }

    dut.mem_d_ready = 1;
    clock_cycle!(dut);
    dut.mem_d_ready = 0;
    dut.eval();

    assert_eq!(
        dut.mem_d_valid, 0,
        "D valid should clear after the response is accepted"
    );
}

#[test]
fn test_sdram_peripheral_byte_reads_extract_expected_lane() {
    let runtime =
        create_sdram_peripheral_runtime().expect("Failed to create SDRAM peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<SdramPeripheralTestWrapper>()
        .expect("Failed to create SDRAM peripheral model");

    reset(&mut dut);
    write_access(&mut dut, SDRAM_BASE_ADDR, 0xA1B2_C3D4);

    assert_eq!(
        read_access_with_size(&mut dut, SDRAM_BASE_ADDR, SIZE_BYTE),
        0x0000_00D4
    );
    assert_eq!(
        read_access_with_size(&mut dut, SDRAM_BASE_ADDR + 1, SIZE_BYTE),
        0x0000_00C3
    );
    assert_eq!(
        read_access_with_size(&mut dut, SDRAM_BASE_ADDR + 2, SIZE_BYTE),
        0x0000_00B2
    );
    assert_eq!(
        read_access_with_size(&mut dut, SDRAM_BASE_ADDR + 3, SIZE_BYTE),
        0x0000_00A1
    );
}
