//! Headless backend implementations for automated testing.
//!
//! These backends capture all video frames and audio samples with timestamps
//! instead of rendering to hardware devices.

use crate::backend_traits::{AudioBackend, EventSource, VideoBackend, ViewerEvent};
use cpu_sim::VideoConfig;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Captured video frame with metadata
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields accessed by tests
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
    captured_frames: Arc<Mutex<Vec<CapturedFrame>>>,

    /// Current frame buffer (before presentation)
    current_frame: Option<(Vec<u8>, VideoConfig)>,

    /// Frame sequence counter
    frame_count: u64,
}

impl HeadlessVideoBackend {
    pub fn new() -> Self {
        Self {
            captured_frames: Arc::new(Mutex::new(Vec::new())),
            current_frame: None,
            frame_count: 0,
        }
    }

    /// Get handle to captured frames (for tests)
    pub fn get_frames_handle(&self) -> Arc<Mutex<Vec<CapturedFrame>>> {
        Arc::clone(&self.captured_frames)
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

            self.captured_frames.lock().unwrap().push(frame);
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
#[allow(dead_code)] // Fields accessed by tests
pub struct CapturedAudioChunk {
    /// Audio samples (owned copy)
    pub samples: Vec<i16>,

    /// Timestamp when samples were received
    pub timestamp: Instant,

    /// Sample sequence number (cumulative sample count)
    pub sample_offset: u64,
}

/// Headless audio backend that captures samples
pub struct HeadlessAudioBackend {
    /// All captured audio chunks
    captured_chunks: Arc<Mutex<Vec<CapturedAudioChunk>>>,

    /// Cumulative sample counter
    sample_count: u64,
}

impl HeadlessAudioBackend {
    pub fn new() -> Self {
        Self {
            captured_chunks: Arc::new(Mutex::new(Vec::new())),
            sample_count: 0,
        }
    }

    /// Get handle to captured audio chunks (for tests)
    pub fn get_chunks_handle(&self) -> Arc<Mutex<Vec<CapturedAudioChunk>>> {
        Arc::clone(&self.captured_chunks)
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
        };

        self.captured_chunks.lock().unwrap().push(chunk);
        self.sample_count += samples.len() as u64;
    }
}

/// Headless event source for programmatic control
pub struct HeadlessEventSource {
    /// Event queue (shared with test driver)
    event_queue: Arc<Mutex<VecDeque<ViewerEvent>>>,
}

impl HeadlessEventSource {
    pub fn new() -> Self {
        Self {
            event_queue: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Get handle for injecting events (for tests)
    #[allow(dead_code)] // Used by integration tests
    pub fn get_event_handle(&self) -> Arc<Mutex<VecDeque<ViewerEvent>>> {
        Arc::clone(&self.event_queue)
    }
}

impl Default for HeadlessEventSource {
    fn default() -> Self {
        Self::new()
    }
}

impl EventSource for HeadlessEventSource {
    fn get_events(&mut self) -> Vec<ViewerEvent> {
        self.event_queue.lock().unwrap().drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpu_sim::VideoFormat;

    #[test]
    fn test_headless_video_captures_frames() {
        let mut backend = HeadlessVideoBackend::new();
        let frames = backend.get_frames_handle();

        let data = vec![0xFF; 320 * 240 * 4];
        let config = VideoConfig {
            width: 320,
            height: 240,
            format: VideoFormat::Rgba8,
        };

        backend.process_frame(&data, &config).unwrap();
        backend.update().unwrap();

        let captured = frames.lock().unwrap();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].data.len(), data.len());
        assert_eq!(captured[0].sequence, 0);
    }

    #[test]
    fn test_headless_audio_captures_samples() {
        let mut backend = HeadlessAudioBackend::new();
        let chunks = backend.get_chunks_handle();

        let samples = vec![100i16, 200, 300];
        backend.push_samples(&samples);

        let captured = chunks.lock().unwrap();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].samples, samples);
        assert_eq!(captured[0].sample_offset, 0);
    }

    #[test]
    fn test_headless_event_injection() {
        let mut backend = HeadlessEventSource::new();
        let event_queue = backend.get_event_handle();

        // Inject a test event
        event_queue.lock().unwrap().push_back(ViewerEvent::Close);

        // Retrieve events
        let events = backend.get_events();
        assert_eq!(events.len(), 1);
        matches!(events[0], ViewerEvent::Close);
    }
}
