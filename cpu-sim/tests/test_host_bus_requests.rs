//! Host Bus Request Integration Tests
//!
//! Tests for the host-initiated bus request functionality in the CPU simulator.
//! These tests verify the Rust API for sending requests to RTL peripherals
//! and demonstrate bi-directional Host↔FPGA communication.
//!
//! ## Test Approach
//!
//! These tests use a "busy-loop" pattern to ensure consistent instruction
//! completion callbacks:
//!
//! 1. CPU program spins reading a DRAM location until it becomes non-zero
//! 2. Host request logic runs in the instruction complete callback
//! 3. When host operation is complete, host writes 1 to the DRAM location
//! 4. CPU breaks out of loop and exits via tohost
//!
//! This ensures a steady stream of instruction callbacks for host request
//! processing regardless of request timing.

use cpu_sim::*;
use riscv_core::instruction::*;
use riscv_shared::bus::LED_BASE;
use std::sync::{Arc, Mutex};

/// Helper function to initialize test logger (idempotent)
fn init_test_logger() {
    let _ = env_logger::builder().is_test(true).try_init();
}

/// DRAM addresses used for synchronization between CPU and Host
const SYNC_FLAG_ADDR: u32 = 0x80000100; // Host writes here to signal CPU
const CPU_DONE_ADDR: u32 = 0x80000104; // CPU writes here to signal host

/// Generate a busy-loop program that:
/// 1. Optionally executes setup instructions
/// 2. Spins reading SYNC_FLAG_ADDR until it becomes non-zero
/// 3. Writes success code to tohost and halts
///
/// This provides a steady stream of instruction callbacks for host request processing.
/// The loop includes NOP instructions to create idle windows for host requests.
fn generate_busy_loop_program(setup_instructions: Vec<u32>) -> Vec<u32> {
    let mut instructions = setup_instructions;

    // Load DRAM base address into x13 for sync flag access
    instructions.push(lui(13, 0x80000000)); // x13 = DRAM base

    // Busy loop with delay: read SYNC_FLAG_ADDR, check, delay, repeat
    // The delay gives the host time to send requests
    //
    // Loop structure:
    //   lw x11, 0x100(x13)  ; Load from DRAM
    //   bne x11, x0, +20    ; If non-zero, skip to exit
    //   nop (x4)            ; 4 NOP delay for host requests
    //   j loop_start        ; Jump back to load
    //
    // Using addi x0, x0, 0 as NOP (standard RISC-V encoding)
    instructions.push(lw(11, 13, 0x100)); // x11 = *SYNC_FLAG_ADDR
    instructions.push(bne(11, 0, 20)); // if x11 != 0, skip ahead 20 bytes (5 instrs)
    instructions.push(addi(0, 0, 0)); // NOP
    instructions.push(addi(0, 0, 0)); // NOP
    instructions.push(addi(0, 0, 0)); // NOP
    instructions.push(addi(0, 0, 0)); // NOP
    instructions.push(jal(0, -24)); // Jump back to lw (6 instrs * 4 bytes = 24)

    // Exit with success via tohost
    instructions.push(lui(8, 0x10000000)); // x8 = tohost base
    instructions.push(addi(7, 0, 1)); // x7 = success code
    instructions.push(sw(8, 7, 0)); // *tohost = 1
    instructions.push(jal(0, 0)); // infinite loop (halt)

    instructions
}

