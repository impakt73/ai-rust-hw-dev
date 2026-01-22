//! Headless backend implementations for automated testing.
//!
//! These backends capture all video frames and audio samples with timestamps
//! instead of rendering to hardware devices.

use crate::backend_traits::{AudioBackend, EventSource, VideoBackend, ViewerEvent};
use cpu_sim::{AudioConfig, VideoConfig};
use std::collections::VecDeque;
use std::time::Instant;

/// Captured video frame with metadata
#[derive(Debug, Clone)]
pub struct CapturedFrame {
    /// Frame data (owned copy for safety)
    pub data: Vec<u8>,

    /// Video configuration at capture time
    pub config: VideoConfig,

    /// Timestamp when frame was presented
    pub timestamp: Instant,

    /// Frame sequence number (monotonic counter)
    pub sequence: u64,
}

/// Headless video backend that captures frames
pub struct HeadlessVideoBackend {
    /// All captured frames
    captured_frames: Vec<CapturedFrame>,

    /// Current frame buffer (before presentation)
    current_frame: Option<(Vec<u8>, VideoConfig)>,

    /// Frame sequence counter
    frame_count: u64,
}

impl HeadlessVideoBackend {
    pub fn new() -> Self {
        Self {
            captured_frames: Vec::new(),
            current_frame: None,
            frame_count: 0,
        }
    }

    /// Get captured frames (for tests)
    pub fn get_frames(&self) -> &[CapturedFrame] {
        &self.captured_frames
    }
}

impl Default for HeadlessVideoBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoBackend for HeadlessVideoBackend {
    fn process_frame(&mut self, data: &[u8], config: &VideoConfig) -> Result<(), String> {
        // Store frame data (will be presented in update())
        self.current_frame = Some((data.to_vec(), *config));
        Ok(())
    }

    fn update(&mut self) -> Result<(), String> {
        // Present the current frame (capture with timestamp)
        if let Some((data, config)) = self.current_frame.take() {
            let frame = CapturedFrame {
                data,
                config,
                timestamp: Instant::now(),
                sequence: self.frame_count,
            };

            log::debug!(
                "Captured frame {} ({}x{}, {:?})",
                self.frame_count,
                config.width,
                config.height,
                config.format
            );

            self.captured_frames.push(frame);
            self.frame_count += 1;
        }

        Ok(())
    }

    fn set_title(&mut self, _title: &str) {
        // No-op in headless mode
    }

    fn is_active(&self) -> bool {
        true // Always active in headless mode
    }
}

/// Captured audio chunk with metadata
#[derive(Debug, Clone)]
pub struct CapturedAudioChunk {
    /// Audio samples (owned copy)
    pub samples: Vec<i16>,

    /// Timestamp when samples were received
    pub timestamp: Instant,

    /// Sample sequence number (cumulative sample count)
    pub sample_offset: u64,

    /// Audio configuration when samples were captured (if known)
    pub config: Option<AudioConfig>,
}

/// Headless audio backend that captures samples
pub struct HeadlessAudioBackend {
    /// All captured audio chunks
    captured_chunks: Vec<CapturedAudioChunk>,

    /// Cumulative sample counter
    sample_count: u64,

    /// Current audio configuration
    current_config: Option<AudioConfig>,
}

impl HeadlessAudioBackend {
    pub fn new() -> Self {
        Self {
            captured_chunks: Vec::new(),
            sample_count: 0,
            current_config: None,
        }
    }

    /// Get captured audio chunks (for tests)
    pub fn get_chunks(&self) -> &[CapturedAudioChunk] {
        &self.captured_chunks
    }

    /// Get the current audio configuration (for tests)
    pub fn get_current_config(&self) -> Option<AudioConfig> {
        self.current_config
    }
}

impl Default for HeadlessAudioBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioBackend for HeadlessAudioBackend {
    fn push_samples(&mut self, samples: &[i16]) {
        if samples.is_empty() {
            return;
        }

        let chunk = CapturedAudioChunk {
            samples: samples.to_vec(),
            timestamp: Instant::now(),
            sample_offset: self.sample_count,
            config: self.current_config,
        };

        self.captured_chunks.push(chunk);
        self.sample_count += samples.len() as u64;
    }

    fn set_config(&mut self, config: &AudioConfig) {
        log::debug!(
            "Audio config changed: {} Hz, {:?}, {} samples",
            config.sample_rate.to_hz(),
            config.channels,
            config.sample_count
        );
        self.current_config = Some(*config);
    }
}

/// Headless event source for programmatic control
pub struct HeadlessEventSource {
    /// Event queue
    event_queue: VecDeque<ViewerEvent>,
}

impl HeadlessEventSource {
    pub fn new() -> Self {
        Self {
            event_queue: VecDeque::new(),
        }
    }

    /// Push an event into the queue (for tests)
    pub fn push_event(&mut self, event: ViewerEvent) {
        self.event_queue.push_back(event);
    }
}

