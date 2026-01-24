//! Helper functions for interacting with Audio device via MMIO
//!
//! This module provides high-level helper functions for common Audio device
//! operations like checking status, triggering DMA, and writing audio samples.

use core::ptr::{read_volatile, write_volatile};

use crate::audio::{AUDIO_DMA, AUDIO_STATUS};

/// Audio status bit flags
pub const DMA_READY: u32 = 1 << 0;
pub const SAMPLE_BUFFER_READY: u32 = 1 << 1;

/// Check if Audio DMA is ready (bit 0 of AUDIO_STATUS)
///
/// Returns `true` if the DMA controller is ready to accept a new transfer request.
///
/// # Safety
/// This function performs volatile MMIO reads and should only be called in a
/// bare-metal environment where the AUDIO_STATUS register is properly mapped.
pub fn is_dma_ready() -> bool {
    unsafe { (read_volatile(AUDIO_STATUS as *const u32) & DMA_READY) != 0 }
}

/// Check if Audio sample buffer is ready (bit 1 of AUDIO_STATUS)
///
/// Returns `true` if the audio sample buffer has space available for new samples.
///
/// # Safety
/// This function performs volatile MMIO reads and should only be called in a
/// bare-metal environment where the AUDIO_STATUS register is properly mapped.
pub fn is_sample_buffer_ready() -> bool {
    unsafe { (read_volatile(AUDIO_STATUS as *const u32) & SAMPLE_BUFFER_READY) != 0 }
}

/// Trigger an Audio DMA operation by writing to AUDIO_DMA register
///
/// This function triggers the audio device to start a DMA transfer from the
/// configured buffer address. The caller should ensure that DMA_READY is set
/// before calling this function.
///
/// # Safety
/// This function performs volatile MMIO writes and should only be called in a
/// bare-metal environment where the AUDIO_DMA register is properly mapped.
pub fn trigger_dma() {
    unsafe {
        write_volatile(AUDIO_DMA as *mut u32, 0);
    }
}

/// Write a stereo sample (left and right channels) to a buffer
///
/// This function writes a stereo sample in little-endian format:
/// - 2 bytes for left channel (i16)
/// - 2 bytes for right channel (i16)
///
/// # Arguments
/// * `buffer_base` - Base address of the audio buffer in memory
/// * `offset` - Byte offset from the buffer base (should be multiple of 4)
/// * `left` - Left channel sample value
/// * `right` - Right channel sample value
///
/// # Safety
/// This function performs volatile memory writes. The caller must ensure:
/// - `buffer_base` points to valid, writable memory
/// - There is at least 4 bytes of space at `buffer_base + offset`
pub fn write_stereo_sample(buffer_base: u32, offset: u32, left: i16, right: i16) {
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

/// Write a mono sample to a buffer
///
/// This function writes a mono sample in little-endian format (2 bytes).
///
/// # Arguments
/// * `buffer_base` - Base address of the audio buffer in memory
/// * `offset` - Byte offset from the buffer base (should be multiple of 2)
/// * `sample` - Sample value
///
/// # Safety
/// This function performs volatile memory writes. The caller must ensure:
/// - `buffer_base` points to valid, writable memory
/// - There is at least 2 bytes of space at `buffer_base + offset`
pub fn write_mono_sample(buffer_base: u32, offset: u32, sample: i16) {
    unsafe {
        let addr = buffer_base + offset;
        let bytes = sample.to_le_bytes();
        write_volatile(addr as *mut u8, bytes[0]);
        write_volatile((addr + 1) as *mut u8, bytes[1]);
    }
}
