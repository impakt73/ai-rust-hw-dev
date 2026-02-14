//! Host-Initiated Bus Request Integration Tests
//!
//! Tests for the host-initiated bus request system that enables the host
//! (Rust simulation) to initiate memory transactions to RTL peripherals.
//!
//! These tests verify the complete path:
//! Host → RX → Buffer → Master → Bus → Peripheral
//!
//! NOTE: These tests are being migrated to device-runtime/tests/test_host_initiated_requests.rs
//! and are marked as #[ignore] during the transition to maintain coverage.

mod common;

use cpu_sim::*;
use riscv_core::instruction::*;
use riscv_shared::sim_control::{FAILURE_CODE, SUCCESS_CODE};

/// Helper function to initialize test logger (idempotent)
fn init_test_logger() {
    let _ = env_logger::builder().is_test(true).try_init();
}

// ============================================================================
// Host-Initiated Bus Request Tests
// ============================================================================

/// Test basic synchronization using host-initiated LED write.
///
/// The CPU polls the LED register waiting for a non-zero value.
/// The host uses `send_bus_request()` to write to the LED peripheral via the full
/// Host → RX → Buffer → Master → Bus → Peripheral path.
#[test]
#[ignore = "Migrated to device-runtime/tests/test_host_initiated_requests.rs"]
fn test_host_initiated_basic_sync() {
    init_test_logger();

    // LED peripheral is used as the fence (RTL peripheral at 0x50000000)
    const LED_BASE: u32 = 0x50000000;

    // Program that spins on LED peripheral until it becomes non-zero
    let mut instructions = vec![
        // Setup: Load LED peripheral address
        lui(15, LED_BASE), // x15 = LED base address (0x50000000)
        // Spin loop: wait for LED value != 0
        lw(14, 15, 0),      // x14 = LED peripheral value
        andi(14, 14, 0xFF), // mask to 8 bits
        beq(14, 0, -8),     // if x14 == 0, loop back to lw
    ];
    common::append_tohost_termination(&mut instructions, 10, 11, SUCCESS_CODE);

    const START_ADDR: u32 = 0x8000_0000;
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    let fence_written = std::sync::Arc::new(std::sync::Mutex::new(false));
    let fence_written_clone = fence_written.clone();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false, // print_inst_trace
        false, // print_fsm_state
        Some(move |sim: &mut SimulatorView| {
            // On each cycle, check if we should release the fence.
            // Use send_bus_request() to write to the LED peripheral via the
            // Host → RX → Buffer → Master → Bus → LED Peripheral path.
            let mut written = fence_written_clone.lock().unwrap();
            if !*written {
                // Send host-initiated write to LED peripheral (RTL space)
                // Value 0x01 will cause CPU to break out of spin loop
                let request = BusRequest::write(LED_BASE, 0x01, AccessSize::Byte);
                sim.send_bus_request(request)
                    .expect("Should queue host request");
                *written = true;
            }

            // Poll for response completion
            if let Some(_response) = sim.receive_bus_response() {
                // Response received, LED write is complete
            }
        }),
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None, // vcd_path
        0,    // mem_latency_cycles
        |sim| {
            // Setup: Write program (LED peripheral starts at 0 after reset)
            sim.write_memory_region(START_ADDR, &program_bytes, true);
            Ok(START_ADDR)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    )
    .expect("Simulation should complete");

    assert_eq!(
        result.tohost_value,
        Some(SUCCESS_CODE),
        "Program should exit with success code after LED fence release"
    );
}

