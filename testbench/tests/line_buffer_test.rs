use riscv_core::AsDynamicVerilatedModel;
use riscv_core::{create_line_buffer_runtime, LineBufferTestWrapper};

const RD_TIMEOUT_CYCLES: usize = 32;

fn tick(dut: &mut LineBufferTestWrapper, wr_rise: bool, rd_rise: bool) {
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

fn reset_line_buffer(dut: &mut LineBufferTestWrapper) {
    dut.rst = 1;
    dut.start_of_frame = 0;
    dut.wr_valid = 0;
    dut.wr_pixel = 0;
    dut.wr_eol = 0;
    dut.rd_ready = 0;

    for _ in 0..3 {
        tick(dut, true, true);
    }

    dut.rst = 0;
    for _ in 0..3 {
        tick(dut, true, true);
    }
}

fn write_pixel(dut: &mut LineBufferTestWrapper, pixel: u8, eol: bool, rd_rise: bool) {
    dut.wr_pixel = pixel;
    dut.wr_eol = if eol { 1 } else { 0 };
    dut.wr_valid = 1;

    let mut accepted = false;
    for _ in 0..RD_TIMEOUT_CYCLES {
        if dut.wr_ready != 0 {
            tick(dut, true, rd_rise);
            accepted = true;
            break;
        }
        tick(dut, true, rd_rise);
    }

    assert!(accepted, "timed out waiting for wr_ready");
    dut.wr_valid = 0;
    dut.wr_eol = 0;
}

fn write_line(dut: &mut LineBufferTestWrapper, pixels: &[u8], rd_rise: bool) {
    for (index, pixel) in pixels.iter().copied().enumerate() {
        write_pixel(dut, pixel, index + 1 == pixels.len(), rd_rise);
    }
}

fn wait_for_rd_valid(dut: &mut LineBufferTestWrapper) {
    for _ in 0..RD_TIMEOUT_CYCLES {
        if dut.rd_valid != 0 {
            return;
        }
        tick(dut, false, true);
    }
    panic!("timed out waiting for rd_valid");
}

fn read_line(dut: &mut LineBufferTestWrapper) -> Vec<(u8, bool)> {
    let mut pixels = Vec::new();

    wait_for_rd_valid(dut);

    loop {
        let pixel = dut.rd_pixel;
        let eol = dut.rd_eol != 0;
        pixels.push((pixel, eol));

        dut.rd_ready = 1;
        tick(dut, false, true);
        dut.rd_ready = 0;

        if eol {
            break;
        }

        assert_ne!(
            dut.rd_valid, 0,
            "rd_valid should remain asserted between pixels of an active line"
        );
    }

    pixels
}

#[test]
fn test_line_buffer_streams_consecutive_pixels_without_read_bubbles() {
    let runtime = create_line_buffer_runtime().expect("Failed to create line_buffer runtime");
    let mut dut = testbench::create_testbench_model::<LineBufferTestWrapper>(&runtime)
        .expect("Failed to create line_buffer model");

    reset_line_buffer(&mut dut);
    write_line(&mut dut, &[0x30, 0x31, 0x32, 0x33], true);

    let observed = read_line(&mut dut);
    assert_eq!(
        observed,
        vec![(0x30, false), (0x31, false), (0x32, false), (0x33, true)],
        "line buffer should stream a complete line without inserting read bubbles"
    );
}

#[test]
fn test_line_buffer_fast_writer_slow_reader_preserves_order_and_eol() {
    let runtime = create_line_buffer_runtime().expect("Failed to create line_buffer runtime");
    let mut dut = testbench::create_testbench_model::<LineBufferTestWrapper>(&runtime)
        .expect("Failed to create line_buffer model");

    reset_line_buffer(&mut dut);

    let lines = [vec![0x10, 0x11, 0x12], vec![0x20, 0x21, 0x22, 0x23]];
    let expected: Vec<(u8, bool)> = lines
        .iter()
        .flat_map(|line| {
            line.iter()
                .enumerate()
                .map(move |(index, pixel)| (*pixel, index + 1 == line.len()))
        })
        .collect();

    let mut observed = Vec::new();
    let mut write_line_index = 0usize;
    let mut write_pixel_index = 0usize;

    for step in 0..160 {
        let rd_rise = step % 3 == 0;
        let rd_do_read = rd_rise && dut.rd_valid != 0;
        dut.rd_ready = if rd_do_read { 1 } else { 0 };
        let read_sample = if rd_do_read {
            Some((dut.rd_pixel, dut.rd_eol != 0))
        } else {
            None
        };

        let mut wr_do_write = false;
        if write_line_index < lines.len() && dut.wr_ready != 0 {
            dut.wr_valid = 1;
            dut.wr_pixel = lines[write_line_index][write_pixel_index];
            dut.wr_eol = if write_pixel_index + 1 == lines[write_line_index].len() {
                1
            } else {
                0
            };
            wr_do_write = true;
        } else {
            dut.wr_valid = 0;
            dut.wr_eol = 0;
        }

        tick(&mut dut, true, rd_rise);

        if let Some(sample) = read_sample {
            observed.push(sample);
        }

        if wr_do_write {
            write_pixel_index += 1;
            if write_pixel_index == lines[write_line_index].len() {
                write_line_index += 1;
                write_pixel_index = 0;
            }
        }

        if observed.len() == expected.len() {
            break;
        }
    }

    assert_eq!(write_line_index, lines.len(), "all queued lines should be written");
    assert_eq!(
        observed, expected,
        "slow read-side draining should preserve pixel order and end-of-line markers"
    );
}

#[test]
fn test_line_buffer_start_of_frame_flushes_partial_line() {
    let runtime = create_line_buffer_runtime().expect("Failed to create line_buffer runtime");
    let mut dut = testbench::create_testbench_model::<LineBufferTestWrapper>(&runtime)
        .expect("Failed to create line_buffer model");

    reset_line_buffer(&mut dut);

    write_pixel(&mut dut, 0x55, false, false);
    write_pixel(&mut dut, 0x56, false, false);

    for _ in 0..6 {
        tick(&mut dut, false, true);
    }
    assert_eq!(
        dut.rd_valid, 0,
        "partial line without wr_eol must not become readable"
    );

    dut.start_of_frame = 1;
    tick(&mut dut, true, true);
    dut.start_of_frame = 0;

    for _ in 0..6 {
        tick(&mut dut, true, true);
    }

    write_line(&mut dut, &[0xA0, 0xA1, 0xA2], true);
    let observed = read_line(&mut dut);

    assert_eq!(
        observed,
        vec![(0xA0, false), (0xA1, false), (0xA2, true)],
        "start_of_frame should discard any partially written line and restart at buffer zero"
    );
}
