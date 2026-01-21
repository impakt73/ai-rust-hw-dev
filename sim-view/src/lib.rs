//! sim-view library - RISC-V CPU Simulator with Video and Audio Output
//!
//! This library provides both GUI and headless modes for running RISC-V
//! programs with real-time video and audio output.

mod audio_stream;
mod video_window;

pub mod backend_traits;
pub mod gui_backends;
pub mod headless_backends;
pub mod viewer;
