use riscv_core::{create_sync_fifo_runtime, SyncFifoTestWrapper};

fn tick(dut: &mut SyncFifoTestWrapper) {
    dut.clk = 0;
    dut.eval();

    dut.clk = 1;
    dut.eval();

    dut.clk = 0;
    dut.eval();
}

fn reset_fifo(dut: &mut SyncFifoTestWrapper) {
    dut.rst_n = 0;
    dut.wr_en = 0;
    dut.rd_en = 0;
    dut.wdata = 0;
    tick(dut);
    tick(dut);
    dut.rst_n = 1;
    tick(dut);
}

#[test]
fn test_sync_fifo_registered_read_behavior() {
    let runtime = create_sync_fifo_runtime().expect("Failed to create sync_fifo runtime");
    let mut dut = runtime
        .create_model_simple::<SyncFifoTestWrapper>()
        .expect("Failed to create sync_fifo model");

    reset_fifo(&mut dut);
    assert_eq!(dut.empty, 1, "FIFO should be empty after reset");
    assert_eq!(dut.full, 0, "FIFO should not be full after reset");
    assert_eq!(dut.count, 0, "FIFO count should be zero after reset");

    dut.wdata = 0xA5;
    dut.wr_en = 1;
    tick(&mut dut);
    dut.wr_en = 0;

    assert_eq!(dut.empty, 0, "FIFO should contain one entry after a write");
    assert_eq!(dut.count, 1, "FIFO count should increment after a write");
    assert_eq!(
        dut.rdata, 0,
        "RAM-backed FIFO output should not update until the following clock"
    );

    tick(&mut dut);
    assert_eq!(
        dut.rdata, 0xA5,
        "FIFO read data should update one clock after the address is presented"
    );
}

#[test]
fn test_sync_fifo_preserves_order_with_registered_output() {
    let runtime = create_sync_fifo_runtime().expect("Failed to create sync_fifo runtime");
    let mut dut = runtime
        .create_model_simple::<SyncFifoTestWrapper>()
        .expect("Failed to create sync_fifo model");

    reset_fifo(&mut dut);

    for value in [0x10u8, 0x11, 0x12, 0x13] {
        dut.wdata = value;
        dut.wr_en = 1;
        tick(&mut dut);
    }
    dut.wr_en = 0;

    assert_eq!(dut.full, 1, "FIFO should be full after four writes");
    assert_eq!(dut.count, 4, "FIFO count should equal depth when full");

    tick(&mut dut);
    assert_eq!(
        dut.rdata, 0x10,
        "First queued value should appear after a settle clock"
    );

    for expected in [0x10u8, 0x11, 0x12, 0x13] {
        dut.rd_en = 1;
        tick(&mut dut);
        dut.rd_en = 0;
        assert_eq!(dut.rdata, expected, "FIFO must preserve write order");
        tick(&mut dut);
    }

    assert_eq!(
        dut.empty, 1,
        "FIFO should be empty after reading all entries"
    );
    assert_eq!(
        dut.count, 0,
        "FIFO count should return to zero after all reads"
    );
}
