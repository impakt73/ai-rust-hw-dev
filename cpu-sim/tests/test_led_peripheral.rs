//! LED Controller RTL Peripheral Tests
//!
//! Tests for the LED controller peripheral (RTL-based).
//! Address: 0x50000000
//! Features: 8-bit output register

use cpu_sim::*;
use device_runtime::SIM_CONTROL_BASE;
use riscv_core::instruction::*;
use riscv_shared::bus::{LED_BASE, LED_OUT_OFFSET, LED_SIZE};

/// Helper function to initialize test logger (idempotent)
fn init_test_logger() {
    let _ = env_logger::builder().is_test(true).try_init();
}

/// Generate tohost termination sequence
fn tohost_termination(addr_reg: u32, value_reg: u32) -> Vec<u32> {
    vec![
        lui(addr_reg, SIM_CONTROL_BASE), // Load SIM_CONTROL_BASE into addr_reg
        addi(value_reg, 0, 1),           // Load success code (1)
        sw(addr_reg, value_reg, 0),      // Store value to tohost address
        jal(0, 0),                       // Infinite loop (jump to self)
    ]
}

// ============================================================================
// LED Controller Tests
// ============================================================================

#[test]
fn test_led_constants() {
    // Verify LED controller memory map constants
    assert_eq!(LED_BASE, 0x50000000, "LED base address");
    assert_eq!(LED_OUT_OFFSET, 0x00, "LED_OUT register offset");
    assert_eq!(LED_SIZE, 0x10, "LED controller size");
}

#[test]
fn test_led_basic_write_word() {
    init_test_logger();

    // Write 0xAA to LED_OUT register using word access
    // x15 = LED base address
    // x14 = value to write (0xAA)
    let mut instructions = vec![
        lui(15, LED_BASE), // Load LED base address
        addi(14, 0, 0xAA), // Load value 0xAA
        sw(15, 14, 0),     // Write to LED_OUT (offset 0)
    ];
    instructions.extend(tohost_termination(7, 8));

    // Run using the standard run_program helper with custom termination callback
    const START_ADDR: u32 = 0x8000_0000;
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    let led_value = std::sync::Arc::new(std::sync::Mutex::new(0u8));
    let led_value_clone = led_value.clone();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            sim.write_memory_region(START_ADDR, &program_bytes, true);
            Ok(START_ADDR)
        },
        Some(move |sim: &SimulatorView, _result: &SimulationResult| {
            // Read LED output after program completion
            *led_value_clone.lock().unwrap() = sim.led_out();
        }),
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(1),
        "Program should exit with success code"
    );

    // Verify LED output value
    let final_led_value = *led_value.lock().unwrap();
    assert_eq!(final_led_value, 0xAA, "LED output should be 0xAA");
}

#[test]
fn test_led_byte_access() {
    init_test_logger();

    // Write 0x55 to LED_OUT register using byte access
    let mut instructions = vec![
        lui(15, LED_BASE), // Load LED base address
        addi(14, 0, 0x55), // Load value 0x55
        sb(15, 14, 0),     // Store byte to LED_OUT
    ];
    instructions.extend(tohost_termination(7, 8));

    const START_ADDR: u32 = 0x8000_0000;
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    let led_value = std::sync::Arc::new(std::sync::Mutex::new(0u8));
    let led_value_clone = led_value.clone();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            sim.write_memory_region(START_ADDR, &program_bytes, true);
            Ok(START_ADDR)
        },
        Some(move |sim: &SimulatorView, _result: &SimulationResult| {
            *led_value_clone.lock().unwrap() = sim.led_out();
        }),
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(1),
        "Program should exit with success code"
    );

    // Verify LED output value
    let final_led_value = *led_value.lock().unwrap();
    assert_eq!(final_led_value, 0x55, "LED output should be 0x55");
}

#[test]
fn test_led_halfword_access() {
    init_test_logger();

    // Write 0x00FF to LED_OUT register using halfword access
    let mut instructions = vec![
        lui(15, LED_BASE), // Load LED base address
        addi(14, 0, 0xFF), // Load value 0xFF
        sh(15, 14, 0),     // Store halfword to LED_OUT
    ];
    instructions.extend(tohost_termination(7, 8));

    const START_ADDR: u32 = 0x8000_0000;
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    let led_value = std::sync::Arc::new(std::sync::Mutex::new(0u8));
    let led_value_clone = led_value.clone();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            sim.write_memory_region(START_ADDR, &program_bytes, true);
            Ok(START_ADDR)
        },
        Some(move |sim: &SimulatorView, _result: &SimulationResult| {
            *led_value_clone.lock().unwrap() = sim.led_out();
        }),
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(1),
        "Program should exit with success code"
    );

    // Verify LED output value
    let final_led_value = *led_value.lock().unwrap();
    assert_eq!(final_led_value, 0xFF, "LED output should be 0xFF");
}

