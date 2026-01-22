//! GUI backend implementations wrapping existing VideoWindow and AudioStream.
//!
//! These backends support multi-threaded access where:
//! - Video frames and audio samples are pushed from the simulation thread
//! - Display updates happen on the main thread

use crate::audio_stream::AudioStream;
use crate::backend_traits::{
    AudioBackend, EventSource, Key, KeyModifiers, VideoBackend, ViewerEvent,
};
use crate::video_window::{Key as VwKey, VideoWindow, WindowEvent};
use cpu_sim::VideoConfig;
use std::sync::{Arc, Mutex};

/// GUI video backend using softbuffer/winit.
///
/// This backend is thread-safe: `process_frame` can be called from any thread
/// (it stores the frame in a buffer), while `update` must be called from the
/// main thread (it displays the buffered frame).
pub struct GuiVideoBackend {
    /// The underlying window (main thread only)
    window: VideoWindow,
    /// Active flag
    is_active: bool,
    /// Pending frame data (thread-safe for cross-thread writes)
    pending_frame: Arc<Mutex<Option<(Vec<u8>, VideoConfig)>>>,
}

// Safety: GuiVideoBackend is Send because:
// - window is only accessed on the main thread (via update/set_title/is_active)
// - pending_frame uses Arc<Mutex<>> for thread-safe access
// The VideoWindow itself is not Send, but we ensure it's only accessed from main thread
unsafe impl Send for GuiVideoBackend {}

impl GuiVideoBackend {
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        let window = VideoWindow::new(width as usize, height as usize)?;
        Ok(Self {
            window,
            is_active: true,
            pending_frame: Arc::new(Mutex::new(None)),
        })
    }

    /// Get a clone of the pending frame handle for the event source.
    pub fn get_pending_frame_handle(&self) -> Arc<Mutex<Option<(Vec<u8>, VideoConfig)>>> {
        Arc::clone(&self.pending_frame)
    }
}

impl VideoBackend for GuiVideoBackend {
    fn process_frame(&mut self, data: &[u8], config: &VideoConfig) -> Result<(), String> {
        // Store frame in thread-safe buffer for later display
        // This can be called from the simulation thread
        if let Ok(mut pending) = self.pending_frame.lock() {
            *pending = Some((data.to_vec(), *config));
        }
        Ok(())
    }

    fn update(&mut self) -> Result<(), String> {
        // Display any pending frame (main thread only)
        let frame_data = {
            if let Ok(mut pending) = self.pending_frame.lock() {
                pending.take()
            } else {
                None
            }
        };

        if let Some((data, config)) = frame_data {
            self.window.process_video_frame(&data, &config)?;
        }

        // Update window events and display
        self.window.update_events()?;
        self.window.update_display()
    }

    fn set_title(&mut self, title: &str) {
        self.window.set_title(title);
    }

    fn is_active(&self) -> bool {
        self.is_active
    }
}

/// GUI audio backend using cpal.
///
/// This backend is thread-safe: both `push_samples` and `set_config` can be
/// called from any thread.
pub struct GuiAudioBackend {
    stream: AudioStream,
}

// AudioStream uses Arc<Mutex<>> internally for the sample buffer,
// and set_config needs &mut self but we wrap it in Arc<Mutex<>> in viewer
unsafe impl Send for GuiAudioBackend {}

impl GuiAudioBackend {
    pub fn new() -> Result<Self, String> {
        let stream = AudioStream::new()?;
        Ok(Self { stream })
    }
}

impl AudioBackend for GuiAudioBackend {
    fn push_samples(&mut self, samples: &[i16]) {
        self.stream.push_samples(samples);
    }

    fn set_config(&mut self, config: &cpu_sim::AudioConfig) {
        if let Err(e) = self.stream.set_config(config) {
            log::error!("Failed to reconfigure audio stream: {}", e);
        }
    }
}

/// GUI event source using VideoWindow events.
///
/// This must be used on the main thread alongside the GuiVideoBackend.
pub struct GuiEventSource {
    /// Reference to window for getting events (main thread only)
    #[allow(dead_code)] // Reserved for future event handling
    window_events: Arc<Mutex<Vec<WindowEvent>>>,
    /// Active flag to set when close received
    #[allow(dead_code)] // Reserved for future event handling
    is_active: Arc<Mutex<bool>>,
}

impl GuiEventSource {
    /// Create a new GUI event source.
    pub fn new() -> Self {
        Self {
            window_events: Arc::new(Mutex::new(Vec::new())),
            is_active: Arc::new(Mutex::new(true)),
        }
    }

    /// Get a clone of the is_active flag.
    pub fn get_active_handle(&self) -> Arc<Mutex<bool>> {
        Arc::clone(&self.is_active)
    }
}

impl Default for GuiEventSource {
    fn default() -> Self {
        Self::new()
    }
}

impl EventSource for GuiEventSource {
    fn get_events(&mut self) -> Vec<ViewerEvent> {
        // Note: In the new architecture, events are collected differently
        // The GuiVideoBackend handles window events in its update() method
        Vec::new()
    }
}

/// Convert VideoWindow key to backend trait key.
fn convert_key(key: VwKey) -> Key {
    match key {
        VwKey::Escape => Key::Escape,
        VwKey::Space => Key::Space,
        VwKey::R => Key::R,
    }
}

/// Helper to convert window events to viewer events.
pub fn convert_window_events(events: Vec<WindowEvent>) -> Vec<ViewerEvent> {
    events
        .into_iter()
        .map(|e| match e {
            WindowEvent::KeyPressed(key, modifiers) => ViewerEvent::KeyPressed(
                convert_key(key),
                KeyModifiers {
                    ctrl: modifiers.ctrl,
                    shift: modifiers.shift,
                    alt: modifiers.alt,
                },
            ),
            WindowEvent::Close => ViewerEvent::Close,
        })
        .collect()
}
