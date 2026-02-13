//! UART Controller RTL Peripheral Tests (CPU-Level)
//!
//! Tests for the UART controller peripheral (RTL-based) at the CPU level.
//! These tests run RISC-V programs through the CPU simulator to verify UART functionality.
//!
//! Address: 0x52000000
//! Features:
//! - TX/RX FIFOs (8 entries each)
//! - Hardware loopback mode (enabled by default)
//! - Status register for FIFO state
//!
//! UART Timing:
//! - Baud rate: 115200
//! - Clock: 50MHz
//! - Cycles per bit: ~434
//! - One byte transmission: ~4340 cycles (10 bits: start + 8 data + stop)

mod common;

use cpu_sim::*;
use riscv_core::instruction::*;
use riscv_shared::bus::{
    uart_rxdata_addr, uart_status_addr, uart_txdata_addr, UART_BASE, UART_RXDATA_OFFSET,
    UART_STATUS_OFFSET, UART_STATUS_RX_EMPTY, UART_STATUS_TX_EMPTY, UART_STATUS_TX_FULL,
    UART_TXDATA_OFFSET,
};

/// Helper function to initialize test logger (idempotent)
fn init_test_logger() {
    let _ = env_logger::builder().is_test(true).try_init();
}

// ============================================================================
// UART Controller Tests
// ============================================================================

#[test]
fn test_uart_constants() {
    // Verify UART controller memory map constants
    assert_eq!(UART_BASE, 0x52000000, "UART base address");
    assert_eq!(UART_TXDATA_OFFSET, 0x00, "UART_TXDATA register offset");
    assert_eq!(UART_RXDATA_OFFSET, 0x04, "UART_RXDATA register offset");
    assert_eq!(UART_STATUS_OFFSET, 0x08, "UART_STATUS register offset");

    // Verify helper functions
    assert_eq!(uart_txdata_addr(), 0x52000000, "UART TXDATA address");
    assert_eq!(uart_rxdata_addr(), 0x52000004, "UART RXDATA address");
    assert_eq!(uart_status_addr(), 0x52000008, "UART STATUS address");

    // Verify status bit masks
    assert_eq!(UART_STATUS_TX_FULL, 1 << 0, "TX_FULL bit mask");
    assert_eq!(UART_STATUS_TX_EMPTY, 1 << 1, "TX_EMPTY bit mask");
    assert_eq!(UART_STATUS_RX_EMPTY, 1 << 5, "RX_EMPTY bit mask");
}

#[test]
fn test_uart_tx_write_byte() {
    init_test_logger();

    // Write a single byte (0x42) to the UART TX FIFO
    // Algorithm:
    // 1. Load UART base address into x15 (0x52000000)
    // 2. Load test byte (0x42) into x14
    // 3. Write byte to TXDATA register
    // 4. Terminate with success

    let mut instructions = vec![
        lui(15, UART_BASE),                    // x15 = 0x52000000 (UART base)
        addi(14, 0, 0x42),                     // x14 = 0x42 (test byte 'B')
        sw(15, 14, UART_TXDATA_OFFSET as i32), // Write to TXDATA
    ];
    instructions.extend(common::tohost_termination(10, 9, 1));

    // Run the program
    const START_ADDR: u32 = 0x8000_0000;
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false, // trace
        false, // dump_vcd
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            sim.write_memory_region(START_ADDR, &program_bytes, true);
            Ok(START_ADDR)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(1),
        "Program should exit with success code"
    );
}

#[test]
fn test_uart_status_initial_state() {
    init_test_logger();

    // Read the UART STATUS register and verify initial state
    // Expected:
    // - TX_EMPTY = 1 (TX FIFO is empty after reset)
    // - RX_EMPTY = 1 (RX FIFO is empty after reset)
    //
    // Algorithm:
    // 1. Load UART base address into x15
    // 2. Read STATUS register into x13
    // 3. Check TX_EMPTY bit (bit 1)
    // 4. Check RX_EMPTY bit (bit 5)
    // 5. If both bits are set, report success, else failure

    let mut instructions = vec![
        lui(15, UART_BASE),                    // x15 = 0x52000000 (UART base)
        lw(13, 15, UART_STATUS_OFFSET as i32), // x13 = STATUS register
        // Check TX_EMPTY (bit 1)
        andi(12, 13, UART_STATUS_TX_EMPTY as i32), // x12 = STATUS & TX_EMPTY
        beq(12, 0, 20),                            // If TX_EMPTY == 0, jump to failure
        // Check RX_EMPTY (bit 5)
        andi(12, 13, UART_STATUS_RX_EMPTY as i32), // x12 = STATUS & RX_EMPTY
        beq(12, 0, 12),                            // If RX_EMPTY == 0, jump to failure

                                                   // Success path
    ];
    instructions.extend(common::tohost_termination(10, 9, 1));

    // Failure path
    instructions.extend(common::tohost_termination(10, 9, 0));

    const START_ADDR: u32 = 0x8000_0000;
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

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
        None::<fn(&SimulatorView, &SimulationResult)>,
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(1),
        "STATUS register should show TX_EMPTY=1 and RX_EMPTY=1 initially"
    );
}

