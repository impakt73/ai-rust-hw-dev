use riscv_core::{create_video_sync_runtime, VideoSyncMinimalWrapper, VideoSyncWrapper};

const VIDEO_SYNC_WRAPPER_H_TOTAL: usize = 8;
const VIDEO_SYNC_WRAPPER_V_TOTAL: usize = 6;
const VIDEO_SYNC_WRAPPER_H_ACTIVE: u8 = 4;
const VIDEO_SYNC_WRAPPER_CYCLES_PER_FRAME: usize =
    VIDEO_SYNC_WRAPPER_H_TOTAL * VIDEO_SYNC_WRAPPER_V_TOTAL;

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

fn reset_default_wrapper(dut: &mut VideoSyncWrapper) {
    dut.rst = 1;
    for _ in 0..3 {
        clock_cycle!(dut);
    }
    dut.rst = 0;
}

fn reset_minimal_wrapper(dut: &mut VideoSyncMinimalWrapper) {
    dut.rst = 1;
    for _ in 0..3 {
        clock_cycle!(dut);
    }
    dut.rst = 0;
}

fn advance_default_wrapper(dut: &mut VideoSyncWrapper, cycles: usize) {
    for _ in 0..cycles {
        clock_cycle!(dut);
    }
}

#[test]
fn test_video_sync_holds_registered_defaults_during_reset() {
    let runtime = create_video_sync_runtime().expect("Failed to create video_sync runtime");
    let mut dut = runtime
        .create_model_simple::<VideoSyncWrapper>()
        .expect("Failed to create video_sync model");

    dut.rst = 1;
    for _ in 0..3 {
        clock_cycle!(&mut dut);
        assert_eq!(dut.hsync, 1, "hsync must stay inactive during reset");
        assert_eq!(dut.vsync, 1, "vsync must stay inactive during reset");
        assert_eq!(
            dut.active_video, 0,
            "active_video must stay low during reset"
        );
        assert_eq!(dut.line_start, 0, "line_start must stay low during reset");
        assert_eq!(dut.frame_start, 0, "frame_start must stay low during reset");
        assert_eq!(dut.active_x, 0, "active_x must reset to zero");
        assert_eq!(dut.active_y, 0, "active_y must reset to zero");
        assert_eq!(
            dut.scan_x, 7,
            "scan_x must hold the final horizontal position during reset"
        );
        assert_eq!(
            dut.scan_y, 5,
            "scan_y must hold the final vertical position during reset"
        );
    }
}

#[test]
fn test_video_sync_generates_expected_first_line_timing() {
    let runtime = create_video_sync_runtime().expect("Failed to create video_sync runtime");
    let mut dut = runtime
        .create_model_simple::<VideoSyncWrapper>()
        .expect("Failed to create video_sync model");

    reset_default_wrapper(&mut dut);

    let expected = [
        (1u8, 1u8, 1u8, 1u8, 1u8, 0u8, 0u8, 0u8, 0u8),
        (1u8, 1u8, 1u8, 0u8, 0u8, 1u8, 0u8, 1u8, 0u8),
        (1u8, 1u8, 1u8, 0u8, 0u8, 2u8, 0u8, 2u8, 0u8),
        (1u8, 1u8, 1u8, 0u8, 0u8, 3u8, 0u8, 3u8, 0u8),
        (1u8, 1u8, 0u8, 0u8, 0u8, 0u8, 0u8, 4u8, 0u8),
        (0u8, 1u8, 0u8, 0u8, 0u8, 0u8, 0u8, 5u8, 0u8),
        (0u8, 1u8, 0u8, 0u8, 0u8, 0u8, 0u8, 6u8, 0u8),
        (1u8, 1u8, 0u8, 0u8, 0u8, 0u8, 0u8, 7u8, 0u8),
    ];

    for (hsync, vsync, active_video, line_start, frame_start, active_x, active_y, scan_x, scan_y) in
        expected
    {
        clock_cycle!(&mut dut);
        assert_eq!(dut.hsync, hsync, "unexpected hsync value");
        assert_eq!(dut.vsync, vsync, "unexpected vsync value");
        assert_eq!(
            dut.active_video, active_video,
            "unexpected active_video value"
        );
        assert_eq!(dut.line_start, line_start, "unexpected line_start pulse");
        assert_eq!(dut.frame_start, frame_start, "unexpected frame_start pulse");
        assert_eq!(dut.active_x, active_x, "unexpected active_x coordinate");
        assert_eq!(dut.active_y, active_y, "unexpected active_y coordinate");
        assert_eq!(dut.scan_x, scan_x, "unexpected scan_x coordinate");
        assert_eq!(dut.scan_y, scan_y, "unexpected scan_y coordinate");
    }
}