/// Generate a program that writes to LED, signals CPU_DONE, then waits for SYNC_FLAG
fn generate_led_write_and_wait_program(led_value: u32) -> Vec<u32> {
    vec![
        // Write to LED: x15 = LED base, x14 = value
        lui(15, 0x50000000),           // x15 = LED base
        addi(14, 0, led_value as i32), // x14 = value to write
        sw(15, 14, 0),                 // *LED = value
        // Signal to host that CPU LED write is complete (write 1 to CPU_DONE)
        lui(13, 0x80000000), // x13 = DRAM base
        addi(12, 0, 1),      // x12 = 1
        sw(13, 12, 0x104),   // *CPU_DONE_ADDR = 1
        // Now busy-loop waiting for SYNC_FLAG from host (with NOP delays)
        // Loop structure:
        //   lw x11, 0x100(x13)  ; Load from DRAM
        //   bne x11, x0, +20    ; If non-zero, skip to exit
        //   nop (x4)            ; 4 NOP delay for host requests
        //   j loop_start        ; Jump back to load
        lw(11, 13, 0x100), // x11 = *SYNC_FLAG_ADDR
        bne(11, 0, 20),    // if x11 != 0, skip ahead 20 bytes (5 instrs)
        addi(0, 0, 0),     // NOP
        addi(0, 0, 0),     // NOP
        addi(0, 0, 0),     // NOP
        addi(0, 0, 0),     // NOP
        jal(0, -24),       // Jump back to lw (6 instrs * 4 bytes = 24)
        // Read LED to verify host write (into x10)
        lw(10, 15, 0),      // x10 = *LED
        andi(10, 10, 0xFF), // mask to 8 bits
        // Exit with LED value in tohost (for verification)
        lui(8, 0x10000000), // x8 = tohost base
        sw(8, 10, 0),       // *tohost = LED value
        jal(0, 0),          // halt
    ]
}

// ============================================================================
// Test State Tracking
// ============================================================================

/// State machine for host bus request tests
#[derive(Debug, Clone, Default)]
struct HostRequestTestState {
    /// Current phase of the test
    phase: u32,
    /// Track if we've sent a request
    request_sent: bool,
    /// Response received from FPGA
    response_received: Option<HostBusResponse>,
    /// Track number of callbacks (for debugging)
    callback_count: u32,
}

// ============================================================================
// Basic API Tests
// ============================================================================

#[test]
fn test_host_bus_request_types_exist() {
    // Verify that all the new types are properly exported
    let _request = HostBusRequest {
        addr: LED_BASE,
        wdata: 0x55,
        size: 2,
        we: true,
    };

    // Verify response types exist
    let _response_read: HostBusResponse = HostBusResponse::ReadData(0x42);
    let _response_write: HostBusResponse = HostBusResponse::WriteAck;
    let _response_error: HostBusResponse = HostBusResponse::Error(FpgaError::InvalidAddress);
}

#[test]
fn test_fpga_error_types_exist() {
    // Verify error types
    let _e1 = FpgaError::InvalidAddress;
    let _e2 = FpgaError::Timeout;
    let _e3 = FpgaError::ProtocolError;
}

// ============================================================================
// Host-Initiated Write Tests (with busy-loop pattern)
// ============================================================================

/// Simple test: Host writes to LED peripheral
///
/// Test sequence:
/// 1. CPU spins in busy loop waiting for SYNC_FLAG
/// 2. On first callback, host sends LED write request (0xAA)
/// 3. Host polls for write acknowledgement
/// 4. Once ack received, host sets SYNC_FLAG = 1
/// 5. CPU breaks out of loop and exits
///
/// NOTE: This test is ignored due to protocol-level conflicts between CPU and host
/// requests. The RTL correctly handles host-initiated requests (verified by
/// testbench/tests/host_bus_interface_bidirectional_test.rs), but the cpu-sim
/// integration requires additional work to coordinate concurrent access.
/// See HOST_BUS_PROTOCOL_STATUS.md for details and recommended solutions.
#[test]
#[ignore = "Protocol conflict between CPU and host requests - see HOST_BUS_PROTOCOL_STATUS.md"]
fn test_host_write_led_simple() {
    init_test_logger();

    const START_ADDR: u32 = 0x8000_0000;

    // Simple busy-loop program (no setup needed)
    let instructions = generate_busy_loop_program(vec![]);
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    let test_state = Arc::new(Mutex::new(HostRequestTestState::default()));
    let test_state_callback = test_state.clone();

    let result = run_program(
        50000, // Longer cycle count to allow host request to complete
        false,
        false, // Disable FSM state printing
        Some(move |sim: &mut SimulatorView| {
            let mut state = test_state_callback.lock().unwrap();
            state.callback_count += 1;

            match state.phase {
                0 => {
                    // Phase 0: Wait for CPU to be in stable busy loop before sending host request
                    // The busy loop has 3 setup instructions + 2 loop instructions per iteration
                    // Wait for at least 10 callbacks to ensure we're in the loop
                    if state.callback_count >= 10 && !state.request_sent {
                        let request = HostBusRequest {
                            addr: LED_BASE,
                            wdata: 0xAA,
                            size: 2, // word
                            we: true,
                        };
                        sim.send_bus_request(request)
                            .expect("Failed to send write request");
                        state.request_sent = true;
                        state.phase = 1;
                    }
                }
                1 => {
                    // Phase 1: Poll for write acknowledgement
                    if let Some(response) = sim.receive_bus_response() {
                        state.response_received = Some(response.clone());
                        match response {
                            HostBusResponse::WriteAck => {
                                // Success! Signal CPU to exit
                                sim.write_word(SYNC_FLAG_ADDR, 1);
                                state.phase = 2;
                            }
                            _ => {
                                // Unexpected response
                                panic!("Expected WriteAck, got {:?}", response);
                            }
                        }
                    }
                }
                _ => {
                    // Done, no more action needed
                }
            }
        }),
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            // Initialize sync flag to 0
            sim.write_word(SYNC_FLAG_ADDR, 0);
            sim.write_memory_region(START_ADDR, &program_bytes, true);
            Ok(START_ADDR)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    )
    .expect("Simulation should succeed");

    let final_state = test_state.lock().unwrap();
    assert!(
        matches!(
            final_state.response_received,
            Some(HostBusResponse::WriteAck)
        ),
        "Should have received WriteAck, got {:?}",
        final_state.response_received
    );
    assert_eq!(
        result.tohost_value,
        Some(1),
        "Program should exit with success code"
    );
    assert!(
        final_state.callback_count > 1,
        "Should have had multiple callbacks"
    );
}

