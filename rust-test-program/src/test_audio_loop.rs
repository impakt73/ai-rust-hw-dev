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

/// Audio configuration
/// Use a reasonable buffer size: 2^14 = 16384 samples (~0.34 seconds at 48kHz)
const LOG2_BUFFER_SIZE: u32 = 14;
const BUFFER_SIZE_SAMPLES: u32 = 1 << LOG2_BUFFER_SIZE;
const BUFFER_SIZE_BYTES: u32 = BUFFER_SIZE_SAMPLES * 4; // stereo, 4 bytes per sample

/// Sine wave frequency divider
/// Lower values = higher frequency
const FREQUENCY_DIV: u32 = 16;

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

/// Read the current read pointer from the audio device
fn read_read_ptr() -> u32 {
    unsafe { read_volatile(AUDIO_READ_PTR as *const u32) }
}

/// Update the write pointer to the audio device
fn write_write_ptr(offset: u32) {
    unsafe {
        write_volatile(AUDIO_WRITE_PTR as *mut u32, offset);
    }
}

/// Calculate how many samples can be written to the buffer
/// Returns the number of bytes available for writing
fn calculate_available_space(write_ptr: u32) -> u32 {
    let read_ptr = read_read_ptr();

    // Calculate free space in the circular buffer
    if write_ptr >= read_ptr {
        // Write pointer is ahead: free space is from write to end, plus start to read
        (BUFFER_SIZE_BYTES - write_ptr) + read_ptr
    } else {
        // Write pointer has wrapped: free space is just read - write
        read_ptr - write_ptr
    }
}

/// Fill the audio buffer with sine wave samples
/// Fills as much of the available space as possible
fn fill_audio_buffer(write_ptr: &mut u32, sample_index: &mut u32) -> u32 {
    let available = calculate_available_space(*write_ptr);

    // Reserve at least 1 sample space to distinguish full from empty
    let samples_to_write = if available > 4 {
        (available - 4) / 4 // Convert bytes to samples (4 bytes per stereo sample)
    } else {
        0
    };

    // Write samples to fill the available space
    for _ in 0..samples_to_write {
        // Generate sine wave samples with phase shift for stereo effect
        let left_sample = common::generate_sine_sample(*sample_index, FREQUENCY_DIV);
        let right_sample =
            common::generate_sine_sample(*sample_index + FREQUENCY_DIV / 4, FREQUENCY_DIV);

        write_stereo_sample(RING_BUFFER_BASE, *write_ptr, left_sample, right_sample);

        // Update write pointer (with wrapping) - 4 bytes per stereo sample
        *write_ptr = (*write_ptr + 4) % BUFFER_SIZE_BYTES;
        *sample_index += 1;
    }

    // Update the device's write pointer
    write_write_ptr(*write_ptr);

    samples_to_write
}

#[entry]
fn main() -> ! {
    unsafe {
        // Configure Audio device
        // 48000Hz, Stereo, 16384 samples
        write_volatile(AUDIO_ADDR as *mut u32, RING_BUFFER_BASE);
        write_volatile(
            AUDIO_CONFIG as *mut u32,
            make_audio_config(0, 1, LOG2_BUFFER_SIZE),
        );

        // Initialize counters
        let mut audio_write_ptr: u32 = 0;
        let mut audio_sample_index: u32 = 0;

        // Main infinite loop
        loop {
            // Check the status of the circular audio buffer pointers
            // and fill any available space in the buffer with new audio samples
            fill_audio_buffer(&mut audio_write_ptr, &mut audio_sample_index);
        }
    }
}
