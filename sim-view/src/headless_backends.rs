//! Headless backend implementations for automated testing.
//!
//! These backends capture all video frames and audio samples with timestamps
//! instead of rendering to hardware devices. They implement the pull-based
//! data flow model by pulling from shared buffers.

use crate::backend_traits::{AudioBackend, EventSource, VideoBackend, ViewerEvent};
use crate::shared_buffers::{SharedAudioBuffer, SharedVideoBuffer};
use bus_shared::{AudioConfig, VideoConfig};
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

    /// Frame sequence counter
    frame_count: u64,

    /// Shared video buffer (set via set_video_source)
    video_source: Option<SharedVideoBuffer>,
}

impl HeadlessVideoBackend {
    pub fn new() -> Self {
        Self {
            captured_frames: Vec::new(),
            frame_count: 0,
            video_source: None,
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
    fn set_video_source(&mut self, buffer: SharedVideoBuffer) {
        self.video_source = Some(buffer);
    }

    fn update(&mut self) -> Result<(), String> {
        // Pull all available frames from shared buffer
        if let Some(ref source) = self.video_source {
            while let Some(frame) = source.pull_frame() {
                let captured = CapturedFrame {
                    data: frame.data,
                    config: frame.config,
                    timestamp: frame.timestamp,
                    sequence: self.frame_count,
                };

                log::debug!(
                    "Captured frame {} ({}x{}, {:?})",
                    self.frame_count,
                    captured.config.width,
                    captured.config.height,
                    captured.config.format
                );

                self.captured_frames.push(captured);
                self.frame_count += 1;
            }
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

    /// Shared audio buffer (set via set_audio_source)
    audio_source: Option<SharedAudioBuffer>,
}

impl HeadlessAudioBackend {
    pub fn new() -> Self {
        Self {
            captured_chunks: Vec::new(),
            sample_count: 0,
            current_config: None,
            audio_source: None,
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

    /// Update by pulling samples from shared buffer (call periodically to capture audio)
    pub fn update(&mut self) {
        // Pull all available samples from shared buffer
        if let Some(ref source) = self.audio_source {
            // Pull in chunks to preserve chunk boundaries
            const CHUNK_SIZE: usize = 1024;
            while source.has_samples() {
                let samples = source.pull_samples(CHUNK_SIZE);
                if samples.is_empty() {
                    break;
                }

                let chunk = CapturedAudioChunk {
                    samples,
                    timestamp: Instant::now(),
                    sample_offset: self.sample_count,
                    config: self.current_config,
                };

                self.sample_count += chunk.samples.len() as u64;
                self.captured_chunks.push(chunk);
            }
        }
    }
}

impl Default for HeadlessAudioBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioBackend for HeadlessAudioBackend {
    fn set_audio_source(&mut self, buffer: SharedAudioBuffer) {
        self.audio_source = Some(buffer);
    }

    fn set_config(&mut self, config: &AudioConfig) {
        log::debug!(
            "Audio config changed: {} Hz, {:?}, {} samples",
            config.sample_rate.to_hz(),
            config.channels,
            config.sample_count
        );
        self.current_config = Some(*config);

        // Update config in shared buffer if available
        if let Some(ref source) = self.audio_source {
            source.set_config(*config);
        }
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
    use crate::shared_buffers::{SharedAudioBuffer, SharedVideoBuffer};
    use bus_shared::{AudioChannels, AudioSampleRate, VideoConfig, VideoFormat};

    #[test]
    fn test_headless_video_captures_frames() {
        let mut backend = HeadlessVideoBackend::new();
        let video_buffer = SharedVideoBuffer::new();

        backend.set_video_source(video_buffer.clone());

        // Push a frame to the shared buffer (simulating callback)
        let data = vec![0xFF; 320 * 240 * 4];
        let config = VideoConfig {
            width: 320,
            height: 240,
            format: VideoFormat::Rgba8,
        };
        video_buffer.push_frame(data.clone(), config);

        // Pull and capture the frame
        backend.update().unwrap();

        let captured = backend.get_frames();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].data.len(), data.len());
        assert_eq!(captured[0].sequence, 0);
    }

    #[test]
    fn test_headless_audio_captures_samples() {
        let mut backend = HeadlessAudioBackend::new();
        let audio_buffer = SharedAudioBuffer::new();

        backend.set_audio_source(audio_buffer.clone());

        // Set config
        let config = AudioConfig {
            sample_rate: AudioSampleRate::Hz48000,
            channels: AudioChannels::Mono,
            sample_count: 1024,
        };
        backend.set_config(&config);

        // Push samples to shared buffer (simulating callback)
        let samples = vec![100i16, 200, 300];
        audio_buffer.push_samples(samples.clone());

        // Pull and capture samples
        backend.update();

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