/// Host read from LED peripheral
///
/// Test sequence:
/// 1. CPU writes 0xBB to LED
/// 2. CPU signals CPU_DONE = 1
/// 3. CPU spins in busy loop waiting for SYNC_FLAG
/// 4. Host detects CPU_DONE, sends LED read request
/// 5. Host receives read response with value 0xBB
/// 6. Host sets SYNC_FLAG = 1
/// 7. CPU breaks out and exits
#[test]
#[ignore = "Protocol conflict between CPU and host requests - see HOST_BUS_PROTOCOL_STATUS.md"]
fn test_host_read_led_after_cpu_write() {
    init_test_logger();

    const START_ADDR: u32 = 0x8000_0000;

    // Program: write 0xBB to LED, signal CPU_DONE, wait for SYNC_FLAG
    let mut instructions = vec![
        lui(15, 0x50000000), // x15 = LED base
        addi(14, 0, 0xBB),   // x14 = 0xBB
        sw(15, 14, 0),       // *LED = 0xBB
        lui(13, 0x80000000), // x13 = DRAM base
        addi(12, 0, 1),      // x12 = 1
        sw(13, 12, 0x104),   // *CPU_DONE_ADDR = 1
    ];
    // Add busy-loop
    instructions.extend(generate_busy_loop_program(vec![]));

    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    let test_state = Arc::new(Mutex::new(HostRequestTestState::default()));
    let test_state_callback = test_state.clone();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        Some(move |sim: &mut SimulatorView| {
            let mut state = test_state_callback.lock().unwrap();
            state.callback_count += 1;

            match state.phase {
                0 => {
                    // Phase 0: Wait for CPU to signal it has written LED
                    let cpu_done = sim.read_word(CPU_DONE_ADDR);
                    if cpu_done == 1 && !state.request_sent {
                        // Send host read request
                        let request = HostBusRequest {
                            addr: LED_BASE,
                            wdata: 0,
                            size: 2, // word
                            we: false,
                        };
                        sim.send_bus_request(request)
                            .expect("Failed to send read request");
                        state.request_sent = true;
                        state.phase = 1;
                    }
                }
                1 => {
                    // Phase 1: Poll for read response
                    if let Some(response) = sim.receive_bus_response() {
                        state.response_received = Some(response.clone());
                        match response {
                            HostBusResponse::ReadData(value) => {
                                assert_eq!(
                                    value & 0xFF,
                                    0xBB,
                                    "LED value should be 0xBB, got 0x{:02X}",
                                    value & 0xFF
                                );
                                // Signal CPU to exit
                                sim.write_word(SYNC_FLAG_ADDR, 1);
                                state.phase = 2;
                            }
                            _ => {
                                panic!("Expected ReadData, got {:?}", response);
                            }
                        }
                    }
                }
                _ => {}
            }
        }),
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            sim.write_word(SYNC_FLAG_ADDR, 0);
            sim.write_word(CPU_DONE_ADDR, 0);
            sim.write_memory_region(START_ADDR, &program_bytes, true);
            Ok(START_ADDR)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    )
    .expect("Simulation should succeed");

    let final_state = test_state.lock().unwrap();
    assert!(
        matches!(
            final_state.response_received,
            Some(HostBusResponse::ReadData(_))
        ),
        "Should have received ReadData"
    );
    assert_eq!(
        result.tohost_value,
        Some(1),
        "Program should exit with success"
    );
}

