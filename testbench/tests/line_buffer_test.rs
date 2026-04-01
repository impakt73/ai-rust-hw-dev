use riscv_core::AsDynamicVerilatedModel;
use riscv_core::{create_line_buffer_runtime, LineBufferTestWrapper};

// Test wrapper parameters: PIXEL_WIDTH=8, MAX_LINE_WIDTH=16, SYNC_STAGES=2
const MAX_LINE_WIDTH: usize = 16;
const SYNC_STAGES: usize = 2;
// sync_dpram has 2-cycle read latency
const DPRAM_READ_LATENCY: usize = 2;
// Maximum cycles to wait for CDC propagation (sync stages + margin)
const CDC_TIMEOUT: usize = SYNC_STAGES + DPRAM_READ_LATENCY + 10;

/// Tick both clocks simultaneously (both rising edges)
fn tick_both(dut: &mut LineBufferTestWrapper) {
    dut.wr_clk = 0;
    dut.rd_clk = 0;
    dut.eval();
    dut.wr_clk = 1;
    dut.rd_clk = 1;
    dut.eval();
    dut.wr_clk = 0;
    dut.rd_clk = 0;
    dut.eval();
}

/// Tick only the write clock
fn tick_wr(dut: &mut LineBufferTestWrapper) {
    dut.wr_clk = 0;
    dut.rd_clk = 0;
    dut.eval();
    dut.wr_clk = 1;
    dut.eval();
    dut.wr_clk = 0;
    dut.eval();
}

/// Tick only the read clock
fn tick_rd(dut: &mut LineBufferTestWrapper) {
    dut.wr_clk = 0;
    dut.rd_clk = 0;
    dut.eval();
    dut.rd_clk = 1;
    dut.eval();
    dut.rd_clk = 0;
    dut.eval();
}

/// Apply reset on both domains
fn reset(dut: &mut LineBufferTestWrapper) {
    dut.wr_rst = 1;
    dut.rd_rst = 1;
    dut.wr_valid = 0;
    dut.wr_eol = 0;
    dut.wr_sof = 0;
    dut.wr_data = 0;
    dut.rd_ready = 0;
    for _ in 0..5 {
        tick_both(dut);
    }
    dut.wr_rst = 0;
    dut.rd_rst = 0;
    for _ in 0..3 {
        tick_both(dut);
    }
}

/// Write a single pixel (asserts wr_valid for one wr_clk, optionally with eol)
fn write_pixel(dut: &mut LineBufferTestWrapper, data: u8, eol: bool) {
    dut.wr_data = data;
    dut.wr_valid = 1;
    dut.wr_eol = if eol { 1 } else { 0 };
    tick_wr(dut);
    dut.wr_valid = 0;
    dut.wr_eol = 0;
}

/// Write a complete line of pixels (last pixel has eol asserted)
fn write_line(dut: &mut LineBufferTestWrapper, pixels: &[u8]) {
    assert!(!pixels.is_empty(), "line must have at least one pixel");
    assert!(
        pixels.len() <= MAX_LINE_WIDTH,
        "line exceeds MAX_LINE_WIDTH"
    );

    for (i, &px) in pixels.iter().enumerate() {
        let is_last = i == pixels.len() - 1;
        assert_eq!(dut.wr_ready, 1, "wr_ready must be high to accept pixel");
        write_pixel(dut, px, is_last);
    }
}

/// Wait for rd_valid to assert, with timeout. Returns true if rd_valid found.
fn wait_for_rd_valid(dut: &mut LineBufferTestWrapper, max_cycles: usize) -> bool {
    for _ in 0..max_cycles {
        if dut.rd_valid != 0 {
            return true;
        }
        tick_rd(dut);
    }
    false
}

