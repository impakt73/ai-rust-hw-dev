//! Host-Initiated Bus Request Integration Tests
//!
//! Tests for the host-initiated bus request system that enables the host
//! to initiate memory transactions to RTL peripherals.
//!
//! These tests verify the complete path:
//! Host → RX → Buffer → Master → Bus → Peripheral
//!
//! Migrated from cpu-sim/tests/test_host_initiated_requests.rs to use backend-agnostic
//! device-runtime APIs.

mod common;

use common::{
    create_test_runtime, drain_events_until_idle, instructions_to_bytes, load_and_boot,
    read_word_with_timeout, tohost_termination, wait_for_cpu_halt, wait_for_host_write_response,
    write_word_with_timeout, LONG_TIMEOUT, MEDIUM_TIMEOUT, SHORT_TIMEOUT, TEST_BOOT_PC,
};
use device_runtime::BusRequest;
use host_bus_handler::AccessSize;
use riscv_core::instruction::{addi, andi, beq, blt, bne, ebreak, jal, lui, lw, sub, sw};
use riscv_shared::bus::{LED_BASE, SIM_CONTROL_BASE, SYSCTRL_BASE, SYSCTRL_STATUS_OFFSET};
use riscv_shared::sim_control::{FAILURE_CODE, SUCCESS_CODE};
use std::time::Duration;

/// Extra CPU delay so the host can issue/read a host-bus transaction before program termination.
const DELAY_NOPS_FOR_HOST_READ: usize = 64;

/// Test basic synchronization using host-initiated LED write.
///
/// The CPU polls the LED register waiting for a non-zero value.
/// The host writes to the LED peripheral via host bus request.
#[test]
fn test_host_initiated_basic_sync() {
    let mut runtime = create_test_runtime();

    // Program that spins on LED peripheral until it becomes non-zero
    let mut instructions = vec![
        lui(15, LED_BASE),  // x15 = LED base address (0x50000000)
        lw(14, 15, 0),      // x14 = LED peripheral value
        andi(14, 14, 0xFF), // mask to 8 bits
        beq(14, 0, -8),     // if x14 == 0, loop back to lw
    ];
    instructions.extend(tohost_termination(10, 11, SUCCESS_CODE));

    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program_bytes);

    // Let CPU spin for a moment before releasing the fence
    std::thread::sleep(Duration::from_millis(10));

    // Write to LED peripheral via host-initiated request to release CPU from spin loop
    write_word_with_timeout(runtime.as_mut(), LED_BASE, 0x01, MEDIUM_TIMEOUT);

    // Wait for tohost termination
    wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT);
}

/// Test host-initiated LED write with CPU verification.
///
/// The host writes a known value to LED peripheral, CPU verifies it.
#[test]
fn test_host_initiated_led_write() {
    let mut runtime = create_test_runtime();

    const LED_EXPECTED_ADDR: u32 = 0x8000_1000;
    const TEST_VALUE: u32 = 0xA5;

    // Program that:
    // 1. Waits for LED peripheral to be non-zero (fence)
    // 2. Reads the expected LED value from DRAM
    // 3. Reads the actual LED value from LED peripheral
    // 4. Compares and writes result to tohost
    let instructions = vec![
        lui(15, LED_BASE),        // x15 = LED base address
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
        ebreak(),
        jal(0, 0), // infinite loop
        // Failure
        addi(7, 0, FAILURE_CODE as i32),
        sw(9, 7, 0), // tohost = FAILURE_CODE
        ebreak(),
        jal(0, 0), // infinite loop
    ];

    let program_bytes = instructions_to_bytes(&instructions);

    // Load program and pre-populate expected value in DRAM
    runtime
        .load_program(TEST_BOOT_PC, &program_bytes)
        .expect("Failed to load program");
    runtime
        .load_program(LED_EXPECTED_ADDR, &TEST_VALUE.to_le_bytes())
        .expect("Failed to load expected value");
    runtime.boot_cpu(TEST_BOOT_PC).expect("Failed to boot CPU");

    // Let CPU spin
    std::thread::sleep(Duration::from_millis(10));

    // Write test value to LED peripheral via host request
    write_word_with_timeout(runtime.as_mut(), LED_BASE, TEST_VALUE, MEDIUM_TIMEOUT);

    // Wait for tohost termination
    wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT);
}