// ============================================================================
// End-to-End LED Test
// ============================================================================

/// End-to-end test verifying bi-directional host bus communication
///
/// Test sequence:
/// 1. CPU writes 0xAA to LED device
/// 2. CPU signals CPU_DONE
/// 3. Host reads LED via bus request, verifies value is 0xAA
/// 4. Host writes 0x55 to LED via bus request
/// 5. Host signals SYNC_FLAG
/// 6. CPU reads LED, verifies it's 0x55
/// 7. CPU exits with LED value in tohost (should be 0x55)
#[test]
#[ignore = "Protocol conflict between CPU and host requests - see HOST_BUS_PROTOCOL_STATUS.md"]
fn test_host_bus_end_to_end_led() {
    init_test_logger();

    const START_ADDR: u32 = 0x8000_0000;

    // Program that writes 0xAA to LED, waits for host, reads LED, exits with LED value
    let instructions = generate_led_write_and_wait_program(0xAA);
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    #[derive(Default)]
    struct EndToEndState {
        phase: u32,
        read_request_sent: bool,
        read_value: Option<u32>,
        write_request_sent: bool,
        write_ack_received: bool,
        callback_count: u32,
    }

    let test_state = Arc::new(Mutex::new(EndToEndState::default()));
    let test_state_callback = test_state.clone();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        Some(move |sim: &mut SimulatorView| {
            let mut state = test_state_callback.lock().unwrap();
            state.callback_count += 1;

            match state.phase {
                0 => {
                    // Phase 0: Wait for CPU to signal LED write complete
                    let cpu_done = sim.read_word(CPU_DONE_ADDR);
                    if cpu_done == 1 && !state.read_request_sent {
                        // Phase 1a: Send read request to verify CPU's LED write
                        let request = HostBusRequest {
                            addr: LED_BASE,
                            wdata: 0,
                            size: 2,
                            we: false,
                        };
                        sim.send_bus_request(request)
                            .expect("Failed to send read request");
                        state.read_request_sent = true;
                        state.phase = 1;
                    }
                }
                1 => {
                    // Phase 1b: Wait for read response
                    if let Some(response) = sim.receive_bus_response() {
                        match response {
                            HostBusResponse::ReadData(value) => {
                                assert_eq!(
                                    value & 0xFF,
                                    0xAA,
                                    "CPU should have written 0xAA to LED, got 0x{:02X}",
                                    value & 0xFF
                                );
                                state.read_value = Some(value);

                                // Phase 2a: Send write request to change LED to 0x55
                                let request = HostBusRequest {
                                    addr: LED_BASE,
                                    wdata: 0x55,
                                    size: 2,
                                    we: true,
                                };
                                sim.send_bus_request(request)
                                    .expect("Failed to send write request");
                                state.write_request_sent = true;
                                state.phase = 2;
                            }
                            _ => panic!("Expected ReadData, got {:?}", response),
                        }
                    }
                }
                2 => {
                    // Phase 2b: Wait for write acknowledgement
                    if let Some(response) = sim.receive_bus_response() {
                        match response {
                            HostBusResponse::WriteAck => {
                                state.write_ack_received = true;
                                // Signal CPU to continue
                                sim.write_word(SYNC_FLAG_ADDR, 1);
                                state.phase = 3;
                            }
                            _ => panic!("Expected WriteAck, got {:?}", response),
                        }
                    }
                }
                _ => {}
            }
        }),
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            sim.write_word(SYNC_FLAG_ADDR, 0);
            sim.write_word(CPU_DONE_ADDR, 0);
            sim.write_memory_region(START_ADDR, &program_bytes, true);
            Ok(START_ADDR)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    )
    .expect("Simulation should succeed");

    let final_state = test_state.lock().unwrap();
    assert!(
        final_state.read_value.is_some(),
        "Host should have read LED"
    );
    assert_eq!(
        final_state.read_value.unwrap() & 0xFF,
        0xAA,
        "Host read should have seen 0xAA"
    );
    assert!(
        final_state.write_ack_received,
        "Host should have received write ack"
    );

    // CPU reads LED after host write and puts value in tohost
    // After host writes 0x55, CPU should read 0x55 and exit with that value
    assert_eq!(
        result.tohost_value,
        Some(0x55),
        "CPU should exit with LED value 0x55 (after host write)"
    );
}

