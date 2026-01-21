//! sim-view library - RISC-V CPU Simulator with Video and Audio Output
//!
//! This library provides both GUI and headless modes for running RISC-V
//! programs with real-time video and audio output.

mod audio_stream;
mod simulation_thread;
mod simulator_controller;
mod threaded_viewer;
mod video_window;

pub mod backend_traits;
pub mod gui_backends;
pub mod headless_backends;
pub mod viewer;

// Re-export simulation thread types for GUI mode
pub use simulation_thread::{
    SharedSimState, SimCommand, SimNotification, SimState, SimulationThread,
};

// Re-export threaded viewer for GUI mode
pub use threaded_viewer::{ThreadedSimViewer, ThreadedViewerConfig};
