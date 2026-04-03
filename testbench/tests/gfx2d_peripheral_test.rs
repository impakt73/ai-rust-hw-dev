use riscv_core::AsDynamicVerilatedModel;
use riscv_core::{create_gfx2d_peripheral_runtime, Gfx2dPeripheralTestWrapper};

const GFX2D_BASE_ADDR: u32 = 0x3000_0000;
const GFX2D_SCROLL_X_ADDR: u32 = GFX2D_BASE_ADDR;
const GFX2D_SCROLL_Y_ADDR: u32 = GFX2D_BASE_ADDR + 4;
const MEM_SIZE_WORD: u8 = 2;

const GFX2D_H_TOTAL: usize = 26;
const GFX2D_V_TOTAL: usize = 19;
const GFX2D_FRAME_CYCLES: usize = GFX2D_H_TOTAL * GFX2D_V_TOTAL;
const GFX2D_ACTIVE_WIDTH: u8 = 16;
const GFX2D_ACTIVE_HEIGHT: u8 = 16;
const GFX2D_SCROLL_X_WRAP_MASK: u8 = GFX2D_ACTIVE_WIDTH - 1;
const GFX2D_SCROLL_Y_WRAP_MASK: u8 = GFX2D_ACTIVE_HEIGHT - 1;
const GFX2D_CHAR_MAP: [u8; 4] = [1, 2, 3, 4];
const GFX2D_FONT_ROWS: [[u8; 8]; 5] = [
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x81, 0x42, 0x24, 0x18, 0x18, 0x24, 0x42, 0x81],
    [0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00],
    [0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0],
    [0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F],
];

macro_rules! clock_cycle {
    ($dut:expr) => {
        // Keep both domains phase-aligned in this unit test so bus writes and
        // raster progress are deterministic while still exercising the CDC path.
        $dut.sys_clk = 0;
        $dut.video_clk = 0;
        $dut.eval();
        $dut.sys_clk = 1;
        $dut.video_clk = 1;
        $dut.eval();
        $dut.sys_clk = 0;
        $dut.video_clk = 0;
        $dut.eval();
    };
}

fn reset(dut: &mut Gfx2dPeripheralTestWrapper) {
    dut.rst = 1;
    dut.mem_a_addr = 0;
    dut.mem_a_wdata = 0;
    dut.mem_a_we = 0;
    dut.mem_a_size = 0;
    dut.mem_a_valid = 0;
    dut.mem_d_ready = 0;

    for _ in 0..6 {
        clock_cycle!(dut);
    }

    dut.rst = 0;
    for _ in 0..6 {
        clock_cycle!(dut);
    }
}

fn wait_for_response(dut: &mut Gfx2dPeripheralTestWrapper, max_cycles: usize) {
    for _ in 0..max_cycles {
        if dut.mem_d_valid != 0 {
            return;
        }
        clock_cycle!(dut);
    }

    panic!("timed out waiting for gfx2d peripheral response");
}

