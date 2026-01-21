#![no_std]
#![no_main]

mod common;

use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};
use riscv_rt::entry;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

/// Audio device base address
const AUDIO_BASE: u32 = 0x3000_0000;

/// Audio register offsets
const AUDIO_ADDR: u32 = AUDIO_BASE;
const AUDIO_CONFIG: u32 = AUDIO_BASE + 0x04;
const AUDIO_READ_PTR: u32 = AUDIO_BASE + 0x08;
const AUDIO_WRITE_PTR: u32 = AUDIO_BASE + 0x0C;

/// Ring buffer base address in DRAM
const RING_BUFFER_BASE: u32 = 0x8000_2000;

/// Helper to create AUDIO_CONFIG register value
/// Bits [1:0]   = sample_rate (0=48000Hz, 1=44100Hz, 2=22050Hz)
/// Bit 2        = channels (0=mono, 1=stereo)
/// Bits [7:3]   = log2(sample_count)
const fn make_audio_config(sample_rate: u32, channels: u32, log2_sample_count: u32) -> u32 {
    (sample_rate & 0x3) | ((channels & 0x1) << 2) | ((log2_sample_count & 0x1F) << 3)
}

/// Write a stereo sample to the ring buffer
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

/// Read the current read pointer
fn read_read_ptr() -> u32 {
    unsafe { read_volatile(AUDIO_READ_PTR as *const u32) }
}

/// Update the write pointer
fn write_write_ptr(offset: u32) {
    unsafe {
        write_volatile(AUDIO_WRITE_PTR as *mut u32, offset);
    }
}

/// Wait until there's space in the ring buffer for more samples
/// Blocks until the required space becomes available
fn wait_for_space(buffer_size: u32, current_write: u32, samples_to_write: u32) {
    // Calculate required space (in bytes) - 4 bytes per stereo sample
    let bytes_to_write = samples_to_write * 4;

    // Simple approach: wait until read pointer has moved past our intended write position
    // This is a simple producer-consumer pattern
    loop {
        let read_ptr = read_read_ptr();

        // Calculate available space
        let available = if read_ptr > current_write {
            read_ptr - current_write
        } else {
            buffer_size - current_write + read_ptr
        };

        // If we have enough space, return
        if available >= bytes_to_write {
            return;
        }

        // If read_ptr hasn't moved at all for a while, we might be done
        // For simplicity, we'll just keep trying
    }
}

#[entry]
fn main() -> ! {
    unsafe {
        // Configure Audio device
        // Use a small buffer (64 samples = 2^6) to ensure wrapping happens quickly
        const LOG2_BUFFER_SIZE: u32 = 6; // 64 samples
        const BUFFER_SIZE_SAMPLES: u32 = 1 << LOG2_BUFFER_SIZE; // 64
        const BUFFER_SIZE_BYTES: u32 = BUFFER_SIZE_SAMPLES * 4; // 256 bytes (stereo, 4 bytes per sample)

        write_volatile(AUDIO_ADDR as *mut u32, RING_BUFFER_BASE);
        write_volatile(
            AUDIO_CONFIG as *mut u32,
            make_audio_config(0, 1, LOG2_BUFFER_SIZE), // 48000Hz, Stereo, 64 samples
        );

        // Generate and write audio samples
        // We'll generate enough samples to cause multiple buffer wraps
        const TOTAL_SAMPLES: u32 = 500; // This will wrap the 64-sample buffer ~8 times
        const FREQUENCY_DIV: u32 = 4; // How many samples per sine wave cycle

        let mut write_ptr: u32 = 0;
        let mut samples_written: u32 = 0;

        while samples_written < TOTAL_SAMPLES {
            // How many samples can we write this iteration?
            const CHUNK_SIZE: u32 = 16; // Write 16 samples at a time
            let samples_to_write = if samples_written + CHUNK_SIZE <= TOTAL_SAMPLES {
                CHUNK_SIZE
            } else {
                TOTAL_SAMPLES - samples_written
            };

            // Wait for space in the buffer
            wait_for_space(BUFFER_SIZE_BYTES, write_ptr, samples_to_write);

            // Write the samples (stereo: left and right channels)
            for i in 0..samples_to_write {
                let sample_index = samples_written + i;
                let left_sample = common::generate_sine_sample(sample_index, FREQUENCY_DIV);
                // Right channel is phase-shifted by 90 degrees for stereo effect
                let right_sample =
                    common::generate_sine_sample(sample_index + FREQUENCY_DIV / 4, FREQUENCY_DIV);

                write_stereo_sample(RING_BUFFER_BASE, write_ptr, left_sample, right_sample);

                // Update write pointer (with wrapping) - 4 bytes per stereo sample
                write_ptr = (write_ptr + 4) % BUFFER_SIZE_BYTES;
            }

            // Update the device's write pointer
            write_write_ptr(write_ptr);

            samples_written += samples_to_write;
        }

        // Wait for all samples to be consumed
        // When read_ptr == write_ptr, all data has been read
        loop {
            let read_ptr = read_read_ptr();
            if read_ptr == write_ptr {
                break;
            }
        }

        // Success!
        common::write_tohost(common::SUCCESS_CODE);
    }
}
