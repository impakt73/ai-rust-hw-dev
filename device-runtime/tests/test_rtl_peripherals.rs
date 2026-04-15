//! RTL Peripheral Integration Tests
//!
//! This module contains integration tests for RTL-based peripherals including:
//!
//! - System controller LED register (0x20000010)
//! - Host-initiated bus request system
//! - Host-to-RTL peripheral communication path
//! - CPU-initiated peripheral access
//! - Multi-access size validation (byte, halfword, word)
//!
//! These tests verify both CPU-initiated peripheral accesses (via programmatic
//! instruction sequences) and host-initiated peripheral operations (via runtime
//! host bus request API).

mod common;

use bus_shared::{AccessSize, HandlerError};
use common::{
    create_test_runtime, instructions_to_bytes, load_and_boot, read_word_with_timeout,
    tohost_termination, wait_for_cpu_halt, wait_for_host_write_response, write_word_with_timeout,
    LONG_TIMEOUT, MEDIUM_TIMEOUT, SHORT_TIMEOUT, TEST_BOOT_PC,
};
use device_runtime::{BusRequest, DeviceError};
use riscv_core::instruction::{addi, andi, beq, blt, bne, lui, lw, ori, sb, sh, sub, sw};
use riscv_shared::bus::{
    sysctrl_led_out_addr, DRAM_BASE, RTL_PERIPH_LIMIT, SYSCTRL_BASE, SYSCTRL_HALT_OFFSET,
    SYSCTRL_LED_OUT_OFFSET, SYSCTRL_SIZE, SYSCTRL_STATUS_OFFSET,
};
use riscv_shared::sim_control::{FAILURE_CODE, SUCCESS_CODE};
use std::time::Duration;

const LED_ADDR: u32 = sysctrl_led_out_addr();
const LED_OFFSET_I32: i32 = SYSCTRL_LED_OUT_OFFSET as i32;

// ============================================================================
// LED Controller Peripheral Tests
// ============================================================================

#[test]
fn test_led_constants() {
    // Verify integrated system-controller LED register memory map constants
    assert_eq!(SYSCTRL_BASE, 0x20000000, "System controller base address");
    assert_eq!(SYSCTRL_LED_OUT_OFFSET, 0x10, "LED_OUT register offset");
    assert_eq!(SYSCTRL_SIZE, 0x28, "System controller size");
    assert_eq!(LED_ADDR, 0x20000010, "LED register address");
}

