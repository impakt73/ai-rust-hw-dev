//! Trait abstractions for video, audio, and event handling backends.
//!
//! These traits allow `SimViewer` to work with different backend implementations:
//! - GUI backends (using softbuffer/winit and cpal) for interactive viewing
//! - Headless backends (capturing data) for automated testing
//!
//! The traits support a **pull-based data flow model** where backends pull data
//! from shared buffers when needed (e.g., during window presents or audio fills).

use crate::shared_buffers::{SharedAudioBuffer, SharedVideoBuffer};
use cpu_sim::AudioConfig;
use std::path::PathBuf;

/// Trait for video output backends (GUI or headless)
///
/// Backends pull video frames from a shared buffer when they need to present/display.
pub trait VideoBackend {
    /// Set the shared video buffer for pull-based data flow
    ///
    /// Called once during initialization to connect the backend to the data source.
    fn set_video_source(&mut self, buffer: SharedVideoBuffer);

    /// Update display/capture (called once per frame in main loop)
    ///
    /// Implementations should pull available frames from the shared buffer
    /// and present/display them as appropriate.
    fn update(&mut self) -> Result<(), String>;

    /// Set window title (no-op for headless)
    fn set_title(&mut self, title: &str);

    /// Check if backend is still active (window not closed)
    fn is_active(&self) -> bool;
}

/// Trait for audio output backends (GUI or headless)
///
/// Backends pull audio samples from a shared buffer when they need to fill device buffers.
pub trait AudioBackend {
    /// Set the shared audio buffer for pull-based data flow
    ///
    /// Called once during initialization to connect the backend to the data source.
    fn set_audio_source(&mut self, buffer: SharedAudioBuffer);

    /// Update audio configuration (sample rate, channels, buffer size)
    ///
    /// Called when the CPU writes to the AUDIO_CONFIG register.
    /// The backend should reconfigure itself to match the new parameters.
    fn set_config(&mut self, config: &AudioConfig);
}

/// Trait for input event sources (keyboard, headless test driver)
pub trait EventSource {
    /// Get pending events (keyboard, close, test commands)
    fn get_events(&mut self) -> Vec<ViewerEvent>;
}

/// Unified event type
#[derive(Debug, Clone)]
pub enum ViewerEvent {
    KeyPressed(Key, KeyModifiers),
    Close,
    #[allow(dead_code)] // Used by integration tests
    TestCommand(TestCommand), // For programmatic control in tests
}

/// Key codes (simplified)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Escape,
    Space,
    R,
}

/// Keyboard modifiers
#[derive(Debug, Clone, Copy, Default)]
pub struct KeyModifiers {
    pub ctrl: bool,
    #[allow(dead_code)] // May be used in future
    pub shift: bool,
    #[allow(dead_code)] // May be used in future
    pub alt: bool,
}

/// Test commands for programmatic control in headless mode
#[derive(Debug, Clone)]
#[allow(dead_code)] // Used by integration tests
pub enum TestCommand {
    LoadELF(PathBuf),
    Pause,
    Resume,
    StepFrames(u64),
    Terminate,
}
