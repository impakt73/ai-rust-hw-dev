use riscv_core::{create_sync_fifo_runtime, SyncFifoTestWrapper};

const READ_DATA_TIMEOUT_CYCLES: usize = 16;

fn tick(dut: &mut SyncFifoTestWrapper) {
    dut.clk = 0;
    dut.eval();

    dut.clk = 1;
    dut.eval();

    dut.clk = 0;
    dut.eval();
}

fn reset_fifo(dut: &mut SyncFifoTestWrapper) {
    dut.rst = 1;
    dut.wr_valid = 0;
    dut.rd_ready = 0;
    dut.wdata = 0;
    tick(dut);
    tick(dut);
    dut.rst = 0;
    tick(dut);
}

fn wait_for_read_data(dut: &mut SyncFifoTestWrapper) {
    // The staged sync_fifo can insert a refill bubble between words, and the
    // sync_dpram output pipeline adds one more cycle to that refill path.
    for _ in 0..READ_DATA_TIMEOUT_CYCLES {
        if dut.rd_valid != 0 {
            return;
        }
        tick(dut);
    }
    panic!("timed out waiting for rd_valid");
}

#[test]
fn test_sync_fifo_ready_valid_first_word_fall_through() {
    let runtime = create_sync_fifo_runtime().expect("Failed to create sync_fifo runtime");
    let mut dut = runtime
        .create_model_simple::<SyncFifoTestWrapper>()
        .expect("Failed to create sync_fifo model");

    reset_fifo(&mut dut);
    assert_eq!(dut.wr_ready, 1, "FIFO should accept writes after reset");
    assert_eq!(
        dut.rd_valid, 0,
        "FIFO should not present read data after reset"
    );
    assert_eq!(dut.count, 0, "FIFO count should be zero after reset");

    dut.wdata = 0xA5;
    dut.wr_valid = 1;
    tick(&mut dut);
    dut.wr_valid = 0;

    assert_eq!(
        dut.wr_ready, 1,
        "FIFO should still be able to accept more writes"
    );
    assert_eq!(
        dut.rd_valid, 1,
        "First write should appear immediately at the output"
    );
    assert_eq!(dut.count, 1, "FIFO count should increment after a write");
    assert_eq!(
        dut.rdata, 0xA5,
        "Output data should match the queued head word"
    );

    dut.rd_ready = 1;
    tick(&mut dut);
    dut.rd_ready = 0;

    assert_eq!(
        dut.rd_valid, 0,
        "FIFO should deassert rd_valid after consuming the last word"
    );
    assert_eq!(
        dut.count, 0,
        "FIFO count should return to zero after a read"
    );
}

#[test]
fn test_sync_fifo_preserves_order_with_ready_valid_reads() {
    let runtime = create_sync_fifo_runtime().expect("Failed to create sync_fifo runtime");
    let mut dut = runtime
        .create_model_simple::<SyncFifoTestWrapper>()
        .expect("Failed to create sync_fifo model");

    reset_fifo(&mut dut);

    for value in [0x10u8, 0x11, 0x12, 0x13] {
        dut.wdata = value;
        dut.wr_valid = 1;
        tick(&mut dut);
    }
    dut.wr_valid = 0;

    assert_eq!(dut.wr_ready, 0, "FIFO should apply backpressure when full");
    assert_eq!(
        dut.rd_valid, 1,
        "FIFO should present the head entry when non-empty"
    );
    assert_eq!(dut.count, 4, "FIFO count should equal depth when full");

    for expected in [0x10u8, 0x11, 0x12, 0x13] {
        wait_for_read_data(&mut dut);
        assert_eq!(dut.rdata, expected, "FIFO must preserve write order");
        dut.rd_ready = 1;
        tick(&mut dut);
        dut.rd_ready = 0;
    }

    assert_eq!(
        dut.rd_valid, 0,
        "FIFO should deassert rd_valid after reading all entries"
    );
    assert_eq!(
        dut.wr_ready, 1,
        "FIFO should accept writes again after draining"
    );
    assert_eq!(
        dut.count, 0,
        "FIFO count should return to zero after all reads"
    );
}

#[test]
fn test_sync_fifo_refill_latency_is_two_cycles() {
    let runtime = create_sync_fifo_runtime().expect("Failed to create sync_fifo runtime");
    let mut dut = runtime
        .create_model_simple::<SyncFifoTestWrapper>()
        .expect("Failed to create sync_fifo model");

    reset_fifo(&mut dut);

    for value in [0x31u8, 0x32] {
        dut.wdata = value;
        dut.wr_valid = 1;
        tick(&mut dut);
    }
    dut.wr_valid = 0;

    assert_eq!(
        dut.rd_valid, 1,
        "head word should be staged before the first read"
    );
    assert_eq!(dut.rdata, 0x31, "first queued word should be visible");

    dut.rd_ready = 1;
    tick(&mut dut);
    dut.rd_ready = 0;

    assert_eq!(
        dut.rd_valid, 0,
        "refill should deassert rd_valid immediately after consuming the staged head word"
    );

    tick(&mut dut);
    assert_eq!(
        dut.rd_valid, 0,
        "first refill cycle should only advance the RAM output pipeline"
    );

    tick(&mut dut);
    assert_eq!(
        dut.rd_valid, 1,
        "second refill cycle should restage the next queued word"
    );
    assert_eq!(
        dut.rdata, 0x32,
        "second queued word should appear after the refill latency"
    );
}

#[test]
fn test_sync_fifo_write_backpressure_and_simultaneous_pop_push() {
    let runtime = create_sync_fifo_runtime().expect("Failed to create sync_fifo runtime");
    let mut dut = runtime
        .create_model_simple::<SyncFifoTestWrapper>()
        .expect("Failed to create sync_fifo model");

    reset_fifo(&mut dut);

    for value in [0x20u8, 0x21, 0x22, 0x23] {
        dut.wdata = value;
        dut.wr_valid = 1;
        tick(&mut dut);
    }
    dut.wr_valid = 0;

    assert_eq!(dut.count, 4, "FIFO should be full before backpressure test");
    assert_eq!(
        dut.wr_ready, 0,
        "wr_ready should drop when the FIFO is full"
    );
    assert_eq!(
        dut.rdata, 0x20,
        "Head word should remain stable while stalled"
    );

    dut.wdata = 0xEE;
    dut.wr_valid = 1;
    tick(&mut dut);
    dut.wr_valid = 0;

    assert_eq!(
        dut.count, 4,
        "Stalled write must not increase FIFO occupancy"
    );
    assert_eq!(
        dut.rdata, 0x20,
        "Blocked write must not disturb the current head word"
    );

    dut.wdata = 0x24;
    dut.wr_valid = 1;
    dut.rd_ready = 1;
    tick(&mut dut);
    dut.wr_valid = 0;
    dut.rd_ready = 0;

    assert_eq!(
        dut.count, 4,
        "Simultaneous pop/push at full should preserve occupancy"
    );

    for expected in [0x21u8, 0x22, 0x23, 0x24] {
        wait_for_read_data(&mut dut);
        assert_eq!(
            dut.rdata, expected,
            "FIFO must keep ordering across pop/push at full"
        );
        dut.rd_ready = 1;
        tick(&mut dut);
        dut.rd_ready = 0;
    }

    assert_eq!(
        dut.rd_valid, 0,
        "FIFO should be empty after draining remaining entries"
    );
    assert_eq!(
        dut.count, 0,
        "FIFO count should return to zero after draining"
    );
}