#[test]
fn test_led_basic_write_word() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![
        lui(15, SYSCTRL_BASE),
        addi(14, 0, 0xAA),
        sw(15, 14, LED_OFFSET_I32),
    ];
    instructions.extend(tohost_termination(7, 8, SUCCESS_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
}

#[test]
fn test_led_byte_access() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![
        lui(15, SYSCTRL_BASE),
        addi(14, 0, 0x55),
        sb(15, 14, LED_OFFSET_I32),
    ];
    instructions.extend(tohost_termination(7, 8, SUCCESS_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
}

#[test]
fn test_led_halfword_access() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![
        lui(15, SYSCTRL_BASE),
        addi(14, 0, 0xFF),
        sh(15, 14, LED_OFFSET_I32),
    ];
    instructions.extend(tohost_termination(7, 8, SUCCESS_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
}

#[test]
fn test_led_read_back() {
    let mut runtime = create_test_runtime();

    // CPU writes, reads back, and verifies - tohost only reached if successful
    let instructions = vec![
        lui(15, SYSCTRL_BASE),
        addi(14, 0, 0xCC),
        sw(15, 14, LED_OFFSET_I32),
        lw(13, 15, LED_OFFSET_I32),
        andi(13, 13, 0xFF),
        addi(12, 0, 0xCC),
        sub(11, 13, 12),
        bne(11, 0, 24),
    ];
    let mut instructions = instructions;
    instructions.extend(tohost_termination(7, 8, SUCCESS_CODE));
    instructions.extend(tohost_termination(7, 8, FAILURE_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
}

#[test]
fn test_led_pattern_sequence() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![
        lui(15, SYSCTRL_BASE),
        addi(14, 0, 0x00),
        sw(15, 14, LED_OFFSET_I32),
        addi(14, 0, 0xFF),
        sw(15, 14, LED_OFFSET_I32),
        addi(14, 0, 0xAA),
        sw(15, 14, LED_OFFSET_I32),
        addi(14, 0, 0x55),
        sw(15, 14, LED_OFFSET_I32),
    ];
    instructions.extend(tohost_termination(7, 8, SUCCESS_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
}

#[test]
fn test_led_upper_bits_ignored() {
    let mut runtime = create_test_runtime();

    // Write value with upper bits set, read back, verify only lower 8 bits
    let instructions = vec![
        lui(15, SYSCTRL_BASE),
        lui(14, 0xFFFFF000),
        ori(14, 14, 0xAA),
        sw(15, 14, LED_OFFSET_I32),
        lw(13, 15, LED_OFFSET_I32),
        andi(13, 13, 0xFF),
        addi(12, 0, 0xAA),
        sub(11, 13, 12),
        bne(11, 0, 24),
    ];
    let mut instructions = instructions;
    instructions.extend(tohost_termination(7, 8, SUCCESS_CODE));
    instructions.extend(tohost_termination(7, 8, FAILURE_CODE));

    load_and_boot(
        runtime.as_mut(),
        TEST_BOOT_PC,
        &instructions_to_bytes(&instructions),
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
}

// ============================================================================
// Host-Initiated Bus Request Tests
// ============================================================================

/// Test basic synchronization using host-initiated LED write.
///
/// The CPU polls the system controller LED register waiting for a non-zero value.
/// The host writes to the LED register via host bus request.
#[test]
fn test_host_initiated_basic_sync() {
    let mut runtime = create_test_runtime();

    // Program that spins on the system controller LED register until it becomes non-zero
    let mut instructions = vec![
        lui(15, SYSCTRL_BASE),      // x15 = system controller base address
        lw(14, 15, LED_OFFSET_I32), // x14 = LED register value
        andi(14, 14, 0xFF),         // mask to 8 bits
        beq(14, 0, -8),             // if x14 == 0, loop back to lw
    ];
    instructions.extend(tohost_termination(10, 11, SUCCESS_CODE));

    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program_bytes);

    // Let CPU spin for a moment before releasing the fence
    std::thread::sleep(Duration::from_millis(10));

    // Write to the LED register via host-initiated request to release CPU from spin loop
    write_word_with_timeout(runtime.as_mut(), LED_ADDR, 0x01, MEDIUM_TIMEOUT);

    // Wait for tohost termination
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
}

/// Test host-initiated LED write with CPU verification.
///
/// The host writes a known value to the system controller LED register, CPU verifies it.
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
        lui(15, SYSCTRL_BASE), // x15 = system controller base address
        lui(14, 0x80001000),   // x14 = DRAM base for expected value
        // Wait for LED fence (non-zero value)
        lw(12, 15, LED_OFFSET_I32), // x12 = LED register value
        andi(12, 12, 0xFF),         // mask to 8 bits
        beq(12, 0, -8),             // spin while LED == 0
        // Read expected value from DRAM and actual LED value
        lw(11, 14, 0),              // x11 = expected LED value from DRAM
        andi(11, 11, 0xFF),         // mask to 8 bits
        lw(10, 15, LED_OFFSET_I32), // x10 = LED register value
        andi(10, 10, 0xFF),         // mask to 8 bits
        // Compare actual vs expected
        sub(8, 10, 11), // x8 = actual - expected
        bne(8, 0, 24),  // if not equal, jump to failure
    ];
    let mut instructions = instructions;
    instructions.extend(tohost_termination(9, 7, SUCCESS_CODE));
    instructions.extend(tohost_termination(9, 7, FAILURE_CODE));

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

    // Write test value to the LED register via host request
    write_word_with_timeout(runtime.as_mut(), LED_ADDR, TEST_VALUE, MEDIUM_TIMEOUT);

    // Wait for tohost termination
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
}

/// Test host-initiated LED read.
///
/// The CPU writes a value to the system controller LED register, then the host reads it back.
#[test]
fn test_host_initiated_led_read() {
    let mut runtime = create_test_runtime();

    const LED_VALUE: u8 = 0xCC;

    // CPU writes to LED, executes delay NOPs so host can read it, then terminates
    let mut instructions = vec![
        lui(15, SYSCTRL_BASE), // system controller base
        addi(14, 0, LED_VALUE as i32),
        sw(15, 14, LED_OFFSET_I32), // Write to LED register
    ];
    instructions.extend(tohost_termination(7, 8, SUCCESS_CODE));

    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program_bytes);

    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );

    // Read back LED value via host bus request
    let led_value = read_word_with_timeout(runtime.as_mut(), LED_ADDR, SHORT_TIMEOUT);
    assert_eq!(
        led_value & 0xFF,
        LED_VALUE as u32,
        "LED value should be 0xCC"
    );
}

/// Test that host request address validation works correctly.
///
/// Verifies valid RTL-space request succeeds and a second request while one is
/// pending is rejected.
#[test]
fn test_host_request_address_validation() {
    let mut runtime = create_test_runtime();

    let mut instructions = vec![
        lui(15, SYSCTRL_BASE),
        lw(14, 15, LED_OFFSET_I32),
        andi(14, 14, 0xFF),
        beq(14, 0, -8),
    ];
    instructions.extend(tohost_termination(7, 8, SUCCESS_CODE));

    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program_bytes);
    std::thread::sleep(Duration::from_millis(10));

    let first_req = BusRequest::write(LED_ADDR, 0x01, AccessSize::Byte);
    runtime
        .send_host_request(first_req)
        .expect("Request to RTL peripheral space should succeed");
    let second_req = BusRequest::write(LED_ADDR + 4, 0x02, AccessSize::Byte);
    assert!(
        runtime.send_host_request(second_req).is_err(),
        "Request while pending should fail"
    );
    let first_wdata = wait_for_host_write_response(runtime.as_mut(), LED_ADDR, MEDIUM_TIMEOUT);
    assert_eq!(
        first_wdata, 0x01,
        "First host write response should match request"
    );
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
}

/// Test multiple sequential host-initiated requests.
///
/// Verifies that multiple host requests can be sent sequentially.
#[test]
fn test_multiple_host_requests() {
    let mut runtime = create_test_runtime();

    // Program that spins on LED until it reaches a specific value
    let mut instructions = vec![
        lui(15, SYSCTRL_BASE), // x15 = system controller base
        addi(14, 0, 3),        // x14 = target count (3)
        // Spin loop: wait until LED value >= 3
        lw(12, 15, LED_OFFSET_I32), // x12 = LED value
        andi(12, 12, 0xFF),         // mask to 8 bits
        blt(12, 14, -8),            // if LED < 3, loop
    ];
    instructions.extend(tohost_termination(10, 11, SUCCESS_CODE));

    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program_bytes);

    // Let CPU spin
    std::thread::sleep(Duration::from_millis(10));

    // Send three sequential write requests
    for count in 1..=3 {
        write_word_with_timeout(runtime.as_mut(), LED_ADDR, count, MEDIUM_TIMEOUT);
    }

    // Wait for tohost termination
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
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

    // Program: request CPU halt through the system controller.
    let instructions = vec![
        lui(15, SYSCTRL_BASE),
        addi(14, 0, 1),
        sw(15, 14, SYSCTRL_HALT_OFFSET as i32),
    ];

    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program_bytes);

    assert_eq!(wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT), None);

    let status = read_word_with_timeout(runtime.as_mut(), SYSCTRL_STATUS, MEDIUM_TIMEOUT);

    // Bit 1 = cpu_halted (should be 1 since CPU is in S_HALT)
    let cpu_halted = (status >> 1) & 1;
    assert_eq!(
        cpu_halted, 1,
        "STATUS register should show cpu_halted=1, got STATUS=0x{:08x}",
        status
    );
}