/// Read all pixels of a line. Returns (pixels, saw_eol, saw_sof_on_first_pixel).
fn read_line(dut: &mut LineBufferTestWrapper) -> (Vec<u8>, bool, bool) {
    let mut pixels = Vec::new();
    let mut saw_eol = false;
    let mut saw_sof_on_first = false;

    // Wait for first pixel
    assert!(
        wait_for_rd_valid(dut, CDC_TIMEOUT * 4),
        "timed out waiting for rd_valid at start of line"
    );

    loop {
        assert_eq!(dut.rd_valid, 1, "rd_valid must be asserted");
        let pixel = dut.rd_data;
        let is_eol = dut.rd_eol != 0;
        let is_sof = dut.rd_sof != 0;

        if pixels.is_empty() {
            saw_sof_on_first = is_sof;
        }
        pixels.push(pixel);

        if is_eol {
            saw_eol = true;
            // Consume the last pixel
            dut.rd_ready = 1;
            tick_rd(dut);
            dut.rd_ready = 0;
            break;
        }

        // Consume this pixel
        dut.rd_ready = 1;
        tick_rd(dut);
        dut.rd_ready = 0;

        // Wait for next pixel (or check if available immediately)
        if dut.rd_valid == 0 {
            assert!(
                wait_for_rd_valid(dut, CDC_TIMEOUT * 2),
                "timed out waiting for next pixel in line after {} pixels",
                pixels.len()
            );
        }
    }

    (pixels, saw_eol, saw_sof_on_first)
}

/// Allow CDC propagation by ticking both clocks
fn propagate_cdc(dut: &mut LineBufferTestWrapper, cycles: usize) {
    for _ in 0..cycles {
        tick_both(dut);
    }
}

// ============================================================================
//  Tests
// ============================================================================

#[test]
fn test_reset_state() {
    let runtime = create_line_buffer_runtime().expect("Failed to create line_buffer runtime");
    let mut dut = runtime
        .create_model_simple::<LineBufferTestWrapper>()
        .expect("Failed to create line_buffer model");

    reset(&mut dut);

    // After reset, writer should be ready to accept pixels
    assert_eq!(dut.wr_ready, 1, "wr_ready must be high after reset");
    // Reader should have no data
    assert_eq!(dut.rd_valid, 0, "rd_valid must be low after reset");
    assert_eq!(dut.rd_eol, 0, "rd_eol must be low after reset");
    assert_eq!(dut.rd_sof, 0, "rd_sof must be low after reset");
}

#[test]
fn test_single_pixel_line() {
    let runtime = create_line_buffer_runtime().expect("Failed to create line_buffer runtime");
    let mut dut = runtime
        .create_model_simple::<LineBufferTestWrapper>()
        .expect("Failed to create line_buffer model");

    reset(&mut dut);

    // Write a single-pixel line
    write_line(&mut dut, &[0xAB]);

    // Let CDC propagate
    propagate_cdc(&mut dut, CDC_TIMEOUT);

    // Read it back
    let (pixels, saw_eol, _) = read_line(&mut dut);
    assert_eq!(pixels, vec![0xAB], "single pixel data mismatch");
    assert!(saw_eol, "eol must be asserted for single-pixel line");
}

#[test]
fn test_multi_pixel_line() {
    let runtime = create_line_buffer_runtime().expect("Failed to create line_buffer runtime");
    let mut dut = runtime
        .create_model_simple::<LineBufferTestWrapper>()
        .expect("Failed to create line_buffer model");

    reset(&mut dut);

    let line: Vec<u8> = (0..8).collect();
    write_line(&mut dut, &line);

    propagate_cdc(&mut dut, CDC_TIMEOUT);

    let (pixels, saw_eol, _) = read_line(&mut dut);
    assert_eq!(pixels, line, "multi-pixel line data mismatch");
    assert!(saw_eol, "eol must be asserted at end of line");
}

#[test]
fn test_max_width_line() {
    let runtime = create_line_buffer_runtime().expect("Failed to create line_buffer runtime");
    let mut dut = runtime
        .create_model_simple::<LineBufferTestWrapper>()
        .expect("Failed to create line_buffer model");

    reset(&mut dut);

    // Write a full MAX_LINE_WIDTH line
    let line: Vec<u8> = (0..MAX_LINE_WIDTH as u8).collect();
    write_line(&mut dut, &line);

    propagate_cdc(&mut dut, CDC_TIMEOUT);

    let (pixels, saw_eol, _) = read_line(&mut dut);
    assert_eq!(pixels, line, "max-width line data mismatch");
    assert!(saw_eol, "eol must be asserted for max-width line");
}

