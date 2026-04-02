use riscv_core::AsDynamicVerilatedModel;
use riscv_core::{
    create_line_buffer_runtime, LineBufferNonpow2TestWrapper, LineBufferTestWrapper,
};

const HANDSHAKE_TIMEOUT_CYCLES: usize = 24;
const READ_TIMEOUT_CYCLES: usize = 32;

macro_rules! tick_buffer {
    ($dut:expr, $wr_rise:expr, $rd_rise:expr) => {{
        $dut.wr_clk = 0;
        $dut.rd_clk = 0;
        $dut.eval();

        $dut.wr_clk = if $wr_rise { 1 } else { 0 };
        $dut.rd_clk = if $rd_rise { 1 } else { 0 };
        $dut.eval();

        $dut.wr_clk = 0;
        $dut.rd_clk = 0;
        $dut.eval();
    }};
}

macro_rules! reset_buffer {
    ($dut:expr) => {{
        $dut.rst = 1;
        $dut.wr_sof = 0;
        $dut.rd_sof = 0;
        $dut.wr_en = 0;
        $dut.rd_en = 0;
        $dut.wdata = 0;
        for _ in 0..3 {
            tick_buffer!($dut, true, true);
        }
        $dut.rst = 0;
        for _ in 0..2 {
            tick_buffer!($dut, true, true);
        }
    }};
}

macro_rules! pulse_sof {
    ($dut:expr) => {{
        $dut.wr_sof = 1;
        $dut.rd_sof = 1;
        tick_buffer!($dut, true, true);
        $dut.wr_sof = 0;
        $dut.rd_sof = 0;
        tick_buffer!($dut, true, true);
    }};
}

macro_rules! wait_for_rd_ready {
    ($dut:expr) => {{
        let mut ready_seen = false;
        for _ in 0..HANDSHAKE_TIMEOUT_CYCLES {
            if $dut.rd_ready != 0 {
                ready_seen = true;
                break;
            }
            tick_buffer!($dut, false, true);
        }
        assert!(ready_seen, "timed out waiting for rd_ready");
    }};
}

macro_rules! wait_for_wr_ready {
    ($dut:expr) => {{
        let mut ready_seen = false;
        for _ in 0..HANDSHAKE_TIMEOUT_CYCLES {
            if $dut.wr_ready != 0 {
                ready_seen = true;
                break;
            }
            tick_buffer!($dut, true, false);
        }
        assert!(ready_seen, "timed out waiting for wr_ready");
    }};
}

#[test]
fn test_line_buffer_basic_transfer_and_ordering() {
    let runtime = create_line_buffer_runtime().expect("Failed to create line buffer runtime");
    let mut dut = testbench::create_testbench_model::<LineBufferTestWrapper>(&runtime)
        .expect("Failed to create line buffer model");

    reset_buffer!(dut);
    assert_eq!(dut.wr_ready, 1, "write side should be ready after reset");
    assert_eq!(dut.rd_ready, 0, "read side should not have a complete line yet");
    assert_eq!(dut.wr_bank, 0, "write bank should start at bank 0");
    assert_eq!(dut.rd_bank, 0, "read bank should start at bank 0");

    let pixels = [0x10u8, 0x11, 0x12, 0x13];
    for &pixel in &pixels {
        dut.wdata = pixel;
        dut.wr_en = 1;
        tick_buffer!(dut, true, false);
    }
    dut.wr_en = 0;

    wait_for_rd_ready!(dut);

    let mut launched = 0usize;
    let mut read_data = Vec::new();
    for _ in 0..READ_TIMEOUT_CYCLES {
        let issue_read = launched < pixels.len() && dut.rd_ready != 0;
        dut.rd_en = if issue_read { 1 } else { 0 };
        tick_buffer!(dut, false, true);
        if issue_read {
            launched += 1;
        }
        if dut.rd_valid != 0 {
            read_data.push(dut.rdata as u8);
        }
        if read_data.len() == pixels.len() {
            break;
        }
    }
    dut.rd_en = 0;

    assert_eq!(launched, pixels.len(), "all reads should launch");
    assert_eq!(
        read_data,
        pixels.to_vec(),
        "line buffer must preserve pixel order"
    );
}

#[test]
fn test_line_buffer_generates_write_and_read_eol_automatically() {
    let runtime = create_line_buffer_runtime().expect("Failed to create line buffer runtime");
    let mut dut = testbench::create_testbench_model::<LineBufferTestWrapper>(&runtime)
        .expect("Failed to create line buffer model");

    reset_buffer!(dut);

    let pixels = [0x20u8, 0x21, 0x22, 0x23];
    let mut wr_eol_flags = Vec::new();
    for &pixel in &pixels {
        dut.wdata = pixel;
        dut.wr_en = 1;
        tick_buffer!(dut, true, false);
        wr_eol_flags.push(dut.wr_eol != 0);
    }
    dut.wr_en = 0;

    wait_for_rd_ready!(dut);

    let mut launched = 0usize;
    let mut rd_eol_flags = Vec::new();
    for _ in 0..READ_TIMEOUT_CYCLES {
        let issue_read = launched < pixels.len() && dut.rd_ready != 0;
        dut.rd_en = if issue_read { 1 } else { 0 };
        tick_buffer!(dut, false, true);
        if issue_read {
            launched += 1;
        }
        if dut.rd_valid != 0 {
            rd_eol_flags.push(dut.rd_eol != 0);
        }
        if rd_eol_flags.len() == pixels.len() {
            break;
        }
    }
    dut.rd_en = 0;

    assert_eq!(
        wr_eol_flags,
        vec![false, false, false, true],
        "wr_eol should only pulse on the last pixel of the line"
    );
    assert_eq!(
        rd_eol_flags,
        vec![false, false, false, true],
        "rd_eol should align with the final returned pixel"
    );
}

