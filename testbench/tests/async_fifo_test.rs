use riscv_core::{
    create_async_fifo_runtime, create_async_fifo_sync3_runtime, AsyncFifoSync3Wrapper,
    AsyncFifoTestWrapper,
};

fn tick(dut: &mut AsyncFifoTestWrapper, wr_rise: bool, rd_rise: bool) {
    dut.wr_clk = 0;
    dut.rd_clk = 0;
    dut.eval();

    dut.wr_clk = if wr_rise { 1 } else { 0 };
    dut.rd_clk = if rd_rise { 1 } else { 0 };
    dut.eval();

    dut.wr_clk = 0;
    dut.rd_clk = 0;
    dut.eval();
}

fn tick_sync3(dut: &mut AsyncFifoSync3Wrapper, wr_rise: bool, rd_rise: bool) {
    dut.wr_clk = 0;
    dut.rd_clk = 0;
    dut.eval();

    dut.wr_clk = if wr_rise { 1 } else { 0 };
    dut.rd_clk = if rd_rise { 1 } else { 0 };
    dut.eval();

    dut.wr_clk = 0;
    dut.rd_clk = 0;
    dut.eval();
}

fn reset_fifo(dut: &mut AsyncFifoTestWrapper) {
    dut.rst_n = 0;
    dut.wr_en = 0;
    dut.rd_en = 0;
    dut.wdata = 0;
    for _ in 0..3 {
        tick(dut, true, true);
    }
    dut.rst_n = 1;
    for _ in 0..2 {
        tick(dut, true, true);
    }
}

#[test]
fn test_async_fifo_sync_stage_parameterization() {
    let runtime =
        create_async_fifo_sync3_runtime().expect("Failed to create async_fifo sync3 runtime");
    let mut dut = runtime
        .create_model_simple::<AsyncFifoSync3Wrapper>()
        .expect("Failed to create async_fifo sync3 model");

    dut.rst_n = 0;
    dut.wr_en = 0;
    dut.rd_en = 0;
    dut.wdata = 0;
    for _ in 0..3 {
        tick_sync3(&mut dut, true, true);
    }
    dut.rst_n = 1;
    tick_sync3(&mut dut, true, true);

    dut.wdata = 0x5A;
    dut.wr_en = 1;
    tick_sync3(&mut dut, true, false);
    dut.wr_en = 0;

    // With SYNC_STAGES=3, empty should remain asserted for the first three rd_clk edges.
    tick_sync3(&mut dut, false, true);
    assert_eq!(dut.empty, 1, "empty should still be high after 1 rd edge");
    tick_sync3(&mut dut, false, true);
    assert_eq!(dut.empty, 1, "empty should still be high after 2 rd edges");
    tick_sync3(&mut dut, false, true);
    assert_eq!(dut.empty, 1, "empty should still be high after 3 rd edges");
    tick_sync3(&mut dut, false, true);
    assert_eq!(dut.empty, 0, "empty should deassert after 4 rd edges");
}

#[test]
fn test_async_fifo_basic_full_empty_and_order() {
    let runtime = create_async_fifo_runtime().expect("Failed to create async_fifo runtime");
    let mut dut = runtime
        .create_model_simple::<AsyncFifoTestWrapper>()
        .expect("Failed to create async_fifo model");

    reset_fifo(&mut dut);
    assert_eq!(dut.empty, 1, "FIFO should be empty after reset");
    assert_eq!(dut.full, 0, "FIFO should not be full after reset");
    assert_eq!(dut.count, 0, "FIFO count should be zero after reset");

    for i in 0..4u8 {
        dut.wdata = 0x10 + i;
        dut.wr_en = 1;
        tick(&mut dut, true, false);
    }
    dut.wr_en = 0;

    assert_eq!(dut.full, 1, "FIFO should assert full after 4 writes");
    assert_eq!(dut.count, 4, "FIFO count should equal depth when full");

    dut.wdata = 0xEE;
    dut.wr_en = 1;
    tick(&mut dut, true, false);
    dut.wr_en = 0;
    assert_eq!(dut.count, 4, "overflow write must not increase count");

    for expected in [0x10u8, 0x11, 0x12, 0x13] {
        while dut.empty != 0 {
            tick(&mut dut, false, true);
        }

        dut.rd_en = 1;
        tick(&mut dut, false, true);
        assert_eq!(dut.rdata, expected, "FIFO must preserve write order");
    }
    dut.rd_en = 0;
    tick(&mut dut, false, true);
    assert_eq!(
        dut.empty, 1,
        "FIFO should be empty after reading all entries"
    );

    for _ in 0..3 {
        tick(&mut dut, true, false);
    }
    assert_eq!(dut.full, 0, "full should clear after reads propagate");
    assert_eq!(dut.count, 0, "count should return to zero after all reads");
}

