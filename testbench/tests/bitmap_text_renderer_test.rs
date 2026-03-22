use riscv_core::{create_bitmap_text_renderer_runtime, BitmapTextRendererTestWrapper};

const BITMAP_TEXT_RENDERER_H_TOTAL: usize = 26;
const BITMAP_TEXT_RENDERER_V_TOTAL: usize = 19;
const BITMAP_TEXT_RENDERER_FRAME_CYCLES: usize =
    BITMAP_TEXT_RENDERER_H_TOTAL * BITMAP_TEXT_RENDERER_V_TOTAL;

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
    dut.rst = 1;
    for _ in 0..6 {
        clock_cycle!(dut);
    }
    dut.rst = 0;
}

fn wait_for_frame_start(dut: &mut BitmapTextRendererTestWrapper, occurrence: usize) {
    let mut seen = 0;
    for _ in 0..(BITMAP_TEXT_RENDERER_FRAME_CYCLES * 3) {
        clock_cycle!(dut);
        if dut.frame_start == 1 {
            assert_eq!(
                dut.video_de, 1,
                "frame_start must coincide with active video"
            );
            assert_eq!(
                dut.line_start, 1,
                "frame_start must also start the first line"
            );
            assert_eq!(dut.active_x, 0, "frame_start must align with x=0");
            assert_eq!(dut.active_y, 0, "frame_start must align with y=0");
            seen += 1;
            if seen == occurrence {
                return;
            }
        }
    }

    panic!("timed out waiting for frame_start occurrence {occurrence}");
}

fn advance_to_active_coordinate(dut: &mut BitmapTextRendererTestWrapper, x: u8, y: u8) {
    for _ in 0..BITMAP_TEXT_RENDERER_FRAME_CYCLES {
        if dut.video_de == 1 && dut.active_x == x && dut.active_y == y {
            return;
        }
        clock_cycle!(dut);
    }

    panic!("timed out waiting for active coordinate ({x}, {y})");
}

#[test]
fn test_bitmap_text_renderer_keeps_registered_output_aligned_to_active_coordinates() {
    let runtime = create_bitmap_text_renderer_runtime()
        .expect("Failed to create bitmap_text_renderer runtime");
    let mut dut = runtime
        .create_model_simple::<BitmapTextRendererTestWrapper>()
        .expect("Failed to create bitmap_text_renderer model");

    reset_wrapper(&mut dut);
    wait_for_frame_start(&mut dut, 2);

    let expected_pixels = [
        (0u8, 0u8, 1u8),
        (1u8, 0u8, 0u8),
        (7u8, 0u8, 1u8),
        (8u8, 0u8, 1u8),
        (8u8, 1u8, 0u8),
        (0u8, 8u8, 1u8),
        (3u8, 8u8, 1u8),
        (4u8, 8u8, 0u8),
        (8u8, 8u8, 0u8),
        (12u8, 8u8, 1u8),
    ];

    for (x, y, pixel_on) in expected_pixels {
        advance_to_active_coordinate(&mut dut, x, y);
        assert_eq!(
            dut.pixel_on, pixel_on,
            "unexpected registered pixel value aligned to active coordinate ({x}, {y})"
        );
        clock_cycle!(dut);
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
    wait_for_frame_start(&mut dut, 1);

    let expected_pixels = [(1u8, 0u8, 0u8), (4u8, 0u8, 0u8), (7u8, 0u8, 1u8)];

    for (x, y, pixel_on) in expected_pixels {
        advance_to_active_coordinate(&mut dut, x, y);
        assert_eq!(
            dut.pixel_on, pixel_on,
            "unexpected first-frame pixel value after reset at ({x}, {y})"
        );
        clock_cycle!(dut);
    }
}

#[test]
fn test_bitmap_text_renderer_drives_registered_output_low_during_blanking() {
    let runtime = create_bitmap_text_renderer_runtime()
        .expect("Failed to create bitmap_text_renderer runtime");
    let mut dut = runtime
        .create_model_simple::<BitmapTextRendererTestWrapper>()
        .expect("Failed to create bitmap_text_renderer model");

    reset_wrapper(&mut dut);
    wait_for_frame_start(&mut dut, 2);

    let mut observed_blanking = false;
    for _ in 0..BITMAP_TEXT_RENDERER_FRAME_CYCLES {
        if dut.video_de == 0 {
            observed_blanking = true;
            assert_eq!(
                dut.pixel_on, 0,
                "registered pixel_on must stay aligned low during blanking"
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
    wait_for_frame_start(&mut dut, 2);

    let mut saw_hsync_pulse = false;
    let mut saw_vsync_pulse = false;

    for _ in 0..BITMAP_TEXT_RENDERER_FRAME_CYCLES {
        if dut.video_hs == 0 {
            saw_hsync_pulse = true;
        }
        if dut.video_vs == 0 {
            saw_vsync_pulse = true;
        }
        if dut.line_start == 1 {
            assert_eq!(dut.active_x, 0, "line_start must align with x=0");
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