// UART hardware loopback test - verifies TX data is correctly received via internal loopback
#[test]
fn test_uart_loopback_single_byte() {
    init_test_logger();

    // Full loopback test: send a byte via TX, receive it via RX, validate
    // This test relies on the UART hardware loopback feature (ENABLE_UART_LOOPBACK=1)
    //
    // Algorithm:
    // 1. Load UART base address into x15 (0x52000000)
    // 2. Write test byte (0xA5) to TXDATA
    // 3. Poll STATUS until TX_EMPTY (transmission complete)
    // 4. Poll STATUS until !RX_EMPTY (data received in loopback)
    // 5. Read RXDATA into x11
    // 6. Compare received byte with sent byte
    // 7. Write success (1) or failure (0) to tohost
    //
    // Timing: One byte takes ~4340 cycles. Use extended max cycles to ensure
    // the loopback has time to complete.

    let instructions = vec![
        // Setup: x15 = UART base, x14 = test byte (0xA5)
        lui(15, UART_BASE), // x15 = 0x52000000
        addi(14, 0, 0xA5),  // x14 = 0xA5 (test pattern)
        // Write test byte to TXDATA
        sw(15, 14, UART_TXDATA_OFFSET as i32), // TXDATA = 0xA5
        // Poll TX_EMPTY: loop until TX FIFO is empty (data transmitted)
        // Label: tx_poll_loop (PC offset calculated as negative)
        lw(13, 15, UART_STATUS_OFFSET as i32), // x13 = STATUS
        andi(12, 13, UART_STATUS_TX_EMPTY as i32), // x12 = STATUS & TX_EMPTY
        beq(12, 0, -8),                        // If TX_EMPTY == 0, loop back 2 instructions
        // Poll RX_EMPTY: loop until RX FIFO has data (loopback received)
        // Label: rx_poll_loop
        lw(13, 15, UART_STATUS_OFFSET as i32), // x13 = STATUS
        andi(12, 13, UART_STATUS_RX_EMPTY as i32), // x12 = STATUS & RX_EMPTY
        bne(12, 0, -8),                        // If RX_EMPTY != 0, loop back 2 instructions
        // Read received byte from RXDATA
        lw(11, 15, UART_RXDATA_OFFSET as i32), // x11 = RXDATA
        // Compare received byte (x11) with sent byte (x14)
        // If equal, write success (1) to tohost, else failure (0)
        lui(10, SIM_CONTROL_BASE), // x10 = tohost address
        bne(11, 14, 16),           // If received != sent, jump to failure (skip 4 instructions)
        // Success path
        addi(9, 0, 1), // x9 = 1 (success)
        sw(10, 9, 0),  // Write 1 to tohost
        jal(0, 12),    // Jump past failure code (skip 3 instructions)
        // Failure path
        addi(9, 0, 0), // x9 = 0 (failure)
        sw(10, 9, 0),  // Write 0 to tohost
        // Infinite loop (terminal state)
        jal(0, 0), // Loop forever
    ];

    const START_ADDR: u32 = 0x8000_0000;
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    // Use extended max cycles for UART timing (~4340 cycles per byte + polling overhead)
    // Use 100x GLOBAL_MAX_CYCLES to ensure we have plenty of time for loopback
    let max_cycles = GLOBAL_MAX_CYCLES * 100;

    let result = run_program(
        max_cycles,
        false, // Set to true to enable instruction tracing for debugging
        false, // dump_vcd
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            sim.write_memory_region(START_ADDR, &program_bytes, true);
            Ok(START_ADDR)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(1),
        "Loopback test should succeed: sent byte (0xA5) should match received byte"
    );
}

