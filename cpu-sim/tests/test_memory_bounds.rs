//! Memory bounds checking tests
//!
//! Tests that validate memory access bounds checking in cpu-sim.
//! The DRAM range is 0x8000_0000 to 0xFFFF_FFFF (2 GiB).
//!
//! These tests ensure that:
//! 1. Direct memory accesses through SimulatorView validate addresses
//! 2. SystemContext methods validate addresses
//! 3. Out-of-bounds accesses are logged and handled safely
//! 4. Boundary conditions are handled correctly

mod common;

use cpu_sim::*;
use riscv_core::instruction::*;
use riscv_shared::sim_control::SUCCESS_CODE;

/// Helper function to create a termination sequence (write to tohost and halt)
fn create_termination_program(tohost_value: u32) -> Vec<u8> {
    common::instructions_to_bytes(&common::tohost_termination(11, 10, tohost_value))
}

/// Test that write_memory_region rejects addresses below DRAM range
#[test]
fn test_write_memory_below_dram_range() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .is_test(true)
        .try_init()
        .ok();

    let program = create_termination_program(SUCCESS_CODE);

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            // Try to write to an address below DRAM range (should be rejected and logged)
            sim.write_memory_region(0x0000_0000, &program, true);

            // Write valid program to DRAM (this should succeed)
            sim.write_memory_region(DRAM_BASE, &program, true);
            Ok(DRAM_BASE)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    );

    // The simulation should complete (the invalid write is just logged and skipped)
    assert!(
        result.is_ok(),
        "Simulation should complete despite invalid write: {:?}",
        result.err()
    );
    assert_eq!(result.unwrap().tohost_value, Some(SUCCESS_CODE));
}

/// Test that write_memory_region rejects addresses spanning below DRAM range
#[test]
fn test_write_memory_spanning_below_dram() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .is_test(true)
        .try_init()
        .ok();

    let instructions = vec![0x13; 16]; // 16 bytes of nops

    let program_bytes = create_termination_program(2);

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            // Try to write starting just below DRAM_BASE, spanning into valid range
            // This should be rejected because the start address is invalid
            sim.write_memory_region(DRAM_BASE - 8, &instructions, true);

            // Write valid program to DRAM
            sim.write_memory_region(DRAM_BASE, &program_bytes, true);
            Ok(DRAM_BASE)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    );

    assert!(
        result.is_ok(),
        "Simulation should complete despite invalid write: {:?}",
        result.err()
    );
    assert_eq!(result.unwrap().tohost_value, Some(2));
}

/// Test that write_memory_region rejects addresses above DRAM range
#[test]
fn test_write_memory_above_dram_range() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .is_test(true)
        .try_init()
        .ok();

    let instructions = vec![0x13, 0x00, 0x00, 0x00];

    let program_bytes = create_termination_program(3);

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            // Try to write beyond the end of DRAM (should be rejected)
            // Note: Writing at exactly DRAM_END should fail because it's a 4-byte write
            sim.write_memory_region(DRAM_END, &instructions, true);

            // Write valid program to DRAM
            sim.write_memory_region(DRAM_BASE, &program_bytes, true);
            Ok(DRAM_BASE)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    );

    assert!(
        result.is_ok(),
        "Simulation should complete despite invalid write: {:?}",
        result.err()
    );
    assert_eq!(result.unwrap().tohost_value, Some(3));
}

/// Test that write_memory_region accepts boundary addresses correctly
#[test]
fn test_write_memory_at_dram_start() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .is_test(true)
        .try_init()
        .ok();

    let mut instructions = vec![addi(10, 0, 42)]; // x10 = 42
    common::append_tohost_termination(&mut instructions, 11, 10, SUCCESS_CODE);
    let program = common::instructions_to_bytes(&instructions);

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            // Write at the very start of DRAM (should succeed)
            sim.write_memory_region(DRAM_BASE, &program, true);
            Ok(DRAM_BASE)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    );

    assert!(result.is_ok(), "Write at DRAM_BASE should succeed");
    let result = result.unwrap();
    assert_eq!(result.tohost_value, Some(42));
}