#[test]
fn test_double_buffer_two_lines() {
    let runtime = create_line_buffer_runtime().expect("Failed to create line_buffer runtime");
    let mut dut = runtime
        .create_model_simple::<LineBufferTestWrapper>()
        .expect("Failed to create line_buffer model");

    reset(&mut dut);

    // Write line 1 to buffer 0
    let line1: Vec<u8> = vec![0x10, 0x11, 0x12, 0x13];
    write_line(&mut dut, &line1);

    // Writer should be ready to immediately write line 2 (buffer 1 is free)
    assert_eq!(
        dut.wr_ready, 1,
        "wr_ready must stay high for second line (double buffering)"
    );

    // Write line 2 to buffer 1
    let line2: Vec<u8> = vec![0x20, 0x21, 0x22];
    write_line(&mut dut, &line2);

    // Propagate CDC and read both lines
    propagate_cdc(&mut dut, CDC_TIMEOUT);

    let (pixels1, eol1, _) = read_line(&mut dut);
    assert_eq!(pixels1, line1, "line 1 data mismatch");
    assert!(eol1, "line 1 must end with eol");

    // After reading line 1, line 2 should become available
    propagate_cdc(&mut dut, CDC_TIMEOUT);

    let (pixels2, eol2, _) = read_line(&mut dut);
    assert_eq!(pixels2, line2, "line 2 data mismatch");
    assert!(eol2, "line 2 must end with eol");
}

#[test]
fn test_writer_stalls_when_both_buffers_full() {
    let runtime = create_line_buffer_runtime().expect("Failed to create line_buffer runtime");
    let mut dut = runtime
        .create_model_simple::<LineBufferTestWrapper>()
        .expect("Failed to create line_buffer model");

    reset(&mut dut);

    // Write line 1 (buffer 0)
    write_line(&mut dut, &[0xAA, 0xBB]);

    // Write line 2 (buffer 1)
    write_line(&mut dut, &[0xCC, 0xDD]);

    // Now both buffers are full. Writer should stall.
    assert_eq!(
        dut.wr_ready, 0,
        "wr_ready must be low when both buffers are full"
    );

    // Read line 1 to free buffer 0
    propagate_cdc(&mut dut, CDC_TIMEOUT);
    let (pixels1, _, _) = read_line(&mut dut);
    assert_eq!(pixels1, vec![0xAA, 0xBB]);

    // After reading and CDC propagation, writer should recover
    propagate_cdc(&mut dut, CDC_TIMEOUT);
    assert_eq!(
        dut.wr_ready, 1,
        "wr_ready must recover after buffer is freed"
    );

    // Can now write line 3
    write_line(&mut dut, &[0xEE, 0xFF]);

    // Read line 2
    propagate_cdc(&mut dut, CDC_TIMEOUT);
    let (pixels2, _, _) = read_line(&mut dut);
    assert_eq!(pixels2, vec![0xCC, 0xDD]);

    // Read line 3
    propagate_cdc(&mut dut, CDC_TIMEOUT);
    let (pixels3, _, _) = read_line(&mut dut);
    assert_eq!(pixels3, vec![0xEE, 0xFF]);
}

#[test]
fn test_varying_line_lengths() {
    let runtime = create_line_buffer_runtime().expect("Failed to create line_buffer runtime");
    let mut dut = runtime
        .create_model_simple::<LineBufferTestWrapper>()
        .expect("Failed to create line_buffer model");

    reset(&mut dut);

    let lines: Vec<Vec<u8>> = vec![
        vec![0x01],                                     // 1 pixel
        vec![0x10, 0x11, 0x12, 0x13, 0x14, 0x15],      // 6 pixels
        vec![0x20, 0x21],                               // 2 pixels
        (0..MAX_LINE_WIDTH as u8).collect(),             // max pixels
    ];

    for (i, line) in lines.iter().enumerate() {
        write_line(&mut dut, line);
        propagate_cdc(&mut dut, CDC_TIMEOUT);

        let (pixels, saw_eol, _) = read_line(&mut dut);
        assert_eq!(pixels, *line, "line {} data mismatch", i);
        assert!(saw_eol, "line {} missing eol", i);

        // Give time for rd_bank toggle to propagate back to write domain
        propagate_cdc(&mut dut, CDC_TIMEOUT);
    }
}