impl Default for HeadlessEventSource {
    fn default() -> Self {
        Self::new()
    }
}

impl EventSource for HeadlessEventSource {
    fn get_events(&mut self) -> Vec<ViewerEvent> {
        self.event_queue.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpu_sim::VideoFormat;

    #[test]
    fn test_headless_video_captures_frames() {
        let mut backend = HeadlessVideoBackend::new();

        let data = vec![0xFF; 320 * 240 * 4];
        let config = VideoConfig {
            width: 320,
            height: 240,
            format: VideoFormat::Rgba8,
        };

        backend.process_frame(&data, &config).unwrap();
        backend.update().unwrap();

        let captured = backend.get_frames();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].data.len(), data.len());
        assert_eq!(captured[0].sequence, 0);
    }

    #[test]
    fn test_headless_audio_captures_samples() {
        let mut backend = HeadlessAudioBackend::new();

        let samples = vec![100i16, 200, 300];
        backend.push_samples(&samples);

        let captured = backend.get_chunks();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].samples, samples);
        assert_eq!(captured[0].sample_offset, 0);
    }

    #[test]
    fn test_headless_event_injection() {
        let mut backend = HeadlessEventSource::new();

        // Inject a test event
        backend.push_event(ViewerEvent::Close);

        // Retrieve events
        let events = backend.get_events();
        assert_eq!(events.len(), 1);
        matches!(events[0], ViewerEvent::Close);
    }
}

// ============================================================================
// Thread-Safe Headless Backend Wrappers
// ============================================================================
//
// These wrappers implement the ThreadSafeVideoBackend and ThreadSafeAudioBackend
// traits for use with the background simulation thread.

use crate::sim_thread::{ThreadSafeAudioBackend, ThreadSafeVideoBackend};
use std::sync::Mutex;

/// Thread-safe wrapper around HeadlessVideoBackend.
///
/// This wrapper allows the video backend to be safely accessed from both
/// the simulation thread (process_frame) and the main thread (get_frames, update).
pub struct ThreadSafeHeadlessVideoBackend {
    inner: Mutex<HeadlessVideoBackend>,
}

impl ThreadSafeHeadlessVideoBackend {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HeadlessVideoBackend::new()),
        }
    }

    /// Get captured frames (for tests) - clones the data
    pub fn get_frames(&self) -> Vec<CapturedFrame> {
        self.inner.lock().unwrap().get_frames().to_vec()
    }
}

impl Default for ThreadSafeHeadlessVideoBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ThreadSafeVideoBackend for ThreadSafeHeadlessVideoBackend {
    fn process_frame(&self, data: &[u8], config: &VideoConfig) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();
        // Directly capture the frame (don't wait for update)
        let frame = CapturedFrame {
            data: data.to_vec(),
            config: *config,
            timestamp: Instant::now(),
            sequence: inner.frame_count,
        };

        log::debug!(
            "ThreadSafe: Captured frame {} ({}x{}, {:?})",
            inner.frame_count,
            config.width,
            config.height,
            config.format
        );

        inner.captured_frames.push(frame);
        inner.frame_count += 1;
        Ok(())
    }

    fn set_title(&self, _title: &str) {
        // No-op in headless mode
    }

    fn is_active(&self) -> bool {
        true // Always active in headless mode
    }

    fn update(&self) -> Result<(), String> {
        // No-op for thread-safe version - frames are captured immediately in process_frame
        Ok(())
    }
}

/// Thread-safe wrapper around HeadlessAudioBackend.
///
/// This wrapper allows the audio backend to be safely accessed from both
/// the simulation thread (push_samples, set_config) and the main thread (get_chunks).
pub struct ThreadSafeHeadlessAudioBackend {
    inner: Mutex<HeadlessAudioBackend>,
}

impl ThreadSafeHeadlessAudioBackend {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HeadlessAudioBackend::new()),
        }
    }

    /// Get captured audio chunks (for tests) - clones the data
    pub fn get_chunks(&self) -> Vec<CapturedAudioChunk> {
        self.inner.lock().unwrap().get_chunks().to_vec()
    }

    /// Get the current audio configuration (for tests)
    pub fn get_current_config(&self) -> Option<AudioConfig> {
        self.inner.lock().unwrap().get_current_config()
    }
}

impl Default for ThreadSafeHeadlessAudioBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ThreadSafeAudioBackend for ThreadSafeHeadlessAudioBackend {
    fn push_samples(&self, samples: &[i16]) {
        let mut inner = self.inner.lock().unwrap();
        // Delegate to the inner backend
        AudioBackend::push_samples(&mut *inner, samples);
    }

    fn set_config(&self, config: &AudioConfig) {
        let mut inner = self.inner.lock().unwrap();
        // Delegate to the inner backend
        AudioBackend::set_config(&mut *inner, config);
    }
}
