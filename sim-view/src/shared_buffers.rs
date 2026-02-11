//! Shared buffer structures for pull-based data flow between simulation and backends.
//!
//! These buffers allow video and audio data to flow from hardware simulation callbacks
//! into a thread-safe shared location, where backends can pull the data when needed
//! (e.g., during window presents or audio buffer fills).

use device_runtime::{AudioConfig, VideoConfig};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Maximum number of video frames to buffer (prevents unbounded memory growth)
const MAX_VIDEO_FRAMES: usize = 10;

/// Maximum number of audio sample chunks to buffer
const MAX_AUDIO_CHUNKS: usize = 100;

/// A single video frame with metadata
#[derive(Debug, Clone)]
pub struct VideoFrame {
    /// Frame pixel data (owned)
    pub data: Vec<u8>,
    /// Video configuration at capture time
    pub config: VideoConfig,
    /// Timestamp when frame was captured
    pub timestamp: Instant,
}

/// Thread-safe shared video buffer for pull-based data flow
///
/// Video callbacks push frames into this buffer, and backends pull frames
/// when they need to present/display them.
#[derive(Clone)]
pub struct SharedVideoBuffer {
    inner: Arc<Mutex<VideoBufferInner>>,
}

struct VideoBufferInner {
    /// Queue of pending video frames
    frames: VecDeque<VideoFrame>,
}

impl SharedVideoBuffer {
    /// Create a new shared video buffer
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(VideoBufferInner {
                frames: VecDeque::new(),
            })),
        }
    }

    /// Push a new video frame into the buffer (called from simulation callback)
    ///
    /// If the buffer is full, the oldest frame is dropped to prevent unbounded growth.
    pub fn push_frame(&self, data: Vec<u8>, config: VideoConfig) {
        let mut inner = self.inner.lock().unwrap();

        // Drop oldest frame if buffer is full
        if inner.frames.len() >= MAX_VIDEO_FRAMES {
            inner.frames.pop_front();
            log::warn!(
                "Video buffer full, dropping oldest frame (limit: {})",
                MAX_VIDEO_FRAMES
            );
        }

        inner.frames.push_back(VideoFrame {
            data,
            config,
            timestamp: Instant::now(),
        });
    }

    /// Pull the next available video frame from the buffer (called from backend)
    ///
    /// Returns `None` if no frames are available.
    pub fn pull_frame(&self) -> Option<VideoFrame> {
        let mut inner = self.inner.lock().unwrap();
        inner.frames.pop_front()
    }

    /// Check if frames are available without removing them
    pub fn has_frames(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        !inner.frames.is_empty()
    }

    /// Get the number of frames currently buffered
    pub fn frame_count(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.frames.len()
    }
}

impl Default for SharedVideoBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// A chunk of audio samples with metadata
#[derive(Debug, Clone)]
pub struct AudioChunk {
    /// Audio samples (i16 PCM)
    pub samples: Vec<i16>,
    /// Audio configuration when samples were captured
    pub config: Option<AudioConfig>,
    /// Timestamp when chunk was captured
    pub timestamp: Instant,
}

/// Thread-safe shared audio buffer for pull-based data flow
///
/// Audio callbacks push sample chunks into this buffer, and backends pull samples
/// when they need to fill audio device buffers.
#[derive(Clone)]
pub struct SharedAudioBuffer {
    inner: Arc<Mutex<AudioBufferInner>>,
}

struct AudioBufferInner {
    /// Queue of pending audio sample chunks
    chunks: VecDeque<AudioChunk>,
    /// Current audio configuration
    current_config: Option<AudioConfig>,
}

