//! Memory Operation Integration Tests
//!
//! This module contains integration tests for memory operations including:
//!
//! - DRAM boundary and range validation
//! - Programmatic instruction loading and execution
//! - Memory region read/write via runtime API
//! - SRAM/DRAM read-write interactions between CPU and host
//! - Memory callback event forwarding
//! - Address space boundary conditions
//!
//! These tests verify both CPU-initiated memory accesses (via programmatic
//! instruction sequences) and host-initiated memory operations (via runtime API).

mod common;

use common::{
    create_test_runtime, instructions_to_bytes, load_and_boot, read_word_with_timeout,
    wait_for_cpu_halt, LONG_TIMEOUT, SHORT_TIMEOUT, TEST_BOOT_PC,
};
use riscv_core::instruction::{addi, beq, ebreak, jal, lbu, lhu, lui, lw, sb, sw};
use riscv_shared::bus::{DRAM_BASE, DRAM_END, SIM_CONTROL_BASE, SRAM_BASE};
use std::time::{Duration, Instant};

const CALLBACK_TEST_TIMEOUT_SECS: u64 = 2;
const CALLBACK_TEST_POLL_INTERVAL_MS: u64 = 5;

// ============================================================================
// DRAM Boundary and Range Tests
// ============================================================================

fn run_and_expect(program: &[u32], expected_tohost: u32) {
    let mut runtime = common::create_test_runtime();
    let program_bytes = instructions_to_bytes(program);
    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program_bytes);
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(expected_tohost)
    );
}

#[test]
fn test_read_word_outside_dram_range() {
    let program = [
        addi(11, 0, 0),            // x11 = 0x00000000 (outside DRAM)
        lw(10, 11, 0),             // x10 = read word (expected 0)
        lui(12, SIM_CONTROL_BASE), // x12 = tohost base
        sw(12, 10, 0),             // tohost = x10
        ebreak(),
        jal(0, 0),
    ];
    run_and_expect(&program, 0);
}

#[test]
fn test_valid_dram_accesses() {
    let mut runtime = common::create_test_runtime();
    let program = instructions_to_bytes(&[
        lui(11, DRAM_BASE + 0x1000), // x11 = 0x80001000 (test data base)
        addi(10, 0, 0xAA),
        sb(11, 10, 0), // [base+0] = 0xAA
        addi(10, 0, 0xBB),
        sb(11, 10, 1), // [base+1] = 0xBB
        addi(10, 0, 0xCC),
        sb(11, 10, 2), // [base+2] = 0xCC
        addi(10, 0, 0xDD),
        sb(11, 10, 3),             // [base+3] = 0xDD
        lw(10, 11, 0),             // x10 = 0xDDCCBBAA
        lui(12, SIM_CONTROL_BASE), // x12 = tohost base
        sw(12, 10, 0),             // tohost = x10
        ebreak(),
        jal(0, 0),
    ]);
    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program);
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(0xDDCC_BBAA)
    );
}

#[test]
fn test_boundary_at_dram_end_byte_read() {
    let dram_after_end = DRAM_END.wrapping_add(1);
    let program = [
        addi(10, 0, 0x42),         // x10 = test byte
        lui(11, dram_after_end),   // x11 = DRAM_END + 1
        addi(11, 11, -2),          // x11 = 0x8FFF_FFFE (DRAM_END - 1)
        sb(11, 10, 0),             // byte write near upper DRAM boundary
        lbu(10, 11, 0),            // read back written byte
        lui(12, SIM_CONTROL_BASE), // x12 = tohost base
        sw(12, 10, 0),             // tohost = x10
        ebreak(),
        jal(0, 0),
    ];
    run_and_expect(&program, 0x42);
}

#[test]
fn test_boundary_at_dram_end_word_read_out_of_bounds() {
    let dram_after_end = DRAM_END.wrapping_add(1);
    let program = [
        lui(11, dram_after_end),   // x11 = DRAM_END + 1
        addi(11, 11, -1),          // x11 = 0x8FFF_FFFF (DRAM_END)
        lw(10, 11, 0),             // word access spans beyond boundary, expected 0
        lui(12, SIM_CONTROL_BASE), // x12 = tohost base
        sw(12, 10, 0),             // tohost = x10
        ebreak(),
        jal(0, 0),
    ];
    run_and_expect(&program, 0);
}