/// Test that write_memory_region accepts writes ending at DRAM_END
#[test]
fn test_write_memory_ending_at_dram_end() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .is_test(true)
        .try_init()
        .ok();

    // Create a 4-byte write that ends exactly at DRAM_END
    let data = vec![0x13, 0x00, 0x00, 0x00];
    let start_addr = DRAM_END - 3; // Write 4 bytes ending at DRAM_END

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            // This write should succeed (ends exactly at DRAM_END)
            sim.write_memory_region(start_addr, &data, false);

            // Write valid program to DRAM_BASE
            let program = create_termination_program(SUCCESS_CODE);
            sim.write_memory_region(DRAM_BASE, &program, true);
            Ok(DRAM_BASE)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    );

    assert!(
        result.is_ok(),
        "Write ending at DRAM_END should succeed: {:?}",
        result.err()
    );
}

/// Test that read_byte rejects addresses below DRAM range
#[test]
fn test_read_byte_below_dram_range() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .is_test(true)
        .try_init()
        .ok();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            let program = create_termination_program(SUCCESS_CODE);
            sim.write_memory_region(DRAM_BASE, &program, true);
            Ok(DRAM_BASE)
        },
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            // Try to read from below DRAM range (should return 0 and log warning)
            let value = sim.read_byte(0x0000_0000);
            assert_eq!(value, 0, "Out-of-bounds read should return 0");

            let value = sim.read_byte(DRAM_BASE - 1);
            assert_eq!(value, 0, "Out-of-bounds read should return 0");
        }),
    );

    assert!(
        result.is_ok(),
        "Simulation should complete: {:?}",
        result.err()
    );
}

/// Test that read_halfword rejects addresses outside DRAM range
#[test]
fn test_read_halfword_outside_dram_range() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .is_test(true)
        .try_init()
        .ok();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            let program = create_termination_program(SUCCESS_CODE);
            sim.write_memory_region(DRAM_BASE, &program, true);
            Ok(DRAM_BASE)
        },
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            // Read below DRAM range (unmapped address, not in any peripheral range)
            let value = sim.read_halfword(0x0100_0000);
            assert_eq!(value, 0, "Out-of-bounds read should return 0");

            // Read spanning the end of DRAM (halfword at DRAM_END would span beyond)
            let value = sim.read_halfword(DRAM_END);
            assert_eq!(value, 0, "Out-of-bounds read should return 0");
        }),
    );

    assert!(
        result.is_ok(),
        "Simulation should complete: {:?}",
        result.err()
    );
}

/// Test that read_word rejects addresses outside DRAM range
#[test]
fn test_read_word_outside_dram_range() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .is_test(true)
        .try_init()
        .ok();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            let program = create_termination_program(SUCCESS_CODE);
            sim.write_memory_region(DRAM_BASE, &program, true);
            Ok(DRAM_BASE)
        },
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            // Read from below DRAM range
            let value = sim.read_word(0x0000_0000);
            assert_eq!(value, 0, "Out-of-bounds read should return 0");

            // Read from unmapped address (not in any peripheral or DRAM range)
            let value = sim.read_word(0x0100_0000);
            assert_eq!(value, 0, "Out-of-bounds read should return 0");

            // Read spanning the end of DRAM
            let value = sim.read_word(DRAM_END - 2); // Would span beyond DRAM_END
            assert_eq!(value, 0, "Out-of-bounds read should return 0");
        }),
    );

    assert!(
        result.is_ok(),
        "Simulation should complete: {:?}",
        result.err()
    );
}

