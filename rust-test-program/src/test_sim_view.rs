#![no_std]
#![no_main]

extern crate alloc;

mod common;

#[global_allocator]
static HEAP: common::Heap = common::Heap::empty();

use alloc::vec;
use common::{
    generate_sine_sample, is_dma_ready, is_sample_buffer_ready, trigger_dma, trigger_present,
    wait_for_frame_ready, wait_for_present_ready, write_stereo_sample,
};
use core::panic::PanicInfo;
use core::ptr::write_volatile;
use riscv_rt::entry;
use riscv_shared::{
    AudioChannels, AudioConfig, AudioSampleRate, VideoConfig, VideoFormat, AUDIO_ADDR,
    AUDIO_CONFIG, VIDEO_ADDR, VIDEO_CONFIG,
};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

/// Test image dimensions
const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;

/// Audio buffer size: 1024 stereo samples as requested
const AUDIO_BUFFER_SIZE_SAMPLES: u32 = 1024;

/// Sine wave frequency divider for audio
const AUDIO_FREQUENCY_DIV: u32 = 16;

/// Write a pixel to the framebuffer at (x, y)
/// Pixel format is RGB8 (3 bytes per pixel)
fn write_pixel(framebuffer_base: u32, x: u32, y: u32, r: u8, g: u8, b: u8) {
    riscv_shared::write_pixel_rgb8(framebuffer_base, WIDTH, x, y, r, g, b);
}

/// Color palette for the scrolling checkerboard
const COLORS: [(u8, u8, u8); 8] = [
    (255, 0, 0),   // Red
    (0, 255, 0),   // Green
    (0, 0, 255),   // Blue
    (255, 255, 0), // Yellow
    (255, 0, 255), // Magenta
    (0, 255, 255), // Cyan
    (255, 128, 0), // Orange
    (128, 0, 255), // Purple
];

/// Render scrolling checkerboard pattern
/// The pattern scrolls diagonally based on the frame index
fn render_scrolling_checkerboard(framebuffer_base: u32, frame_index: u32) {
    // Checkerboard size
    const CHECKER_SIZE: u32 = 8;

    // Calculate scroll offset (wraps around)
    let scroll_x = (frame_index * 2) % CHECKER_SIZE;
    let scroll_y = (frame_index * 2) % CHECKER_SIZE;

    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            // Apply scrolling offset
            let scrolled_x = (x + scroll_x) / CHECKER_SIZE;
            let scrolled_y = (y + scroll_y) / CHECKER_SIZE;

            // Determine if this is a light or dark checker
            let is_light = (scrolled_x + scrolled_y).is_multiple_of(2);

            // Select color based on position and time
            let color_index = ((scrolled_x + scrolled_y + frame_index / 4) % 8) as usize;
            let (r, g, b) = COLORS[color_index];

            if is_light {
                write_pixel(framebuffer_base, x, y, r, g, b);
            } else {
                // Dark checker - use dimmed color
                write_pixel(framebuffer_base, x, y, r / 4, g / 4, b / 4);
            }
        }
    }
}

/// Precompute audio buffer with sine wave samples (called once at startup)
fn precompute_audio_buffer(audio_buffer_base: u32) {
    // Precompute 1024 stereo samples
    for i in 0..AUDIO_BUFFER_SIZE_SAMPLES {
        // Generate sine wave samples with phase shift for stereo effect
        let left_sample = generate_sine_sample(i, AUDIO_FREQUENCY_DIV);
        let right_sample = generate_sine_sample(i + AUDIO_FREQUENCY_DIV / 4, AUDIO_FREQUENCY_DIV);

        let offset = i * 4; // 4 bytes per stereo sample
        write_stereo_sample(audio_buffer_base, offset, left_sample, right_sample);
    }
}

#[entry]
fn main() -> ! {
    unsafe {
        common::init_heap(&HEAP);
        let mut framebuffer = vec![0u8; (WIDTH * HEIGHT * 3) as usize];
        let framebuffer_base = framebuffer.as_mut_ptr() as u32;
        let mut audio_buffer = vec![0u8; (AUDIO_BUFFER_SIZE_SAMPLES * 4) as usize];
        let audio_buffer_base = audio_buffer.as_mut_ptr() as u32;

        // Configure Video device
        let video_config = VideoConfig {
            width: WIDTH,
            height: HEIGHT,
            format: VideoFormat::Rgb8,
        };
        write_volatile(VIDEO_ADDR as *mut u32, framebuffer_base);
        write_volatile(VIDEO_CONFIG as *mut u32, video_config.to_register());

        // Precompute the audio buffer once at startup
        precompute_audio_buffer(audio_buffer_base);

        // Configure Audio device
        // 48000Hz, Stereo, 1024 samples
        let audio_config = AudioConfig {
            sample_rate: AudioSampleRate::Hz48000,
            channels: AudioChannels::Stereo,
            sample_count: AUDIO_BUFFER_SIZE_SAMPLES,
        };
        write_volatile(AUDIO_ADDR as *mut u32, audio_buffer_base);
        write_volatile(AUDIO_CONFIG as *mut u32, audio_config.to_register());

        // Initialize frame counter
        let mut frame_index: u32 = 0;

        // Main infinite loop
        loop {
            // Wait for video to be ready for a new frame
            wait_for_frame_ready();

            // Render the scrolling checkerboard pattern
            render_scrolling_checkerboard(framebuffer_base, frame_index);

            // Wait until we can present
            wait_for_present_ready();

            // Trigger the present operation
            trigger_present();

            // Increment frame index for next frame
            frame_index += 1;

            // Check if audio DMA is ready and sample buffer is ready, then trigger with precomputed buffer
            if is_dma_ready() && is_sample_buffer_ready() {
                trigger_dma();
            }
        }
    }
}
