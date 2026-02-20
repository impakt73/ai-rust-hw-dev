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
use core::hint::black_box;
use core::panic::PanicInfo;
use core::ptr::write_volatile;
use riscv_rt::entry;
use riscv_shared::{AudioChannels, AudioConfig, AudioSampleRate, AUDIO_ADDR, AUDIO_CONFIG};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

/// Audio configuration
/// Use 1024 stereo samples as requested
const BUFFER_SIZE_SAMPLES: u32 = 1024;

/// Sine wave frequency divider
/// Lower values = higher frequency
const FREQUENCY_DIV: u32 = 16;

/// Fill the audio buffer with sine wave samples (called once at startup)
fn precompute_audio_buffer(buffer: &mut [u32]) {
    let buffer_base = buffer.as_mut_ptr() as u32;
    // Precompute 1024 stereo samples
    for i in 0..BUFFER_SIZE_SAMPLES {
        // Generate sine wave samples with phase shift for stereo effect
        let left_sample = generate_sine_sample(i, FREQUENCY_DIV);
        let right_sample = generate_sine_sample(i + FREQUENCY_DIV / 4, FREQUENCY_DIV);

        let offset = i * 4; // 4 bytes per stereo sample
        write_stereo_sample(buffer_base, offset, left_sample, right_sample);
    }
}

fn configure_audio_buffer(buffer: &mut [u32], config_register: u32) {
    unsafe {
        write_volatile(AUDIO_ADDR as *mut u32, buffer.as_mut_ptr() as u32);
        write_volatile(AUDIO_CONFIG as *mut u32, config_register);
    }
}

#[entry]
fn main() -> ! {
    common::init_heap(&HEAP);
    let mut buffer = vec![0u32; BUFFER_SIZE_SAMPLES as usize];

    // Precompute the audio buffer once at startup
    precompute_audio_buffer(buffer.as_mut_slice());

    // Configure Audio device
    // 48000Hz, Stereo, 1024 samples
    let config = AudioConfig {
        sample_rate: AudioSampleRate::Hz48000,
        channels: AudioChannels::Stereo,
        sample_count: BUFFER_SIZE_SAMPLES,
    };
    let config_register = config.to_register();

    configure_audio_buffer(buffer.as_mut_slice(), config_register);

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
        black_box(buffer.as_mut_slice());
        trigger_dma();
    }
}
