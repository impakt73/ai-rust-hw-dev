//! Shared constants, types, and utilities for RISC-V hardware simulation
//!
//! This crate provides common definitions used across cpu-sim and rust test programs,
//! including:
//! - Bus device base addresses (DRAM, FIFO, Audio, Video, SimControl)
//! - Memory-mapped register offsets and bit definitions
//! - Configuration types for Audio and Video devices
//! - Helper functions for test pattern generation
//! - FIFO helper utilities for host-CPU communication
//!
//! This crate is `no_std` compatible and requires `alloc`.

#![no_std]

// Re-export alloc for macro use
pub extern crate alloc;

// Hardware peripheral modules
pub mod audio;
pub mod audio_helpers;
pub mod bus;
pub mod dma;
pub mod fifo;
pub mod sim_control;
pub mod video;
pub mod video_helpers;

// Re-exports for convenience
pub use audio::{
    generate_sine_sample, AudioChannels, AudioConfig, AudioSampleRate, AUDIO_ADDR, AUDIO_CONFIG,
    AUDIO_DMA, AUDIO_STATUS,
};
pub use audio_helpers::{
    is_dma_ready, is_sample_buffer_ready, trigger_dma, write_mono_sample, write_stereo_sample,
    DMA_READY, SAMPLE_BUFFER_READY,
};
pub use bus::{
    is_valid_dram_range, AUDIO_BASE, DRAM_BASE, DRAM_END, FIFO_BASE, RUST_PERIPH_BASE,
    RUST_PERIPH_LIMIT, SIM_CONTROL_BASE, VIDEO_BASE,
};
pub use dma::{
    is_dma_busy, start_transfer, wait_for_completion, DMA_BASE, DMA_DISPATCH, DMA_DST_ADDR,
    DMA_SIZE, DMA_SRC_ADDR, DMA_STATUS, DMA_STATUS_BUSY,
};
pub use fifo::{FIFO_DATA, FIFO_STATUS, RX_VALID, TX_READY};
pub use sim_control::{FAILURE_CODE, PANIC_CODE, SUCCESS_CODE, TOHOST_ADDR};
pub use video::{VideoConfig, VideoFormat, VIDEO_ADDR, VIDEO_CONFIG, VIDEO_PRESENT, VIDEO_STATUS};
pub use video_helpers::{
    trigger_present, wait_for_frame_ready, wait_for_present_ready, write_pixel, write_pixel_r8,
    write_pixel_rgb565, write_pixel_rgb8, write_pixel_rgba8, FRAME_READY, PRESENT_READY,
};
