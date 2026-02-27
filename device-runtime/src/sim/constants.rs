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
/// The value of 500,000 cycles was chosen based on the serialized bus protocol
/// where each memory operation requires multiple byte transfers:
///
/// - **Read operation**: 5 bytes TX (header + 4 addr) + 1-4 bytes RX = ~6-9 cycles
/// - **Write operation**: 5-9 bytes TX + 1 byte RX ack = ~7-10 cycles
/// - **Previous limit**: 40,000 cycles (with direct memory interface)
/// - **New limit**: ~12.5× higher to account for serialized protocol overhead
///
/// This value:
/// 1. Provides ample headroom for legitimate tests with serialized bus protocol
/// 2. Remains well below limits that would cause unreasonable test durations
/// 3. Acts as a safety net while the per-instruction limit (10,000 cycles)
///    remains the primary hung detection mechanism
///
/// # Exceptions
///
/// Tests that intentionally test hung detection or long instruction scenarios
/// may use higher limits with documented justification. The per-instruction
/// hung detector (default: 10,000 cycles per instruction) handles most edge cases.
pub const GLOBAL_MAX_CYCLES: u64 = 500_000;
