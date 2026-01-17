use cpu_sim::*;
use std::path::PathBuf;

fn test_program_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("test_programs")
        .join(name)
}

#[test]
fn test_video_pattern() {
    let _ = env_logger::builder().is_test(true).try_init();

    let elf_path = test_program_path("test_video_pattern.elf");

    // Video device base address (must match test program)
    const VIDEO_BASE: u32 = 0x3000_0000;

    // Setup callback to register Video device
    let setup_callback = |view: &mut SimulatorView| {
        // Register Video device at 0x3000_0000 with very high FPS for testing
        // At 100MHz CPU, 10000 FPS = 10,000 cycles per frame
        let video = Box::new(Video::with_fps(10000));
        view.register_device(VIDEO_BASE, video)
            .expect("Failed to register Video device");
        log::info!("Video device registered at 0x{:08x}", VIDEO_BASE);
    };

    // Termination callback to verify generated images
    let termination_callback = |_view: &SimulatorView, result: &SimulationResult| {
        // Verify the program completed successfully
        assert_eq!(
            result.tohost_value,
            Some(42),
            "Video test should exit with success code 42"
        );

        println!("\n=== Video Pattern Test Results ===");
        println!("Cycles: {}", result.cycles);
        println!("Test program completed successfully");

        // Verify that 3 frame images were created
        for frame in 0..3 {
            let filename = format!("frame_{:04}.png", frame);
            assert!(
                std::path::Path::new(&filename).exists(),
                "Frame {} should exist: {}",
                frame,
                filename
            );
            println!("✓ Frame {} generated: {}", frame, filename);
        }

        // Verify image contents by loading them back
        verify_frame_0_checkerboard();
        verify_frame_1_diagonal_stripes();
        verify_frame_2_gradient();

        println!("✓ All frame patterns verified successfully");
    };

    let result = run_elf(
        &elf_path,
        2_000_000, // Max cycles - Video operations take significant time (3 frames × 64×64×4 bytes + rendering + pacing)
        false,     // print_inst_trace
        false,     // print_fsm_state
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        None,                       // vcd_path
        0,                          // mem_latency_cycles
        Some(setup_callback),       // Register Video device
        Some(termination_callback), // Verify images after completion
    )
    .expect("Simulation should succeed");

    println!("\n=== Video Pattern Test Summary ===");
    println!("Total cycles: {}", result.cycles);
    println!("Test passed: 3 frames rendered and verified");
}

/// Verify frame 0: Red/Green checkerboard pattern
fn verify_frame_0_checkerboard() {
    use image::{GenericImageView, Rgba};

    let img = image::open("frame_0000.png").expect("Failed to open frame_0000.png");
    assert_eq!(img.width(), 64, "Frame 0 width should be 64");
    assert_eq!(img.height(), 64, "Frame 0 height should be 64");

    // Verify checkerboard pattern at key points
    // (0, 0): even+even -> Red
    let pixel = img.get_pixel(0, 0);
    assert_eq!(pixel, Rgba([255, 0, 0, 255]), "Pixel (0,0) should be red");

    // (1, 0): odd+even -> Green
    let pixel = img.get_pixel(1, 0);
    assert_eq!(pixel, Rgba([0, 255, 0, 255]), "Pixel (1,0) should be green");

    // (0, 1): even+odd -> Green
    let pixel = img.get_pixel(0, 1);
    assert_eq!(pixel, Rgba([0, 255, 0, 255]), "Pixel (0,1) should be green");

    // (1, 1): odd+odd -> Red
    let pixel = img.get_pixel(1, 1);
    assert_eq!(pixel, Rgba([255, 0, 0, 255]), "Pixel (1,1) should be red");

    // (32, 32): even+even -> Red
    let pixel = img.get_pixel(32, 32);
    assert_eq!(pixel, Rgba([255, 0, 0, 255]), "Pixel (32,32) should be red");

    println!("  ✓ Frame 0 checkerboard pattern verified");
}

/// Verify frame 1: Blue/Yellow diagonal stripes
fn verify_frame_1_diagonal_stripes() {
    use image::{GenericImageView, Rgba};

    let img = image::open("frame_0001.png").expect("Failed to open frame_0001.png");
    assert_eq!(img.width(), 64, "Frame 1 width should be 64");
    assert_eq!(img.height(), 64, "Frame 1 height should be 64");

    // (0, 0): (0+0) % 16 = 0 < 8 -> Blue
    let pixel = img.get_pixel(0, 0);
    assert_eq!(pixel, Rgba([0, 0, 255, 255]), "Pixel (0,0) should be blue");

    // (8, 0): (8+0) % 16 = 8 >= 8 -> Yellow
    let pixel = img.get_pixel(8, 0);
    assert_eq!(
        pixel,
        Rgba([255, 255, 0, 255]),
        "Pixel (8,0) should be yellow"
    );

    // (0, 8): (0+8) % 16 = 8 >= 8 -> Yellow
    let pixel = img.get_pixel(0, 8);
    assert_eq!(
        pixel,
        Rgba([255, 255, 0, 255]),
        "Pixel (0,8) should be yellow"
    );

    // (4, 4): (4+4) % 16 = 8 >= 8 -> Yellow
    let pixel = img.get_pixel(4, 4);
    assert_eq!(
        pixel,
        Rgba([255, 255, 0, 255]),
        "Pixel (4,4) should be yellow"
    );

    println!("  ✓ Frame 1 diagonal stripes pattern verified");
}

/// Verify frame 2: Grayscale gradient
fn verify_frame_2_gradient() {
    use image::{GenericImageView, Rgba};

    let img = image::open("frame_0002.png").expect("Failed to open frame_0002.png");
    assert_eq!(img.width(), 64, "Frame 2 width should be 64");
    assert_eq!(img.height(), 64, "Frame 2 height should be 64");

    // (0, 0): gray = 0
    let pixel = img.get_pixel(0, 0);
    assert_eq!(pixel, Rgba([0, 0, 0, 255]), "Pixel (0,0) should be black");

    // (63, 0): gray = (63 * 255) / 64 = 251
    let pixel = img.get_pixel(63, 0);
    let expected_gray = ((63 * 255) / 64) as u8;
    assert_eq!(
        pixel,
        Rgba([expected_gray, expected_gray, expected_gray, 255]),
        "Pixel (63,0) should be near white"
    );

    // (32, 0): gray = (32 * 255) / 64 = 127
    let pixel = img.get_pixel(32, 0);
    let expected_gray = ((32 * 255) / 64) as u8;
    assert_eq!(
        pixel,
        Rgba([expected_gray, expected_gray, expected_gray, 255]),
        "Pixel (32,0) should be mid-gray"
    );

    // Verify gradient increases along x-axis
    let pixel_10 = img.get_pixel(10, 0);
    let pixel_20 = img.get_pixel(20, 0);
    let pixel_30 = img.get_pixel(30, 0);
    assert!(
        pixel_10[0] < pixel_20[0] && pixel_20[0] < pixel_30[0],
        "Gradient should increase from left to right"
    );

    println!("  ✓ Frame 2 gradient pattern verified");
}

#[test]
#[ignore] // Run separately to avoid interference with the main test
fn test_cleanup_frame_files() {
    // Clean up generated frame files after tests
    for frame in 0..10 {
        let filename = format!("frame_{:04}.png", frame);
        if std::path::Path::new(&filename).exists() {
            std::fs::remove_file(&filename).ok();
        }
    }
}