/// Test host-initiated LED read.
///
/// The CPU writes a value to the LED peripheral, then the host reads it back.
#[test]
fn test_host_initiated_led_read() {
    let mut runtime = create_test_runtime();

    const LED_VALUE: u8 = 0xCC;

    // CPU writes to LED, executes delay NOPs so host can read it, then terminates
    let mut instructions = vec![
        lui(15, LED_BASE), // LED base
        addi(14, 0, LED_VALUE as i32),
        sw(15, 14, 0), // Write to LED
    ];
    instructions.extend(std::iter::repeat_n(addi(0, 0, 0), DELAY_NOPS_FOR_HOST_READ));
    instructions.extend(tohost_termination(7, 8, SUCCESS_CODE));

    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program_bytes);

    // Read back LED value via host bus request
    let led_value = read_word_with_timeout(runtime.as_mut(), LED_BASE, SHORT_TIMEOUT);
    assert_eq!(
        led_value & 0xFF,
        LED_VALUE as u32,
        "LED value should be 0xCC"
    );

    wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT);
}

/// Test that host request address validation works correctly.
///
/// Verifies valid RTL-space request succeeds and a second request while one is
/// pending is rejected.
#[test]
fn test_host_request_address_validation() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![
        lui(15, LED_BASE),
        lw(14, 15, 0),
        andi(14, 14, 0xFF),
        beq(14, 0, -8),
    ];
    instructions.extend(tohost_termination(7, 8, SUCCESS_CODE));

    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program_bytes);
    std::thread::sleep(Duration::from_millis(10));

    let first_req = BusRequest::write(LED_BASE, 0x01, AccessSize::Byte);
    runtime
        .send_host_request(first_req)
        .expect("Request to RTL peripheral space should succeed");
    let second_req = BusRequest::write(LED_BASE + 4, 0x02, AccessSize::Byte);
    assert!(
        runtime.send_host_request(second_req).is_err(),
        "Request while pending should fail"
    );
    let first_wdata = wait_for_host_write_response(runtime.as_mut(), LED_BASE, MEDIUM_TIMEOUT);
    assert_eq!(
        first_wdata, 0x01,
        "First host write response should match request"
    );
    wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT);
}

/// Test multiple sequential host-initiated requests.
///
/// Verifies that multiple host requests can be sent sequentially.
#[test]
fn test_multiple_host_requests() {
    let mut runtime = create_test_runtime();

    // Program that spins on LED until it reaches a specific value
    let mut instructions = vec![
        lui(15, LED_BASE), // x15 = LED base
        addi(14, 0, 3),    // x14 = target count (3)
        // Spin loop: wait until LED value >= 3
        lw(12, 15, 0),      // x12 = LED value
        andi(12, 12, 0xFF), // mask to 8 bits
        blt(12, 14, -8),    // if LED < 3, loop
    ];
    instructions.extend(tohost_termination(10, 11, SUCCESS_CODE));

    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program_bytes);

    // Let CPU spin
    std::thread::sleep(Duration::from_millis(10));

    // Send three sequential write requests
    for count in 1..=3 {
        write_word_with_timeout(runtime.as_mut(), LED_BASE, count, MEDIUM_TIMEOUT);
    }

    // Wait for tohost termination
    wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT);
}

/// Test that host bus interface works after CPU enters S_HALT.
///
/// The CPU executes an invalid instruction which causes it to halt.
/// After the CPU halts, the host should be able to read the system
/// controller's STATUS register via a host-initiated bus request.
#[test]
fn test_host_bus_works_after_halt() {
    let mut runtime = create_test_runtime();

    // System controller STATUS register address
    const SYSCTRL_STATUS: u32 = SYSCTRL_BASE + SYSCTRL_STATUS_OFFSET;

    // Program: a single zero instruction (invalid) which will cause the CPU to halt
    let instructions: Vec<u32> = vec![0, 0, 0, 0];

    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program_bytes);

    // Wait a bit for CPU to halt
    std::thread::sleep(Duration::from_millis(50));

    // Drain any pending events
    drain_events_until_idle(runtime.as_mut(), MEDIUM_TIMEOUT);

    let status = read_word_with_timeout(runtime.as_mut(), SYSCTRL_STATUS, MEDIUM_TIMEOUT);

    // Bit 1 = cpu_halted (should be 1 since CPU is in S_HALT)
    let cpu_halted = (status >> 1) & 1;
    assert_eq!(
        cpu_halted, 1,
        "STATUS register should show cpu_halted=1, got STATUS=0x{:08x}",
        status
    );
}