#[test]
fn test_sof_resets_pointers() {
    let runtime = create_line_buffer_runtime().expect("Failed to create line_buffer runtime");
    let mut dut = runtime
        .create_model_simple::<LineBufferTestWrapper>()
        .expect("Failed to create line_buffer model");

    reset(&mut dut);

    // Write and read a line to advance the internal state
    write_line(&mut dut, &[0xAA, 0xBB, 0xCC]);
    propagate_cdc(&mut dut, CDC_TIMEOUT);
    let (pixels, _, _) = read_line(&mut dut);
    assert_eq!(pixels, vec![0xAA, 0xBB, 0xCC]);
    propagate_cdc(&mut dut, CDC_TIMEOUT);

    // Assert SOF to reset everything
    dut.wr_sof = 1;
    tick_wr(&mut dut);
    dut.wr_sof = 0;

    // Allow SOF to propagate to read domain
    propagate_cdc(&mut dut, CDC_TIMEOUT * 2);

    // Writer should be ready
    assert_eq!(
        dut.wr_ready, 1,
        "wr_ready must be high after SOF reset"
    );

    // Write a new line after SOF
    write_line(&mut dut, &[0x11, 0x22]);
    propagate_cdc(&mut dut, CDC_TIMEOUT);

    // Read and verify - should get rd_sof on first pixel
    let (pixels, saw_eol, saw_sof) = read_line(&mut dut);
    assert_eq!(pixels, vec![0x11, 0x22], "data after SOF mismatch");
    assert!(saw_eol, "eol after SOF");
    assert!(saw_sof, "rd_sof must be asserted on first pixel after SOF");
}

#[test]
fn test_sof_clears_after_first_pixel() {
    let runtime = create_line_buffer_runtime().expect("Failed to create line_buffer runtime");
    let mut dut = runtime
        .create_model_simple::<LineBufferTestWrapper>()
        .expect("Failed to create line_buffer model");

    reset(&mut dut);

    // SOF + write a line
    dut.wr_sof = 1;
    tick_wr(&mut dut);
    dut.wr_sof = 0;
    propagate_cdc(&mut dut, CDC_TIMEOUT);

    write_line(&mut dut, &[0x01, 0x02, 0x03]);
    propagate_cdc(&mut dut, CDC_TIMEOUT);

    // Read first pixel - should have SOF
    assert!(
        wait_for_rd_valid(&mut dut, CDC_TIMEOUT * 4),
        "timed out waiting for first pixel"
    );
    assert_eq!(dut.rd_sof, 1, "rd_sof must be high on first pixel");
    assert_eq!(dut.rd_data, 0x01);

    // Consume first pixel
    dut.rd_ready = 1;
    tick_rd(&mut dut);
    dut.rd_ready = 0;

    // Wait for second pixel
    assert!(
        wait_for_rd_valid(&mut dut, CDC_TIMEOUT * 2),
        "timed out waiting for second pixel"
    );
    // SOF should be cleared for subsequent pixels
    assert_eq!(dut.rd_sof, 0, "rd_sof must be low on second pixel");
    assert_eq!(dut.rd_data, 0x02);
}

#[test]
fn test_rd_ready_backpressure() {
    let runtime = create_line_buffer_runtime().expect("Failed to create line_buffer runtime");
    let mut dut = runtime
        .create_model_simple::<LineBufferTestWrapper>()
        .expect("Failed to create line_buffer model");

    reset(&mut dut);

    let line: Vec<u8> = vec![0xA0, 0xA1, 0xA2, 0xA3];
    write_line(&mut dut, &line);
    propagate_cdc(&mut dut, CDC_TIMEOUT);

    // Wait for first pixel
    assert!(wait_for_rd_valid(&mut dut, CDC_TIMEOUT * 4));

    // Read with backpressure: don't assert rd_ready for several cycles
    let mut read_pixels = Vec::new();
    let mut cycles = 0;
    let max_cycles = 200;

    while cycles < max_cycles {
        if dut.rd_valid != 0 {
            // Apply backpressure: only accept every 5th cycle
            if cycles % 5 == 0 {
                read_pixels.push(dut.rd_data);
                let is_eol = dut.rd_eol != 0;
                dut.rd_ready = 1;
                tick_rd(&mut dut);
                dut.rd_ready = 0;
                if is_eol {
                    break;
                }
            } else {
                // Backpressure: don't accept
                dut.rd_ready = 0;
                tick_rd(&mut dut);
            }
        } else {
            tick_rd(&mut dut);
        }
        cycles += 1;
    }

    assert_eq!(
        read_pixels, line,
        "data must be preserved under backpressure"
    );
}

