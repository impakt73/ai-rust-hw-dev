//! Tests for FPGA runtime startup reset functionality.
//!
//! These tests verify that the startup reset enum options are correctly
//! wired through the API. Due to serial port dependency, direct behavior
//! testing is limited to API-level validation.

use device_runtime::{create_device_runtime, DeviceRuntimeType, StartupReset};

/// Test that StartupReset::None compiles and can be used in Fpga variant
#[test]
fn test_startup_reset_none_api() {
    let runtime_type = DeviceRuntimeType::Fpga {
        device: "/dev/null".to_string(),
        baud: 115200,
        startup_reset: StartupReset::None,
    };

    // We expect this to fail to connect (no valid serial device),
    // but it validates the API compiles and accepts StartupReset::None
    let _result = create_device_runtime(runtime_type);
}

/// Test that StartupReset::Cpu compiles and can be used in Fpga variant
#[test]
fn test_startup_reset_cpu_api() {
    let runtime_type = DeviceRuntimeType::Fpga {
        device: "/dev/null".to_string(),
        baud: 115200,
        startup_reset: StartupReset::Cpu,
    };

    // We expect this to fail to connect (no valid serial device),
    // but it validates the API compiles and accepts StartupReset::Cpu
    let _result = create_device_runtime(runtime_type);
}

/// Test that StartupReset::System compiles and can be used in Fpga variant
#[test]
fn test_startup_reset_system_api() {
    let runtime_type = DeviceRuntimeType::Fpga {
        device: "/dev/null".to_string(),
        baud: 115200,
        startup_reset: StartupReset::System,
    };

    // We expect this to fail to connect (no valid serial device),
    // but it validates the API compiles and accepts StartupReset::System
    let _result = create_device_runtime(runtime_type);
}

/// Test that StartupReset enum variants can be compared and copied
#[test]
fn test_startup_reset_enum_properties() {
    let none1 = StartupReset::None;
    let none2 = StartupReset::None;
    let cpu = StartupReset::Cpu;
    let system = StartupReset::System;

    // Test PartialEq
    assert_eq!(none1, none2);
    assert_ne!(none1, cpu);
    assert_ne!(none1, system);
    assert_ne!(cpu, system);

    // Test Copy (implicit in assignment)
    let _copy = none1;
    let _another = none1; // Would fail if not Copy
}
