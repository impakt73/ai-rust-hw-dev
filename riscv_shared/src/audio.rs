//! Audio device register offsets, configuration types, and helpers

use crate::bus::AUDIO_BASE;

/// Audio buffer address register offset (0x00)
pub const AUDIO_ADDR: u32 = AUDIO_BASE;

/// Audio configuration register offset (0x04)
pub const AUDIO_CONFIG: u32 = AUDIO_BASE + 0x04;

/// Audio status register offset (0x08)
pub const AUDIO_STATUS: u32 = AUDIO_BASE + 0x08;

/// Audio DMA trigger register offset (0x0C)
pub const AUDIO_DMA: u32 = AUDIO_BASE + 0x0C;

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
    /// Number of samples (1 to 65536)
    pub sample_count: u32,
}

impl AudioConfig {
    /// Parse configuration from register value
    /// Bits [1:0]   = sample_rate (2 bits)
    /// Bit 2        = channels (1 bit: 0=mono, 1=stereo)
    /// Bits [18:3]  = sample_count - 1 (16 bits, allowing 1-65536 samples with +1 bias)
    pub fn from_register(value: u32) -> Option<Self> {
        let sample_rate_field = (value & 0x3) as u8;
        let channels_field = ((value >> 2) & 0x1) as u8;
        let sample_count_minus_1 = (value >> 3) & 0xFFFF;

        let sample_rate = AudioSampleRate::from_u8(sample_rate_field)?;
        let channels = AudioChannels::from_u8(channels_field);

        // Add 1 bias to get actual sample count (0 in register = 1 sample, 65535 = 65536 samples)
        let sample_count = sample_count_minus_1 + 1;

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

        let sample_count = self.sample_count;
        debug_assert!(
            sample_count > 0 && sample_count <= 65536,
            "AudioConfig::to_register: sample_count must be 1-65536, got {}",
            sample_count
        );

        // Apply -1 bias (1 sample = 0 in register, 65536 samples = 65535 in register)
        // Clamp to valid range in release builds
        let sample_count_minus_1 = if sample_count > 0 && sample_count <= 65536 {
            (sample_count - 1) & 0xFFFF
        } else {
            0
        };

        sample_rate_field | (channels_field << 2) | (sample_count_minus_1 << 3)
    }

    /// Calculate total number of bytes in the ring buffer
    /// Each sample is 2 bytes (i16) per channel
    ///
    /// # Panics
    /// Panics in debug mode if the calculation would overflow u32
    pub fn buffer_bytes(&self) -> u32 {
        // Use checked arithmetic to detect overflow
        let bytes_per_sample = 2u32
            .checked_mul(self.channels.count() as u32)
            .expect("overflow in bytes_per_sample");
        self.sample_count
            .checked_mul(bytes_per_sample)
            .expect("AudioConfig::buffer_bytes overflow: buffer size exceeds u32::MAX")
    }

    /// Calculate bytes per sample
    pub fn bytes_per_sample(&self) -> u32 {
        2 * self.channels.count() as u32
    }
}

/// Generate a sine wave sample at a given index
/// Uses a lookup table approach for consistency between test and test program
///
/// # Arguments
/// * `index` - Sample index
/// * `frequency_div` - Frequency divider (must be non-zero)
///
/// # Panics
/// Panics in debug builds if `frequency_div` is 0
pub fn generate_sine_sample(index: u32, frequency_div: u32) -> i16 {
    debug_assert!(
        frequency_div != 0,
        "generate_sine_sample: frequency_div must be non-zero"
    );

    // Guard against division by zero: treat a zero divider as 1 to avoid traps
    let div = if frequency_div == 0 { 1 } else { frequency_div };

    // Simple sine wave using lookup table approximation
    // We'll use a 32-entry lookup table for a quarter wave
    const QUARTER_WAVE_LEN: u32 = 32;
    const FULL_WAVE_LEN: u32 = QUARTER_WAVE_LEN * 4;

    // Normalize index to position in full wave
    let phase = (index / div) % FULL_WAVE_LEN;

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