fn write_access(dut: &mut Gfx2dPeripheralTestWrapper, addr: u32, wdata: u32) {
    dut.mem_a_addr = addr;
    dut.mem_a_wdata = wdata;
    dut.mem_a_we = 1;
    dut.mem_a_size = MEM_SIZE_WORD;
    dut.mem_a_valid = 1;
    dut.eval();

    assert_eq!(
        dut.mem_a_ready, 1,
        "expected system bus request to be accepted"
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

fn read_access(dut: &mut Gfx2dPeripheralTestWrapper, addr: u32) -> u32 {
    dut.mem_a_addr = addr;
    dut.mem_a_wdata = 0;
    dut.mem_a_we = 0;
    dut.mem_a_size = MEM_SIZE_WORD;
    dut.mem_a_valid = 1;
    dut.eval();

    assert_eq!(
        dut.mem_a_ready, 1,
        "expected system bus request to be accepted"
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

fn wait_for_active_frame_start(dut: &mut Gfx2dPeripheralTestWrapper, occurrence: usize) {
    let mut seen = 0;
    let mut saw_vsync_pulse = false;

    for _ in 0..(GFX2D_FRAME_CYCLES * 4) {
        clock_cycle!(dut);
        if dut.video_vs == 0 {
            saw_vsync_pulse = true;
        }
        if saw_vsync_pulse && dut.video_vs == 1 && dut.video_de == 1 {
            seen += 1;
            if seen == occurrence {
                return;
            }
            saw_vsync_pulse = false;
        }
    }

    panic!("timed out waiting for active frame start occurrence {occurrence}");
}

fn capture_active_frame_pixels(
    dut: &mut Gfx2dPeripheralTestWrapper,
    occurrence: usize,
) -> Vec<u32> {
    let width = usize::from(GFX2D_ACTIVE_WIDTH);
    let height = usize::from(GFX2D_ACTIVE_HEIGHT);
    let mut pixels = Vec::with_capacity(width * height);

    wait_for_active_frame_start(dut, occurrence);

    for row in 0..height {
        for col in 0..width {
            assert_eq!(
                dut.video_de, 1,
                "expected active video at raster coordinate ({col}, {row})"
            );
            pixels.push(dut.video_rgb);

            if col + 1 != width {
                clock_cycle!(dut);
            }
        }

        if row + 1 != height {
            clock_cycle!(dut);
            // A full horizontal period is a safe upper bound for reaching the
            // next active row after the current line's blanking interval.
            for _ in 0..GFX2D_H_TOTAL {
                if dut.video_de == 1 {
                    break;
                }
                clock_cycle!(dut);
            }

            assert_eq!(
                dut.video_de,
                1,
                "timed out waiting for active video on row {}",
                row + 1
            );
        }
    }

    pixels
}

fn active_frame_pixel(pixels: &[u32], x: u8, y: u8) -> u32 {
    pixels[(usize::from(y) * usize::from(GFX2D_ACTIVE_WIDTH)) + usize::from(x)]
}

fn expected_pixel(x: u8, y: u8) -> u32 {
    let tile_x = usize::from(x / 8);
    let tile_y = usize::from(y / 8);
    let glyph = usize::from(GFX2D_CHAR_MAP[(tile_y * 2) + tile_x]);
    let glyph_row = GFX2D_FONT_ROWS[glyph][usize::from(y % 8)];
    if ((glyph_row >> (7 - (x % 8))) & 1) != 0 {
        0xFF_FF_FF
    } else {
        0x00_00_00
    }
}

fn expected_scrolled_pixel(x: u8, y: u8, scroll_x: u8, scroll_y: u8) -> u32 {
    expected_pixel(
        (x.wrapping_add(scroll_x)) & GFX2D_SCROLL_X_WRAP_MASK,
        (y.wrapping_add(scroll_y)) & GFX2D_SCROLL_Y_WRAP_MASK,
    )
}

#[test]
fn test_gfx2d_scroll_registers_reset_low_and_read_back() {
    let runtime = create_gfx2d_peripheral_runtime().expect("Failed to create gfx2d runtime");
    let mut dut = runtime
        .create_model_simple::<Gfx2dPeripheralTestWrapper>()
        .expect("Failed to create gfx2d model");

    reset(&mut dut);

    assert_eq!(read_access(&mut dut, GFX2D_SCROLL_X_ADDR), 0);
    assert_eq!(read_access(&mut dut, GFX2D_SCROLL_Y_ADDR), 0);
}

#[test]
fn test_gfx2d_bus_writes_change_rendered_scroll_output() {
    let runtime = create_gfx2d_peripheral_runtime().expect("Failed to create gfx2d runtime");
    let mut dut = runtime
        .create_model_simple::<Gfx2dPeripheralTestWrapper>()
        .expect("Failed to create gfx2d model");

    reset(&mut dut);

    let baseline_pixels = capture_active_frame_pixels(&mut dut, 2);
    for y in 0..GFX2D_ACTIVE_HEIGHT {
        for x in 0..GFX2D_ACTIVE_WIDTH {
            assert_eq!(
                active_frame_pixel(&baseline_pixels, x, y),
                expected_pixel(x, y),
                "unexpected baseline pixel at ({x}, {y})"
            );
        }
    }

    for (scroll_x, scroll_y) in [(1u8, 0u8), (15u8, 0u8), (0u8, 1u8), (9u8, 10u8)] {
        write_access(&mut dut, GFX2D_SCROLL_X_ADDR, u32::from(scroll_x));
        write_access(&mut dut, GFX2D_SCROLL_Y_ADDR, u32::from(scroll_y));

        assert_eq!(
            read_access(&mut dut, GFX2D_SCROLL_X_ADDR),
            u32::from(scroll_x)
        );
        assert_eq!(
            read_access(&mut dut, GFX2D_SCROLL_Y_ADDR),
            u32::from(scroll_y)
        );

        let pixels = capture_active_frame_pixels(&mut dut, 2);
        for y in 0..GFX2D_ACTIVE_HEIGHT {
            for x in 0..GFX2D_ACTIVE_WIDTH {
                assert_eq!(
                    active_frame_pixel(&pixels, x, y),
                    expected_scrolled_pixel(x, y, scroll_x, scroll_y),
                    "unexpected pixel at ({x}, {y}) with scroll_x={scroll_x} scroll_y={scroll_y}"
                );
            }
        }

        let mut observed_blanking = false;
        let mut saw_hsync_pulse = false;
        let mut saw_vsync_pulse = false;
        for _ in 0..GFX2D_FRAME_CYCLES {
            if dut.video_de == 0 {
                observed_blanking = true;
                assert_eq!(dut.video_rgb, 0, "video_rgb must stay low during blanking");
            }
            if dut.video_hs == 0 {
                saw_hsync_pulse = true;
            }
            if dut.video_vs == 0 {
                saw_vsync_pulse = true;
            }
            clock_cycle!(dut);
        }

        assert!(observed_blanking, "expected blanking interval");
        assert!(saw_hsync_pulse, "expected horizontal sync pulse");
        assert!(saw_vsync_pulse, "expected vertical sync pulse");
        assert_eq!(dut.video_skip, 0, "video_skip should stay low");
    }
}
