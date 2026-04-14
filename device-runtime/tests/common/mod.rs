//! Shared test harness and utilities for device-runtime integration tests.

#![allow(dead_code)]

use bus_shared::AccessSize;
use device_runtime::{
    create_device_runtime, BusDeviceRegistration, BusEvent, BusRequest, DeviceRuntime,
    DeviceRuntimeType, SimDeviceRuntimeArgs,
};
use riscv_core::instruction::{addi, jal, lui, sw};
use riscv_shared::bus::{
    sysctrl_halt_addr, sysctrl_status_addr, SIM_CONTROL_BASE, SYSCTRL_STATUS_CPU_HALTED,
};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Timeout for quick operations such as simple host register reads/writes.
pub const SHORT_TIMEOUT: Duration = Duration::from_secs(2);
/// Timeout for medium operations such as reset or boot-state transitions.
pub const MEDIUM_TIMEOUT: Duration = Duration::from_secs(5);
/// Timeout for long operations such as full program execution and termination.
pub const LONG_TIMEOUT: Duration = Duration::from_secs(30);
/// Common test boot PC for loaded instruction programs.
pub const TEST_BOOT_PC: u32 = 0x8000_0000;

fn fpga_startup_reset_from_env(fpga_hard_reset: Option<&str>) -> device_runtime::StartupReset {
    match fpga_hard_reset {
        None => device_runtime::StartupReset::Cpu,
        Some(value) => match value
            .trim()
            .parse::<u32>()
            .expect("FPGA_HARD_RESET must be a valid u32")
        {
            0 => device_runtime::StartupReset::Cpu,
            _ => device_runtime::StartupReset::System,
        },
    }
}

/// Create a device runtime based on environment variables.
///
/// If `FPGA_DEVICE_PATH` and `FPGA_BAUD_RATE` are set, the FPGA backend is used.
/// Otherwise, the simulation backend is used by default.
pub fn create_test_runtime() -> Box<dyn DeviceRuntime> {
    create_test_runtime_with_registrations(None)
}

/// Create a device runtime based on environment variables with optional custom
/// bus-device registrations.
pub fn create_test_runtime_with_registrations(
    registrations: Option<Vec<BusDeviceRegistration>>,
) -> Box<dyn DeviceRuntime> {
    let runtime_type = match (
        std::env::var("FPGA_DEVICE_PATH"),
        std::env::var("FPGA_BAUD_RATE"),
    ) {
        (Ok(device), Ok(baud_str)) => {
            let baud: u32 = baud_str
                .parse()
                .expect("FPGA_BAUD_RATE must be a valid u32");
            DeviceRuntimeType::Fpga {
                device,
                baud,
                startup_reset: fpga_startup_reset_from_env(
                    std::env::var("FPGA_HARD_RESET").ok().as_deref(),
                ),
            }
        }
        _ => DeviceRuntimeType::Sim {
            args: SimDeviceRuntimeArgs::default(),
        },
    };

    create_device_runtime(runtime_type, registrations).expect("Failed to create device runtime")
}

/// Load program bytes at `boot_pc` and issue a CPU boot from the same address.
pub fn load_and_boot(runtime: &mut dyn DeviceRuntime, boot_pc: u32, program_bytes: &[u8]) {
    runtime
        .load_program(boot_pc, program_bytes)
        .expect("Failed to load program");
    runtime.boot_cpu(boot_pc).expect("Failed to boot CPU");
}

/// Resolve a named test program ELF path using `sim-tests`.
pub fn resolve_test_elf_path(test_program: &str) -> PathBuf {
    sim_tests::test_program_path(test_program)
        .unwrap_or_else(|e| panic!("Failed to find {test_program}: {e}"))
}

/// Load an ELF into runtime, boot CPU at ELF entry point, and return that entry.
pub fn load_and_boot_elf(runtime: &mut dyn DeviceRuntime, elf_path: &Path) -> u32 {
    let entry = runtime.load_elf(elf_path).expect("Failed to load ELF");
    runtime.boot_cpu(entry).expect("Failed to boot CPU");
    entry
}

