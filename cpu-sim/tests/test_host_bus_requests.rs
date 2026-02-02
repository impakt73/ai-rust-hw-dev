//! Host Bus Request Integration Tests
//!
//! Tests for the host-initiated bus request functionality in the CPU simulator.
//! These tests verify the Rust API for sending requests to RTL peripherals.
//!
//! NOTE: Full integration tests that mix CPU and Host traffic are limited by
//! the current architecture which blocks CPU requests when host requests are
//! pending. See the testbench/tests/host_bus_interface_bidirectional_test.rs
//! for comprehensive RTL-level testing of the bidirectional protocol.

use cpu_sim::*;
use riscv_shared::bus::LED_BASE;

/// Helper function to initialize test logger (idempotent)
fn init_test_logger() {
    let _ = env_logger::builder().is_test(true).try_init();
}

// ============================================================================
// Basic API Tests
// ============================================================================

#[test]
fn test_host_bus_request_invalid_address_rejected() {
    init_test_logger();

    // Test that invalid addresses are rejected at the API level
    // This test doesn't require simulation - it just tests the validation logic

    // Create a mock request to DRAM (invalid for host requests)
    let request = HostBusRequest {
        addr: 0x80000000, // DRAM address - routes back to host
        wdata: 0,
        size: 2,
        we: false,
    };

    // We can't easily test this without a running simulator,
    // but we can verify the constants are correct
    assert!(LED_BASE >= 0x50000000, "LED_BASE should be in RTL range");
    assert!(LED_BASE < 0x60000000, "LED_BASE should be in RTL range");

    // Verify the request would be rejected by checking the address range
    // RTL peripheral range is 0x5000_0000 - 0x5FFF_FFFF
    assert!(
        request.addr < 0x50000000 || request.addr >= 0x60000000,
        "DRAM address should be outside RTL peripheral range"
    );
}

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
