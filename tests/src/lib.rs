// Main library file for CPU verification tests
// Tests will be added as modules here

#[cfg(test)]
mod alu_test;

#[cfg(test)]
mod regfile_test;

#[cfg(test)]
mod decompress_test;

// cpu_test module has been migrated to cpu-sim/src/test_rtl_verification.rs
// This consolidates programmatic instruction testing in the cpu-sim crate
// which provides better infrastructure (SystemBus, VCD dumps, instruction tracing)