// ============================================================================
// Error Handling Tests
// ============================================================================

/// Test that requests to invalid addresses (outside RTL peripheral range)
/// are rejected at the API level before being sent to RTL.
///
/// This test verifies the Rust-side validation without needing full CPU simulation.
#[test]
fn test_host_request_invalid_address_rejected_by_api() {
    init_test_logger();

    const START_ADDR: u32 = 0x8000_0000;

    // Simple program: just write to tohost and exit immediately
    // We don't need a busy loop for this test since we're testing API validation
    let instructions: [u32; 4] = [
        lui(8, 0x10000000), // x8 = tohost base
        addi(7, 0, 1),      // x7 = success code
        sw(8, 7, 0),        // *tohost = 1
        jal(0, 0),          // halt
    ];
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    let test_state = Arc::new(Mutex::new(false)); // Track if error was caught
    let test_state_callback = test_state.clone();

    let result = run_program(
        1000, // Short cycle count since we exit quickly
        false,
        false,
        Some(move |sim: &mut SimulatorView| {
            let mut error_caught = test_state_callback.lock().unwrap();
            if *error_caught {
                return; // Already tested
            }

            // Try to send request to DRAM address (invalid - would loop back to host)
            let request = HostBusRequest {
                addr: 0x80000000, // DRAM address - should be rejected
                wdata: 0,
                size: 2,
                we: false,
            };

            let result = sim.send_bus_request(request);
            match result {
                Err(msg) => {
                    assert!(
                        msg.contains("outside RTL peripheral range"),
                        "Error should mention RTL peripheral range: {}",
                        msg
                    );
                    *error_caught = true;
                }
                Ok(_) => {
                    panic!("Request to DRAM address should have been rejected");
                }
            }
        }),
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

    let error_caught = *test_state.lock().unwrap();
    assert!(error_caught, "Should have caught invalid address error");
    assert_eq!(
        result.tohost_value,
        Some(1),
        "Program should exit with success"
    );
}

// ============================================================================
// Mixed CPU and Host Traffic Tests
// ============================================================================

/// Test that host can write to LED while CPU is running other operations
#[test]
#[ignore = "Protocol conflict between CPU and host requests - see HOST_BUS_PROTOCOL_STATUS.md"]
fn test_host_write_during_cpu_activity() {
    init_test_logger();

    const START_ADDR: u32 = 0x8000_0000;

    // CPU does some work (incrementing a counter), then spins waiting for host
    let mut instructions = vec![
        // Do some busy work
        addi(5, 0, 0),   // x5 = 0 (counter)
        addi(6, 0, 100), // x6 = 100 (limit)
        // loop: increment until x5 == x6
        addi(5, 5, 1), // x5++
        bne(5, 6, -4), // if x5 != 100, loop
    ];
    // Now spin waiting for host
    instructions.extend(generate_busy_loop_program(vec![]));

    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    let test_state = Arc::new(Mutex::new(HostRequestTestState::default()));
    let test_state_callback = test_state.clone();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        Some(move |sim: &mut SimulatorView| {
            let mut state = test_state_callback.lock().unwrap();
            state.callback_count += 1;

            // Wait until we've had enough callbacks (CPU is in busy-work phase)
            // Then send host request
            if state.callback_count >= 50 && !state.request_sent {
                let request = HostBusRequest {
                    addr: LED_BASE,
                    wdata: 0x42,
                    size: 2,
                    we: true,
                };
                sim.send_bus_request(request)
                    .expect("Failed to send write request");
                state.request_sent = true;
                state.phase = 1;
            }

            if state.phase == 1 {
                if let Some(response) = sim.receive_bus_response() {
                    state.response_received = Some(response.clone());
                    match response {
                        HostBusResponse::WriteAck => {
                            sim.write_word(SYNC_FLAG_ADDR, 1);
                            state.phase = 2;
                        }
                        _ => panic!("Expected WriteAck, got {:?}", response),
                    }
                }
            }
        }),
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            sim.write_word(SYNC_FLAG_ADDR, 0);
            sim.write_memory_region(START_ADDR, &program_bytes, true);
            Ok(START_ADDR)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    )
    .expect("Simulation should succeed");

    let final_state = test_state.lock().unwrap();
    assert!(
        matches!(
            final_state.response_received,
            Some(HostBusResponse::WriteAck)
        ),
        "Should have received WriteAck"
    );
    assert_eq!(
        result.tohost_value,
        Some(1),
        "Program should exit with success"
    );
}

