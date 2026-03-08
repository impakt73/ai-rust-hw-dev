use riscv_core::{create_sram_peripheral_runtime, SramPeripheralTestWrapper};

const SIZE_HALFWORD: u8 = 0b01;
const SIZE_WORD: u8 = 0b10;

fn clock_cycle(dut: &mut SramPeripheralTestWrapper) {
    dut.clk = 0;
    dut.eval();
    dut.clk = 1;
    dut.eval();
}

fn reset(dut: &mut SramPeripheralTestWrapper) {
    dut.mem_a_valid = 0;
    dut.mem_a_we = 0;
    dut.mem_a_addr = 0;
    dut.mem_a_wdata = 0;
    dut.mem_a_size = SIZE_WORD;
    dut.mem_d_ready = 0;
    dut.rst_n = 0;
    clock_cycle(dut);
    clock_cycle(dut);
    dut.rst_n = 1;
    dut.eval();
}

fn write_access(dut: &mut SramPeripheralTestWrapper, addr: u32, wdata: u32, size: u8) -> u32 {
    dut.mem_a_addr = addr;
    dut.mem_a_wdata = wdata;
    dut.mem_a_size = size;
    dut.mem_a_we = 1;
    dut.mem_a_valid = 1;
    dut.eval();
    assert_eq!(dut.mem_a_ready, 1, "SRAM should accept the write request");

    clock_cycle(dut);
    dut.mem_a_valid = 0;
    dut.eval();

    let mut wait_cycles = 0;
    while dut.mem_d_valid == 0 {
        clock_cycle(dut);
        wait_cycles += 1;
    }

    dut.mem_d_ready = 1;
    dut.eval();
    clock_cycle(dut);
    dut.mem_d_ready = 0;
    dut.mem_a_we = 0;
    dut.eval();

    // Count the first D-channel response cycle as part of the operation latency.
    wait_cycles + 1
}

fn read_access(dut: &mut SramPeripheralTestWrapper, addr: u32, size: u8) -> (u32, u32) {
    dut.mem_a_addr = addr;
    dut.mem_a_size = size;
    dut.mem_a_we = 0;
    dut.mem_a_valid = 1;
    dut.eval();
    assert_eq!(dut.mem_a_ready, 1, "SRAM should accept the read request");

    clock_cycle(dut);
    dut.mem_a_valid = 0;
    dut.eval();

    let mut wait_cycles = 0;
    while dut.mem_d_valid == 0 {
        clock_cycle(dut);
        wait_cycles += 1;
    }

    let rdata = dut.mem_d_rdata;
    dut.mem_d_ready = 1;
    dut.eval();
    clock_cycle(dut);
    dut.mem_d_ready = 0;
    dut.eval();

    // Count the first D-channel response cycle as part of the operation latency.
    (rdata, wait_cycles + 1)
}

#[test]
fn test_sram_peripheral_aligned_access_uses_d_channel_completion() {
    let runtime =
        create_sram_peripheral_runtime().expect("Failed to create SRAM peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<SramPeripheralTestWrapper>()
        .expect("Failed to create SRAM peripheral model");
    reset(&mut dut);

    let write_wait = write_access(&mut dut, 0, 0xDEAD_BEEF, SIZE_WORD);
    assert_eq!(
        write_wait, 1,
        "aligned word writes should acknowledge on the next D-channel cycle"
    );

    let (read_data, read_wait) = read_access(&mut dut, 0, SIZE_WORD);
    assert_eq!(
        read_wait, 1,
        "aligned word reads should return one cycle after request acceptance"
    );
    assert_eq!(read_data, 0xDEAD_BEEF);
}

#[test]
fn test_sram_peripheral_unaligned_word_store_and_load() {
    let runtime =
        create_sram_peripheral_runtime().expect("Failed to create SRAM peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<SramPeripheralTestWrapper>()
        .expect("Failed to create SRAM peripheral model");
    reset(&mut dut);

    write_access(&mut dut, 0, 0x1122_3344, SIZE_WORD);
    write_access(&mut dut, 4, 0x5566_7788, SIZE_WORD);

    let split_write_wait = write_access(&mut dut, 1, 0xAABB_CCDD, SIZE_WORD);
    assert_eq!(
        split_write_wait, 2,
        "unaligned word write should wait an extra cycle before D completion"
    );

    let (word0, _) = read_access(&mut dut, 0, SIZE_WORD);
    let (word1, _) = read_access(&mut dut, 4, SIZE_WORD);
    assert_eq!(word0, 0xBBCC_DD44);
    assert_eq!(word1, 0x5566_77AA);

    let (unaligned_word, split_read_wait) = read_access(&mut dut, 1, SIZE_WORD);
    assert_eq!(
        split_read_wait, 2,
        "unaligned word read should use two SRAM read cycles before D completion"
    );
    assert_eq!(unaligned_word, 0xAABB_CCDD);
}

#[test]
fn test_sram_peripheral_unaligned_halfword_store_and_load() {
    let runtime =
        create_sram_peripheral_runtime().expect("Failed to create SRAM peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<SramPeripheralTestWrapper>()
        .expect("Failed to create SRAM peripheral model");
    reset(&mut dut);

    write_access(&mut dut, 0, 0x1122_3344, SIZE_WORD);
    write_access(&mut dut, 4, 0x5566_7788, SIZE_WORD);

    let split_halfword_write_wait = write_access(&mut dut, 3, 0x0000_ABCD, SIZE_HALFWORD);
    assert_eq!(
        split_halfword_write_wait, 2,
        "cross-boundary unaligned halfword write should wait an extra cycle"
    );

    let (word0, _) = read_access(&mut dut, 0, SIZE_WORD);
    let (word1, _) = read_access(&mut dut, 4, SIZE_WORD);
    assert_eq!(word0, 0xCD22_3344);
    assert_eq!(word1, 0x5566_77AB);

    let (unaligned_halfword, split_halfword_read_wait) = read_access(&mut dut, 3, SIZE_HALFWORD);
    assert_eq!(
        split_halfword_read_wait, 2,
        "cross-boundary unaligned halfword read should use two SRAM read cycles"
    );
    assert_eq!(unaligned_halfword, 0x0000_ABCD);

    let (intra_word_halfword, intra_word_wait) = read_access(&mut dut, 1, SIZE_HALFWORD);
    assert_eq!(intra_word_wait, 1);
    assert_eq!(intra_word_halfword, 0x0000_2233);
}

#[test]
fn test_sram_peripheral_access_beyond_8kb() {
    let runtime =
        create_sram_peripheral_runtime().expect("Failed to create SRAM peripheral runtime");
    let mut dut = runtime
        .create_model_simple::<SramPeripheralTestWrapper>()
        .expect("Failed to create SRAM peripheral model");
    reset(&mut dut);

    write_access(&mut dut, 0x0000_0000, 0x1111_2222, SIZE_WORD);
    write_access(&mut dut, 0x0000_2000, 0x3333_4444, SIZE_WORD);

    let (base_word, _) = read_access(&mut dut, 0x0000_0000, SIZE_WORD);
    let (beyond_8kb_word, _) = read_access(&mut dut, 0x0000_2000, SIZE_WORD);

    assert_eq!(base_word, 0x1111_2222);
    assert_eq!(beyond_8kb_word, 0x3333_4444);
}
