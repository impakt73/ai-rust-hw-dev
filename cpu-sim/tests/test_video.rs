use cpu_sim::*;
use image::{ImageBuffer, Rgba};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

fn test_program_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("test_programs")
        .join(name)
}

/// Convert pixel data from the video format to RGBA8 and save as PNG
fn save_video_frame_as_png(
    pixel_data: &[u8],
    config: &VideoConfig,
    filename: &str,
) -> Result<(), String> {
    let width = config.width;
    let height = config.height;

    // Convert pixel data to RGBA8 format
    let rgba_data = match config.format {
        VideoFormat::Rgba8 => {
            // Already in RGBA8 format
            pixel_data.to_vec()
        }
        VideoFormat::Rgb8 => {
            // Convert RGB8 to RGBA8 (add alpha channel)
            let mut rgba = Vec::with_capacity((width * height * 4) as usize);
            for chunk in pixel_data.chunks_exact(3) {
                rgba.push(chunk[0]); // R
                rgba.push(chunk[1]); // G
                rgba.push(chunk[2]); // B
                rgba.push(255); // A (opaque)
            }
            rgba
        }
        VideoFormat::Rgb565 => {
            // Convert RGB565 to RGBA8
            let mut rgba = Vec::with_capacity((width * height * 4) as usize);
            for chunk in pixel_data.chunks_exact(2) {
                let rgb565 = u16::from_le_bytes([chunk[0], chunk[1]]);
                let r = ((rgb565 >> 11) & 0x1F) as u8;
                let g = ((rgb565 >> 5) & 0x3F) as u8;
                let b = (rgb565 & 0x1F) as u8;

                // Scale to 8-bit
                rgba.push((r << 3) | (r >> 2)); // R: 5-bit to 8-bit
                rgba.push((g << 2) | (g >> 4)); // G: 6-bit to 8-bit
                rgba.push((b << 3) | (b >> 2)); // B: 5-bit to 8-bit
                rgba.push(255); // A: opaque
            }
            rgba
        }
        VideoFormat::R8 => {
            // Convert grayscale to RGBA8
            let mut rgba = Vec::with_capacity((width * height * 4) as usize);
            for &gray in pixel_data {
                rgba.push(gray); // R
                rgba.push(gray); // G
                rgba.push(gray); // B
                rgba.push(255); // A (opaque)
            }
            rgba
        }
    };

    // Create image buffer from RGBA8 data
    let img_buffer = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(width, height, rgba_data)
        .ok_or_else(|| "Failed to create image buffer from pixel data".to_string())?;

    // Save the image
    img_buffer
        .save(filename)
        .map_err(|e| format!("Failed to save image: {}", e))?;

    Ok(())
}

#[test]
fn test_video_pattern() {
    let _ = env_logger::builder().is_test(true).try_init();

    let elf_path = test_program_path("test_video_pattern.elf");

    // Video device base address (must match test program)
    const VIDEO_BASE: u32 = 0x3000_0000;

    // Track frame counter for generating filenames
    let frame_counter = Rc::new(RefCell::new(0));

    // Setup callback to register Video device
    let frame_counter_setup = frame_counter.clone();
    let setup_callback = move |view: &mut SimulatorView| {
        let frame_counter_present = frame_counter_setup.clone();

        // Create callback that saves frames as PNG files
        let present_callback = move |data: &[u8], config: &VideoConfig| {
            let mut counter = frame_counter_present.borrow_mut();
            let filename = format!("frame_{:04}.png", *counter);

            match save_video_frame_as_png(data, config, &filename) {
                Ok(()) => {
                    log::info!(
                        "Frame {} saved to {} ({}x{} {:?})",
                        *counter,
                        filename,
                        config.width,
                        config.height,
                        config.format
                    );
                }
                Err(e) => {
                    log::error!("Failed to save frame {}: {}", *counter, e);
                }
            }

            *counter += 1;
        };

        // Register Video device at 0x3000_0000 with very high FPS for testing
        // At 100MHz CPU, 10000 FPS = 10,000 cycles per frame
        let video = Box::new(Video::with_fps(10000, present_callback));
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
    use image::GenericImageView;

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
    use image::GenericImageView;

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
    use image::GenericImageView;

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
