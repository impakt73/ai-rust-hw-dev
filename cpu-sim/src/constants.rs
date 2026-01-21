//! CPU Simulator Constants
//!
//! This module defines project-wide constants for the CPU simulator.

/// Global maximum cycles limit for test execution
///
/// This constant defines the maximum number of cycles any test should run
/// before being considered a runaway or hung simulation. It serves as a
/// safety backstop to prevent infinite loops in tests.
///
/// # Value Selection Rationale
///
/// The value of 40,000 cycles was chosen based on empirical measurement of
/// all cpu-sim tests:
///
/// - **Maximum observed cycles**: 17,296 (from test_println_macro)
/// - **Safety margin**: 2.3× the maximum observed value
/// - **Unmeasured tests**: Conservative estimates suggest < 5,000 cycles for most ELF tests
///
/// This value:
/// 1. Provides ample headroom (2.3× maximum observed) for legitimate tests
/// 2. Is a clean, memorable round number
/// 3. Remains well below previous high limits (100,000) that were unnecessarily large
/// 4. Acts as a safety net while the per-instruction limit (10,000 cycles)
///    remains the primary hung detection mechanism
/// 5. Should never be reached by any legitimate test in normal operation
///
/// # Exceptions
///
/// Tests that intentionally test hung detection or long instruction scenarios
/// may use higher limits with documented justification. The per-instruction
/// hung detector (default: 10,000 cycles per instruction) handles most edge cases.
///
/// # Measurement Data
///
/// See `reports/max_cycles_report.csv` for detailed cycle measurements across
/// all tests that informed this value.
pub const GLOBAL_MAX_CYCLES: u64 = 40_000;