#[test]
fn test_fast_writer_slow_reader() {
    let runtime = create_line_buffer_runtime().expect("Failed to create line_buffer runtime");
    let mut dut = runtime
        .create_model_simple::<LineBufferTestWrapper>()
        .expect("Failed to create line_buffer model");

    reset(&mut dut);

    // Simulate fast writer / slow reader with different clock rates
    // Writer clocks 3x faster than reader
    let total_lines = 6;
    let mut lines_written = 0;
    let mut lines_read = 0;
    let mut write_data: Vec<Vec<u8>> = Vec::new();
    let mut read_data: Vec<Vec<u8>> = Vec::new();
    let mut current_read_line: Vec<u8> = Vec::new();
    let mut wr_pixel_idx: usize = 0;
    let line_len = 4;

    for step in 0..2000 {
        let wr_rise = true; // Writer clocks every step
        let rd_rise = step % 3 == 0; // Reader clocks every 3rd step

        // Write logic
        if wr_rise && lines_written < total_lines && dut.wr_ready != 0 {
            let pixel = ((lines_written * line_len + wr_pixel_idx) & 0xFF) as u8;
            let is_eol = wr_pixel_idx == line_len - 1;

            dut.wr_data = pixel;
            dut.wr_valid = 1;
            dut.wr_eol = if is_eol { 1 } else { 0 };

            if is_eol {
                let mut line_data: Vec<u8> = Vec::new();
                for j in 0..line_len {
                    line_data.push(((lines_written * line_len + j) & 0xFF) as u8);
                }
                write_data.push(line_data);
                lines_written += 1;
                wr_pixel_idx = 0;
            } else {
                wr_pixel_idx += 1;
            }
        } else {
            dut.wr_valid = 0;
            dut.wr_eol = 0;
        }

        // Read logic
        if rd_rise && dut.rd_valid != 0 {
            current_read_line.push(dut.rd_data);
            dut.rd_ready = 1;

            if dut.rd_eol != 0 {
                read_data.push(current_read_line.clone());
                current_read_line.clear();
                lines_read += 1;
            }
        } else if !rd_rise {
            // Don't change rd_ready on non-read cycles
        } else {
            dut.rd_ready = 0;
        }

        // Clock edges
        dut.wr_clk = 0;
        dut.rd_clk = 0;
        dut.eval();
        dut.wr_clk = if wr_rise { 1 } else { 0 };
        dut.rd_clk = if rd_rise { 1 } else { 0 };
        dut.eval();
        dut.wr_clk = 0;
        dut.rd_clk = 0;
        dut.eval();

        // Clear write signals after the tick
        dut.wr_valid = 0;
        dut.wr_eol = 0;
        if rd_rise {
            dut.rd_ready = 0;
        }

        if lines_read == total_lines {
            break;
        }
    }

    assert_eq!(
        lines_written, total_lines,
        "all lines must be written"
    );
    assert_eq!(lines_read, total_lines, "all lines must be read");
    assert_eq!(
        read_data, write_data,
        "read data must match written data across all lines"
    );
}

#[test]
fn test_continuous_streaming() {
    let runtime = create_line_buffer_runtime().expect("Failed to create line_buffer runtime");
    let mut dut = runtime
        .create_model_simple::<LineBufferTestWrapper>()
        .expect("Failed to create line_buffer model");

    reset(&mut dut);

    // Stream multiple lines continuously (write a line, read it, repeat)
    let num_lines = 8;
    for i in 0..num_lines {
        let line: Vec<u8> = (0..5).map(|j| (i * 5 + j) as u8).collect();
        write_line(&mut dut, &line);
        propagate_cdc(&mut dut, CDC_TIMEOUT);

        let (pixels, saw_eol, _) = read_line(&mut dut);
        assert_eq!(pixels, line, "line {} data mismatch", i);
        assert!(saw_eol, "line {} must end with eol", i);

        // Allow bank release to propagate
        propagate_cdc(&mut dut, CDC_TIMEOUT);
    }
}

#[test]
fn test_two_pixel_line() {
    let runtime = create_line_buffer_runtime().expect("Failed to create line_buffer runtime");
    let mut dut = runtime
        .create_model_simple::<LineBufferTestWrapper>()
        .expect("Failed to create line_buffer model");

    reset(&mut dut);

    write_line(&mut dut, &[0xDE, 0xAD]);
    propagate_cdc(&mut dut, CDC_TIMEOUT);

    let (pixels, saw_eol, _) = read_line(&mut dut);
    assert_eq!(pixels, vec![0xDE, 0xAD]);
    assert!(saw_eol);
}

