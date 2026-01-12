// Main library file for CPU verification tests
// Tests will be added as modules here

#[cfg(test)]
mod alu_test;

#[cfg(test)]
mod regfile_test;

#[cfg(test)]
mod decompress_test;

#[cfg(test)]
mod fp_regfile_test;

// Note: FPU unit tests are skipped due to Verilator limitations with shortreal/floating-point.
// The FPU will be tested at the CPU integration level once integrated into top.sv.
// See docs/rv32f-upgrade-plan.md Phase 6 for CPU-level FP testing strategy.

// cpu_test module has been migrated to cpu-sim/src/test_rtl_verification.rs
// This consolidates programmatic instruction testing in the cpu-sim crate
// which provides better infrastructure (SystemBus, VCD dumps, instruction tracing)
