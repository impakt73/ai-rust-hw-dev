//! Trait abstractions for video, audio, and event handling backends.
//!
//! These traits allow `SimViewer` to work with different backend implementations:
//! - GUI backends (using minifb and cpal) for interactive viewing
//! - Headless backends (capturing data) for automated testing

use cpu_sim::VideoConfig;
use std::path::PathBuf;

/// Trait for video output backends (GUI or headless)
pub trait VideoBackend {
    /// Process a video frame from the simulator
    fn process_frame(&mut self, data: &[u8], config: &VideoConfig) -> Result<(), String>;

    /// Update display/capture (called once per frame in main loop)
    fn update(&mut self) -> Result<(), String>;

    /// Set window title (no-op for headless)
    fn set_title(&mut self, title: &str);

    /// Check if backend is still active (window not closed)
    fn is_active(&self) -> bool;
}

/// Trait for audio output backends (GUI or headless)
pub trait AudioBackend {
    /// Push audio samples for playback/capture
    fn push_samples(&mut self, samples: &[i16]);
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