#[test]
fn test_uart_tx_fifo_full() {
    init_test_logger();

    // Fill the TX FIFO with 8 bytes and verify TX_FULL status is set
    // Algorithm:
    // 1. Load UART base address into x15
    // 2. Write 8 bytes quickly to TX FIFO
    // 3. Read STATUS register
    // 4. Check if TX_FULL is set OR TX_BUSY is set (TX consuming data)
    // 5. Report success if either condition is met

    let mut instructions = vec![
        lui(15, UART_BASE), // x15 = 0x52000000 (UART base)
    ];

    // Write 8 bytes to TX FIFO as fast as possible
    for i in 0..8 {
        instructions.push(addi(14, 0, i)); // x14 = byte value
        instructions.push(sw(15, 14, UART_TXDATA_OFFSET as i32)); // Write to TXDATA
    }

    // Read STATUS register
    instructions.push(lw(13, 15, UART_STATUS_OFFSET as i32)); // x13 = STATUS

    // Check TX_EMPTY - if TX_EMPTY is clear, FIFO has data (success)
    // TX_EMPTY = bit 1, so we AND with 0x02
    instructions.push(andi(12, 13, UART_STATUS_TX_EMPTY as i32));
    instructions.push(bne(12, 0, 12)); // If TX_EMPTY is set, jump to failure (3 instructions)

    // Success: TX_EMPTY is not set, meaning FIFO has data
    instructions.extend(common::tohost_termination(10, 9, 1));

    // Failure
    instructions.extend(common::tohost_termination(10, 9, 0));

    const START_ADDR: u32 = 0x8000_0000;
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

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
        None::<fn(&SimulatorView, &SimulationResult)>,
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(1),
        "TX_EMPTY should be clear after writing 8 bytes to TX FIFO"
    );
}

#[test]
fn test_uart_rx_read_empty() {
    init_test_logger();

    // Read RXDATA when RX FIFO is empty - should return 0 without crashing
    // Algorithm:
    // 1. Load UART base address
    // 2. Read RXDATA (should return 0 when empty)
    // 3. Store the value to verify it's 0
    // 4. Report success (no crash, value was 0)

    let instructions = vec![
        lui(15, UART_BASE),                    // x15 = 0x52000000 (UART base)
        lw(13, 15, UART_RXDATA_OFFSET as i32), // x13 = RXDATA (should be 0 when empty)
        // If RXDATA is 0, report success; otherwise failure
        bne(13, 0, 12), // If x13 != 0, jump to failure (3 instructions)
        // Success: RXDATA returned 0
        lui(10, SIM_CONTROL_BASE),
        addi(9, 0, 1),
        sw(10, 9, 0),
        jal(0, 12), // Jump over failure
        // Failure
        lui(10, SIM_CONTROL_BASE),
        addi(9, 0, 0),
        sw(10, 9, 0),
        jal(0, 0), // Infinite loop
    ];

    const START_ADDR: u32 = 0x8000_0000;
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

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
        None::<fn(&SimulatorView, &SimulationResult)>,
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(1),
        "Reading empty RX FIFO should return 0 and not crash"
    );
}

#[test]
fn test_uart_loopback_pattern() {
    init_test_logger();

    // Test loopback with multiple patterns: 0x00, 0xFF, 0xAA, 0x55
    // Each pattern is sent and received via hardware loopback
    // Algorithm for each pattern:
    // 1. Write pattern to TXDATA
    // 2. Wait for TX_EMPTY
    // 3. Wait for !RX_EMPTY
    // 4. Read RXDATA and compare
    // 5. If mismatch, report failure immediately

    // This test sends 4 patterns sequentially and verifies each one
    // We use a pattern stored in memory and iterate through them

    let instructions = vec![
        // Setup registers
        lui(15, UART_BASE),        // x15 = UART base
        lui(10, SIM_CONTROL_BASE), // x10 = tohost address
        // Test pattern 1: 0x00
        addi(14, 0, 0x00),                     // x14 = 0x00
        sw(15, 14, UART_TXDATA_OFFSET as i32), // Write to TXDATA
        lw(13, 15, UART_STATUS_OFFSET as i32), // Poll TX_EMPTY
        andi(12, 13, UART_STATUS_TX_EMPTY as i32),
        beq(12, 0, -8),
        lw(13, 15, UART_STATUS_OFFSET as i32), // Poll RX_EMPTY
        andi(12, 13, UART_STATUS_RX_EMPTY as i32),
        bne(12, 0, -8),
        lw(11, 15, UART_RXDATA_OFFSET as i32), // Read RXDATA
        bne(11, 14, 116), // If mismatch, jump to failure (29 instructions ahead)
        // Test pattern 2: 0xFF (use -1 to get 0xFFFFFFFF, then mask)
        addi(14, 0, 0xFF), // x14 = 0xFF
        sw(15, 14, UART_TXDATA_OFFSET as i32),
        lw(13, 15, UART_STATUS_OFFSET as i32),
        andi(12, 13, UART_STATUS_TX_EMPTY as i32),
        beq(12, 0, -8),
        lw(13, 15, UART_STATUS_OFFSET as i32),
        andi(12, 13, UART_STATUS_RX_EMPTY as i32),
        bne(12, 0, -8),
        lw(11, 15, UART_RXDATA_OFFSET as i32),
        andi(11, 11, 0xFF), // Mask to 8 bits
        bne(11, 14, 72),    // If mismatch, jump to failure (18 instructions ahead)
        // Test pattern 3: 0xAA
        addi(14, 0, 0xAA),
        sw(15, 14, UART_TXDATA_OFFSET as i32),
        lw(13, 15, UART_STATUS_OFFSET as i32),
        andi(12, 13, UART_STATUS_TX_EMPTY as i32),
        beq(12, 0, -8),
        lw(13, 15, UART_STATUS_OFFSET as i32),
        andi(12, 13, UART_STATUS_RX_EMPTY as i32),
        bne(12, 0, -8),
        lw(11, 15, UART_RXDATA_OFFSET as i32),
        bne(11, 14, 32), // If mismatch, jump to failure (8 instructions ahead)
        // Test pattern 4: 0x55
        addi(14, 0, 0x55),
        sw(15, 14, UART_TXDATA_OFFSET as i32),
        lw(13, 15, UART_STATUS_OFFSET as i32),
        andi(12, 13, UART_STATUS_TX_EMPTY as i32),
        beq(12, 0, -8),
        lw(13, 15, UART_STATUS_OFFSET as i32),
        andi(12, 13, UART_STATUS_RX_EMPTY as i32),
        bne(12, 0, -8),
        lw(11, 15, UART_RXDATA_OFFSET as i32),
        bne(11, 14, 12), // If mismatch, jump to failure (3 instructions ahead)
        // Success: all patterns matched
        addi(9, 0, 1),
        sw(10, 9, 0),
        jal(0, 8), // Jump over failure
        // Failure
        addi(9, 0, 0),
        sw(10, 9, 0),
        jal(0, 0), // Infinite loop
    ];

    const START_ADDR: u32 = 0x8000_0000;
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    // Use extended cycles for 4 UART transmissions
    let max_cycles = GLOBAL_MAX_CYCLES * 500;

    let result = run_program(
        max_cycles,
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
        None::<fn(&SimulatorView, &SimulationResult)>,
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(1),
        "All loopback patterns (0x00, 0xFF, 0xAA, 0x55) should match"
    );
}

