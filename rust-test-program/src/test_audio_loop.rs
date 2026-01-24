#![no_std]
#![no_main]

mod common;

use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};
use riscv_rt::entry;
use riscv_shared::{AUDIO_ADDR, AUDIO_CONFIG, AUDIO_DMA, AUDIO_STATUS};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

/// Buffer base address in DRAM
const BUFFER_BASE: u32 = 0x8000_2000;

/// Audio configuration
/// Use 1024 stereo samples as requested
const BUFFER_SIZE_SAMPLES: u32 = 1024;

/// Sine wave frequency divider
/// Lower values = higher frequency
const FREQUENCY_DIV: u32 = 16;

/// Helper to create AUDIO_CONFIG register value
/// Bits [1:0]   = sample_rate (0=48000Hz, 1=44100Hz, 2=22050Hz)
/// Bit 2        = channels (0=mono, 1=stereo)
/// Bits [18:3]  = sample_count - 1 (16 bits, allows 1-65536 samples with +1 bias)
const fn make_audio_config(sample_rate: u32, channels: u32, sample_count: u32) -> u32 {
    let sample_count_minus_1 = (sample_count - 1) & 0xFFFF;
    (sample_rate & 0x3) | ((channels & 0x1) << 2) | (sample_count_minus_1 << 3)
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
fn is_dma_ready() -> bool {
    unsafe { (read_volatile(AUDIO_STATUS as *const u32) & 1) != 0 }
}

/// Check if sample buffer is ready (bit 1 of AUDIO_STATUS)
fn is_sample_buffer_ready() -> bool {
    unsafe { (read_volatile(AUDIO_STATUS as *const u32) & 2) != 0 }
}

/// Trigger DMA operation
fn trigger_dma() {
    unsafe {
        write_volatile(AUDIO_DMA as *mut u32, 0);
    }
}

/// Fill the audio buffer with sine wave samples (called once at startup)
fn precompute_audio_buffer() {
    // Precompute 1024 stereo samples
    for i in 0..BUFFER_SIZE_SAMPLES {
        // Generate sine wave samples with phase shift for stereo effect
        let left_sample = common::generate_sine_sample(i, FREQUENCY_DIV);
        let right_sample = common::generate_sine_sample(i + FREQUENCY_DIV / 4, FREQUENCY_DIV);

        let offset = i * 4; // 4 bytes per stereo sample
        write_stereo_sample(BUFFER_BASE, offset, left_sample, right_sample);
    }
}

#[entry]
fn main() -> ! {
    unsafe {
        // Configure Audio device
        // 48000Hz, Stereo, 1024 samples
        write_volatile(AUDIO_ADDR as *mut u32, BUFFER_BASE);
        write_volatile(
            AUDIO_CONFIG as *mut u32,
            make_audio_config(0, 1, BUFFER_SIZE_SAMPLES),
        );

        // Precompute the audio buffer once at startup
        precompute_audio_buffer();

        // Main infinite loop
        loop {
            // Wait for DMA to be ready
            while !is_dma_ready() {
                // Spin wait
            }

            // Wait for sample buffer to be ready (back pressure)
            while !is_sample_buffer_ready() {
                // Spin wait
            }

            // Trigger DMA to read the same precomputed buffer
            trigger_dma();
        }
    }
}