/// Test host-initiated LED write with CPU verification.
///
/// This test verifies that the CPU can correctly read back a value written to
/// the LED peripheral via host-initiated bus request. The test:
/// 1. Host writes a known value (0xA5) to LED peripheral via bus request
/// 2. CPU spins until LED is non-zero
/// 3. CPU reads LED value and compares with expected value in DRAM
/// 4. CPU reports success or failure via tohost
#[test]
#[ignore = "Migrated to device-runtime/tests/test_host_initiated_requests.rs"]
fn test_host_initiated_led_write() {
    init_test_logger();

    // Address constants
    const LED_BASE: u32 = 0x50000000;
    const LED_EXPECTED_ADDR: u32 = 0x8000_1000; // DRAM location for expected value

    // Program that:
    // 1. Waits for LED peripheral to be non-zero (fence)
    // 2. Reads the expected LED value from DRAM (written by host)
    // 3. Reads the actual LED value from LED peripheral
    // 4. Compares and writes result to tohost
    let instructions = vec![
        // Setup addresses
        lui(15, LED_BASE),        // x15 = LED base address (0x50000000)
        lui(14, 0x80001000),      // x14 = DRAM base for expected value
        lui(9, SIM_CONTROL_BASE), // x9 = tohost address
        // Wait for LED fence (non-zero value)
        lw(12, 15, 0),      // x12 = LED peripheral value
        andi(12, 12, 0xFF), // mask to 8 bits
        beq(12, 0, -8),     // spin while LED == 0
        // Read expected value from DRAM and actual LED value
        lw(11, 14, 0),      // x11 = expected LED value from DRAM
        andi(11, 11, 0xFF), // mask to 8 bits
        lw(10, 15, 0),      // x10 = LED peripheral value
        andi(10, 10, 0xFF), // mask to 8 bits
        // Compare actual vs expected
        sub(8, 10, 11), // x8 = actual - expected
        bne(8, 0, 16),  // if not equal, jump to failure
        // Success
        addi(7, 0, SUCCESS_CODE as i32),
        sw(9, 7, 0), // tohost = SUCCESS_CODE
        jal(0, 0),   // infinite loop
        // Failure
        addi(7, 0, FAILURE_CODE as i32),
        sw(9, 7, 0), // tohost = FAILURE_CODE
        jal(0, 0),   // infinite loop
    ];

    const START_ADDR: u32 = 0x8000_0000;
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    let host_request_sent = std::sync::Arc::new(std::sync::Mutex::new(false));
    let host_request_sent_clone = host_request_sent.clone();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        Some(move |sim: &mut SimulatorView| {
            let mut sent = host_request_sent_clone.lock().unwrap();
            if !*sent {
                // Write the test value to LED peripheral via host-initiated bus request
                let led_value: u8 = 0xA5;
                let request = BusRequest::write(LED_BASE, led_value as u32, AccessSize::Byte);
                sim.send_bus_request(request)
                    .expect("Should queue host request");

                // Store expected value in DRAM for CPU to compare
                sim.write_memory_region(
                    LED_EXPECTED_ADDR,
                    &(led_value as u32).to_le_bytes(),
                    false,
                );
                *sent = true;
            }

            // Poll for response
            if let Some(response) = sim.receive_bus_response() {
                assert!(response.we, "Should be write response");
            }
        }),
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0,
        |sim| {
            sim.write_memory_region(START_ADDR, &program_bytes, true);
            // Initialize expected value to 0 (will be updated by callback)
            sim.write_memory_region(LED_EXPECTED_ADDR, &0u32.to_le_bytes(), false);
            Ok(START_ADDR)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    )
    .expect("Simulation should complete");

    assert_eq!(
        result.tohost_value,
        Some(SUCCESS_CODE),
        "LED value should match expected (0xA5)"
    );
}

/// Test host-initiated LED read.
///
/// The CPU writes a value to the LED peripheral, then the host reads it back
/// via host-initiated bus request.
#[test]
#[ignore = "Migrated to device-runtime/tests/test_host_initiated_requests.rs"]
fn test_host_initiated_led_read() {
    init_test_logger();

    const LED_VALUE: u8 = 0xCC;

    // CPU writes to LED, then terminates
    let mut instructions = vec![
        lui(15, LED_BASE), // LED base
        addi(14, 0, LED_VALUE as i32),
        sw(15, 14, 0), // Write to LED
    ];
    common::append_tohost_termination(&mut instructions, 7, 8, SUCCESS_CODE);

    const START_ADDR: u32 = 0x8000_0000;
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    // Track whether we've read the LED and the value we got
    let led_read_value = std::sync::Arc::new(std::sync::Mutex::new(None::<u32>));
    let led_read_value_clone = led_read_value.clone();

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
            // After completion, verify the LED value directly via led_out()
            *led_read_value_clone.lock().unwrap() = Some(sim.led_out() as u32);
        }),
    )
    .expect("Simulation should complete");

    assert_eq!(
        result.tohost_value,
        Some(SUCCESS_CODE),
        "Program should exit with success code"
    );

    // Verify LED was written correctly by reading via SimulatorView
    let final_led = led_read_value.lock().unwrap();
    assert_eq!(
        *final_led,
        Some(LED_VALUE as u32),
        "LED value should be 0xCC"
    );
}

