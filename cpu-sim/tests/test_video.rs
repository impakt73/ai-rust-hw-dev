use bus_shared::{Video, VideoConfig, VideoFormat, VIDEO_BASE};
use cpu_sim::{run_elf, InstructionTrace, SimulationResult, SimulatorView};
use std::sync::{Arc, Mutex};

/// Helper to convert a single pixel from any format to RGBA8 for comparison
fn pixel_to_rgba8(pixel_data: &[u8], format: VideoFormat) -> [u8; 4] {
    match format {
        VideoFormat::Rgba8 => [pixel_data[0], pixel_data[1], pixel_data[2], pixel_data[3]],
        VideoFormat::Rgb8 => [pixel_data[0], pixel_data[1], pixel_data[2], 255],
        VideoFormat::Rgb565 => {
            let rgb565 = u16::from_le_bytes([pixel_data[0], pixel_data[1]]);
            let r = ((rgb565 >> 11) & 0x1F) as u8;
            let g = ((rgb565 >> 5) & 0x3F) as u8;
            let b = (rgb565 & 0x1F) as u8;
            [
                (r << 3) | (r >> 2), // R: 5-bit to 8-bit
                (g << 2) | (g >> 4), // G: 6-bit to 8-bit
                (b << 3) | (b >> 2), // B: 5-bit to 8-bit
                255,
            ]
        }
        VideoFormat::R8 => {
            let gray = pixel_data[0];
            [gray, gray, gray, 255]
        }
    }
}

/// Helper to get pixel at (x, y) from raw pixel data
fn get_pixel_rgba8(
    pixel_data: &[u8],
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    format: VideoFormat,
) -> [u8; 4] {
    assert!(x < width && y < height, "Pixel coordinates out of bounds");
    let bytes_per_pixel = format.bytes_per_pixel();
    let offset = ((y * width + x) * bytes_per_pixel) as usize;
    pixel_to_rgba8(&pixel_data[offset..], format)
}

#[test]
fn test_video_pattern() {
    let _ = env_logger::builder().is_test(true).try_init();

    let elf_path = sim_tests::test_program_path("test_video_pattern")
        .expect("Failed to find test_video_pattern");

    // Storage for captured frames
    type CapturedFrames = Arc<Mutex<Vec<(Vec<u8>, VideoConfig)>>>;
    let captured_frames: CapturedFrames = Arc::new(Mutex::new(Vec::new()));

    // Setup callback to register Video device
    let frames_for_setup = captured_frames.clone();
    let setup_callback = move |view: &mut SimulatorView| {
        let frames_for_callback = Arc::clone(&frames_for_setup);

        // Create callback that captures frame data in memory
        let present_callback = move |data: &[u8], config: &VideoConfig| {
            let mut frames = frames_for_callback.lock().expect("frames lock poisoned");
            frames.push((data.to_vec(), *config));
            log::info!(
                "Frame {} captured ({}x{} {:?}, {} bytes)",
                frames.len() - 1,
                config.width,
                config.height,
                config.format,
                data.len()
            );
        };

        // Register Video device at VIDEO_BASE with 60 FPS (real-time pacing)
        // Frame pacing is now based on elapsed host time, not CPU cycles
        let video = Box::new(Video::with_fps(60, Some(present_callback)));
        view.register_device(VIDEO_BASE, video)
            .expect("Failed to register Video device");
        log::info!("Video device registered at 0x{:08x}", VIDEO_BASE);
    };

    // Termination callback to verify captured frames
    let frames_for_verify = captured_frames.clone();
    let termination_callback = move |_view: &SimulatorView, result: &SimulationResult| {
        // Verify the program completed successfully
        assert_eq!(
            result.tohost_value,
            Some(42),
            "Video test should exit with success code 42"
        );

        println!("\n=== Video Pattern Test Results ===");
        println!("Cycles: {}", result.cycles);
        println!("Test program completed successfully");

        let frames = frames_for_verify.lock().expect("frames lock poisoned");
        assert_eq!(frames.len(), 3, "Should have captured 3 frames");

        println!("✓ Captured {} frames", frames.len());

        // Verify frame contents directly from memory
        verify_frame_0_checkerboard(&frames[0].0, &frames[0].1);
        verify_frame_1_diagonal_stripes(&frames[1].0, &frames[1].1);
        verify_frame_2_gradient(&frames[2].0, &frames[2].1);

        println!("✓ All frame patterns verified successfully");
    };

    let result = run_elf(
        &elf_path,
        10_000_000, // High limit for video frame rendering (increased for serialized bus protocol)
        false,      // print_inst_trace
        false,      // print_fsm_state
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        None,                       // vcd_path
        0,                          // mem_latency_cycles
        Some(setup_callback),       // Register Video device
        Some(termination_callback), // Verify frames after completion
    )
    .expect("Simulation should succeed");

    println!("\n=== Video Pattern Test Summary ===");
    println!("Total cycles: {}", result.cycles);
    println!("Test passed: 3 frames rendered and verified");
}

