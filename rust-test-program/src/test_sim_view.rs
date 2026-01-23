#![no_std]
#![no_main]

mod common;

use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};
use riscv_rt::entry;
use riscv_shared::{
    AUDIO_ADDR, AUDIO_CONFIG, AUDIO_DMA, AUDIO_STATUS, VIDEO_ADDR, VIDEO_CONFIG, VIDEO_PRESENT,
    VIDEO_STATUS,
};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

/// Video status bits
const FRAME_READY: u32 = 1 << 0;
const PRESENT_READY: u32 = 1 << 1;

/// Video formats
const FORMAT_RGB8: u32 = 1;

/// Test image dimensions
const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;

/// Framebuffer base address in DRAM
const FRAMEBUFFER_BASE: u32 = 0x8000_1000;

/// Audio buffer base address in DRAM (after framebuffer)
/// Framebuffer is 64*64*3 = 12288 bytes (0x3000), so start audio buffer at 0x8000_1000 + 0x3000 = 0x8000_4000
const AUDIO_BUFFER_BASE: u32 = 0x8000_4000;

/// Helper to create VIDEO_CONFIG register value
/// Bits [11:0]   = width - 1
/// Bits [23:12]  = height - 1
/// Bits [31:24]  = format
const fn make_video_config(width: u32, height: u32, format: u32) -> u32 {
    ((width - 1) & 0xFFF) | (((height - 1) & 0xFFF) << 12) | ((format & 0xFF) << 24)
}

/// Helper to create AUDIO_CONFIG register value
/// Bits [1:0]   = sample_rate (0=48000Hz, 1=44100Hz, 2=22050Hz)
/// Bit 2        = channels (0=mono, 1=stereo)
/// Bits [18:3]  = sample_count - 1 (16 bits, allows 1-65536 samples with +1 bias)
const fn make_audio_config(sample_rate: u32, channels: u32, sample_count: u32) -> u32 {
    let sample_count_minus_1 = (sample_count - 1) & 0xFFFF;
    (sample_rate & 0x3) | ((channels & 0x1) << 2) | (sample_count_minus_1 << 3)
}

/// Wait for FRAME_READY bit to be set
fn wait_for_frame_ready() {
    unsafe {
        loop {
            let status = read_volatile(VIDEO_STATUS as *const u32);
            if (status & FRAME_READY) != 0 {
                break;
            }
        }
    }
}

/// Wait for PRESENT_READY bit to be set
fn wait_for_present_ready() {
    unsafe {
        loop {
            let status = read_volatile(VIDEO_STATUS as *const u32);
            if (status & PRESENT_READY) != 0 {
                break;
            }
        }
    }
}

/// Trigger a present operation
fn trigger_present() {
    unsafe {
        write_volatile(VIDEO_PRESENT as *mut u32, 0);
    }
}

/// Write a pixel to the framebuffer at (x, y)
/// Pixel format is RGB8 (3 bytes per pixel)
fn write_pixel(x: u32, y: u32, r: u8, g: u8, b: u8) {
    unsafe {
        let offset = (y * WIDTH + x) * 3;
        let addr = FRAMEBUFFER_BASE + offset;
        write_volatile(addr as *mut u8, r);
        write_volatile((addr + 1) as *mut u8, g);
        write_volatile((addr + 2) as *mut u8, b);
    }
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
fn render_scrolling_checkerboard(frame_index: u32) {
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
                write_pixel(x, y, r, g, b);
            } else {
                // Dark checker - use dimmed color
                write_pixel(x, y, r / 4, g / 4, b / 4);
            }
        }
    }
}

/// Write a stereo sample to the buffer
/// For stereo: writes 4 bytes (2 × i16, one for left and one for right)
fn write_stereo_sample(buffer_base: u32, offset: u32, left: i16, right: i16) {
    unsafe {
        let addr = buffer_base + offset;
        let left_bytes = left.to_le_bytes();
        let right_bytes = right.to_le_bytes();
        write_volatile(addr as *mut u8, left_bytes[0]);
        write_volatile((addr + 1) as *mut u8, left_bytes[1]);
        write_volatile((addr + 2) as *mut u8, right_bytes[0]);
        write_volatile((addr + 3) as *mut u8, right_bytes[1]);
    }
}

/// Check if DMA is ready (bit 0 of AUDIO_STATUS)
fn is_audio_dma_ready() -> bool {
    unsafe { (read_volatile(AUDIO_STATUS as *const u32) & 1) != 0 }
}

/// Trigger audio DMA operation
fn trigger_audio_dma() {
    unsafe {
        write_volatile(AUDIO_DMA as *mut u32, 0);
    }
}

/// Fill audio buffer with samples and trigger DMA
fn fill_audio_buffer_and_trigger(buffer_size_samples: u32, sample_index: &mut u32) {
    const AUDIO_FREQUENCY_DIV: u32 = 16; // Sine wave frequency divider

    // Fill buffer with samples
    for i in 0..buffer_size_samples {
        // Generate sine wave samples with phase shift for stereo effect
        let left_sample = common::generate_sine_sample(*sample_index, AUDIO_FREQUENCY_DIV);
        let right_sample = common::generate_sine_sample(*sample_index + 4, AUDIO_FREQUENCY_DIV);

        let offset = (i * 4) as u32; // 4 bytes per stereo sample
        write_stereo_sample(AUDIO_BUFFER_BASE, offset, left_sample, right_sample);

        *sample_index += 1;
    }

    // Wait for DMA to be ready
    while !is_audio_dma_ready() {
        // Spin wait
    }

    // Trigger DMA to read the buffer
    trigger_audio_dma();
}

#[entry]
fn main() -> ! {
    unsafe {
        // Configure Video device
        write_volatile(VIDEO_ADDR as *mut u32, FRAMEBUFFER_BASE);
        write_volatile(
            VIDEO_CONFIG as *mut u32,
            make_video_config(WIDTH, HEIGHT, FORMAT_RGB8),
        );

        // Configure Audio device
        // Use ~0.5 second buffer at 48kHz stereo
        // 48000 samples/sec * 0.5 sec = 24000 samples
        // Round to 16384 samples (~0.34 seconds)
        const BUFFER_SIZE_SAMPLES: u32 = 16384;

        write_volatile(AUDIO_ADDR as *mut u32, AUDIO_BUFFER_BASE);
        write_volatile(
            AUDIO_CONFIG as *mut u32,
            make_audio_config(0, 1, BUFFER_SIZE_SAMPLES), // 48000Hz, Stereo, 16384 samples
        );

        // Initialize counters
        let mut frame_index: u32 = 0;
        let mut audio_sample_index: u32 = 0;

        // Pre-fill audio buffer and trigger initial DMA
        fill_audio_buffer_and_trigger(BUFFER_SIZE_SAMPLES, &mut audio_sample_index);

        // Main infinite loop
        loop {
            // Wait for video to be ready for a new frame
            wait_for_frame_ready();

            // Render the scrolling checkerboard pattern
            render_scrolling_checkerboard(frame_index);

            // Wait until we can present
            wait_for_present_ready();

            // Trigger the present operation
            trigger_present();

            // Increment frame index for next frame
            frame_index += 1;

            // Check if audio DMA is ready and refill buffer
            if is_audio_dma_ready() {
                fill_audio_buffer_and_trigger(BUFFER_SIZE_SAMPLES, &mut audio_sample_index);
            }
        }
    }
}