#[test]
fn test_host_request_routes_non_rtl_to_system_bus() {
    let mut runtime = create_test_runtime();
    let addr = DRAM_BASE + 0x100;
    let value = 0xDEAD_BEEF;

    runtime
        .send_host_request(BusRequest::write(addr, value, AccessSize::Word))
        .expect("DRAM write should be routed to SystemBus");
    let ack = wait_for_host_write_response(runtime.as_mut(), addr, MEDIUM_TIMEOUT);
    assert_eq!(ack, value);

    runtime
        .send_host_request(BusRequest::read(addr, AccessSize::Word))
        .expect("DRAM read should be routed to SystemBus");
    let read_back = common::wait_for_host_read_response(runtime.as_mut(), addr, MEDIUM_TIMEOUT);
    assert_eq!(read_back, value);
}

#[test]
fn test_host_request_spanning_rtl_boundary_is_rejected() {
    let mut runtime = create_test_runtime();
    // Last address in lower-half (RTL) space; a word access crosses into upper-half.
    let request = BusRequest::read(RTL_PERIPH_LIMIT - 1, AccessSize::Word);

    assert!(matches!(
        runtime.send_host_request(request),
        Err(DeviceError::HandlerError(HandlerError::InvalidAddressRange))
    ));
}

#[test]
fn test_host_request_overflow_range_is_rejected() {
    let mut runtime = create_test_runtime();
    let request = BusRequest::read(0xFFFF_FFFE, AccessSize::Word);

    assert!(matches!(
        runtime.send_host_request(request),
        Err(DeviceError::HandlerError(HandlerError::InvalidAddressRange))
    ));
}