// ============================================================================
// Programmatic Instruction Loading and Execution
// ============================================================================

/// Test that demonstrates loading and executing programmatic instructions without an ELF file.
#[test]
fn test_programmatic_instruction_loading() {
    let mut runtime = create_test_runtime();

    let instructions = vec![
        addi(10, 0, 42),
        lui(11, SIM_CONTROL_BASE),
        sw(11, 10, 0),
        ebreak(),
        jal(0, 0),
    ];
    let program = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program);
    assert_eq!(wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT), Some(42));
}

/// Test programmatic memory write/overwrite patterns using CPU-side DRAM accesses.
#[test]
fn test_write_memory_region_patterns() {
    let mut runtime = create_test_runtime();
    let checker_program = instructions_to_bytes(&[
        lui(12, DRAM_BASE),  // x12 = DRAM base
        addi(13, 12, 0x400), // x13 = DRAM_BASE + 0x400
        addi(13, 13, 0x400), // x13 = DRAM_BASE + 0x800
        addi(13, 13, 0x400), // x13 = DRAM_BASE + 0xC00
        addi(13, 13, 0x400), // x13 = DRAM_BASE + 0x1000
        addi(10, 0, 0x12),
        sw(13, 10, 0), // seed pattern at +0x1000
        addi(10, 0, 0x34),
        sw(13, 10, 4), // independent region write at +0x1004
        addi(10, 0, 0xFF),
        sw(13, 10, 0),  // overwrite first region
        lbu(10, 13, 0), // read overwritten byte
        lui(11, SIM_CONTROL_BASE),
        sw(11, 10, 0), // tohost = 0xFF
        ebreak(),
        jal(0, 0),
    ]);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &checker_program);
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(0xFF)
    );
}

// ============================================================================
// Runtime Memory Region API Tests
// ============================================================================

#[test]
fn test_runtime_write_and_read_memory_region_sram_without_cpu_program() {
    let mut runtime = create_test_runtime();
    let addr = SRAM_BASE + 0x100;
    let base = vec![0xEE; 9];
    let payload = vec![0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77];

    runtime
        .write_memory_region(addr, &base, None)
        .expect("Failed to initialize SRAM base pattern");
    runtime
        .write_memory_region(addr + 1, &payload, None)
        .expect("Failed to write SRAM via runtime");

    let read_full = runtime
        .read_memory_region(addr, 9, None)
        .expect("Failed to read full SRAM window via runtime");
    assert_eq!(
        read_full,
        vec![0xEE, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0xEE]
    );

    let read_back = runtime
        .read_memory_region(addr + 1, payload.len() as u32, None)
        .expect("Failed to read SRAM via runtime");
    assert_eq!(read_back, payload);
}

#[test]
fn test_runtime_memory_region_aligned_word_block_and_single_tail_bytes() {
    let mut runtime = create_test_runtime();
    let addr = DRAM_BASE + 0x200;

    // Aligned interior (word path) plus single-byte tail/head behavior.
    let payload = vec![
        0xAA, 0xBB, 0xCC, 0xDD, // word 0
        0x11, 0x22, 0x33, 0x44, // word 1
        0x99, // tail byte
    ];

    runtime
        .write_memory_region(addr + 1, &payload, None)
        .expect("unaligned write should succeed");

    let read_back = runtime
        .read_memory_region(addr + 1, payload.len() as u32, None)
        .expect("unaligned read should succeed");
    assert_eq!(read_back, payload);
}

