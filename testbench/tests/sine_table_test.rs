use riscv_core::{create_sine_table_runtime, SineTableTestWrapper};

use riscv_core::AsDynamicVerilatedModel;
// Parameters matching sine_table_test_wrapper / sine_table_test_init.hex
const TABLE_SIZE: u16 = 1024;
const PIPELINE_STAGES: usize = 4; // Stage 1 + 2 ROM stages + Stage 4

/// Compute the expected u16 output for a given full-period index.
///
/// Mirrors the RTL quadrant logic and the same formula used to build the hex
/// init file: ROM[k] = round(32767 * sin(2π * ((k + 0.5) / TABLE_SIZE))).
fn expected_sample(index: u16) -> u16 {
    const IDX_W: u32 = TABLE_SIZE.ilog2(); // log2(TABLE_SIZE), so 10 for 1024 entries
    const QADDR_W: u32 = IDX_W - 2; // 8
    const QADDR_MASK: u16 = (1u16 << QADDR_W) - 1; // 0xFF
    const MAX_SIGNED: f64 = 32767.0;
    const MID_TREAD_OFFSET: f64 = 0.5;

    let invert_result = (index >> (IDX_W - 1)) & 1 != 0;
    let invert_index = (index >> (IDX_W - 2)) & 1 != 0;
    let qaddr = index & QADDR_MASK;
    let rom_addr = if invert_index {
        (!qaddr) & QADDR_MASK
    } else {
        qaddr
    };

    let normalized_phase = (rom_addr as f64 + MID_TREAD_OFFSET) / (TABLE_SIZE as f64);
    let phase = 2.0 * std::f64::consts::PI * normalized_phase;
    let raw = (MAX_SIGNED * phase.sin()).round() as i16;
    let raw_bits = raw as u16;

    if invert_result {
        raw_bits.wrapping_neg()
    } else {
        raw_bits
    }
}

fn clock_cycle(dut: &mut SineTableTestWrapper) {
    dut.clk = 0;
    dut.eval();
    dut.clk = 1;
    dut.eval();
}

/// Advance the pipeline by PIPELINE_STAGES clock edges so the sample
/// registered for the index that was set before this call appears at
/// `dut.sample`.
fn advance_pipeline(dut: &mut SineTableTestWrapper) {
    for _ in 0..PIPELINE_STAGES {
        clock_cycle(dut);
    }
}

// ---------------------------------------------------------------------------
// Individual quadrant / output tests
// ---------------------------------------------------------------------------

/// Q0 first sample: mid-tread initialization means index 0 is slightly above 0.
#[test]
fn test_sine_table_q0_first_sample() {
    let runtime = create_sine_table_runtime().expect("Failed to create sine_table runtime");
    let mut dut = runtime
        .create_model_simple::<SineTableTestWrapper>()
        .expect("Failed to create sine_table model");

    dut.index = 0;
    advance_pipeline(&mut dut);
    assert_eq!(
        dut.sample,
        expected_sample(0),
        "index 0 should reflect the first mid-tread positive sample"
    );
}

/// Q0 mid value: index 128, approximately sin(45°).
#[test]
fn test_sine_table_q0_mid() {
    let runtime = create_sine_table_runtime().expect("Failed to create sine_table runtime");
    let mut dut = runtime
        .create_model_simple::<SineTableTestWrapper>()
        .expect("Failed to create sine_table model");

    dut.index = 128;
    advance_pipeline(&mut dut);
    assert_eq!(
        dut.sample,
        expected_sample(128),
        "index 128 (sin ~45°) should match the mid-tread expected sample"
    );
}

/// Q1 peak: index 256 triggers invert_index, ROM address mirrors to 255.
/// Output is positive (invert_result = 0).
#[test]
fn test_sine_table_q1_peak() {
    let runtime = create_sine_table_runtime().expect("Failed to create sine_table runtime");
    let mut dut = runtime
        .create_model_simple::<SineTableTestWrapper>()
        .expect("Failed to create sine_table model");

    dut.index = 256;
    advance_pipeline(&mut dut);
    let got = dut.sample;
    let want = expected_sample(256); // ROM[255] is the mirrored near-peak sample
    assert_eq!(
        got, want,
        "index 256 (Q1 peak, mirrored): expected 0x{want:04X}, got 0x{got:04X}"
    );
}

