//! GUI backend implementations wrapping existing VideoWindow and AudioStream.

use crate::audio_stream::AudioStream;
use crate::backend_traits::{
    AudioBackend, EventSource, Key, KeyModifiers, VideoBackend, ViewerEvent,
};
use crate::sim_thread::{ThreadSafeAudioBackend, ThreadSafeVideoBackend};
use crate::video_window::{Key as VwKey, VideoWindow, WindowEvent};
use cpu_sim::{AudioConfig, VideoConfig};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

/// GUI video backend using softbuffer/winit
pub struct GuiVideoBackend {
    window: Rc<RefCell<VideoWindow>>,
    is_active: Rc<RefCell<bool>>,
}

impl GuiVideoBackend {
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        let window = VideoWindow::new(width as usize, height as usize)?;
        Ok(Self {
            window: Rc::new(RefCell::new(window)),
            is_active: Rc::new(RefCell::new(true)),
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
    fn process_frame(&mut self, data: &[u8], config: &VideoConfig) -> Result<(), String> {
        self.window.borrow_mut().process_video_frame(data, config)
    }

    fn update(&mut self) -> Result<(), String> {
        let mut window = self.window.borrow_mut();
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
}

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

// ============================================================================
// Thread-Safe GUI Backend Wrappers
// ============================================================================
//
// These wrappers allow GUI backends to be used with the background simulation
// thread. They use internal buffers to decouple the simulation thread from
// the main UI thread.
//
// NOTE: The thread-safe video backend stores frame data but does NOT contain
// the actual window. The window must be created and updated separately on
// the main thread using ThreadSafeGuiVideoPresenter.

/// Pending video frame data to be presented on the main thread.
#[derive(Clone)]
pub struct PendingFrame {
    /// Frame pixel data
    pub data: Vec<u8>,
    /// Frame configuration
    pub config: VideoConfig,
}

/// Thread-safe video frame buffer.
///
/// This struct is used by the simulation thread to store frame data,
/// which is then read by the main thread for presentation.
///
/// This implements ThreadSafeVideoBackend and can be used with SimThread.
pub struct ThreadSafeGuiVideoBuffer {
    /// Pending frame to be presented (written by sim thread, read by main thread)
    pending_frame: Arc<Mutex<Option<PendingFrame>>>,
    /// Active flag (set to false when window is closed)
    is_active: Arc<std::sync::atomic::AtomicBool>,
    /// Pending title to set
    pending_title: Arc<Mutex<Option<String>>>,
}

impl ThreadSafeGuiVideoBuffer {
    /// Create a new thread-safe video buffer.
    pub fn new() -> Self {
        Self {
            pending_frame: Arc::new(Mutex::new(None)),
            is_active: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            pending_title: Arc::new(Mutex::new(None)),
        }
    }

    /// Take the pending frame (for the main thread to present).
    pub fn take_pending_frame(&self) -> Option<PendingFrame> {
        self.pending_frame.lock().unwrap().take()
    }

    /// Take the pending title (for the main thread to set).
    pub fn take_pending_title(&self) -> Option<String> {
        self.pending_title.lock().unwrap().take()
    }

    /// Mark the backend as inactive (window closed).
    pub fn set_inactive(&self) {
        self.is_active
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Default for ThreadSafeGuiVideoBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl ThreadSafeVideoBackend for ThreadSafeGuiVideoBuffer {
    fn process_frame(&self, data: &[u8], config: &VideoConfig) -> Result<(), String> {
        // Store the frame data for later presentation
        let mut pending = self.pending_frame.lock().unwrap();
        *pending = Some(PendingFrame {
            data: data.to_vec(),
            config: *config,
        });
        Ok(())
    }

    fn set_title(&self, title: &str) {
        let mut pending = self.pending_title.lock().unwrap();
        *pending = Some(title.to_string());
    }

    fn is_active(&self) -> bool {
        self.is_active.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn update(&self) -> Result<(), String> {
        // No-op - actual presentation happens in the main thread via the presenter
        Ok(())
    }
}

/// Video presenter that runs on the main thread.
///
/// This struct holds the actual VideoWindow and reads frame data from
/// a ThreadSafeGuiVideoBuffer to present to the screen.
pub struct GuiVideoPresenter {
    /// The actual window (main thread only)
    window: VideoWindow,
    /// Buffer to read frames from (shared with simulation thread)
    buffer: Arc<ThreadSafeGuiVideoBuffer>,
}

impl GuiVideoPresenter {
    /// Create a new video presenter with window.
    pub fn new(width: u32, height: u32, buffer: Arc<ThreadSafeGuiVideoBuffer>) -> Result<Self, String> {
        let window = VideoWindow::new(width as usize, height as usize)?;
        Ok(Self { window, buffer })
    }

    /// Update the window (must be called from main thread).
    ///
    /// This reads any pending frame from the buffer and presents it.
    pub fn update(&mut self) -> Result<(), String> {
        // Apply pending title
        if let Some(title) = self.buffer.take_pending_title() {
            self.window.set_title(&title);
        }

        // Process any pending frame
        if let Some(frame) = self.buffer.take_pending_frame() {
            self.window.process_video_frame(&frame.data, &frame.config)?;
        }

        // Update window events and display
        self.window.update_events()?;
        self.window.update_display()?;

        // Check for close events
        let events = self.window.get_events();
        for event in events {
            if matches!(event, WindowEvent::Close) {
                self.buffer.set_inactive();
            }
        }

        Ok(())
    }

    /// Get window events for input handling.
    pub fn get_events(&mut self) -> Vec<WindowEvent> {
        self.window.get_events()
    }

    /// Check if window is still active.
    pub fn is_active(&self) -> bool {
        self.buffer.is_active()
    }
}

/// Thread-safe audio sample buffer.
///
/// This struct is used by the simulation thread to store audio samples,
/// which are then read by the main thread and pushed to the audio stream.
///
/// This implements ThreadSafeAudioBackend and can be used with SimThread.
pub struct ThreadSafeGuiAudioBuffer {
    /// Pending samples (written by sim thread, read by main thread)
    pending_samples: Arc<Mutex<Vec<i16>>>,
    /// Pending config change
    pending_config: Arc<Mutex<Option<AudioConfig>>>,
}

impl ThreadSafeGuiAudioBuffer {
    /// Create a new thread-safe audio buffer.
    pub fn new() -> Self {
        Self {
            pending_samples: Arc::new(Mutex::new(Vec::new())),
            pending_config: Arc::new(Mutex::new(None)),
        }
    }

    /// Take pending samples (for the main thread to push to audio stream).
    pub fn take_pending_samples(&self) -> Vec<i16> {
        std::mem::take(&mut *self.pending_samples.lock().unwrap())
    }

    /// Take pending config (for the main thread to apply to audio stream).
    pub fn take_pending_config(&self) -> Option<AudioConfig> {
        self.pending_config.lock().unwrap().take()
    }
}

impl Default for ThreadSafeGuiAudioBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl ThreadSafeAudioBackend for ThreadSafeGuiAudioBuffer {
    fn push_samples(&self, samples: &[i16]) {
        let mut pending = self.pending_samples.lock().unwrap();
        pending.extend_from_slice(samples);
        // Limit buffer size to prevent unbounded growth
        const MAX_PENDING: usize = 48000 * 2; // ~2 seconds at 48kHz mono
        if pending.len() > MAX_PENDING {
            let excess = pending.len() - MAX_PENDING;
            pending.drain(0..excess);
        }
    }

    fn set_config(&self, config: &AudioConfig) {
        let mut pending = self.pending_config.lock().unwrap();
        *pending = Some(*config);
    }
}

/// Audio presenter that runs on the main thread.
///
/// This struct holds the actual AudioStream and reads samples/config from
/// a ThreadSafeGuiAudioBuffer to play through the audio device.
pub struct GuiAudioPresenter {
    /// The actual audio stream (main thread only)
    stream: AudioStream,
    /// Buffer to read samples from (shared with simulation thread)
    buffer: Arc<ThreadSafeGuiAudioBuffer>,
}

impl GuiAudioPresenter {
    /// Create a new audio presenter.
    pub fn new(buffer: Arc<ThreadSafeGuiAudioBuffer>) -> Result<Self, String> {
        let stream = AudioStream::new()?;
        Ok(Self { stream, buffer })
    }

    /// Update the audio stream (can be called frequently).
    ///
    /// This reads any pending config and samples from the buffer.
    pub fn update(&mut self) {
        // Apply pending config
        if let Some(config) = self.buffer.take_pending_config() {
            if let Err(e) = self.stream.set_config(&config) {
                log::error!("Failed to set audio config: {}", e);
            }
        }

        // Push pending samples
        let samples = self.buffer.take_pending_samples();
        if !samples.is_empty() {
            self.stream.push_samples(&samples);
        }
    }
}
