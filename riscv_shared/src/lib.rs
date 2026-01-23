//! Shared constants, types, and utilities for RISC-V hardware simulation
//!
//! This crate provides common definitions used across cpu-sim and rust test programs,
//! including:
//! - Bus device base addresses (DRAM, FIFO, Audio, Video, SimControl)
//! - Memory-mapped register offsets and bit definitions
//! - Configuration types for Audio and Video devices
//! - Helper functions for test pattern generation

#![cfg_attr(not(test), no_std)]

// Modules
pub mod audio;
pub mod bus;
pub mod fifo;
pub mod sim_control;
pub mod video;

// Re-exports for convenience
pub use audio::{
    generate_sine_sample, AudioChannels, AudioConfig, AudioSampleRate, AUDIO_ADDR, AUDIO_CONFIG,
    AUDIO_DMA, AUDIO_STATUS,
};
pub use bus::{
    is_valid_dram_range, AUDIO_BASE, DRAM_BASE, DRAM_END, FIFO_BASE, SIM_CONTROL_BASE, VIDEO_BASE,
};
pub use fifo::{FIFO_DATA, FIFO_STATUS, RX_VALID, TX_READY};
pub use sim_control::{FAILURE_CODE, PANIC_CODE, SUCCESS_CODE, TOHOST_ADDR};
pub use video::{VideoConfig, VideoFormat, VIDEO_ADDR, VIDEO_CONFIG, VIDEO_PRESENT, VIDEO_STATUS};
