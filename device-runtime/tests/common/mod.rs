#![allow(dead_code)]

use device_runtime::{
    create_device_runtime, BusEvent, BusRequest, DeviceRuntime, DeviceRuntimeType,
};
use host_bus_handler::AccessSize;
use riscv_core::instruction::{addi, jal, lui, sw};
use riscv_shared::bus::SIM_CONTROL_BASE;
use std::time::{Duration, Instant};

pub const SHORT_TIMEOUT: Duration = Duration::from_secs(2);
pub const MEDIUM_TIMEOUT: Duration = Duration::from_secs(10);
pub const LONG_TIMEOUT: Duration = Duration::from_secs(30);
pub const DEFAULT_BOOT_PC: u32 = 0x8000_0000;

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

pub fn instructions_to_bytes(instructions: &[u32]) -> Vec<u8> {
    instructions
        .iter()
        .flat_map(|instr| instr.to_le_bytes())
        .collect()
}

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

pub fn append_tohost_termination(
    instructions: &mut Vec<u32>,
    addr_reg: u32,
    value_reg: u32,
    tohost_value: u32,
) {
    instructions.extend(tohost_termination(addr_reg, value_reg, tohost_value));
}

pub fn load_and_boot(runtime: &mut dyn DeviceRuntime, boot_pc: u32, program_bytes: &[u8]) {
    runtime
        .load_program(boot_pc, program_bytes)
        .expect("Failed to load program");
    runtime.boot_cpu(boot_pc).expect("Failed to boot CPU");
}

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
            Ok(Some(BusEvent::HostRequestTimeout { addr: timeout_addr }))
                if timeout_addr == addr =>
            {
                panic!("Timed out waiting for host read response at 0x{addr:08X}");
            }
            Ok(Some(_)) => {}
            Ok(None) => std::thread::sleep(Duration::from_millis(1)),
            Err(e) => panic!("Poll error while waiting for read response at 0x{addr:08X}: {e}"),
        }
    }
    panic!("Timed out waiting for host read response at 0x{addr:08X}");
}

pub fn wait_for_host_write_response(runtime: &mut dyn DeviceRuntime, addr: u32, timeout: Duration) {
    let start = Instant::now();
    while start.elapsed() < timeout {
        match runtime.poll() {
            Ok(Some(BusEvent::HostWriteResponse {
                addr: resp_addr, ..
            })) if resp_addr == addr && !runtime.has_pending_host_request() => {
                return;
            }
            Ok(Some(BusEvent::HostRequestTimeout { addr: timeout_addr }))
                if timeout_addr == addr =>
            {
                panic!("Timed out waiting for host write response at 0x{addr:08X}");
            }
            Ok(Some(_)) => {}
            Ok(None) => std::thread::sleep(Duration::from_millis(1)),
            Err(e) => panic!("Poll error while waiting for write response at 0x{addr:08X}: {e}"),
        }
    }
    panic!("Timed out waiting for host write response at 0x{addr:08X}");
}

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

pub fn write_word_with_timeout(
    runtime: &mut dyn DeviceRuntime,
    addr: u32,
    data: u32,
    timeout: Duration,
) {
    runtime
        .send_host_request(BusRequest::write(addr, data, AccessSize::Word))
        .expect("Failed to send host write request");
    wait_for_host_write_response(runtime, addr, timeout);
}

pub fn drain_events_until_idle(runtime: &mut dyn DeviceRuntime, timeout: Duration) {
    let start = Instant::now();
    while start.elapsed() < timeout {
        let has_pending = runtime.has_pending_host_request();
        match runtime.poll() {
            Ok(Some(_)) => {}
            Ok(None) if !has_pending => return,
            Ok(None) => std::thread::sleep(Duration::from_millis(1)),
            Err(e) => panic!("Poll error while draining events: {e}"),
        }
    }
    panic!("Timed out draining runtime events");
}