#[test]
fn test_led_read_back() {
    init_test_logger();

    // Write to LED, then read back to verify
    // This tests that LED_OUT is readable
    let mut instructions = vec![
        lui(15, LED_BASE), // Load LED base address
        addi(14, 0, 0xCC), // Load value 0xCC
        sw(15, 14, 0),     // Write to LED_OUT
        lw(13, 15, 0),     // Read back from LED_OUT into x13
        // Verify the read value matches (checking lower 8 bits)
        andi(13, 13, 0xFF), // Mask to lower 8 bits
        addi(12, 0, 0xCC),  // Expected value
                            // If equal, write 1 to tohost, else write 0
    ];
    instructions.extend(tohost_termination(7, 8));

    const START_ADDR: u32 = 0x8000_0000;
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    let led_value = std::sync::Arc::new(std::sync::Mutex::new(0u8));
    let led_value_clone = led_value.clone();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            sim.write_memory_region(START_ADDR, &program_bytes, true);
            Ok(START_ADDR)
        },
        Some(move |sim: &SimulatorView, _result: &SimulationResult| {
            *led_value_clone.lock().unwrap() = sim.led_out();
        }),
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(1),
        "Program should exit with success code"
    );

    // Verify LED output value
    let final_led_value = *led_value.lock().unwrap();
    assert_eq!(final_led_value, 0xCC, "LED output should be 0xCC");
}

#[test]
fn test_led_pattern_sequence() {
    init_test_logger();

    // Write a sequence of different patterns to LED
    let mut instructions = vec![
        lui(15, LED_BASE), // Load LED base address
        // Pattern 1: 0x00 (all off)
        addi(14, 0, 0x00),
        sw(15, 14, 0),
        // Pattern 2: 0xFF (all on)
        addi(14, 0, 0xFF),
        sw(15, 14, 0),
        // Pattern 3: 0xAA (alternating)
        addi(14, 0, 0xAA),
        sw(15, 14, 0),
        // Pattern 4: 0x55 (alternating opposite)
        addi(14, 0, 0x55),
        sw(15, 14, 0),
    ];
    instructions.extend(tohost_termination(7, 8));

    const START_ADDR: u32 = 0x8000_0000;
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    let led_value = std::sync::Arc::new(std::sync::Mutex::new(0u8));
    let led_value_clone = led_value.clone();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            sim.write_memory_region(START_ADDR, &program_bytes, true);
            Ok(START_ADDR)
        },
        Some(move |sim: &SimulatorView, _result: &SimulationResult| {
            *led_value_clone.lock().unwrap() = sim.led_out();
        }),
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(1),
        "Program should exit with success code"
    );

    // Verify final LED output value is 0x55 (last pattern written)
    let final_led_value = *led_value.lock().unwrap();
    assert_eq!(
        final_led_value, 0x55,
        "LED output should be 0x55 (last pattern)"
    );
}

#[test]
fn test_led_upper_bits_ignored() {
    init_test_logger();

    // Write 0xFFFFFFAA to LED_OUT - upper 24 bits should be ignored
    let mut instructions = vec![
        lui(15, LED_BASE),   // Load LED base address
        lui(14, 0xFFFFF000), // Load 0xFFFFF000
        ori(14, 14, 0xAA),   // OR with 0xAA -> 0xFFFFFFAA
        sw(15, 14, 0),       // Write to LED_OUT
        lw(13, 15, 0),       // Read back
        // Verify only lower 8 bits are set
        andi(13, 13, 0xFF), // Mask to lower 8 bits
        addi(12, 0, 0xAA),  // Expected value
    ];
    instructions.extend(tohost_termination(7, 8));

    const START_ADDR: u32 = 0x8000_0000;
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    let led_value = std::sync::Arc::new(std::sync::Mutex::new(0u8));
    let led_value_clone = led_value.clone();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            sim.write_memory_region(START_ADDR, &program_bytes, true);
            Ok(START_ADDR)
        },
        Some(move |sim: &SimulatorView, _result: &SimulationResult| {
            *led_value_clone.lock().unwrap() = sim.led_out();
        }),
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(1),
        "Program should exit with success code"
    );

    // Verify LED output value (only lower 8 bits should be set)
    let final_led_value = *led_value.lock().unwrap();
    assert_eq!(
        final_led_value, 0xAA,
        "LED output should be 0xAA (upper bits ignored)"
    );
}
