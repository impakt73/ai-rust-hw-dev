/// Shared audio test utilities for generating and validating test patterns
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

/// Generate expected stereo sample pair (left and right channels)
/// Right channel is phase-shifted by 90 degrees
pub fn generate_stereo_sample(index: u32, frequency_div: u32) -> (i16, i16) {
    let left = generate_sine_sample(index, frequency_div);
    let right = generate_sine_sample(index + frequency_div / 4, frequency_div);
    (left, right)
}