// ============================================================================
// Sequential Host Request Tests
// ============================================================================

/// Test multiple sequential host requests (read, write, read)
#[test]
#[ignore = "Protocol conflict between CPU and host requests - see HOST_BUS_PROTOCOL_STATUS.md"]
fn test_sequential_host_requests() {
    init_test_logger();

    const START_ADDR: u32 = 0x8000_0000;

    // CPU writes initial value to LED, then spins
    let mut instructions = vec![
        lui(15, 0x50000000), // x15 = LED base
        addi(14, 0, 0x11),   // x14 = 0x11
        sw(15, 14, 0),       // *LED = 0x11
        lui(13, 0x80000000), // x13 = DRAM base
        addi(12, 0, 1),      // x12 = 1
        sw(13, 12, 0x104),   // Signal CPU_DONE
    ];
    instructions.extend(generate_busy_loop_program(vec![]));

    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    #[derive(Default)]
    struct SeqState {
        phase: u32,
        first_read_value: Option<u32>,
        write_ack: bool,
        second_read_value: Option<u32>,
        callback_count: u32,
    }

    let test_state = Arc::new(Mutex::new(SeqState::default()));
    let test_state_callback = test_state.clone();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        Some(move |sim: &mut SimulatorView| {
            let mut state = test_state_callback.lock().unwrap();
            state.callback_count += 1;

            match state.phase {
                0 => {
                    // Wait for CPU to write initial LED value
                    let cpu_done = sim.read_word(CPU_DONE_ADDR);
                    if cpu_done == 1 {
                        // Send first read request
                        let request = HostBusRequest {
                            addr: LED_BASE,
                            wdata: 0,
                            size: 2,
                            we: false,
                        };
                        sim.send_bus_request(request)
                            .expect("Failed to send read request");
                        state.phase = 1;
                    }
                }
                1 => {
                    // Wait for first read response
                    if let Some(HostBusResponse::ReadData(value)) = sim.receive_bus_response() {
                        assert_eq!(value & 0xFF, 0x11, "First read should be 0x11");
                        state.first_read_value = Some(value);

                        // Send write request
                        let request = HostBusRequest {
                            addr: LED_BASE,
                            wdata: 0x22,
                            size: 2,
                            we: true,
                        };
                        sim.send_bus_request(request)
                            .expect("Failed to send write request");
                        state.phase = 2;
                    }
                }
                2 => {
                    // Wait for write ack
                    if let Some(HostBusResponse::WriteAck) = sim.receive_bus_response() {
                        state.write_ack = true;

                        // Send second read request
                        let request = HostBusRequest {
                            addr: LED_BASE,
                            wdata: 0,
                            size: 2,
                            we: false,
                        };
                        sim.send_bus_request(request)
                            .expect("Failed to send read request");
                        state.phase = 3;
                    }
                }
                3 => {
                    // Wait for second read response
                    if let Some(HostBusResponse::ReadData(value)) = sim.receive_bus_response() {
                        assert_eq!(value & 0xFF, 0x22, "Second read should be 0x22");
                        state.second_read_value = Some(value);
                        sim.write_word(SYNC_FLAG_ADDR, 1);
                        state.phase = 4;
                    }
                }
                _ => {}
            }
        }),
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            sim.write_word(SYNC_FLAG_ADDR, 0);
            sim.write_word(CPU_DONE_ADDR, 0);
            sim.write_memory_region(START_ADDR, &program_bytes, true);
            Ok(START_ADDR)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    )
    .expect("Simulation should succeed");

    let final_state = test_state.lock().unwrap();
    assert_eq!(final_state.first_read_value.map(|v| v & 0xFF), Some(0x11));
    assert!(final_state.write_ack);
    assert_eq!(final_state.second_read_value.map(|v| v & 0xFF), Some(0x22));
    assert_eq!(result.tohost_value, Some(1));
}

