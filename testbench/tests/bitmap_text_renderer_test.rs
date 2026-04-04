use riscv_core::{create_bitmap_text_renderer_runtime, BitmapTextRendererTestWrapper};

use riscv_core::AsDynamicVerilatedModel;
const BITMAP_TEXT_RENDERER_H_TOTAL: usize = 26;
const BITMAP_TEXT_RENDERER_V_TOTAL: usize = 19;
const BITMAP_TEXT_RENDERER_FRAME_CYCLES: usize =
    BITMAP_TEXT_RENDERER_H_TOTAL * BITMAP_TEXT_RENDERER_V_TOTAL;
const BITMAP_TEXT_RENDERER_ACTIVE_WIDTH: u8 = 16;
const BITMAP_TEXT_RENDERER_ACTIVE_HEIGHT: u8 = 16;
const BITMAP_TEXT_RENDERER_SCROLL_X_MASK: u8 = BITMAP_TEXT_RENDERER_ACTIVE_WIDTH - 1;
const BITMAP_TEXT_RENDERER_SCROLL_Y_MASK: u8 = BITMAP_TEXT_RENDERER_ACTIVE_HEIGHT - 1;
const BITMAP_TEXT_RENDERER_CHAR_MAP: [u8; 4] = [1, 2, 3, 4];
const BITMAP_TEXT_RENDERER_FONT_ROWS: [[u8; 8]; 5] = [
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x81, 0x42, 0x24, 0x18, 0x18, 0x24, 0x42, 0x81],
    [0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00],
    [0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0],
    [0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F],
];

macro_rules! clock_cycle {
    ($dut:expr) => {
        $dut.clk = 0;
        $dut.eval();
        $dut.clk = 1;
        $dut.eval();
        $dut.clk = 0;
        $dut.eval();
    };
}

fn reset_wrapper(dut: &mut BitmapTextRendererTestWrapper) {
    dut.scroll_x = 0;
    dut.scroll_y = 0;
    dut.rst = 1;
    for _ in 0..6 {
        clock_cycle!(dut);
    }
    dut.rst = 0;
}