impl SharedAudioBuffer {
    /// Create a new shared audio buffer
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(AudioBufferInner {
                chunks: VecDeque::new(),
                current_config: None,
            })),
        }
    }

    /// Push new audio samples into the buffer (called from simulation callback)
    ///
    /// If the buffer is full, the oldest chunk is dropped to prevent unbounded growth.
    pub fn push_samples(&self, samples: Vec<i16>) {
        if samples.is_empty() {
            return;
        }

        let mut inner = self.inner.lock().unwrap();

        // Drop oldest chunk if buffer is full
        if inner.chunks.len() >= MAX_AUDIO_CHUNKS {
            inner.chunks.pop_front();
            log::warn!(
                "Audio buffer full, dropping oldest chunk (limit: {})",
                MAX_AUDIO_CHUNKS
            );
        }

        let config = inner.current_config;
        inner.chunks.push_back(AudioChunk {
            samples,
            config,
            timestamp: Instant::now(),
        });
    }

    /// Update the audio configuration (called when config changes)
    pub fn set_config(&self, config: AudioConfig) {
        let mut inner = self.inner.lock().unwrap();
        inner.current_config = Some(config);
    }

    /// Pull audio samples from the buffer (called from backend)
    ///
    /// Returns up to `max_samples` samples. If fewer samples are available,
    /// returns what is available. Returns an empty Vec if no samples are buffered.
    pub fn pull_samples(&self, max_samples: usize) -> Vec<i16> {
        let mut inner = self.inner.lock().unwrap();
        let mut result = Vec::new();

        while result.len() < max_samples && !inner.chunks.is_empty() {
            if let Some(mut chunk) = inner.chunks.pop_front() {
                let remaining_space = max_samples - result.len();

                if chunk.samples.len() <= remaining_space {
                    // Take entire chunk
                    result.extend_from_slice(&chunk.samples);
                } else {
                    // Take partial chunk and put remainder back
                    result.extend_from_slice(&chunk.samples[..remaining_space]);
                    chunk.samples.drain(..remaining_space);
                    inner.chunks.push_front(chunk);
                    break;
                }
            }
        }

        result
    }

    /// Get the current audio configuration
    pub fn get_config(&self) -> Option<AudioConfig> {
        let inner = self.inner.lock().unwrap();
        inner.current_config
    }

    /// Check if samples are available without removing them
    pub fn has_samples(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        !inner.chunks.is_empty()
    }

    /// Get the total number of samples currently buffered
    pub fn sample_count(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.chunks.iter().map(|c| c.samples.len()).sum()
    }
}

impl Default for SharedAudioBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use device_runtime::{AudioChannels, AudioSampleRate, VideoFormat};

    #[test]
    fn test_video_buffer_push_pull() {
        let buffer = SharedVideoBuffer::new();

        // Push a frame
        let data = vec![0xFF; 320 * 240 * 4];
        let config = VideoConfig {
            width: 320,
            height: 240,
            format: VideoFormat::Rgba8,
        };
        buffer.push_frame(data.clone(), config);

        // Pull the frame
        let frame = buffer.pull_frame().expect("Frame should be available");
        assert_eq!(frame.data, data);
        assert_eq!(frame.config.width, 320);

        // Buffer should be empty
        assert!(!buffer.has_frames());
    }

    #[test]
    fn test_video_buffer_overflow() {
        let buffer = SharedVideoBuffer::new();

        // Fill buffer beyond max
        for i in 0..MAX_VIDEO_FRAMES + 5 {
            let data = vec![i as u8; 100];
            let config = VideoConfig {
                width: 10,
                height: 10,
                format: VideoFormat::Rgba8,
            };
            buffer.push_frame(data, config);
        }

        // Should have at most MAX_VIDEO_FRAMES
        assert_eq!(buffer.frame_count(), MAX_VIDEO_FRAMES);

        // Oldest frames should have been dropped (first 5 frames)
        let first_frame = buffer.pull_frame().unwrap();
        assert_eq!(first_frame.data[0], 5); // Frame 5 (0-4 were dropped)
    }

    #[test]
    fn test_audio_buffer_push_pull() {
        let buffer = SharedAudioBuffer::new();

        // Set config
        let config = AudioConfig {
            sample_rate: AudioSampleRate::Hz48000,
            channels: AudioChannels::Stereo,
            sample_count: 1024,
        };
        buffer.set_config(config);

        // Push samples
        let samples = vec![100i16, 200, 300, 400];
        buffer.push_samples(samples.clone());

        // Pull samples
        let pulled = buffer.pull_samples(10);
        assert_eq!(pulled, samples);

        // Buffer should be empty
        assert!(!buffer.has_samples());
    }

    #[test]
    fn test_audio_buffer_partial_pull() {
        let buffer = SharedAudioBuffer::new();

        // Push samples
        buffer.push_samples(vec![1, 2, 3, 4, 5, 6, 7, 8]);

        // Pull partial
        let pulled1 = buffer.pull_samples(3);
        assert_eq!(pulled1, vec![1, 2, 3]);

        // Pull remainder
        let pulled2 = buffer.pull_samples(10);
        assert_eq!(pulled2, vec![4, 5, 6, 7, 8]);

        // Buffer should be empty
        assert_eq!(buffer.sample_count(), 0);
    }

    #[test]
    fn test_audio_buffer_overflow() {
        let buffer = SharedAudioBuffer::new();

        // Fill buffer beyond max
        for i in 0..MAX_AUDIO_CHUNKS + 5 {
            buffer.push_samples(vec![i as i16; 10]);
        }

        // Should have dropped oldest chunks
        assert!(buffer.sample_count() <= MAX_AUDIO_CHUNKS * 10);
    }
}