/// Q2 first sample: index 512 negates the first positive quarter-wave entry.
#[test]
fn test_sine_table_q2_first_sample_negated() {
    let runtime = create_sine_table_runtime().expect("Failed to create sine_table runtime");
    let mut dut = runtime
        .create_model_simple::<SineTableTestWrapper>()
        .expect("Failed to create sine_table model");

    dut.index = 512;
    advance_pipeline(&mut dut);
    assert_eq!(
        dut.sample,
        expected_sample(512),
        "index 512 should be the negated first mid-tread sample"
    );
}

/// Q2 mid: index 640 has invert_result=1, producing a negative result.
/// The magnitude should equal the Q0 mid value at index 128.
#[test]
fn test_sine_table_q2_mid_negated() {
    let runtime = create_sine_table_runtime().expect("Failed to create sine_table runtime");
    let mut dut = runtime
        .create_model_simple::<SineTableTestWrapper>()
        .expect("Failed to create sine_table model");

    dut.index = 640;
    advance_pipeline(&mut dut);
    let got = dut.sample;
    let want = expected_sample(640);
    assert_eq!(
        got, want,
        "index 640 (Q2 mid): expected 0x{want:04X}, got 0x{got:04X}"
    );
    // Magnitude check: Q2 sample + Q0 sample at same offset must be 0 mod 2^16
    assert_eq!(
        got.wrapping_add(expected_sample(128)),
        0,
        "Q2[640] must be the two's-complement negation of Q0[128]"
    );
}

/// Q3 negative peak: index 768 has both invert_index and invert_result active.
#[test]
fn test_sine_table_q3_negative_peak() {
    let runtime = create_sine_table_runtime().expect("Failed to create sine_table runtime");
    let mut dut = runtime
        .create_model_simple::<SineTableTestWrapper>()
        .expect("Failed to create sine_table model");

    dut.index = 768;
    advance_pipeline(&mut dut);
    let got = dut.sample;
    let want = expected_sample(768);
    assert_eq!(
        got, want,
        "index 768 (Q3 negative peak): expected 0x{want:04X}, got 0x{got:04X}"
    );
    // Q3 peak must negate Q1 peak
    assert_eq!(
        got.wrapping_add(expected_sample(256)),
        0,
        "Q3[768] must be the two's-complement negation of Q1[256]"
    );
}

// ---------------------------------------------------------------------------
// Pipeline latency test
// ---------------------------------------------------------------------------

/// Verify that the pipeline latency is exactly PIPELINE_STAGES clock cycles.
///
/// Strategy:
///   1. Prime the pipeline with index=128.
///   2. Switch to index=0. The new output must not appear for the first
///      PIPELINE_STAGES-1 clock edges and must appear on the PIPELINE_STAGES-th
///      edge.
#[test]
fn test_sine_table_pipeline_latency() {
    let runtime = create_sine_table_runtime().expect("Failed to create sine_table runtime");
    let mut dut = runtime
        .create_model_simple::<SineTableTestWrapper>()
        .expect("Failed to create sine_table model");

    // Prime: present index=128 for exactly PIPELINE_STAGES clocks.
    dut.index = 128;
    advance_pipeline(&mut dut);
    let primed = dut.sample;
    assert_eq!(
        primed,
        expected_sample(128),
        "priming read should return the expected index-128 sample"
    );

    let first_mid_tread = expected_sample(0);

    // Switch to index=0 and verify the stale primed value persists for
    // exactly PIPELINE_STAGES-1 more clocks.
    dut.index = 0;
    for cycle in 1..PIPELINE_STAGES {
        clock_cycle(&mut dut);
        assert_eq!(
            dut.sample, primed,
            "output must retain primed value at cycle {cycle} \
             (new value not yet through the {PIPELINE_STAGES}-stage pipeline)"
        );
    }

    // The PIPELINE_STAGES-th clock flushes the new value to the output.
    clock_cycle(&mut dut);
    assert_eq!(
        dut.sample, first_mid_tread,
        "output must update to the first mid-tread sample after exactly {PIPELINE_STAGES} clock cycles"
    );
}

