use riscv_core::{create_sram_runtime, SramTestWrapper};

fn clock_cycle(dut: &mut SramTestWrapper) {
    dut.clk = 0;
    dut.eval();
    dut.clk = 1;
    dut.eval();
    dut.clk = 0;
    dut.eval();
}

#[test]
fn test_sram_word_write_and_read() {
    let runtime = create_sram_runtime().expect("Failed to create SRAM runtime");
    let mut dut = runtime
        .create_model_simple::<SramTestWrapper>()
        .expect("Failed to create SRAM model");

    dut.we = 0;
    dut.wmask = 0;
    dut.waddr = 0;
    dut.wdata = 0;
    dut.raddr = 3;
    clock_cycle(&mut dut);
    assert_eq!(dut.rdata, 0, "SRAM should initialize to zero");

    dut.we = 1;
    dut.wmask = 0xF;
    dut.waddr = 3;
    dut.wdata = 0xDEADBEEF;
    dut.raddr = 3;
    clock_cycle(&mut dut);

    dut.we = 0;
    clock_cycle(&mut dut);
    assert_eq!(dut.rdata, 0xDEADBEEF, "full-word write should be readable");
}

#[test]
fn test_sram_halfword_write_mask() {
    let runtime = create_sram_runtime().expect("Failed to create SRAM runtime");
    let mut dut = runtime
        .create_model_simple::<SramTestWrapper>()
        .expect("Failed to create SRAM model");

    dut.we = 1;
    dut.wmask = 0xF;
    dut.waddr = 5;
    dut.wdata = 0x1234ABCD;
    dut.raddr = 5;
    clock_cycle(&mut dut);

    dut.wmask = 0x3;
    dut.wdata = 0x00005678;
    clock_cycle(&mut dut);

    dut.we = 0;
    clock_cycle(&mut dut);
    assert_eq!(
        dut.rdata, 0x12345678,
        "lower halfword masked write should preserve upper halfword"
    );
}

#[test]
fn test_sram_byte_write_mask() {
    let runtime = create_sram_runtime().expect("Failed to create SRAM runtime");
    let mut dut = runtime
        .create_model_simple::<SramTestWrapper>()
        .expect("Failed to create SRAM model");

    dut.we = 1;
    dut.wmask = 0xF;
    dut.waddr = 7;
    dut.wdata = 0x11223344;
    dut.raddr = 7;
    clock_cycle(&mut dut);

    dut.wmask = 0x4;
    dut.wdata = 0x00AA0000;
    clock_cycle(&mut dut);

    dut.we = 0;
    clock_cycle(&mut dut);
    assert_eq!(
        dut.rdata, 0x11AA3344,
        "single-byte masked write should update only selected byte lane"
    );
}
