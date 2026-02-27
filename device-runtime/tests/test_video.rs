mod common;

use bus_shared::{Video, VideoConfig, VideoFormat};
use common::{
    create_test_runtime_with_registrations, load_and_boot_elf, read_word_with_timeout,
    wait_for_cpu_halt, LONG_TIMEOUT, SHORT_TIMEOUT,
};
use device_runtime::BusDeviceRegistration;
use riscv_shared::bus::VIDEO_BASE;
use std::sync::{Arc, Mutex};

const TEST_WIDTH: u32 = 16;
const TEST_HEIGHT: u32 = 16;

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
    let elf_path = sim_tests::test_program_path("test_video_pattern")
        .expect("Failed to find test_video_pattern");

    // Storage for captured frames
    type CapturedFrames = Arc<Mutex<Vec<(Vec<u8>, VideoConfig)>>>;
    let captured_frames: CapturedFrames = Arc::new(Mutex::new(Vec::new()));

    // Create callback that captures frame data in memory
    let frames_for_callback = Arc::clone(&captured_frames);
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

    // Register Video device at VIDEO_BASE at runtime creation.
    let video = Box::new(Video::with_fps(60, Some(present_callback)));
    let mut runtime = create_test_runtime_with_registrations(Some(vec![BusDeviceRegistration {
        base_addr: VIDEO_BASE,
        device: video,
    }]));

    // Startup-time memory transactions can be performed before loading/booting.
    let startup_status = read_word_with_timeout(runtime.as_mut(), VIDEO_BASE + 0x08, SHORT_TIMEOUT);
    assert_ne!(
        startup_status & (1 << 1),
        0,
        "PRESENT_READY should be set before boot"
    );

    load_and_boot_elf(runtime.as_mut(), &elf_path);

    // Handle termination by waiting for halt, then performing host memory transactions.
    let tohost_value = wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT);
    assert_eq!(
        tohost_value,
        Some(42),
        "Video test should exit with success code 42"
    );
    let final_status = read_word_with_timeout(runtime.as_mut(), VIDEO_BASE + 0x08, SHORT_TIMEOUT);
    assert_ne!(
        final_status & (1 << 1),
        0,
        "PRESENT_READY should be set after completion"
    );

    println!("\n=== Video Pattern Test Results ===");
    println!("Test program completed successfully");

    let frames = captured_frames.lock().expect("frames lock poisoned");
    assert_eq!(frames.len(), 3, "Should have captured 3 frames");

    println!("✓ Captured {} frames", frames.len());

    // Verify frame contents directly from memory
    verify_frame_0_checkerboard(&frames[0].0, &frames[0].1);
    verify_frame_1_diagonal_stripes(&frames[1].0, &frames[1].1);
    verify_frame_2_gradient(&frames[2].0, &frames[2].1);

    println!("✓ All frame patterns verified successfully");
    println!("\n=== Video Pattern Test Summary ===");
    println!("Test passed: 3 frames rendered and verified");
}

/// Verify frame 0: Red/Green checkerboard pattern
fn verify_frame_0_checkerboard(pixel_data: &[u8], config: &VideoConfig) {
    assert_eq!(config.width, TEST_WIDTH, "Frame 0 width should be 16");
    assert_eq!(config.height, TEST_HEIGHT, "Frame 0 height should be 16");
    assert_eq!(config.format, VideoFormat::Rgba8);

    // Verify checkerboard pattern at key points
    // (0, 0): even+even -> Red
    let pixel = get_pixel_rgba8(pixel_data, TEST_WIDTH, TEST_HEIGHT, 0, 0, config.format);
    assert_eq!(pixel, [255, 0, 0, 255], "Pixel (0,0) should be red");

    // (1, 0): odd+even -> Green
    let pixel = get_pixel_rgba8(pixel_data, TEST_WIDTH, TEST_HEIGHT, 1, 0, config.format);
    assert_eq!(pixel, [0, 255, 0, 255], "Pixel (1,0) should be green");

    // (0, 1): even+odd -> Green
    let pixel = get_pixel_rgba8(pixel_data, TEST_WIDTH, TEST_HEIGHT, 0, 1, config.format);
    assert_eq!(pixel, [0, 255, 0, 255], "Pixel (0,1) should be green");

    // (1, 1): odd+odd -> Red
    let pixel = get_pixel_rgba8(pixel_data, TEST_WIDTH, TEST_HEIGHT, 1, 1, config.format);
    assert_eq!(pixel, [255, 0, 0, 255], "Pixel (1,1) should be red");

    // (8, 8): even+even -> Red
    let pixel = get_pixel_rgba8(pixel_data, TEST_WIDTH, TEST_HEIGHT, 8, 8, config.format);
    assert_eq!(pixel, [255, 0, 0, 255], "Pixel (8,8) should be red");

    println!("  ✓ Frame 0 checkerboard pattern verified");
}