#[test]
fn test_video_sync_line_and_frame_markers_match_frame_geometry() {
    let runtime = create_video_sync_runtime().expect("Failed to create video_sync runtime");
    let mut dut = runtime
        .create_model_simple::<VideoSyncWrapper>()
        .expect("Failed to create video_sync model");

    reset_default_wrapper(&mut dut);

    let mut line_starts = 0;
    let mut frame_starts = 0;

    for cycle in 0..VIDEO_SYNC_WRAPPER_CYCLES_PER_FRAME {
        clock_cycle!(&mut dut);

        if cycle % VIDEO_SYNC_WRAPPER_H_TOTAL == 0 {
            assert_eq!(dut.line_start, 1, "line_start should pulse once per line");
            line_starts += 1;
        } else {
            assert_eq!(dut.line_start, 0, "line_start should be a one-cycle pulse");
        }

        if cycle == 0 {
            assert_eq!(
                dut.frame_start, 1,
                "frame_start should pulse at frame origin"
            );
            frame_starts += 1;
            assert_eq!(dut.active_video, 1, "frame should start in active video");
            assert_eq!(dut.active_x, 0, "frame should start at x=0");
            assert_eq!(dut.active_y, 0, "frame should start at y=0");
            assert_eq!(dut.scan_x, 0, "frame should start at scan_x=0");
            assert_eq!(dut.scan_y, 0, "frame should start at scan_y=0");
        } else {
            assert_eq!(dut.frame_start, 0, "frame_start should only pulse once");
        }
    }

    assert_eq!(
        line_starts, VIDEO_SYNC_WRAPPER_V_TOTAL,
        "wrapper timing should contain 6 lines per frame"
    );
    assert_eq!(
        frame_starts, 1,
        "wrapper timing should contain 1 frame_start per frame"
    );

    clock_cycle!(&mut dut);
    assert_eq!(
        dut.frame_start, 1,
        "frame_start should repeat at the next frame"
    );
    assert_eq!(dut.line_start, 1, "new frame must also start a new line");
    assert_eq!(
        dut.active_video, 1,
        "next frame must restart in active video"
    );
    assert_eq!(dut.active_x, 0, "next frame must restart x at zero");
    assert_eq!(dut.active_y, 0, "next frame must restart y at zero");
    assert_eq!(dut.scan_x, 0, "next frame must restart scan_x at zero");
    assert_eq!(dut.scan_y, 0, "next frame must restart scan_y at zero");
}

