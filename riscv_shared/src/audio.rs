//! Audio device register offsets, configuration types, and helpers

use crate::bus::AUDIO_BASE;

/// Audio buffer address register offset (0x00)
pub const AUDIO_ADDR: u32 = AUDIO_BASE;

/// Audio configuration register offset (0x04)
pub const AUDIO_CONFIG: u32 = AUDIO_BASE + 0x04;

/// Audio read pointer register offset (0x08)
pub const AUDIO_READ_PTR: u32 = AUDIO_BASE + 0x08;

/// Audio write pointer register offset (0x0C)
pub const AUDIO_WRITE_PTR: u32 = AUDIO_BASE + 0x0C;

/// Audio sample rate enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioSampleRate {
    /// 48000 Hz sample rate
    Hz48000,
    /// 44100 Hz sample rate
    Hz44100,
    /// 22050 Hz sample rate
    Hz22050,
}

impl AudioSampleRate {
    /// Get the numeric sample rate value
    pub fn to_hz(&self) -> u32 {
        match self {
            AudioSampleRate::Hz48000 => 48000,
            AudioSampleRate::Hz44100 => 44100,
            AudioSampleRate::Hz22050 => 22050,
        }
    }

    /// Parse sample rate from 2-bit field
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(AudioSampleRate::Hz48000),
            1 => Some(AudioSampleRate::Hz44100),
            2 => Some(AudioSampleRate::Hz22050),
            _ => None,
        }
    }

    /// Convert sample rate to 2-bit field
    pub fn to_u8(self) -> u8 {
        match self {
            AudioSampleRate::Hz48000 => 0,
            AudioSampleRate::Hz44100 => 1,
            AudioSampleRate::Hz22050 => 2,
        }
    }
}

/// Audio channel configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioChannels {
    /// Mono audio (1 channel, 1 × i16 per sample)
    Mono,
    /// Stereo audio (2 channels, 2 × i16 per sample)
    Stereo,
}

impl AudioChannels {
    /// Get the number of channels
    pub fn count(&self) -> usize {
        match self {
            AudioChannels::Mono => 1,
            AudioChannels::Stereo => 2,
        }
    }

    /// Parse channel configuration from 1-bit field
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => AudioChannels::Mono,
            _ => AudioChannels::Stereo,
        }
    }

    /// Convert channel configuration to 1-bit field
    pub fn to_u8(self) -> u8 {
        match self {
            AudioChannels::Mono => 0,
            AudioChannels::Stereo => 1,
        }
    }
}

/// Configuration parsed from AUDIO_CONFIG register
#[derive(Debug, Clone, Copy)]
pub struct AudioConfig {
    /// Sample rate
    pub sample_rate: AudioSampleRate,
    /// Number of audio channels
    pub channels: AudioChannels,
    /// Number of samples in the ring buffer (must be power of 2)
    pub sample_count: u32,
}

impl AudioConfig {
    /// Parse configuration from register value
    /// Bits [1:0]   = sample_rate (2 bits)
    /// Bit 2        = channels (1 bit: 0=mono, 1=stereo)
    /// Bits [7:3]   = log2(sample_count) (5 bits, allowing 1-32 bit values = 2^0 to 2^31 samples)
    pub fn from_register(value: u32) -> Option<Self> {
        let sample_rate_field = (value & 0x3) as u8;
        let channels_field = ((value >> 2) & 0x1) as u8;
        let log2_sample_count = ((value >> 3) & 0x1F) as u8;

        let sample_rate = AudioSampleRate::from_u8(sample_rate_field)?;
        let channels = AudioChannels::from_u8(channels_field);

        // Compute sample_count from log2 value
        // log2_sample_count=0 means 2^0=1, log2_sample_count=10 means 2^10=1024
        let sample_count = 1u32 << log2_sample_count;

        Some(AudioConfig {
            sample_rate,
            channels,
            sample_count,
        })
    }

    /// Convert configuration to register value
    pub fn to_register(self) -> u32 {
        let sample_rate_field = self.sample_rate.to_u8() as u32;
        let channels_field = self.channels.to_u8() as u32;

        // Compute log2 of sample_count
        let log2_sample_count = 31 - self.sample_count.leading_zeros();

        sample_rate_field | (channels_field << 2) | (log2_sample_count << 3)
    }

    /// Calculate total number of bytes in the ring buffer
    /// Each sample is 2 bytes (i16) per channel
    pub fn buffer_bytes(&self) -> u32 {
        self.sample_count * 2 * self.channels.count() as u32
    }

    /// Calculate bytes per sample
    pub fn bytes_per_sample(&self) -> u32 {
        2 * self.channels.count() as u32
    }
}

/// Generate a sine wave sample at a given index
/// Uses a lookup table approach for consistency between test and test program
pub fn generate_sine_sample(index: u32, frequency_div: u32) -> i16 {
    // Simple sine wave using lookup table approximation
    // We'll use a 32-entry lookup table for a quarter wave
    const QUARTER_WAVE_LEN: u32 = 32;
    const FULL_WAVE_LEN: u32 = QUARTER_WAVE_LEN * 4;

    // Normalize index to position in full wave
    let phase = (index / frequency_div) % FULL_WAVE_LEN;

    // Quarter wave lookup table (0 to pi/2, scaled to 0-32767)
    const SINE_TABLE: [i16; 32] = [
        0, 1608, 3212, 4808, 6393, 7962, 9512, 11039, 12539, 14010, 15446, 16846, 18204, 19519,
        20787, 22005, 23170, 24279, 25329, 26319, 27245, 28105, 28898, 29621, 30273, 30852, 31356,
        31785, 32137, 32412, 32609, 32728,
    ];

    // Determine which quarter of the wave we're in and compute the value
    if phase < QUARTER_WAVE_LEN {
        // First quarter (0 to π/2): rising, positive
        SINE_TABLE[phase as usize]
    } else if phase < QUARTER_WAVE_LEN * 2 {
        // Second quarter (π/2 to π): falling, positive
        SINE_TABLE[(QUARTER_WAVE_LEN * 2 - 1 - phase) as usize]
    } else if phase < QUARTER_WAVE_LEN * 3 {
        // Third quarter (π to 3π/2): falling, negative
        -SINE_TABLE[(phase - QUARTER_WAVE_LEN * 2) as usize]
    } else {
        // Fourth quarter (3π/2 to 2π): rising, negative
        -SINE_TABLE[(FULL_WAVE_LEN - 1 - phase) as usize]
    }
}