#[test]
fn test_runtime_write_sram_then_cpu_reads_it() {
    let mut runtime = create_test_runtime();
    let payload = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07];

    let checker_program = instructions_to_bytes(&[
        lui(12, SRAM_BASE),
        lbu(9, 12, 6),
        beq(9, 0, -4),
        lw(10, 12, 0),  // 0x04030201
        lhu(11, 12, 4), // 0x00000605
        lbu(13, 12, 6), // 0x00000007
        lui(14, DRAM_BASE),
        sw(14, 10, 0),
        sw(14, 11, 4),
        sw(14, 13, 8),
        lui(15, SIM_CONTROL_BASE),
        addi(16, 0, 1),
        sw(15, 16, 0),
        ebreak(),
        jal(0, 0),
    ]);
    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &checker_program);

    runtime
        .write_memory_region(SRAM_BASE, &payload, None)
        .expect("Failed to write SRAM via runtime");

    let _ = wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT);
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE, SHORT_TIMEOUT),
        0x0403_0201
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 4, SHORT_TIMEOUT),
        0x0000_0605
    );
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE + 8, SHORT_TIMEOUT),
        0x0000_0007
    );
}

#[test]
fn test_cpu_writes_sram_then_runtime_reads_it() {
    let mut runtime = create_test_runtime();

    let writer_program = instructions_to_bytes(&[
        lui(12, SRAM_BASE),
        addi(10, 0, 0x11),
        sb(12, 10, 0),
        addi(10, 0, 0x22),
        sb(12, 10, 1),
        addi(10, 0, 0x33),
        sb(12, 10, 2),
        addi(10, 0, 0x44),
        sb(12, 10, 3),
        addi(10, 0, 0x55),
        sb(12, 10, 4),
        addi(10, 0, 0x66),
        sb(12, 10, 5),
        addi(10, 0, 0x77),
        sb(12, 10, 6),
        lui(11, SIM_CONTROL_BASE),
        addi(10, 0, 1),
        sw(11, 10, 0),
        ebreak(),
        jal(0, 0),
    ]);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &writer_program);
    assert_eq!(wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT), Some(1));

    let read_back = runtime
        .read_memory_region(SRAM_BASE, 7, None)
        .expect("Failed to read SRAM via runtime");
    assert_eq!(read_back, vec![0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77]);
}

#[test]
fn test_runtime_memory_region_callback_receives_unrelated_events() {
    let mut runtime = create_test_runtime();
    const FENCE_ADDR: u32 = DRAM_BASE + 0x2000;

    runtime
        .write_memory_region(FENCE_ADDR, &[0], None)
        .expect("Failed to initialize fence memory");

    let program = instructions_to_bytes(&[
        lui(10, SIM_CONTROL_BASE),
        addi(11, 0, 0x2A),
        sw(10, 11, 0),
        lui(12, FENCE_ADDR),
        addi(13, 0, 1),
        sw(12, 13, 0),
        ebreak(),
        jal(0, 0),
    ]);
    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program);

    let mut observed_tohost = None;
    let mut callback = |event: device_runtime::BusEvent| {
        if let device_runtime::BusEvent::TohostTermination { value } = event {
            observed_tohost = Some(value);
        }
    };

    let deadline = Instant::now() + Duration::from_secs(CALLBACK_TEST_TIMEOUT_SECS);
    let expected_fence = [1u8];
    loop {
        let read_back = runtime
            .read_memory_region(FENCE_ADDR, 1, Some(&mut callback))
            .expect("read_memory_region should succeed while forwarding unrelated events");
        if read_back.as_slice() == expected_fence {
            break;
        }
        if Instant::now() >= deadline {
            panic!("Timed out waiting for CPU fence update");
        }
        std::thread::sleep(Duration::from_millis(CALLBACK_TEST_POLL_INTERVAL_MS));
    }

    assert_eq!(observed_tohost, Some(0x2A));

    assert_eq!(wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT), None);
}

#[test]
fn test_runtime_memory_region_dram_end_boundary_accesses_are_valid() {
    let mut runtime = create_test_runtime();
    runtime
        .write_memory_region(DRAM_END, &[0xA5], None)
        .expect("1-byte write at DRAM_END should be valid");
    assert_eq!(
        runtime
            .read_memory_region(DRAM_END, 1, None)
            .expect("1-byte read at DRAM_END should be valid")
            .len(),
        1
    );

    runtime
        .write_memory_region(DRAM_END - 1, &[0x34, 0x12], None)
        .expect("2-byte write ending at DRAM_END should be valid");
    assert_eq!(
        runtime
            .read_memory_region(DRAM_END - 1, 2, None)
            .expect("2-byte read ending at DRAM_END should be valid")
            .len(),
        2
    );
}
