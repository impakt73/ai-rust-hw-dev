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
const AUDIO_ADDR: u32 = AUDIO_BASE + 0x00;
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

/// Generate a sine wave sample
/// Uses a simple approximation: sin(x) ≈ x for small x
/// For a full sine wave, we'll use a lookup table approach
fn generate_sine_sample(index: u32, frequency_div: u32) -> i16 {
    // Simple sine wave using lookup table approximation
    // We'll use a 32-entry lookup table for a quarter wave
    const QUARTER_WAVE_LEN: u32 = 32;
    const FULL_WAVE_LEN: u32 = QUARTER_WAVE_LEN * 4;
    
    // Normalize index to position in full wave
    let phase = (index / frequency_div) % FULL_WAVE_LEN;
    
    // Quarter wave lookup table (0 to pi/2, scaled to 0-32767)
    const SINE_TABLE: [i16; 32] = [
        0, 1608, 3212, 4808, 6393, 7962, 9512, 11039,
        12539, 14010, 15446, 16846, 18204, 19519, 20787, 22005,
        23170, 24279, 25329, 26319, 27245, 28105, 28898, 29621,
        30273, 30852, 31356, 31785, 32137, 32412, 32609, 32728,
    ];
    
    // Determine which quarter of the wave we're in and compute the value
    if phase < QUARTER_WAVE_LEN {
        // First quarter (0 to π/2): rising, positive
        SINE_TABLE[phase as usize]
    } else if phase < QUARTER_WAVE_LEN * 2 {
        // Second quarter (π/2 to π): falling, positive
        SINE_TABLE[(QUARTER_WAVE_LEN * 2 - 1 - phase) as usize]
    } else if phase < QUARTER_WAVE_LEN * 3 {
        // Third quarter (π to 3π/2): falling, negative
        -SINE_TABLE[(phase - QUARTER_WAVE_LEN * 2) as usize]
    } else {
        // Fourth quarter (3π/2 to 2π): rising, negative
        -SINE_TABLE[(QUARTER_WAVE_LEN * 4 - 1 - phase) as usize]
    }
}

/// Write a sample to the ring buffer
/// For mono: writes 2 bytes (i16)
fn write_mono_sample(buffer_base: u32, offset: u32, sample: i16) {
    unsafe {
        let addr = buffer_base + offset;
        let bytes = sample.to_le_bytes();
        write_volatile(addr as *mut u8, bytes[0]);
        write_volatile((addr + 1) as *mut u8, bytes[1]);
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
/// Returns true if space is available, false if we should stop
fn wait_for_space(buffer_size: u32, current_write: u32, samples_to_write: u32) -> bool {
    // Calculate required space (in bytes)
    let bytes_to_write = samples_to_write * 2; // 2 bytes per mono sample
    
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
        
        // If we have enough space, return true
        if available >= bytes_to_write {
            return true;
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
        const BUFFER_SIZE_BYTES: u32 = BUFFER_SIZE_SAMPLES * 2; // 128 bytes (mono, 2 bytes per sample)
        
        write_volatile(AUDIO_ADDR as *mut u32, RING_BUFFER_BASE);
        write_volatile(
            AUDIO_CONFIG as *mut u32,
            make_audio_config(0, 0, LOG2_BUFFER_SIZE), // 48000Hz, Mono, 64 samples
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
            
            // Write the samples
            for i in 0..samples_to_write {
                let sample_index = samples_written + i;
                let sample = generate_sine_sample(sample_index, FREQUENCY_DIV);
                
                write_mono_sample(RING_BUFFER_BASE, write_ptr, sample);
                
                // Update write pointer (with wrapping)
                write_ptr = (write_ptr + 2) % BUFFER_SIZE_BYTES;
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