/// Resolve and run a named test ELF until CPU halt and return observed tohost value.
pub fn run_elf_until_halt(
    runtime: &mut dyn DeviceRuntime,
    test_program: &str,
    timeout: Duration,
) -> Option<u32> {
    let elf_path = resolve_test_elf_path(test_program);
    load_and_boot_elf(runtime, &elf_path);
    wait_for_cpu_halt(runtime, timeout)
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

/// Poll system controller status until CPU halted bit is set, while capturing
/// any `TohostTermination` event seen during polling.
///
/// Returns the observed tohost termination value if one was emitted before
/// CPU halt. If the CPU halts through another mechanism (e.g. HALT register or
/// invalid instruction), this returns `None`.
///
/// Panics if polling fails or if the timeout expires first.
pub fn wait_for_cpu_halt(runtime: &mut dyn DeviceRuntime, timeout: Duration) -> Option<u32> {
    let start = Instant::now();
    let mut tohost_value = None;

    while start.elapsed() < timeout {
        let remaining = timeout.saturating_sub(start.elapsed());
        if remaining == Duration::ZERO {
            let status = read_word_with_timeout(runtime, sysctrl_status_addr(), SHORT_TIMEOUT);
            if (status & SYSCTRL_STATUS_CPU_HALTED) != 0 {
                return tohost_value;
            }
            break;
        }

        loop {
            match runtime.poll() {
                Ok(Some(BusEvent::TohostTermination { value })) => {
                    if tohost_value.is_none() {
                        tohost_value = Some(value);
                    }
                }
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(e) => panic!("Poll error while waiting for cpu halt: {e}"),
            }
        }

        runtime
            .send_host_request(BusRequest::read(sysctrl_status_addr(), AccessSize::Word))
            .expect("Failed to send host read request for SYSCTRL status");

        let request_start = Instant::now();
        let mut status = None;
        while request_start.elapsed() < remaining {
            match runtime.poll() {
                Ok(Some(BusEvent::TohostTermination { value })) => {
                    if tohost_value.is_none() {
                        tohost_value = Some(value);
                    }
                }
                Ok(Some(BusEvent::HostReadResponse {
                    addr: resp_addr,
                    data,
                    ..
                })) if resp_addr == sysctrl_status_addr()
                    && !runtime.has_pending_host_request() =>
                {
                    status = Some(data);
                    break;
                }
                Ok(Some(_)) => std::thread::sleep(Duration::from_millis(1)),
                Ok(None) => std::thread::sleep(Duration::from_millis(1)),
                Err(e) => panic!("Poll error while waiting for cpu halt: {e}"),
            }
        }

        let status = status.unwrap_or_else(|| {
            panic!("Timed out waiting for SYSCTRL status read response while waiting for cpu halt")
        });
        if (status & SYSCTRL_STATUS_CPU_HALTED) != 0 {
            return tohost_value;
        }
        std::thread::sleep(Duration::from_millis(1));
    }

    panic!(
        "Timed out waiting for cpu halted status bit (last tohost value: {:?})",
        tohost_value
    );
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
/// The sequence writes `tohost_value` to `SIM_CONTROL_BASE`, requests a sticky
/// system-controller halt, and then loops locally as a fallback.
pub fn tohost_termination(addr_reg: u32, value_reg: u32, tohost_value: u32) -> [u32; 6] {
    [
        lui(addr_reg, SIM_CONTROL_BASE),
        addi(
            value_reg,
            0,
            i32::try_from(tohost_value).expect("tohost value must fit in i32 immediate"),
        ),
        sw(addr_reg, value_reg, 0),
        lui(addr_reg, sysctrl_halt_addr() & 0xFFFF_F000),
        sw(
            addr_reg,
            value_reg,
            i32::try_from(sysctrl_halt_addr() & 0xFFF).expect("sysctrl halt offset must fit"),
        ),
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
