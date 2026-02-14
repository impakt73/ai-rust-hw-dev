//! Shared test harness and utilities for device-runtime integration tests.

#![allow(dead_code)]

use device_runtime::{
    create_device_runtime, BusEvent, BusRequest, DeviceRuntime, DeviceRuntimeType,
};
use host_bus_handler::AccessSize;
use riscv_core::instruction::{addi, jal, lui, sw};
use riscv_shared::bus::SIM_CONTROL_BASE;
use std::time::{Duration, Instant};

/// Timeout for quick operations such as simple host register reads/writes.
pub const SHORT_TIMEOUT: Duration = Duration::from_secs(2);
/// Timeout for medium operations such as reset or boot-state transitions.
pub const MEDIUM_TIMEOUT: Duration = Duration::from_secs(5);
/// Timeout for long operations such as full program execution and termination.
pub const LONG_TIMEOUT: Duration = Duration::from_secs(10);

/// Create and return the integration-test runtime for the selected backend.
///
/// Create a device runtime based on environment variables.
///
/// If `FPGA_DEVICE_PATH` and `FPGA_BAUD_RATE` are set, the FPGA backend is used.
/// Otherwise, the simulation backend is used by default.
pub fn create_test_runtime() -> Box<dyn DeviceRuntime> {
    let runtime_type = match (
        std::env::var("FPGA_DEVICE_PATH"),
        std::env::var("FPGA_BAUD_RATE"),
    ) {
        (Ok(device), Ok(baud_str)) => {
            let baud: u32 = baud_str
                .parse()
                .expect("FPGA_BAUD_RATE must be a valid u32");
            DeviceRuntimeType::Fpga { device, baud }
        }
        _ => DeviceRuntimeType::Sim,
    };

    create_device_runtime(runtime_type).expect("Failed to create device runtime")
}

/// Load program bytes at `boot_pc` and issue a CPU boot from the same address.
pub fn load_and_boot(runtime: &mut dyn DeviceRuntime, boot_pc: u32, program_bytes: &[u8]) {
    runtime
        .load_program(boot_pc, program_bytes)
        .expect("Failed to load program");
    runtime.boot_cpu(boot_pc).expect("Failed to boot CPU");
}

/// Wait until a `TohostTermination` event is received and return its value.
///
/// Panics if polling fails or if the timeout expires first.
pub fn wait_for_tohost(runtime: &mut dyn DeviceRuntime, timeout: Duration) -> u32 {
    let start = Instant::now();
    while start.elapsed() < timeout {
        match runtime.poll() {
            Ok(Some(BusEvent::TohostTermination { value })) => return value,
            Ok(Some(_)) => {}
            Ok(None) => std::thread::sleep(Duration::from_millis(1)),
            Err(e) => panic!("Poll error while waiting for tohost: {e}"),
        }
    }

    panic!("Timed out waiting for tohost termination");
}

/// Wait for a host read response matching `addr` and return the read data.
///
/// Panics if polling fails or if the timeout expires first.
pub fn wait_for_host_read_response(
    runtime: &mut dyn DeviceRuntime,
    addr: u32,
    timeout: Duration,
) -> u32 {
    let start = Instant::now();
    while start.elapsed() < timeout {
        match runtime.poll() {
            Ok(Some(BusEvent::HostReadResponse {
                addr: resp_addr,
                data,
                ..
            })) if resp_addr == addr && !runtime.has_pending_host_request() => return data,
            Ok(Some(_)) => {}
            Ok(None) => std::thread::sleep(Duration::from_millis(1)),
            Err(e) => panic!("Poll error while reading 0x{addr:08X}: {e}"),
        }
    }

    panic!("Timed out waiting for read response at 0x{addr:08X}");
}

/// Wait for a host write response matching `addr` and return the acknowledged write data.
///
/// Panics if polling fails or if the timeout expires first.
pub fn wait_for_host_write_response(
    runtime: &mut dyn DeviceRuntime,
    addr: u32,
    timeout: Duration,
) -> u32 {
    let start = Instant::now();
    while start.elapsed() < timeout {
        match runtime.poll() {
            Ok(Some(BusEvent::HostWriteResponse {
                addr: resp_addr,
                wdata,
                ..
            })) if resp_addr == addr && !runtime.has_pending_host_request() => return wdata,
            Ok(Some(_)) => {}
            Ok(None) => std::thread::sleep(Duration::from_millis(1)),
            Err(e) => panic!("Poll error while writing 0x{addr:08X}: {e}"),
        }
    }

    panic!("Timed out waiting for write response at 0x{addr:08X}");
}

/// Issue a host word read and wait for the matching response.
///
/// Panics if request submission fails, polling fails, or the timeout expires first.
pub fn read_word_with_timeout(
    runtime: &mut dyn DeviceRuntime,
    addr: u32,
    timeout: Duration,
) -> u32 {
    runtime
        .send_host_request(BusRequest::read(addr, AccessSize::Word))
        .expect("Failed to send host read request");
    wait_for_host_read_response(runtime, addr, timeout)
}

/// Issue a host word write and wait for the matching write acknowledgment.
///
/// Panics if request submission fails, polling fails, or the timeout expires first.
pub fn write_word_with_timeout(
    runtime: &mut dyn DeviceRuntime,
    addr: u32,
    data: u32,
    timeout: Duration,
) -> u32 {
    runtime
        .send_host_request(BusRequest::write(addr, data, AccessSize::Word))
        .expect("Failed to send host write request");
    wait_for_host_write_response(runtime, addr, timeout)
}

/// Poll and discard events until the runtime reports idle state.
///
/// Panics if polling fails or if the timeout expires before idle is reached.
pub fn drain_events_until_idle(runtime: &mut dyn DeviceRuntime, timeout: Duration) {
    let start = Instant::now();
    while start.elapsed() < timeout {
        match runtime.poll() {
            Ok(Some(_)) => {}
            Ok(None) if !runtime.has_pending_host_request() => return,
            Ok(None) => std::thread::sleep(Duration::from_millis(1)),
            Err(e) => panic!("Poll error while draining events: {e}"),
        }
    }

    panic!("Timed out waiting for runtime to become idle");
}

/// Helper to convert instructions to little-endian bytes.
pub fn instructions_to_bytes(instructions: &[u32]) -> Vec<u8> {
    instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect()
}

/// Build a standard tohost termination sequence.
///
/// The sequence writes `tohost_value` to `SIM_CONTROL_BASE` and then loops.
pub fn tohost_termination(addr_reg: u32, value_reg: u32, tohost_value: u32) -> [u32; 4] {
    [
        lui(addr_reg, SIM_CONTROL_BASE),
        addi(
            value_reg,
            0,
            i32::try_from(tohost_value).expect("tohost value must fit in i32 immediate"),
        ),
        sw(addr_reg, value_reg, 0),
        jal(0, 0),
    ]
}

/// Append a standard tohost termination sequence to an instruction vector.
///
/// This extends `instructions` in place with the output of [`tohost_termination`].
pub fn append_tohost_termination(
    instructions: &mut Vec<u32>,
    addr_reg: u32,
    value_reg: u32,
    tohost_value: u32,
) {
    instructions.extend(tohost_termination(addr_reg, value_reg, tohost_value));
}
