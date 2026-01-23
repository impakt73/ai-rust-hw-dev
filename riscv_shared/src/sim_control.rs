//! SimControl device constants

use crate::bus::SIM_CONTROL_BASE;

/// TOHOST address for signaling halt to the simulator
///
/// This register is provided by the SimControl device and is used to signal
/// program termination to the simulator. Writing any value to this address
/// will cause the simulator to halt and capture the written value.
///
/// Note: The tohost register is write-only. Attempting to read from it will
/// result in a bus error.
pub const TOHOST_ADDR: u32 = SIM_CONTROL_BASE;

/// Standard success code for tests (expected by cpu-sim)
pub const SUCCESS_CODE: u32 = 42;

/// Standard failure code for tests (indicates test logic failure, not panic)
pub const FAILURE_CODE: u32 = 1;

/// Standard panic/failure code (different from success to aid debugging)
pub const PANIC_CODE: u32 = 0xDEAD;