/// Verify frame 1: Blue/Yellow diagonal stripes
fn verify_frame_1_diagonal_stripes(pixel_data: &[u8], config: &VideoConfig) {
    assert_eq!(config.width, TEST_WIDTH, "Frame 1 width should be 16");
    assert_eq!(config.height, TEST_HEIGHT, "Frame 1 height should be 16");
    assert_eq!(config.format, VideoFormat::Rgba8);

    // (0, 0): (0+0) % 16 = 0 < 8 -> Blue
    let pixel = get_pixel_rgba8(pixel_data, TEST_WIDTH, TEST_HEIGHT, 0, 0, config.format);
    assert_eq!(pixel, [0, 0, 255, 255], "Pixel (0,0) should be blue");

    // (8, 0): (8+0) % 16 = 8 >= 8 -> Yellow
    let pixel = get_pixel_rgba8(pixel_data, TEST_WIDTH, TEST_HEIGHT, 8, 0, config.format);
    assert_eq!(pixel, [255, 255, 0, 255], "Pixel (8,0) should be yellow");

    // (0, 8): (0+8) % 16 = 8 >= 8 -> Yellow
    let pixel = get_pixel_rgba8(pixel_data, TEST_WIDTH, TEST_HEIGHT, 0, 8, config.format);
    assert_eq!(pixel, [255, 255, 0, 255], "Pixel (0,8) should be yellow");

    // (4, 4): (4+4) % 16 = 8 >= 8 -> Yellow
    let pixel = get_pixel_rgba8(pixel_data, TEST_WIDTH, TEST_HEIGHT, 4, 4, config.format);
    assert_eq!(pixel, [255, 255, 0, 255], "Pixel (4,4) should be yellow");

    println!("  ✓ Frame 1 diagonal stripes pattern verified");
}

/// Verify frame 2: Grayscale gradient
fn verify_frame_2_gradient(pixel_data: &[u8], config: &VideoConfig) {
    assert_eq!(config.width, TEST_WIDTH, "Frame 2 width should be 16");
    assert_eq!(config.height, TEST_HEIGHT, "Frame 2 height should be 16");
    assert_eq!(config.format, VideoFormat::Rgba8);

    // (0, 0): gray = 0
    let pixel = get_pixel_rgba8(pixel_data, TEST_WIDTH, TEST_HEIGHT, 0, 0, config.format);
    assert_eq!(pixel, [0, 0, 0, 255], "Pixel (0,0) should be black");

    // (15, 0): gray = (15 * 255) / 16 = 239
    let pixel = get_pixel_rgba8(pixel_data, TEST_WIDTH, TEST_HEIGHT, 15, 0, config.format);
    let expected_gray = ((15 * 255) / TEST_WIDTH) as u8;
    assert_eq!(
        pixel,
        [expected_gray, expected_gray, expected_gray, 255],
        "Pixel (15,0) should be near white"
    );

    // (8, 0): gray = (8 * 255) / 16 = 127
    let pixel = get_pixel_rgba8(pixel_data, TEST_WIDTH, TEST_HEIGHT, 8, 0, config.format);
    let expected_gray = ((8 * 255) / TEST_WIDTH) as u8;
    assert_eq!(
        pixel,
        [expected_gray, expected_gray, expected_gray, 255],
        "Pixel (8,0) should be mid-gray"
    );

    // Verify gradient increases along x-axis
    let pixel_2 = get_pixel_rgba8(pixel_data, TEST_WIDTH, TEST_HEIGHT, 2, 0, config.format);
    let pixel_4 = get_pixel_rgba8(pixel_data, TEST_WIDTH, TEST_HEIGHT, 4, 0, config.format);
    let pixel_6 = get_pixel_rgba8(pixel_data, TEST_WIDTH, TEST_HEIGHT, 6, 0, config.format);
    assert!(
        pixel_2[0] < pixel_4[0] && pixel_4[0] < pixel_6[0],
        "Gradient should increase from left to right"
    );

    println!("  ✓ Frame 2 gradient pattern verified");
}