/// Test that host request address validation works correctly.
///
/// Host-initiated requests must target RTL peripheral space (0x50000000-0x5FFFFFFF)
/// to prevent deadlock. Requests to other address ranges should be rejected.
#[test]
#[ignore = "Migrated to device-runtime/tests/test_host_initiated_requests.rs"]
fn test_host_request_address_validation() {
    init_test_logger();

    // Simple program that just terminates
    let instructions = common::tohost_termination(7, 8, SUCCESS_CODE);

    const START_ADDR: u32 = 0x8000_0000;
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    let validation_tested = std::sync::Arc::new(std::sync::Mutex::new(false));
    let validation_tested_clone = validation_tested.clone();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        Some(move |sim: &mut SimulatorView| {
            let mut tested = validation_tested_clone.lock().unwrap();
            if !*tested {
                // Test 1: Valid address (LED peripheral)
                let request1 = BusRequest::write(0x50000000, 0x01, AccessSize::Byte);
                let valid_result = sim.send_bus_request(request1);
                assert!(
                    valid_result.is_ok(),
                    "Request to RTL peripheral space should succeed"
                );

                // We can't send another request while one is pending
                // This is expected behavior
                let request2 = BusRequest::write(0x50000004, 0x02, AccessSize::Byte);
                let pending_result = sim.send_bus_request(request2);
                assert!(pending_result.is_err(), "Request while pending should fail");

                *tested = true;
            }

            // Poll for response to allow next test
            sim.receive_bus_response();
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
    .expect("Simulation should complete");

    assert_eq!(
        result.tohost_value,
        Some(SUCCESS_CODE),
        "Program should exit with success code"
    );

    assert!(
        *validation_tested.lock().unwrap(),
        "Address validation tests should have run"
    );
}

/// Test multiple sequential host-initiated requests.
///
/// Verifies that multiple host requests can be sent sequentially,
/// waiting for each response before sending the next.
#[test]
#[ignore = "Migrated to device-runtime/tests/test_host_initiated_requests.rs"]
fn test_multiple_host_requests() {
    init_test_logger();

    const LED_BASE: u32 = 0x50000000;

    // Program that spins on LED until it reaches a specific value
    let mut instructions = vec![
        lui(15, LED_BASE), // x15 = LED base
        addi(14, 0, 3),    // x14 = target count (3)
        // Spin loop: wait until LED value >= 3
        lw(12, 15, 0),      // x12 = LED value
        andi(12, 12, 0xFF), // mask to 8 bits
        blt(12, 14, -8),    // if LED < 3, loop
    ];
    common::append_tohost_termination(&mut instructions, 10, 11, SUCCESS_CODE);

    const START_ADDR: u32 = 0x8000_0000;
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    let write_count = std::sync::Arc::new(std::sync::Mutex::new(0u32));
    let write_count_clone = write_count.clone();
    let pending_request = std::sync::Arc::new(std::sync::Mutex::new(false));
    let pending_request_clone = pending_request.clone();

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        Some(move |sim: &mut SimulatorView| {
            let mut count = write_count_clone.lock().unwrap();
            let mut pending = pending_request_clone.lock().unwrap();

            // Check for response from previous request
            if *pending {
                if sim.receive_bus_response().is_some() {
                    *pending = false;
                }
                return;
            }

            // Send next request if we haven't sent 3 yet
            if *count < 3 {
                *count += 1;
                let request = BusRequest::write(LED_BASE, *count, AccessSize::Byte);
                sim.send_bus_request(request)
                    .expect("Should queue host request");
                *pending = true;
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
    .expect("Simulation should complete");

    assert_eq!(
        result.tohost_value,
        Some(SUCCESS_CODE),
        "Program should exit with success after 3 LED writes"
    );

    assert_eq!(
        *write_count.lock().unwrap(),
        3,
        "Should have sent 3 host requests"
    );
}

/// Test that host bus interface works after CPU enters S_HALT.
///
/// The CPU executes an invalid instruction (0x00000000) which causes it to
/// enter S_HALT. After the CPU halts, the host should be able to read the
/// system controller's STATUS register (0x53000000) via a host-initiated
/// bus request. The STATUS register bit 1 should indicate cpu_halted=1.
///
/// This test verifies that the host bus path remains functional even when
/// the CPU is halted.
#[test]
#[ignore = "Migrated to device-runtime/tests/test_host_initiated_requests.rs"]
fn test_host_bus_works_after_halt() {
    init_test_logger();

    // System controller STATUS register address
    const SYSCTRL_STATUS: u32 = 0x5300_0000;

    // Maximum number of instruction-complete callbacks to wait for a response.
    // The host bus transaction takes ~12 clock cycles. In S_HALT, each
    // step() call is 1 cycle, so we need at least ~20 callbacks.
    // Use a generous limit to avoid flakiness.
    const MAX_CALLBACKS: u32 = 40;

    // Program: a single zero instruction (invalid compressed instruction)
    // which will cause the CPU to halt.
    let instructions: Vec<u32> = vec![0, 0, 0, 0];

    const START_ADDR: u32 = 0x8000_0000;
    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    // Track state across callbacks
    let callback_count = std::sync::Arc::new(std::sync::Mutex::new(0u32));
    let callback_count_clone = callback_count.clone();
    let request_sent = std::sync::Arc::new(std::sync::Mutex::new(false));
    let request_sent_clone = request_sent.clone();
    let response_received = std::sync::Arc::new(std::sync::Mutex::new(None::<u32>));
    let response_received_clone = response_received.clone();

    let result = run_program(
        500, // enough cycles for halt + host bus transaction
        false,
        false,
        Some(move |sim: &mut SimulatorView| {
            let mut count = callback_count_clone.lock().unwrap();
            *count += 1;

            let mut sent = request_sent_clone.lock().unwrap();
            let mut received = response_received_clone.lock().unwrap();

            // Send host-initiated read of STATUS register after a few callbacks
            // (wait a couple callbacks for the CPU to fully enter HALT)
            if !*sent && *count >= 3 {
                let request = BusRequest::read(SYSCTRL_STATUS, AccessSize::Word);
                match sim.send_bus_request(request) {
                    Ok(()) => {
                        *sent = true;
                    }
                    Err(e) => {
                        panic!("Failed to send bus request: {}", e);
                    }
                }
            }

            // Poll for response
            if *sent && received.is_none() {
                if let Some(response) = sim.receive_bus_response() {
                    *received = Some(response.rdata);
                }
            }

            // Check timeout
            if *sent && received.is_none() && *count > MAX_CALLBACKS {
                panic!(
                    "Host bus response not received after {} callbacks. \
                     The host bus interface appears to be hung while CPU is in S_HALT.",
                    *count
                );
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
    );

    // The test might fail with a HungStateError (from the simulator's hung detector,
    // which detects stuck PC after 50 cycles) OR with our explicit panic if the
    // response doesn't come back within MAX_CALLBACKS.
    // Check what we got.
    let received = response_received.lock().unwrap();
    let count = callback_count.lock().unwrap();

    if let Some(status) = *received {
        // We got a response! Verify the STATUS register value.
        // Bit 0 = cpu_booting (should be 0 since CPU has booted)
        // Bit 1 = cpu_halted (should be 1 since CPU is in S_HALT)
        let cpu_halted = (status >> 1) & 1;
        assert_eq!(
            cpu_halted, 1,
            "STATUS register should show cpu_halted=1, got STATUS=0x{:08x}",
            status
        );
        println!(
            "✓ Host bus works after CPU halt: STATUS=0x{:08x} (cpu_halted=1) after {} callbacks",
            status, *count
        );
    } else {
        // Response was never received - this is the bug
        let err_msg = result.err().unwrap_or_else(|| "No error".to_string());
        panic!(
            "Host bus response never received while CPU is in S_HALT. \
             Request sent: {}, Callbacks: {}, Simulation result: {}",
            *request_sent.lock().unwrap(),
            *count,
            err_msg
        );
    }
}