#[test]
fn test_async_fifo_fast_writer_slow_reader() {
    let runtime = create_async_fifo_runtime().expect("Failed to create async_fifo runtime");
    let mut dut = runtime
        .create_model_simple::<AsyncFifoTestWrapper>()
        .expect("Failed to create async_fifo model");

    reset_fifo(&mut dut);

    const TOTAL_WORDS: u8 = 24;
    let mut write_next: u8 = 0;
    let mut read_next: u8 = 0;

    for step in 0..300 {
        let rd_rise = step % 3 == 0;

        let wr_do_write = write_next < TOTAL_WORDS && dut.full == 0;
        dut.wr_en = if wr_do_write { 1 } else { 0 };
        if wr_do_write {
            dut.wdata = write_next;
        }

        let rd_do_read = rd_rise && dut.empty == 0;
        dut.rd_en = if rd_do_read { 1 } else { 0 };

        tick(&mut dut, true, rd_rise);

        if wr_do_write {
            write_next = write_next.wrapping_add(1);
        }
        if rd_do_read {
            assert_eq!(
                dut.rdata, read_next,
                "fast-writer/slow-reader data mismatch at value {}",
                read_next
            );
            read_next = read_next.wrapping_add(1);
        }

        if write_next == TOTAL_WORDS && read_next == TOTAL_WORDS {
            break;
        }
    }

    assert_eq!(write_next, TOTAL_WORDS, "all writes should complete");
    assert_eq!(read_next, TOTAL_WORDS, "all reads should complete");
}

#[test]
fn test_async_fifo_slow_writer_fast_reader() {
    let runtime = create_async_fifo_runtime().expect("Failed to create async_fifo runtime");
    let mut dut = runtime
        .create_model_simple::<AsyncFifoTestWrapper>()
        .expect("Failed to create async_fifo model");

    reset_fifo(&mut dut);

    const TOTAL_WORDS: u8 = 24;
    let mut write_next: u8 = 0;
    let mut read_next: u8 = 0;

    for step in 0..400 {
        let wr_rise = step % 3 == 0;
        let wr_do_write = wr_rise && write_next < TOTAL_WORDS && dut.full == 0;
        dut.wr_en = if wr_do_write { 1 } else { 0 };
        if wr_do_write {
            dut.wdata = write_next;
        }

        let rd_do_read = dut.empty == 0;
        dut.rd_en = if rd_do_read { 1 } else { 0 };

        tick(&mut dut, wr_rise, true);

        if wr_do_write {
            write_next = write_next.wrapping_add(1);
        }
        if rd_do_read {
            assert_eq!(
                dut.rdata, read_next,
                "slow-writer/fast-reader data mismatch at value {}",
                read_next
            );
            read_next = read_next.wrapping_add(1);
        }

        if write_next == TOTAL_WORDS && read_next == TOTAL_WORDS {
            break;
        }
    }

    assert_eq!(write_next, TOTAL_WORDS, "all writes should complete");
    assert_eq!(read_next, TOTAL_WORDS, "all reads should complete");
}