#[test]
fn test_video_sync_supports_minimal_geometry_and_active_high_syncs() {
    let runtime = create_video_sync_runtime().expect("Failed to create video_sync runtime");
    let mut dut = runtime
        .create_model_simple::<VideoSyncMinimalWrapper>()
        .expect("Failed to create minimal video_sync model");

    reset_minimal_wrapper(&mut dut);

    let expected = [
        (0u8, 0u8, 1u8, 1u8, 1u8, 0u8, 0u8, 0u8, 0u8),
        (1u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 1u8, 0u8),
        (0u8, 1u8, 0u8, 1u8, 0u8, 0u8, 0u8, 0u8, 1u8),
        (1u8, 1u8, 0u8, 0u8, 0u8, 0u8, 0u8, 1u8, 1u8),
    ];

    for (hsync, vsync, active_video, line_start, frame_start, active_x, active_y, scan_x, scan_y) in
        expected
    {
        clock_cycle!(&mut dut);
        assert_eq!(dut.hsync, hsync, "unexpected minimal-wrapper hsync value");
        assert_eq!(dut.vsync, vsync, "unexpected minimal-wrapper vsync value");
        assert_eq!(
            dut.active_video, active_video,
            "unexpected minimal-wrapper active_video value"
        );
        assert_eq!(
            dut.line_start, line_start,
            "unexpected minimal-wrapper line_start pulse"
        );
        assert_eq!(
            dut.frame_start, frame_start,
            "unexpected minimal-wrapper frame_start pulse"
        );
        assert_eq!(
            dut.active_x, active_x,
            "unexpected minimal-wrapper active_x coordinate"
        );
        assert_eq!(
            dut.active_y, active_y,
            "unexpected minimal-wrapper active_y coordinate"
        );
        assert_eq!(
            dut.scan_x, scan_x,
            "unexpected minimal-wrapper scan_x coordinate"
        );
        assert_eq!(
            dut.scan_y, scan_y,
            "unexpected minimal-wrapper scan_y coordinate"
        );
    }
}

#[test]
fn test_video_sync_zeroes_coordinates_outside_active_region() {
    let runtime = create_video_sync_runtime().expect("Failed to create video_sync runtime");
    let mut dut = runtime
        .create_model_simple::<VideoSyncWrapper>()
        .expect("Failed to create video_sync model");

    reset_default_wrapper(&mut dut);

    advance_default_wrapper(&mut dut, VIDEO_SYNC_WRAPPER_H_TOTAL + 1);
    assert_eq!(
        dut.active_video, 1,
        "second line should begin in active video"
    );
    assert_eq!(dut.active_x, 0, "second line should restart x at zero");
    assert_eq!(
        dut.active_y, 1,
        "second line should increment y in active region"
    );
    assert_eq!(dut.scan_x, 0, "second line should restart scan_x at zero");
    assert_eq!(dut.scan_y, 1, "second line should increment scan_y");

    advance_default_wrapper(&mut dut, 4);
    for _ in 0..(VIDEO_SYNC_WRAPPER_H_TOTAL - 4) {
        assert_eq!(
            dut.active_video, 0,
            "horizontal blanking should deassert active_video"
        );
        assert_eq!(
            dut.active_x, 0,
            "active_x must be zero during horizontal blanking"
        );
        assert_eq!(
            dut.active_y, 0,
            "active_y must be zero during horizontal blanking"
        );
        assert!(
            dut.scan_x >= VIDEO_SYNC_WRAPPER_H_ACTIVE,
            "scan_x must continue counting during horizontal blanking"
        );
        assert_eq!(dut.scan_y, 1, "scan_y must stay on the same active line");
        clock_cycle!(&mut dut);
    }

    advance_default_wrapper(&mut dut, VIDEO_SYNC_WRAPPER_H_TOTAL);
    assert_eq!(
        dut.active_video, 0,
        "vertical blanking line should be inactive"
    );
    assert_eq!(
        dut.line_start, 1,
        "vertical blanking line should still start a line"
    );
    assert_eq!(
        dut.active_x, 0,
        "active_x must be zero in vertical blanking"
    );
    assert_eq!(
        dut.active_y, 0,
        "active_y must be zero in vertical blanking"
    );
    assert_eq!(dut.scan_x, 0, "vertical blanking line should restart scan_x");
    assert_eq!(dut.scan_y, 3, "vertical blanking line should expose absolute y");

    for _ in 0..(VIDEO_SYNC_WRAPPER_H_TOTAL - 1) {
        clock_cycle!(&mut dut);
        assert_eq!(dut.active_video, 0, "vertical blanking must stay inactive");
        assert_eq!(
            dut.active_x, 0,
            "active_x must remain zero in vertical blanking"
        );
        assert_eq!(
            dut.active_y, 0,
            "active_y must remain zero in vertical blanking"
        );
        assert_eq!(dut.scan_y, 3, "scan_y must remain on the blanking line");
    }
}