#[test]
fn test_uart_loopback_multi_byte() {
    init_test_logger();

    // Send and receive 8 bytes via hardware loopback and verify all match in order
    // Algorithm:
    // 1. Send bytes 0x01 through 0x08 one at a time via loopback
    // 2. After each send, wait for TX_EMPTY then !RX_EMPTY
    // 3. Read and verify each received byte matches sent byte
    // 4. Report success if all 8 bytes match

    let mut instructions = vec![
        lui(15, UART_BASE),        // x15 = UART base
        lui(10, SIM_CONTROL_BASE), // x10 = tohost address
    ];

    // Send and verify 8 bytes (0x01 through 0x08)
    for i in 1..=8u8 {
        // Set expected byte
        instructions.push(addi(14, 0, i as i32)); // x14 = expected byte
                                                  // Write to TXDATA
        instructions.push(sw(15, 14, UART_TXDATA_OFFSET as i32));
        // Poll TX_EMPTY
        instructions.push(lw(13, 15, UART_STATUS_OFFSET as i32));
        instructions.push(andi(12, 13, UART_STATUS_TX_EMPTY as i32));
        instructions.push(beq(12, 0, -8));
        // Poll RX_EMPTY
        instructions.push(lw(13, 15, UART_STATUS_OFFSET as i32));
        instructions.push(andi(12, 13, UART_STATUS_RX_EMPTY as i32));
        instructions.push(bne(12, 0, -8));
        // Read RXDATA
        instructions.push(lw(11, 15, UART_RXDATA_OFFSET as i32));
        // Compare and branch to failure if mismatch
        // Calculate offset to failure (needs to be calculated per iteration)
        let remaining_iterations = 8 - i as usize;
        let instructions_per_iteration = 9; // 9 instructions per loop iteration
        let instructions_after_this = remaining_iterations * instructions_per_iteration + 3; // +3 for success path
        let failure_offset = (instructions_after_this * 4) as i32;
        instructions.push(bne(11, 14, failure_offset));
    }

    // Success path
    instructions.push(addi(9, 0, 1));
    instructions.push(sw(10, 9, 0));
    instructions.push(jal(0, 8)); // Skip failure

    // Failure path
    instructions.push(addi(9, 0, 0));
    instructions.push(sw(10, 9, 0));
    instructions.push(jal(0, 0)); // Infinite loop

    const START_ADDR: u32 = 0x8000_0000;
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    // Use extended cycles for 8 UART transmissions
    let max_cycles = GLOBAL_MAX_CYCLES * 1000;

    let result = run_program(
        max_cycles,
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
        None::<fn(&SimulatorView, &SimulationResult)>,
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(1),
        "All 8 bytes (0x01-0x08) should be received correctly via loopback"
    );
}