/// Test that dump_memory_region validates the range
#[test]
fn test_dump_memory_region_outside_dram() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .is_test(true)
        .try_init()
        .ok();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            let program = create_termination_program(SUCCESS_CODE);
            sim.write_memory_region(DRAM_BASE, &program, true);
            Ok(DRAM_BASE)
        },
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            // Dump from below DRAM range (should return all zeros and log warning)
            let bytes: Vec<u8> = sim.dump_memory_region(0x0000_0000, 16).collect();
            assert_eq!(bytes.len(), 16);
            assert!(
                bytes.iter().all(|&b| b == 0),
                "Out-of-bounds dump should return zeros"
            );

            // Dump spanning beyond DRAM_END (should return zeros and log warning)
            let bytes: Vec<u8> = sim.dump_memory_region(DRAM_END - 4, 16).collect();
            assert_eq!(bytes.len(), 16);
            assert!(
                bytes.iter().all(|&b| b == 0),
                "Out-of-bounds dump should return zeros"
            );
        }),
    );

    assert!(
        result.is_ok(),
        "Simulation should complete: {:?}",
        result.err()
    );
}

/// Test that valid DRAM accesses still work correctly
#[test]
fn test_valid_dram_accesses() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .is_test(true)
        .try_init()
        .ok();

    let test_data = vec![0xAA, 0xBB, 0xCC, 0xDD];

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            // Write test data to DRAM
            sim.write_memory_region(DRAM_BASE + 0x1000, &test_data, false);

            let program = create_termination_program(SUCCESS_CODE);
            sim.write_memory_region(DRAM_BASE, &program, true);
            Ok(DRAM_BASE)
        },
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            // Verify we can read the data back
            let byte = sim.read_byte(DRAM_BASE + 0x1000);
            assert_eq!(byte, 0xAA, "Valid read should return correct value");

            let halfword = sim.read_halfword(DRAM_BASE + 0x1000);
            assert_eq!(
                halfword, 0xBBAA,
                "Valid read should return correct value (little-endian)"
            );

            let word = sim.read_word(DRAM_BASE + 0x1000);
            assert_eq!(
                word, 0xDDCCBBAA,
                "Valid read should return correct value (little-endian)"
            );

            // Verify dump_memory_region
            let bytes: Vec<u8> = sim.dump_memory_region(DRAM_BASE + 0x1000, 4).collect();
            assert_eq!(bytes, test_data, "Valid dump should return correct data");
        }),
    );

    assert!(
        result.is_ok(),
        "Simulation should complete: {:?}",
        result.err()
    );
}

/// Test that boundary condition at DRAM_BASE works correctly
#[test]
fn test_boundary_at_dram_start() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .is_test(true)
        .try_init()
        .ok();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            let program = create_termination_program(SUCCESS_CODE);
            sim.write_memory_region(DRAM_BASE, &program, true);
            Ok(DRAM_BASE)
        },
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            // Read at exactly DRAM_BASE (should succeed)
            let value = sim.read_word(DRAM_BASE);
            assert_ne!(value, 0, "Read at DRAM_BASE should succeed");

            // Read just before DRAM_BASE (should fail)
            let value = sim.read_byte(DRAM_BASE - 1);
            assert_eq!(value, 0, "Read before DRAM_BASE should return 0");
        }),
    );

    assert!(
        result.is_ok(),
        "Simulation should complete: {:?}",
        result.err()
    );
}

/// Test that boundary condition at DRAM_END works correctly
#[test]
fn test_boundary_at_dram_end() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .is_test(true)
        .try_init()
        .ok();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            // Write to the end of DRAM
            let data = vec![0x42];
            sim.write_memory_region(DRAM_END, &data, false); // Single byte at DRAM_END

            let program = create_termination_program(SUCCESS_CODE);
            sim.write_memory_region(DRAM_BASE, &program, true);
            Ok(DRAM_BASE)
        },
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            // Read at exactly DRAM_END (should succeed for single byte)
            let value = sim.read_byte(DRAM_END);
            assert_eq!(value, 0x42, "Read at DRAM_END should succeed");

            // Try to read a word starting at DRAM_END (should fail, spans beyond)
            let value = sim.read_word(DRAM_END);
            assert_eq!(value, 0, "Word read at DRAM_END should fail (spans beyond)");
        }),
    );

    assert!(
        result.is_ok(),
        "Simulation should complete: {:?}",
        result.err()
    );
}