fn wait_for_active_frame_start(dut: &mut BitmapTextRendererTestWrapper, occurrence: usize) {
    let mut seen = 0;
    let mut saw_vsync_pulse = false;

    for _ in 0..(BITMAP_TEXT_RENDERER_FRAME_CYCLES * 3) {
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
    dut: &mut BitmapTextRendererTestWrapper,
    occurrence: usize,
) -> Vec<u32> {
    let width = usize::from(BITMAP_TEXT_RENDERER_ACTIVE_WIDTH);
    let height = usize::from(BITMAP_TEXT_RENDERER_ACTIVE_HEIGHT);
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

        if (row + 1) != height {
            clock_cycle!(dut);

            // At most one full horizontal line period should elapse before the
            // next active row starts.
            for _ in 0..BITMAP_TEXT_RENDERER_H_TOTAL {
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
    pixels[(usize::from(y) * usize::from(BITMAP_TEXT_RENDERER_ACTIVE_WIDTH)) + usize::from(x)]
}

fn expected_pixel(x: u8, y: u8) -> u32 {
    let tile_x = usize::from(x / 8);
    let tile_y = usize::from(y / 8);
    let glyph = usize::from(BITMAP_TEXT_RENDERER_CHAR_MAP[(tile_y * 2) + tile_x]);
    let glyph_row = BITMAP_TEXT_RENDERER_FONT_ROWS[glyph][usize::from(y % 8)];
    if ((glyph_row >> (7 - (x % 8))) & 1) != 0 {
        0xFF_FF_FF
    } else {
        0x00_00_00
    }
}

fn expected_scrolled_pixel(x: u8, y: u8, scroll_x: u8, scroll_y: u8) -> u32 {
    expected_pixel(
        (x.wrapping_add(scroll_x)) & BITMAP_TEXT_RENDERER_SCROLL_X_MASK,
        (y.wrapping_add(scroll_y)) & BITMAP_TEXT_RENDERER_SCROLL_Y_MASK,
    )
}

#[test]
fn test_bitmap_text_renderer_keeps_aligned_output_stage_coordinates() {
    let runtime = create_bitmap_text_renderer_runtime()
        .expect("Failed to create bitmap_text_renderer runtime");
    let mut dut = runtime
        .create_model_simple::<BitmapTextRendererTestWrapper>()
        .expect("Failed to create bitmap_text_renderer model");

    reset_wrapper(&mut dut);
    let pixels = capture_active_frame_pixels(&mut dut, 2);

    let expected_pixels = [
        (0u8, 0u8, 0xFF_FF_FFu32),
        (1u8, 0u8, 0x00_00_00u32),
        (7u8, 0u8, 0xFF_FF_FFu32),
        (8u8, 0u8, 0xFF_FF_FFu32),
        (8u8, 1u8, 0x00_00_00u32),
        (0u8, 8u8, 0xFF_FF_FFu32),
        (3u8, 8u8, 0xFF_FF_FFu32),
        (4u8, 8u8, 0x00_00_00u32),
        (8u8, 8u8, 0x00_00_00u32),
        (12u8, 8u8, 0xFF_FF_FFu32),
    ];

    for (x, y, pixel_rgb) in expected_pixels {
        assert_eq!(
            active_frame_pixel(&pixels, x, y),
            pixel_rgb,
            "unexpected aligned pixel value at raster coordinate ({x}, {y})"
        );
    }
}

#[test]
fn test_bitmap_text_renderer_primes_first_frame_tile_zero_after_reset() {
    let runtime = create_bitmap_text_renderer_runtime()
        .expect("Failed to create bitmap_text_renderer runtime");
    let mut dut = runtime
        .create_model_simple::<BitmapTextRendererTestWrapper>()
        .expect("Failed to create bitmap_text_renderer model");

    reset_wrapper(&mut dut);
    let pixels = capture_active_frame_pixels(&mut dut, 1);

    let expected_pixels = [
        (1u8, 0u8, 0x00_00_00u32),
        (4u8, 0u8, 0x00_00_00u32),
        (6u8, 0u8, 0x00_00_00u32),
        (7u8, 0u8, 0xFF_FF_FFu32),
    ];

    for (x, y, pixel_rgb) in expected_pixels {
        assert_eq!(
            active_frame_pixel(&pixels, x, y),
            pixel_rgb,
            "unexpected first-frame pixel value after reset at ({x}, {y})"
        );
    }
}

#[test]
fn test_bitmap_text_renderer_matches_expected_bitmap_in_steady_state() {
    let runtime = create_bitmap_text_renderer_runtime()
        .expect("Failed to create bitmap_text_renderer runtime");
    let mut dut = runtime
        .create_model_simple::<BitmapTextRendererTestWrapper>()
        .expect("Failed to create bitmap_text_renderer model");

    reset_wrapper(&mut dut);
    let pixels = capture_active_frame_pixels(&mut dut, 2);

    let mut active_pixel_count = 0usize;
    for y in 0..BITMAP_TEXT_RENDERER_ACTIVE_HEIGHT {
        for x in 0..BITMAP_TEXT_RENDERER_ACTIVE_WIDTH {
            let expected = expected_pixel(x, y);
            assert_eq!(
                active_frame_pixel(&pixels, x, y),
                expected,
                "unexpected pixel at steady-state coordinate ({}, {})",
                x,
                y
            );
            active_pixel_count += 1;
        }
    }

    assert_eq!(
        active_pixel_count,
        usize::from(BITMAP_TEXT_RENDERER_ACTIVE_WIDTH)
            * usize::from(BITMAP_TEXT_RENDERER_ACTIVE_HEIGHT),
        "expected to validate every active pixel in the 16x16 test frame"
    );
}

#[test]
fn test_bitmap_text_renderer_scrolls_tilemap_pixels_with_wraparound() {
    let runtime = create_bitmap_text_renderer_runtime()
        .expect("Failed to create bitmap_text_renderer runtime");
    let mut dut = runtime
        .create_model_simple::<BitmapTextRendererTestWrapper>()
        .expect("Failed to create bitmap_text_renderer model");

    reset_wrapper(&mut dut);

    let scroll_cases = [
        (1u8, 0u8),
        (15u8, 0u8),
        (0u8, 1u8),
        (0u8, 15u8),
        (9u8, 10u8),
    ];

    for (scroll_x, scroll_y) in scroll_cases {
        dut.scroll_x = scroll_x;
        dut.scroll_y = scroll_y;

        let pixels = capture_active_frame_pixels(&mut dut, 2);

        for y in 0..BITMAP_TEXT_RENDERER_ACTIVE_HEIGHT {
            for x in 0..BITMAP_TEXT_RENDERER_ACTIVE_WIDTH {
                assert_eq!(
                    active_frame_pixel(&pixels, x, y),
                    expected_scrolled_pixel(x, y, scroll_x, scroll_y),
                    "unexpected pixel at ({x}, {y}) with scroll_x={scroll_x} scroll_y={scroll_y}"
                );
            }
        }

        let mut saw_hsync_pulse = false;
        let mut saw_vsync_pulse = false;
        let mut observed_blanking = false;

        for _ in 0..BITMAP_TEXT_RENDERER_FRAME_CYCLES {
            if dut.video_de == 0 {
                observed_blanking = true;
                assert_eq!(
                    dut.video_rgb, 0,
                    "video_rgb must stay low during blanking with scroll_x={scroll_x} scroll_y={scroll_y}"
                );
            }
            if dut.video_hs == 0 {
                saw_hsync_pulse = true;
            }
            if dut.video_vs == 0 {
                saw_vsync_pulse = true;
            }
            clock_cycle!(dut);
        }

        assert!(
            observed_blanking,
            "expected blanking intervals with scroll_x={scroll_x} scroll_y={scroll_y}"
        );
        assert!(
            saw_hsync_pulse,
            "expected horizontal sync pulse with scroll_x={scroll_x} scroll_y={scroll_y}"
        );
        assert!(
            saw_vsync_pulse,
            "expected vertical sync pulse with scroll_x={scroll_x} scroll_y={scroll_y}"
        );
    }
}

#[test]
fn test_bitmap_text_renderer_drives_aligned_output_stage_low_during_blanking() {
    let runtime = create_bitmap_text_renderer_runtime()
        .expect("Failed to create bitmap_text_renderer runtime");
    let mut dut = runtime
        .create_model_simple::<BitmapTextRendererTestWrapper>()
        .expect("Failed to create bitmap_text_renderer model");

    reset_wrapper(&mut dut);
    wait_for_active_frame_start(&mut dut, 2);

    let mut observed_blanking = false;
    for _ in 0..BITMAP_TEXT_RENDERER_FRAME_CYCLES {
        if dut.video_de == 0 {
            observed_blanking = true;
            assert_eq!(
                dut.video_rgb, 0,
                "aligned video_rgb must stay low during blanking"
            );
        }
        clock_cycle!(dut);
    }

    assert!(
        observed_blanking,
        "test frame should include blanking intervals"
    );
}

#[test]
fn test_bitmap_text_renderer_exposes_aligned_video_control_outputs() {
    let runtime = create_bitmap_text_renderer_runtime()
        .expect("Failed to create bitmap_text_renderer runtime");
    let mut dut = runtime
        .create_model_simple::<BitmapTextRendererTestWrapper>()
        .expect("Failed to create bitmap_text_renderer model");

    reset_wrapper(&mut dut);
    wait_for_active_frame_start(&mut dut, 2);

    let mut saw_hsync_pulse = false;
    let mut saw_vsync_pulse = false;

    for _ in 0..BITMAP_TEXT_RENDERER_FRAME_CYCLES {
        if dut.video_hs == 0 {
            saw_hsync_pulse = true;
        }
        if dut.video_vs == 0 {
            saw_vsync_pulse = true;
        }
        clock_cycle!(dut);
    }

    assert!(
        saw_hsync_pulse,
        "test frame should include a horizontal sync pulse"
    );
    assert!(
        saw_vsync_pulse,
        "test frame should include a vertical sync pulse"
    );
}
