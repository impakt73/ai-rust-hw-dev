//! GUI backend implementations wrapping existing VideoWindow and AudioStream.
//!
//! These backends implement the pull-based data flow model by pulling frames
//! and samples from shared buffers when needed.

use crate::audio_stream::AudioStream;
use crate::backend_traits::{
    AudioBackend, EventSource, Key, KeyModifiers, VideoBackend, ViewerEvent,
};
use crate::shared_buffers::{SharedAudioBuffer, SharedVideoBuffer};
use crate::video_window::{Key as VwKey, VideoWindow, WindowEvent};
use std::cell::RefCell;
use std::rc::Rc;

/// GUI video backend using softbuffer/winit
pub struct GuiVideoBackend {
    window: Rc<RefCell<VideoWindow>>,
    is_active: Rc<RefCell<bool>>,
    video_source: Option<SharedVideoBuffer>,
}

impl GuiVideoBackend {
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        let window = VideoWindow::new(width as usize, height as usize)?;
        Ok(Self {
            window: Rc::new(RefCell::new(window)),
            is_active: Rc::new(RefCell::new(true)),
            video_source: None,
        })
    }

    /// Get a handle to the underlying window (for event source)
    pub fn get_window_handle(&self) -> Rc<RefCell<VideoWindow>> {
        Rc::clone(&self.window)
    }

    /// Get a handle to the active flag (for event source)
    pub fn get_active_handle(&self) -> Rc<RefCell<bool>> {
        Rc::clone(&self.is_active)
    }
}

impl VideoBackend for GuiVideoBackend {
    fn set_video_source(&mut self, buffer: SharedVideoBuffer) {
        self.video_source = Some(buffer);
    }

    fn update(&mut self) -> Result<(), String> {
        let mut window = self.window.borrow_mut();

        // Pull all available frames from shared buffer and process them
        if let Some(ref source) = self.video_source {
            while let Some(frame) = source.pull_frame() {
                window.process_video_frame(&frame.data, &frame.config)?;
            }
        }

        // Update window events and display
        window.update_events()?;
        window.update_display()
    }

    fn set_title(&mut self, title: &str) {
        self.window.borrow_mut().set_title(title);
    }

    fn is_active(&self) -> bool {
        *self.is_active.borrow()
    }
}

/// GUI audio backend using cpal
pub struct GuiAudioBackend {
    stream: AudioStream,
    audio_source: Option<SharedAudioBuffer>,
}

impl GuiAudioBackend {
    pub fn new() -> Result<Self, String> {
        let stream = AudioStream::new()?;
        Ok(Self {
            stream,
            audio_source: None,
        })
    }
}

impl AudioBackend for GuiAudioBackend {
    fn set_audio_source(&mut self, buffer: SharedAudioBuffer) {
        // Connect the audio stream to pull from the shared buffer
        self.stream.set_audio_source(buffer.clone());
        self.audio_source = Some(buffer);
    }

    fn set_config(&mut self, config: &device_runtime::AudioConfig) {
        if let Err(e) = self.stream.set_config(config) {
            log::error!("Failed to reconfigure audio stream: {}", e);
        }

        // Update config in shared buffer if available
        if let Some(ref source) = self.audio_source {
            source.set_config(*config);
        }
    }
}

/// GUI event source using VideoWindow events
pub struct GuiEventSource {
    window: Rc<RefCell<VideoWindow>>,
    is_active: Rc<RefCell<bool>>,
}

impl GuiEventSource {
    /// Create a new GUI event source from a video backend
    pub fn new(window: Rc<RefCell<VideoWindow>>, is_active: Rc<RefCell<bool>>) -> Self {
        Self { window, is_active }
    }
}

impl EventSource for GuiEventSource {
    fn get_events(&mut self) -> Vec<ViewerEvent> {
        let mut window = self.window.borrow_mut();
        let events = window.get_events();

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
                WindowEvent::Close => {
                    // Mark window as inactive when close event received
                    *self.is_active.borrow_mut() = false;
                    ViewerEvent::Close
                }
            })
            .collect()
    }
}

/// Convert VideoWindow key to backend trait key
fn convert_key(key: VwKey) -> Key {
    match key {
        VwKey::Escape => Key::Escape,
        VwKey::Space => Key::Space,
        VwKey::R => Key::R,
    }
}