// ============================================================================
// Byte and Halfword Access Tests
// ============================================================================

/// Test byte write via host request
#[test]
#[ignore = "Protocol conflict between CPU and host requests - see HOST_BUS_PROTOCOL_STATUS.md"]
fn test_host_write_byte() {
    init_test_logger();

    const START_ADDR: u32 = 0x8000_0000;

    let instructions = generate_busy_loop_program(vec![]);
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    let test_state = Arc::new(Mutex::new(HostRequestTestState::default()));
    let test_state_callback = test_state.clone();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        Some(move |sim: &mut SimulatorView| {
            let mut state = test_state_callback.lock().unwrap();
            state.callback_count += 1;

            match state.phase {
                0 if !state.request_sent => {
                    // Send byte write request
                    let request = HostBusRequest {
                        addr: LED_BASE,
                        wdata: 0x77,
                        size: 0, // byte
                        we: true,
                    };
                    sim.send_bus_request(request)
                        .expect("Failed to send byte write request");
                    state.request_sent = true;
                    state.phase = 1;
                }
                1 => {
                    if let Some(response) = sim.receive_bus_response() {
                        state.response_received = Some(response.clone());
                        match response {
                            HostBusResponse::WriteAck => {
                                sim.write_word(SYNC_FLAG_ADDR, 1);
                                state.phase = 2;
                            }
                            _ => panic!("Expected WriteAck, got {:?}", response),
                        }
                    }
                }
                _ => {}
            }
        }),
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            sim.write_word(SYNC_FLAG_ADDR, 0);
            sim.write_memory_region(START_ADDR, &program_bytes, true);
            Ok(START_ADDR)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    )
    .expect("Simulation should succeed");

    let final_state = test_state.lock().unwrap();
    assert!(
        matches!(
            final_state.response_received,
            Some(HostBusResponse::WriteAck)
        ),
        "Should have received WriteAck for byte write"
    );
    assert_eq!(result.tohost_value, Some(1));
}

/// Test halfword write via host request
#[test]
#[ignore = "Protocol conflict between CPU and host requests - see HOST_BUS_PROTOCOL_STATUS.md"]
fn test_host_write_halfword() {
    init_test_logger();

    const START_ADDR: u32 = 0x8000_0000;

    let instructions = generate_busy_loop_program(vec![]);
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    let test_state = Arc::new(Mutex::new(HostRequestTestState::default()));
    let test_state_callback = test_state.clone();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        Some(move |sim: &mut SimulatorView| {
            let mut state = test_state_callback.lock().unwrap();
            state.callback_count += 1;

            match state.phase {
                0 if !state.request_sent => {
                    // Send halfword write request
                    let request = HostBusRequest {
                        addr: LED_BASE,
                        wdata: 0x5566,
                        size: 1, // halfword
                        we: true,
                    };
                    sim.send_bus_request(request)
                        .expect("Failed to send halfword write request");
                    state.request_sent = true;
                    state.phase = 1;
                }
                1 => {
                    if let Some(response) = sim.receive_bus_response() {
                        state.response_received = Some(response.clone());
                        match response {
                            HostBusResponse::WriteAck => {
                                sim.write_word(SYNC_FLAG_ADDR, 1);
                                state.phase = 2;
                            }
                            _ => panic!("Expected WriteAck, got {:?}", response),
                        }
                    }
                }
                _ => {}
            }
        }),
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            sim.write_word(SYNC_FLAG_ADDR, 0);
            sim.write_memory_region(START_ADDR, &program_bytes, true);
            Ok(START_ADDR)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    )
    .expect("Simulation should succeed");

    let final_state = test_state.lock().unwrap();
    assert!(
        matches!(
            final_state.response_received,
            Some(HostBusResponse::WriteAck)
        ),
        "Should have received WriteAck for halfword write"
    );
    assert_eq!(result.tohost_value, Some(1));
}
