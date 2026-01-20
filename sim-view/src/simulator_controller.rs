use cpu_sim::{
    Audio, AudioConfig, InteractiveSimulator, SimulationStepResult, Video, VideoConfig, AUDIO_BASE,
    VIDEO_BASE,
};
use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};

// Audio buffer size limit (0.5 seconds at 48kHz)
const MAX_AUDIO_BUFFER_SAMPLES: usize = 48000;

// Type aliases to simplify complex types
type VideoFrameQueue = Arc<Mutex<VecDeque<(Vec<u8>, VideoConfig)>>>;
type AudioSampleQueue = Arc<Mutex<VecDeque<i16>>>;
type AudioConfigStorage = Arc<Mutex<Option<AudioConfig>>>;

pub struct SimulatorController {
    /// Interactive simulator instance
    simulator: InteractiveSimulator,

    /// Video frame queue (shared with Video device callback)
    video_frames: VideoFrameQueue,

    /// Audio sample queue (shared with Audio device callback)
    audio_samples: AudioSampleQueue,

    /// Audio config (shared with Audio device callback)
    _audio_config: AudioConfigStorage,
}

impl SimulatorController {
    /// Create a new simulator controller with video and audio support
    pub fn new() -> Result<Self, String> {
        // Create the interactive simulator
        let mut simulator = InteractiveSimulator::new()?;

        // Create shared queues for video and audio data
        let video_frames = Arc::new(Mutex::new(VecDeque::new()));
        let audio_samples = Arc::new(Mutex::new(VecDeque::new()));
        let audio_config = Arc::new(Mutex::new(None));

        // Create Video device with callback
        let video_frames_clone = Arc::clone(&video_frames);
        let video_callback = move |data: &[u8], config: &VideoConfig| {
            let mut frames = video_frames_clone.lock().unwrap();
            frames.push_back((data.to_vec(), *config));

            // Keep only last 2 frames to prevent unbounded growth
            while frames.len() > 2 {
                frames.pop_front();
            }
        };
        let video_device = Box::new(Video::new(Some(video_callback)));

        // Create Audio device with callbacks
        let audio_samples_clone = Arc::clone(&audio_samples);
        let sample_callback = move |samples: &[i16]| {
            let mut buf = audio_samples_clone.lock().unwrap();
            for &sample in samples {
                buf.push_back(sample);
            }

            // Limit buffer size (0.5 seconds at 48kHz)
            while buf.len() > MAX_AUDIO_BUFFER_SAMPLES {
                buf.pop_front();
            }
        };

        let audio_config_clone = Arc::clone(&audio_config);
        let config_callback = move |config: &AudioConfig| {
            let mut cfg = audio_config_clone.lock().unwrap();
            *cfg = Some(*config);
        };
        let audio_device = Box::new(Audio::new(Some(sample_callback), Some(config_callback)));

        // Register devices with simulator at their standard base addresses
        simulator.register_device(VIDEO_BASE, video_device)?;
        simulator.register_device(AUDIO_BASE, audio_device)?;

        Ok(SimulatorController {
            simulator,
            video_frames,
            audio_samples,
            _audio_config: audio_config,
        })
    }

    /// Load an ELF file and reset the simulation
    pub fn load_elf(&mut self, path: &Path) -> Result<(), String> {
        // Clear any pending frames/samples from previous program
        self.video_frames.lock().unwrap().clear();
        self.audio_samples.lock().unwrap().clear();
        *self._audio_config.lock().unwrap() = None;

        // Load ELF into simulator (this resets the CPU)
        self.simulator.load_elf(path)?;

        Ok(())
    }

    /// Step the simulation for N instructions
    ///
    /// Returns the result of the last instruction executed, which may contain
    /// a tohost termination value if the program halted.
    pub fn step_instructions(&mut self, count: u64) -> Result<SimulationStepResult, String> {
        let mut last_result = None;

        for _ in 0..count {
            let result = self.simulator.step_instruction()?;

            // If program terminated, return early
            if result.tohost_value.is_some() {
                return Ok(result);
            }

            last_result = Some(result);
        }

        // Return last result (or error if no steps were taken)
        last_result.ok_or_else(|| "No instructions executed".to_string())
    }

    /// Get the next available video frame, if any
    pub fn get_video_frame(&self) -> Option<(Vec<u8>, VideoConfig)> {
        self.video_frames.lock().unwrap().pop_front()
    }

    /// Get available audio samples (up to max_samples)
    pub fn get_audio_samples(&self, max_samples: usize) -> Vec<i16> {
        let mut samples = self.audio_samples.lock().unwrap();
        let available = samples.len();
        let count = samples.len().min(max_samples);
        let result: Vec<i16> = samples.drain(..count).collect();

        if available > 0 {
            log::debug!(
                "Controller: get_audio_samples - available: {}, requested: {}, returning: {}",
                available,
                max_samples,
                result.len()
            );
        }

        result
    }
}