/// Verify frame 0: Red/Green checkerboard pattern
fn verify_frame_0_checkerboard(pixel_data: &[u8], config: &VideoConfig) {
    assert_eq!(config.width, 64, "Frame 0 width should be 64");
    assert_eq!(config.height, 64, "Frame 0 height should be 64");
    assert_eq!(config.format, VideoFormat::Rgba8);

    // Verify checkerboard pattern at key points
    // (0, 0): even+even -> Red
    let pixel = get_pixel_rgba8(pixel_data, 64, 64, 0, 0, config.format);
    assert_eq!(pixel, [255, 0, 0, 255], "Pixel (0,0) should be red");

    // (1, 0): odd+even -> Green
    let pixel = get_pixel_rgba8(pixel_data, 64, 64, 1, 0, config.format);
    assert_eq!(pixel, [0, 255, 0, 255], "Pixel (1,0) should be green");

    // (0, 1): even+odd -> Green
    let pixel = get_pixel_rgba8(pixel_data, 64, 64, 0, 1, config.format);
    assert_eq!(pixel, [0, 255, 0, 255], "Pixel (0,1) should be green");

    // (1, 1): odd+odd -> Red
    let pixel = get_pixel_rgba8(pixel_data, 64, 64, 1, 1, config.format);
    assert_eq!(pixel, [255, 0, 0, 255], "Pixel (1,1) should be red");

    // (32, 32): even+even -> Red
    let pixel = get_pixel_rgba8(pixel_data, 64, 64, 32, 32, config.format);
    assert_eq!(pixel, [255, 0, 0, 255], "Pixel (32,32) should be red");

    println!("  ✓ Frame 0 checkerboard pattern verified");
}

/// Verify frame 1: Blue/Yellow diagonal stripes
fn verify_frame_1_diagonal_stripes(pixel_data: &[u8], config: &VideoConfig) {
    assert_eq!(config.width, 64, "Frame 1 width should be 64");
    assert_eq!(config.height, 64, "Frame 1 height should be 64");
    assert_eq!(config.format, VideoFormat::Rgba8);

    // (0, 0): (0+0) % 16 = 0 < 8 -> Blue
    let pixel = get_pixel_rgba8(pixel_data, 64, 64, 0, 0, config.format);
    assert_eq!(pixel, [0, 0, 255, 255], "Pixel (0,0) should be blue");

    // (8, 0): (8+0) % 16 = 8 >= 8 -> Yellow
    let pixel = get_pixel_rgba8(pixel_data, 64, 64, 8, 0, config.format);
    assert_eq!(pixel, [255, 255, 0, 255], "Pixel (8,0) should be yellow");

    // (0, 8): (0+8) % 16 = 8 >= 8 -> Yellow
    let pixel = get_pixel_rgba8(pixel_data, 64, 64, 0, 8, config.format);
    assert_eq!(pixel, [255, 255, 0, 255], "Pixel (0,8) should be yellow");

    // (4, 4): (4+4) % 16 = 8 >= 8 -> Yellow
    let pixel = get_pixel_rgba8(pixel_data, 64, 64, 4, 4, config.format);
    assert_eq!(pixel, [255, 255, 0, 255], "Pixel (4,4) should be yellow");

    println!("  ✓ Frame 1 diagonal stripes pattern verified");
}

/// Verify frame 2: Grayscale gradient
fn verify_frame_2_gradient(pixel_data: &[u8], config: &VideoConfig) {
    assert_eq!(config.width, 64, "Frame 2 width should be 64");
    assert_eq!(config.height, 64, "Frame 2 height should be 64");
    assert_eq!(config.format, VideoFormat::Rgba8);

    // (0, 0): gray = 0
    let pixel = get_pixel_rgba8(pixel_data, 64, 64, 0, 0, config.format);
    assert_eq!(pixel, [0, 0, 0, 255], "Pixel (0,0) should be black");

    // (63, 0): gray = (63 * 255) / 64 = 251
    let pixel = get_pixel_rgba8(pixel_data, 64, 64, 63, 0, config.format);
    let expected_gray = ((63 * 255) / 64) as u8;
    assert_eq!(
        pixel,
        [expected_gray, expected_gray, expected_gray, 255],
        "Pixel (63,0) should be near white"
    );

    // (32, 0): gray = (32 * 255) / 64 = 127
    let pixel = get_pixel_rgba8(pixel_data, 64, 64, 32, 0, config.format);
    let expected_gray = ((32 * 255) / 64) as u8;
    assert_eq!(
        pixel,
        [expected_gray, expected_gray, expected_gray, 255],
        "Pixel (32,0) should be mid-gray"
    );

    // Verify gradient increases along x-axis
    let pixel_10 = get_pixel_rgba8(pixel_data, 64, 64, 10, 0, config.format);
    let pixel_20 = get_pixel_rgba8(pixel_data, 64, 64, 20, 0, config.format);
    let pixel_30 = get_pixel_rgba8(pixel_data, 64, 64, 30, 0, config.format);
    assert!(
        pixel_10[0] < pixel_20[0] && pixel_20[0] < pixel_30[0],
        "Gradient should increase from left to right"
    );

    println!("  ✓ Frame 2 gradient pattern verified");
}