#[test]
fn test_line_buffer_start_of_frame_resets_state() {
    let runtime = create_line_buffer_runtime().expect("Failed to create line buffer runtime");
    let mut dut = testbench::create_testbench_model::<LineBufferTestWrapper>(&runtime)
        .expect("Failed to create line buffer model");

    reset_buffer!(dut);

    for &pixel in &[0x30u8, 0x31, 0x32, 0x33] {
        dut.wdata = pixel;
        dut.wr_en = 1;
        tick_buffer!(dut, true, false);
    }
    dut.wr_en = 0;

    wait_for_rd_ready!(dut);
    assert_eq!(dut.wr_bank, 1, "write bank should toggle after a complete line");

    pulse_sof!(dut);

    assert_eq!(dut.wr_bank, 0, "start-of-frame should reset write bank");
    assert_eq!(dut.rd_bank, 0, "start-of-frame should reset read bank");
    assert_eq!(dut.wr_ready, 1, "write side should be ready for the new frame");
    assert_eq!(dut.rd_ready, 0, "old completed line must be discarded on new frame");
    assert_eq!(dut.rd_valid, 0, "read pipeline should clear on new frame");

    let next_pixels = [0x40u8, 0x41, 0x42, 0x43];
    for &pixel in &next_pixels {
        dut.wdata = pixel;
        dut.wr_en = 1;
        tick_buffer!(dut, true, false);
    }
    dut.wr_en = 0;

    wait_for_rd_ready!(dut);

    let mut launched = 0usize;
    let mut read_data = Vec::new();
    for _ in 0..READ_TIMEOUT_CYCLES {
        let issue_read = launched < next_pixels.len() && dut.rd_ready != 0;
        dut.rd_en = if issue_read { 1 } else { 0 };
        tick_buffer!(dut, false, true);
        if issue_read {
            launched += 1;
        }
        if dut.rd_valid != 0 {
            read_data.push(dut.rdata as u8);
        }
        if read_data.len() == next_pixels.len() {
            break;
        }
    }
    dut.rd_en = 0;

    assert_eq!(
        read_data,
        next_pixels.to_vec(),
        "reads after start-of-frame should return only new-frame data"
    );
}

#[test]
fn test_line_buffer_rounded_depth_handles_non_power_of_two_lines() {
    let runtime = create_line_buffer_runtime().expect("Failed to create non-power-of-two runtime");
    let mut dut = testbench::create_testbench_model::<LineBufferNonpow2TestWrapper>(&runtime)
        .expect("Failed to create non-power-of-two line buffer model");

    reset_buffer!(dut);

    let pixels = [0x50u8, 0x51, 0x52];
    let mut wr_eol_flags = Vec::new();
    for &pixel in &pixels {
        dut.wdata = pixel;
        dut.wr_en = 1;
        tick_buffer!(dut, true, false);
        wr_eol_flags.push(dut.wr_eol != 0);
    }
    dut.wr_en = 0;

    wait_for_rd_ready!(dut);

    let mut launched = 0usize;
    let mut read_data = Vec::new();
    let mut rd_eol_flags = Vec::new();
    for _ in 0..READ_TIMEOUT_CYCLES {
        let issue_read = launched < pixels.len() && dut.rd_ready != 0;
        dut.rd_en = if issue_read { 1 } else { 0 };
        tick_buffer!(dut, false, true);
        if issue_read {
            launched += 1;
        }
        if dut.rd_valid != 0 {
            read_data.push(dut.rdata as u8);
            rd_eol_flags.push(dut.rd_eol != 0);
        }
        if read_data.len() == pixels.len() {
            break;
        }
    }
    dut.rd_en = 0;

    for _ in 0..3 {
        tick_buffer!(dut, false, true);
        assert_eq!(
            dut.rd_valid, 0,
            "rounded-up storage must not leak an extra phantom pixel"
        );
    }

    assert_eq!(
        read_data,
        pixels.to_vec(),
        "non-power-of-two line data should remain ordered"
    );
    assert_eq!(wr_eol_flags, vec![false, false, true]);
    assert_eq!(rd_eol_flags, vec![false, false, true]);
}

