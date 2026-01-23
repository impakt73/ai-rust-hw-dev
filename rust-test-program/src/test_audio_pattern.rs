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

/// Trigger DMA operation
fn trigger_dma() {
    unsafe {
        write_volatile(AUDIO_DMA as *mut u32, 0);
    }
}

#[entry]
fn main() -> ! {
    unsafe {
        // Configure Audio device
        // Use a small buffer (64 samples) for the test
        const BUFFER_SIZE_SAMPLES: u32 = 64;
        const TOTAL_SAMPLES: u32 = 500; // Generate 500 total samples
        const FREQUENCY_DIV: u32 = 4; // Sine wave frequency divider

        write_volatile(AUDIO_ADDR as *mut u32, BUFFER_BASE);
        write_volatile(
            AUDIO_CONFIG as *mut u32,
            make_audio_config(0, 1, BUFFER_SIZE_SAMPLES), // 48000Hz, Stereo, 64 samples
        );

        let mut samples_written: u32 = 0;

        // Generate samples in batches and trigger DMA for each batch
        while samples_written < TOTAL_SAMPLES {
            // Calculate how many samples to generate in this batch
            let samples_remaining = TOTAL_SAMPLES - samples_written;
            let batch_size = if samples_remaining > BUFFER_SIZE_SAMPLES {
                BUFFER_SIZE_SAMPLES
            } else {
                samples_remaining
            };

            // Write samples to buffer memory
            for i in 0..batch_size {
                let sample_index = samples_written + i;
                let left_sample = common::generate_sine_sample(sample_index, FREQUENCY_DIV);
                // Right channel is phase-shifted by 90 degrees for stereo effect
                let right_sample =
                    common::generate_sine_sample(sample_index + FREQUENCY_DIV / 4, FREQUENCY_DIV);

                let offset = i * 4; // 4 bytes per stereo sample
                write_stereo_sample(BUFFER_BASE, offset, left_sample, right_sample);
            }

            // Update config to specify how many samples to read in this DMA operation
            write_volatile(
                AUDIO_CONFIG as *mut u32,
                make_audio_config(0, 1, batch_size),
            );

            // Wait for DMA to be ready
            while !is_dma_ready() {
                // Spin wait
            }

            // Trigger DMA operation
            trigger_dma();

            samples_written += batch_size;
        }

        // Wait for final DMA to complete
        while !is_dma_ready() {
            // Spin wait
        }

        // Success!
        common::write_tohost(common::SUCCESS_CODE);
    }
}
