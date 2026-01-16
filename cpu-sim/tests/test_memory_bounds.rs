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

/// DRAM memory range constants (must match those in bus_device.rs and sim.rs)
const DRAM_START: u32 = 0x8000_0000;
const DRAM_END: u32 = 0xFFFF_FFFF;

/// Test that write_memory_region rejects addresses below DRAM range
#[test]
fn test_write_memory_below_dram_range() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .is_test(true)
        .try_init()
        .ok();

    let instructions = vec![0x13, 0x00, 0x00, 0x00]; // nop

    let result = run_program(
        100,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            // Try to write to an address below DRAM range (should be rejected)
            sim.write_memory_region(0x0000_0000, &instructions, true);

            // Write valid program to DRAM
            sim.write_memory_region(DRAM_START, &instructions, true);
            Ok(DRAM_START)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    );

    // The simulation should complete (the invalid write is just logged and skipped)
    assert!(
        result.is_ok(),
        "Simulation should complete despite invalid write"
    );
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

    let result = run_program(
        100,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            // Try to write starting just below DRAM_START, spanning into valid range
            // This should be rejected because the start address is invalid
            sim.write_memory_region(DRAM_START - 8, &instructions, true);

            // Write valid program to DRAM
            let nop = vec![0x13, 0x00, 0x00, 0x00];
            sim.write_memory_region(DRAM_START, &nop, true);
            Ok(DRAM_START)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    );

    assert!(
        result.is_ok(),
        "Simulation should complete despite invalid write"
    );
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

    let result = run_program(
        100,
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
            sim.write_memory_region(DRAM_START, &instructions, true);
            Ok(DRAM_START)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    );

    assert!(
        result.is_ok(),
        "Simulation should complete despite invalid write"
    );
}

/// Test that write_memory_region accepts boundary addresses correctly
#[test]
fn test_write_memory_at_dram_start() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .is_test(true)
        .try_init()
        .ok();

    let instructions = vec![
        addi(10, 0, 42),      // x10 = 42
        lui(11, 0x10000000),  // x11 = 0x10000000 (tohost)
        sw(11, 10, 0),        // tohost = 42
        jal(0, 0),            // halt
    ];
    let program = common::instructions_to_bytes(&instructions);

    let result = run_program(
        1000,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            // Write at the very start of DRAM (should succeed)
            sim.write_memory_region(DRAM_START, &program, true);
            Ok(DRAM_START)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    );

    assert!(result.is_ok(), "Write at DRAM_START should succeed");
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
        100,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            // This write should succeed (ends exactly at DRAM_END)
            sim.write_memory_region(start_addr, &data, false);

            // Write valid program to DRAM_START
            let instructions = vec![
                addi(10, 0, 1),       // x10 = 1
                lui(11, 0x10000000),  // x11 = tohost
                sw(11, 10, 0),        // tohost = 1
            ];
            let program = common::instructions_to_bytes(&instructions);
            sim.write_memory_region(DRAM_START, &program, true);
            Ok(DRAM_START)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    );

    assert!(
        result.is_ok(),
        "Write ending at DRAM_END should succeed"
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
        100,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            let instructions = vec![
                addi(10, 0, 1),
                lui(11, 0x10000000),
                sw(11, 10, 0),
            ];
            let program = common::instructions_to_bytes(&instructions);
            sim.write_memory_region(DRAM_START, &program, true);
            Ok(DRAM_START)
        },
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            // Try to read from below DRAM range (should return 0 and log warning)
            let value = sim.read_byte(0x0000_0000);
            assert_eq!(value, 0, "Out-of-bounds read should return 0");

            let value = sim.read_byte(DRAM_START - 1);
            assert_eq!(value, 0, "Out-of-bounds read should return 0");
        }),
    );

    assert!(result.is_ok());
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
        100,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            let instructions = vec![
                addi(10, 0, 1),
                lui(11, 0x10000000),
                sw(11, 10, 0),
            ];
            let program = common::instructions_to_bytes(&instructions);
            sim.write_memory_region(DRAM_START, &program, true);
            Ok(DRAM_START)
        },
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            // Read below DRAM range
            let value = sim.read_halfword(0x1000_0000);
            assert_eq!(value, 0, "Out-of-bounds read should return 0");

            // Read spanning the end of DRAM (halfword at DRAM_END would span beyond)
            let value = sim.read_halfword(DRAM_END);
            assert_eq!(value, 0, "Out-of-bounds read should return 0");
        }),
    );

    assert!(result.is_ok());
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
        100,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            let instructions = vec![
                addi(10, 0, 1),
                lui(11, 0x10000000),
                sw(11, 10, 0),
            ];
            let program = common::instructions_to_bytes(&instructions);
            sim.write_memory_region(DRAM_START, &program, true);
            Ok(DRAM_START)
        },
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            // Read from below DRAM range
            let value = sim.read_word(0x0000_0000);
            assert_eq!(value, 0, "Out-of-bounds read should return 0");

            // Read from FIFO range (not DRAM)
            let value = sim.read_word(0x4000_0000);
            assert_eq!(value, 0, "Out-of-bounds read should return 0");

            // Read spanning the end of DRAM
            let value = sim.read_word(DRAM_END - 2); // Would span beyond DRAM_END
            assert_eq!(value, 0, "Out-of-bounds read should return 0");
        }),
    );

    assert!(result.is_ok());
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
        100,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            let instructions = vec![
                addi(10, 0, 1),
                lui(11, 0x10000000),
                sw(11, 10, 0),
            ];
            let program = common::instructions_to_bytes(&instructions);
            sim.write_memory_region(DRAM_START, &program, true);
            Ok(DRAM_START)
        },
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            // Dump from below DRAM range (should return all zeros and log warning)
            let bytes: Vec<u8> = sim.dump_memory_region(0x0000_0000, 16).collect();
            assert_eq!(bytes.len(), 16);
            assert!(bytes.iter().all(|&b| b == 0), "Out-of-bounds dump should return zeros");

            // Dump spanning beyond DRAM_END (should return zeros and log warning)
            let bytes: Vec<u8> = sim.dump_memory_region(DRAM_END - 4, 16).collect();
            assert_eq!(bytes.len(), 16);
            assert!(bytes.iter().all(|&b| b == 0), "Out-of-bounds dump should return zeros");
        }),
    );

    assert!(result.is_ok());
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
        100,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            // Write test data to DRAM
            sim.write_memory_region(DRAM_START + 0x1000, &test_data, false);

            let instructions = vec![
                addi(10, 0, 1),
                lui(11, 0x10000000),
                sw(11, 10, 0),
            ];
            let program = common::instructions_to_bytes(&instructions);
            sim.write_memory_region(DRAM_START, &program, true);
            Ok(DRAM_START)
        },
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            // Verify we can read the data back
            let byte = sim.read_byte(DRAM_START + 0x1000);
            assert_eq!(byte, 0xAA, "Valid read should return correct value");

            let halfword = sim.read_halfword(DRAM_START + 0x1000);
            assert_eq!(halfword, 0xBBAA, "Valid read should return correct value (little-endian)");

            let word = sim.read_word(DRAM_START + 0x1000);
            assert_eq!(word, 0xDDCCBBAA, "Valid read should return correct value (little-endian)");

            // Verify dump_memory_region
            let bytes: Vec<u8> = sim.dump_memory_region(DRAM_START + 0x1000, 4).collect();
            assert_eq!(bytes, test_data, "Valid dump should return correct data");
        }),
    );

    assert!(result.is_ok());
}