#[test]
fn test_sof_mid_write_discards_partial() {
    let runtime = create_line_buffer_runtime().expect("Failed to create line_buffer runtime");
    let mut dut = runtime
        .create_model_simple::<LineBufferTestWrapper>()
        .expect("Failed to create line_buffer model");

    reset(&mut dut);

    // Write a complete line first
    write_line(&mut dut, &[0xAA, 0xBB]);
    propagate_cdc(&mut dut, CDC_TIMEOUT);

    // Read it to clear state
    let (p, _, _) = read_line(&mut dut);
    assert_eq!(p, vec![0xAA, 0xBB]);
    propagate_cdc(&mut dut, CDC_TIMEOUT);

    // Start writing a partial line (no EOL)
    write_pixel(&mut dut, 0x01, false);
    write_pixel(&mut dut, 0x02, false);

    // Assert SOF to discard the partial line
    dut.wr_sof = 1;
    tick_wr(&mut dut);
    dut.wr_sof = 0;
    propagate_cdc(&mut dut, CDC_TIMEOUT * 2);

    // Writer should be ready to start fresh
    assert_eq!(dut.wr_ready, 1, "wr_ready after SOF");

    // Write a new line after SOF
    write_line(&mut dut, &[0xCC, 0xDD]);
    propagate_cdc(&mut dut, CDC_TIMEOUT);

    let (pixels, _, saw_sof) = read_line(&mut dut);
    assert_eq!(pixels, vec![0xCC, 0xDD], "new data after SOF");
    assert!(saw_sof, "must see SOF on first pixel of new frame");
}

#[test]
fn test_eol_only_on_last_pixel() {
    let runtime = create_line_buffer_runtime().expect("Failed to create line_buffer runtime");
    let mut dut = runtime
        .create_model_simple::<LineBufferTestWrapper>()
        .expect("Failed to create line_buffer model");

    reset(&mut dut);

    let line: Vec<u8> = vec![0x10, 0x20, 0x30, 0x40, 0x50];
    write_line(&mut dut, &line);
    propagate_cdc(&mut dut, CDC_TIMEOUT);

    // Read each pixel and verify eol is only on the last one
    assert!(wait_for_rd_valid(&mut dut, CDC_TIMEOUT * 4));

    for i in 0..line.len() {
        assert_eq!(dut.rd_valid, 1, "pixel {} must be valid", i);
        assert_eq!(dut.rd_data, line[i], "pixel {} data mismatch", i);

        if i == line.len() - 1 {
            assert_eq!(dut.rd_eol, 1, "eol must be high on last pixel");
        } else {
            assert_eq!(dut.rd_eol, 0, "eol must be low on pixel {}", i);
        }

        dut.rd_ready = 1;
        tick_rd(&mut dut);
        dut.rd_ready = 0;

        if i < line.len() - 1 {
            // Wait for next pixel
            if dut.rd_valid == 0 {
                assert!(
                    wait_for_rd_valid(&mut dut, CDC_TIMEOUT * 2),
                    "timed out on pixel {}",
                    i + 1
                );
            }
        }
    }
}

#[test]
fn test_no_data_before_eol() {
    let runtime = create_line_buffer_runtime().expect("Failed to create line_buffer runtime");
    let mut dut = runtime
        .create_model_simple::<LineBufferTestWrapper>()
        .expect("Failed to create line_buffer model");

    reset(&mut dut);

    // Write some pixels without EOL
    write_pixel(&mut dut, 0x01, false);
    write_pixel(&mut dut, 0x02, false);
    write_pixel(&mut dut, 0x03, false);

    // Propagate CDC
    propagate_cdc(&mut dut, CDC_TIMEOUT * 2);

    // Reader should see no data (writer hasn't completed the line)
    assert_eq!(
        dut.rd_valid, 0,
        "rd_valid must be low before writer completes a line with eol"
    );
}

#[test]
fn test_write_read_alternating_banks() {
    let runtime = create_line_buffer_runtime().expect("Failed to create line_buffer runtime");
    let mut dut = runtime
        .create_model_simple::<LineBufferTestWrapper>()
        .expect("Failed to create line_buffer model");

    reset(&mut dut);

    // Test that bank alternation works correctly over multiple iterations
    for iteration in 0..4 {
        let base = (iteration * 3) as u8;
        let line: Vec<u8> = vec![base, base + 1, base + 2];

        write_line(&mut dut, &line);
        propagate_cdc(&mut dut, CDC_TIMEOUT);

        let (pixels, saw_eol, _) = read_line(&mut dut);
        assert_eq!(
            pixels, line,
            "iteration {} data mismatch",
            iteration
        );
        assert!(saw_eol, "iteration {} missing eol", iteration);

        propagate_cdc(&mut dut, CDC_TIMEOUT);
    }
}