#[test]
fn test_line_buffer_sustains_one_pixel_per_cycle_after_startup_latency() {
    let runtime = create_line_buffer_runtime().expect("Failed to create line buffer runtime");
    let mut dut = testbench::create_testbench_model::<LineBufferTestWrapper>(&runtime)
        .expect("Failed to create line buffer model");

    reset_buffer!(dut);

    let pixels = [0x60u8, 0x61, 0x62, 0x63];
    for &pixel in &pixels {
        dut.wdata = pixel;
        dut.wr_en = 1;
        tick_buffer!(dut, true, false);
    }
    dut.wr_en = 0;

    wait_for_rd_ready!(dut);

    let mut launched = 0usize;
    let mut valid_cycles = Vec::new();
    let mut observed = Vec::new();
    for cycle in 0..READ_TIMEOUT_CYCLES {
        let issue_read = launched < pixels.len() && dut.rd_ready != 0;
        dut.rd_en = if issue_read { 1 } else { 0 };
        tick_buffer!(dut, false, true);
        if issue_read {
            launched += 1;
        }
        if dut.rd_valid != 0 {
            valid_cycles.push(cycle);
            observed.push(dut.rdata as u8);
        }
        if observed.len() == pixels.len() {
            break;
        }
    }
    dut.rd_en = 0;

    assert_eq!(observed, pixels.to_vec(), "all pixels should emerge in order");
    assert_eq!(valid_cycles.len(), pixels.len(), "all launched reads should return data");

    let first_valid = valid_cycles[0];
    assert_eq!(
        valid_cycles,
        vec![first_valid, first_valid + 1, first_valid + 2, first_valid + 3],
        "after startup latency, rd_valid should remain asserted every cycle"
    );
}

#[test]
fn test_line_buffer_bank_handoff_alternates_lines_between_domains() {
    let runtime = create_line_buffer_runtime().expect("Failed to create line buffer runtime");
    let mut dut = testbench::create_testbench_model::<LineBufferTestWrapper>(&runtime)
        .expect("Failed to create line buffer model");

    reset_buffer!(dut);
    assert_eq!(dut.wr_bank, 0, "frame should start writing bank 0");
    assert_eq!(dut.rd_bank, 0, "frame should start reading bank 0");

    let line0 = [0x70u8, 0x71, 0x72, 0x73];
    for &pixel in &line0 {
        dut.wdata = pixel;
        dut.wr_en = 1;
        tick_buffer!(dut, true, false);
    }
    dut.wr_en = 0;

    assert_eq!(dut.wr_bank, 1, "writer should move to bank 1 after line 0");
    assert_eq!(
        dut.wr_ready, 1,
        "writer should be able to fill bank 1 while bank 0 is being read"
    );

    wait_for_rd_ready!(dut);
    assert_eq!(dut.rd_bank, 0, "reader should consume bank 0 first");

    let line1 = [0x80u8, 0x81, 0x82, 0x83];
    for &pixel in &line1 {
        dut.wdata = pixel;
        dut.wr_en = 1;
        tick_buffer!(dut, true, false);
    }
    dut.wr_en = 0;

    assert_eq!(dut.wr_bank, 0, "writer should wrap back to bank 0 after line 1");
    assert_eq!(
        dut.wr_ready, 0,
        "writer should stall once both banks contain unread lines"
    );

    let mut launched0 = 0usize;
    let mut observed0 = Vec::new();
    for _ in 0..READ_TIMEOUT_CYCLES {
        let issue_read = launched0 < line0.len() && dut.rd_ready != 0;
        dut.rd_en = if issue_read { 1 } else { 0 };
        tick_buffer!(dut, false, true);
        if issue_read {
            launched0 += 1;
        }
        if dut.rd_valid != 0 {
            observed0.push(dut.rdata as u8);
        }
        if observed0.len() == line0.len() {
            break;
        }
    }
    dut.rd_en = 0;

    assert_eq!(dut.rd_bank, 1, "reader should move to bank 1 after line 0");
    assert_eq!(
        observed0,
        line0.to_vec(),
        "first line should read back from bank 0"
    );

    wait_for_wr_ready!(dut);
    assert_eq!(
        dut.wr_bank, 0,
        "writer should become ready to reuse bank 0 after bank 0 is consumed"
    );

    wait_for_rd_ready!(dut);
    assert_eq!(dut.rd_bank, 1, "reader should consume bank 1 for the second line");

    let mut launched1 = 0usize;
    let mut observed1 = Vec::new();
    for _ in 0..READ_TIMEOUT_CYCLES {
        let issue_read = launched1 < line1.len() && dut.rd_ready != 0;
        dut.rd_en = if issue_read { 1 } else { 0 };
        tick_buffer!(dut, false, true);
        if issue_read {
            launched1 += 1;
        }
        if dut.rd_valid != 0 {
            observed1.push(dut.rdata as u8);
        }
        if observed1.len() == line1.len() {
            break;
        }
    }
    dut.rd_en = 0;

    assert_eq!(dut.rd_bank, 0, "reader should wrap back to bank 0 after line 1");
    assert_eq!(
        observed1,
        line1.to_vec(),
        "second line should read back from bank 1"
    );
}