/// Test that boundary condition at DRAM_START works correctly
#[test]
fn test_boundary_at_dram_start() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .is_test(true)
        .try_init()
        .ok();

    let result = run_program(
        100,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            let instructions = vec![
                addi(10, 0, 1),
                lui(11, 0x10000000),
                sw(11, 10, 0),
            ];
            let program = common::instructions_to_bytes(&instructions);
            sim.write_memory_region(DRAM_START, &program, true);
            Ok(DRAM_START)
        },
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            // Read at exactly DRAM_START (should succeed)
            let value = sim.read_word(DRAM_START);
            assert_ne!(value, 0, "Read at DRAM_START should succeed");

            // Read just before DRAM_START (should fail)
            let value = sim.read_byte(DRAM_START - 1);
            assert_eq!(value, 0, "Read before DRAM_START should return 0");
        }),
    );

    assert!(result.is_ok());
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
        100,
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

            let instructions = vec![
                addi(10, 0, 1),
                lui(11, 0x10000000),
                sw(11, 10, 0),
            ];
            let program = common::instructions_to_bytes(&instructions);
            sim.write_memory_region(DRAM_START, &program, true);
            Ok(DRAM_START)
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

    assert!(result.is_ok());
}
#[test]
fn test_write_memory_below_dram_range_debug() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .is_test(true)
        .try_init()
        .ok();

    use cpu_sim::*;
    use riscv_core::instruction::*;
    
    const DRAM_START: u32 = 0x8000_0000;
    
    let instructions = vec![0x13, 0x00, 0x00, 0x00]; // nop

    let result = run_program(
        100,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            // Try to write to an address below DRAM range (should be rejected)
            sim.write_memory_region(0x0000_0000, &instructions, true);

            // Write valid program to DRAM
            sim.write_memory_region(DRAM_START, &instructions, true);
            Ok(DRAM_START)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    );

    match &result {
        Ok(_) => println!("SUCCESS"),
        Err(e) => println!("ERROR: {}", e),
    }
}
