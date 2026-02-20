#![no_std]
#![no_main]

extern crate alloc;

mod common;

#[global_allocator]
static HEAP: common::Heap = common::Heap::empty();

use alloc::vec;
use common::{
    generate_sine_sample, is_dma_ready, is_sample_buffer_ready, trigger_dma, write_stereo_sample,
};
use core::panic::PanicInfo;
use core::ptr::write_volatile;
use riscv_rt::entry;
use riscv_shared::{AudioChannels, AudioConfig, AudioSampleRate, AUDIO_ADDR, AUDIO_CONFIG};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

#[entry]
fn main() -> ! {
    unsafe {
        common::init_heap(&HEAP);

        // Configure Audio device
        // Use a small buffer (64 samples) for the test
        const BUFFER_SIZE_SAMPLES: u32 = 64;
        const TOTAL_SAMPLES: u32 = 500; // Generate 500 total samples
        const FREQUENCY_DIV: u32 = 4; // Sine wave frequency divider

        let mut buffer = vec![0u8; (BUFFER_SIZE_SAMPLES * 4) as usize];
        let buffer_base = buffer.as_mut_ptr() as u32;

        let config = AudioConfig {
            sample_rate: AudioSampleRate::Hz48000,
            channels: AudioChannels::Stereo,
            sample_count: BUFFER_SIZE_SAMPLES,
        };

        write_volatile(AUDIO_ADDR as *mut u32, buffer_base);
        write_volatile(AUDIO_CONFIG as *mut u32, config.to_register());

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
                let left_sample = generate_sine_sample(sample_index, FREQUENCY_DIV);
                // Right channel is phase-shifted by 90 degrees for stereo effect
                let right_sample =
                    generate_sine_sample(sample_index + FREQUENCY_DIV / 4, FREQUENCY_DIV);

                let offset = i * 4; // 4 bytes per stereo sample
                write_stereo_sample(buffer_base, offset, left_sample, right_sample);
            }

            // Update config to specify how many samples to read in this DMA operation
            let batch_config = AudioConfig {
                sample_rate: AudioSampleRate::Hz48000,
                channels: AudioChannels::Stereo,
                sample_count: batch_size,
            };
            write_volatile(AUDIO_CONFIG as *mut u32, batch_config.to_register());

            // Wait for DMA to be ready
            while !is_dma_ready() {
                // Spin wait
            }

            // Wait for sample buffer to be ready (back pressure)
            while !is_sample_buffer_ready() {
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