// ---------------------------------------------------------------------------
// Stability test
// ---------------------------------------------------------------------------

/// Re-reading the same index must produce identical output each time.
#[test]
fn test_sine_table_stable_repeated_reads() {
    let runtime = create_sine_table_runtime().expect("Failed to create sine_table runtime");
    let mut dut = runtime
        .create_model_simple::<SineTableTestWrapper>()
        .expect("Failed to create sine_table model");

    dut.index = 192;
    advance_pipeline(&mut dut);
    let first = dut.sample;

    advance_pipeline(&mut dut);
    assert_eq!(
        dut.sample, first,
        "re-reading index 192 must yield the same output"
    );
    assert_eq!(
        first,
        expected_sample(192),
        "index 192 must match the pre-computed expected value"
    );
}

// ---------------------------------------------------------------------------
// Quadrant symmetry properties (verified against hardware)
// ---------------------------------------------------------------------------

/// Verify three fundamental sine symmetry properties in hardware:
///
/// 1. Mirror (fold) symmetry — Q0 and Q1 share the same ROM entry:
///    sample(k) == sample(511 - k) for k in Q0.
///    The pair (k, 511-k) maps to the same ROM address because
///    invert_index flips the lower bits: ~(511-k & 0xFF) == k & 0xFF.
///
/// 2. Anti-period (half-period negation):
///    sample(k) + sample(k + 512) == 0 (mod 2^16) for any k.
///
/// 3. Q1/Q3 anti-period:
///    sample(N/4 + j) + sample(N/4 + j + N/2) == 0 (mod 2^16).
#[test]
fn test_sine_table_quadrant_symmetry() {
    let runtime = create_sine_table_runtime().expect("Failed to create sine_table runtime");
    let mut dut = runtime
        .create_model_simple::<SineTableTestWrapper>()
        .expect("Failed to create sine_table model");

    // ---- Property 1: mirror symmetry (k=64, mirror=447) ----
    // sample(64) and sample(447) both map to ROM[64] with no invert.
    dut.index = 64;
    advance_pipeline(&mut dut);
    let s64 = dut.sample;

    dut.index = 511 - 64; // 447: Q1, invert_index, qaddr=191, ROM addr=(~191) & 0xFF = 64
    advance_pipeline(&mut dut);
    let s447 = dut.sample;
    assert_eq!(
        s64, s447,
        "mirror symmetry: sample(64) must equal sample(447); \
         got sample(64)=0x{s64:04X}, sample(447)=0x{s447:04X}"
    );

    // ---- Property 2: anti-period (k=64, k+512=576) ----
    dut.index = 64 + 512; // 576: Q2, invert_result, same ROM addr as Q0[64]
    advance_pipeline(&mut dut);
    let s576 = dut.sample;
    assert_eq!(
        s64.wrapping_add(s576),
        0,
        "anti-period: sample(64) + sample(576) must be 0 mod 2^16; \
         got 0x{s64:04X} + 0x{s576:04X} = 0x{:04X}",
        s64.wrapping_add(s576)
    );

    // ---- Property 3: Q1/Q3 anti-period (j=64) ----
    let q1_idx: u16 = TABLE_SIZE / 4 + 64; // 320
    let q3_idx: u16 = q1_idx + TABLE_SIZE / 2; // 832

    dut.index = q1_idx;
    advance_pipeline(&mut dut);
    let sq1 = dut.sample;

    dut.index = q3_idx;
    advance_pipeline(&mut dut);
    let sq3 = dut.sample;

    assert_eq!(
        sq1.wrapping_add(sq3),
        0,
        "Q1/Q3 anti-period: sample({q1_idx}) + sample({q3_idx}) must be 0 mod 2^16; \
         got 0x{sq1:04X} + 0x{sq3:04X} = 0x{:04X}",
        sq1.wrapping_add(sq3)
    );
}
