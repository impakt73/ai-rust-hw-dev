//! Host-Initiated Bus Request Integration Tests

mod common;

use device_runtime::BusRequest;
use host_bus_handler::AccessSize;
use riscv_core::instruction::*;
use riscv_shared::bus::{led_out_addr, sysctrl_status_addr, LED_BASE, SYSCTRL_STATUS_CPU_HALTED};
use riscv_shared::sim_control::{FAILURE_CODE, SUCCESS_CODE};
use std::time::{Duration, Instant};

#[test]
fn test_host_initiated_basic_sync() {
    let mut instructions = vec![
        lui(15, LED_BASE),
        lw(14, 15, 0),
        andi(14, 14, 0xFF),
        beq(14, 0, -8),
    ];
    common::append_tohost_termination(&mut instructions, 10, 11, SUCCESS_CODE);

    let mut runtime = common::create_test_runtime();
    let program_bytes = common::instructions_to_bytes(&instructions);
    common::load_and_boot(runtime.as_mut(), common::DEFAULT_BOOT_PC, &program_bytes);

    runtime
        .send_host_request(BusRequest::write(LED_BASE, 0x01, AccessSize::Byte))
        .expect("Should queue host request");
    common::wait_for_host_write_response(runtime.as_mut(), LED_BASE, common::MEDIUM_TIMEOUT);

    assert_eq!(
        common::wait_for_tohost(runtime.as_mut(), common::MEDIUM_TIMEOUT),
        SUCCESS_CODE
    );
}

#[test]
fn test_host_initiated_led_write() {
    let instructions = vec![
        lui(15, LED_BASE),
        lw(12, 15, 0),
        andi(12, 12, 0xFF),
        beq(12, 0, -8),
        andi(10, 12, 0xFF),
        addi(11, 0, 0xA5),
        bne(10, 11, 16),
        lui(9, riscv_shared::bus::SIM_CONTROL_BASE),
        addi(7, 0, SUCCESS_CODE as i32),
        sw(9, 7, 0),
        jal(0, 0),
        lui(9, riscv_shared::bus::SIM_CONTROL_BASE),
        addi(7, 0, FAILURE_CODE as i32),
        sw(9, 7, 0),
        jal(0, 0),
    ];

    let mut runtime = common::create_test_runtime();
    let program_bytes = common::instructions_to_bytes(&instructions);
    common::load_and_boot(runtime.as_mut(), common::DEFAULT_BOOT_PC, &program_bytes);

    runtime
        .send_host_request(BusRequest::write(LED_BASE, 0xA5, AccessSize::Byte))
        .expect("Should queue host request");
    common::wait_for_host_write_response(runtime.as_mut(), LED_BASE, common::MEDIUM_TIMEOUT);

    assert_eq!(
        common::wait_for_tohost(runtime.as_mut(), common::MEDIUM_TIMEOUT),
        SUCCESS_CODE
    );
}

#[test]
fn test_host_initiated_led_read() {
    let mut instructions = vec![lui(15, LED_BASE), addi(14, 0, 0xCC), sw(15, 14, 0)];
    common::append_tohost_termination(&mut instructions, 7, 8, SUCCESS_CODE);

    let mut runtime = common::create_test_runtime();
    let program_bytes = common::instructions_to_bytes(&instructions);
    common::load_and_boot(runtime.as_mut(), common::DEFAULT_BOOT_PC, &program_bytes);
    assert_eq!(
        common::wait_for_tohost(runtime.as_mut(), common::MEDIUM_TIMEOUT),
        SUCCESS_CODE
    );

    runtime
        .send_host_request(BusRequest::read(led_out_addr(), AccessSize::Word))
        .expect("Should queue host read request");
    let led_value = common::wait_for_host_read_response(
        runtime.as_mut(),
        led_out_addr(),
        common::SHORT_TIMEOUT,
    );
    assert_eq!(led_value & 0xFF, 0xCC, "LED value should be 0xCC");
}

#[test]
fn test_host_request_address_validation() {
    let mut instructions = vec![
        lui(15, LED_BASE),
        lw(14, 15, 0),
        andi(14, 14, 0xFF),
        beq(14, 0, -8),
    ];
    common::append_tohost_termination(&mut instructions, 7, 8, SUCCESS_CODE);

    let mut runtime = common::create_test_runtime();
    let program_bytes = common::instructions_to_bytes(&instructions);
    common::load_and_boot(runtime.as_mut(), common::DEFAULT_BOOT_PC, &program_bytes);

    runtime
        .send_host_request(BusRequest::write(LED_BASE, 0x01, AccessSize::Byte))
        .expect("Request to RTL peripheral space should succeed");
    let pending_result =
        runtime.send_host_request(BusRequest::write(LED_BASE + 4, 0x02, AccessSize::Byte));
    assert!(pending_result.is_err(), "Request while pending should fail");
    common::wait_for_host_write_response(runtime.as_mut(), LED_BASE, common::MEDIUM_TIMEOUT);

    assert_eq!(
        common::wait_for_tohost(runtime.as_mut(), common::MEDIUM_TIMEOUT),
        SUCCESS_CODE
    );
}

#[test]
fn test_multiple_host_requests() {
    let mut instructions = vec![
        lui(15, LED_BASE),
        addi(14, 0, 3),
        lw(12, 15, 0),
        andi(12, 12, 0xFF),
        blt(12, 14, -8),
    ];
    common::append_tohost_termination(&mut instructions, 10, 11, SUCCESS_CODE);

    let mut runtime = common::create_test_runtime();
    let program_bytes = common::instructions_to_bytes(&instructions);
    common::load_and_boot(runtime.as_mut(), common::DEFAULT_BOOT_PC, &program_bytes);

    for value in 1..=3u32 {
        runtime
            .send_host_request(BusRequest::write(LED_BASE, value, AccessSize::Byte))
            .expect("Should queue host request");
        common::wait_for_host_write_response(runtime.as_mut(), LED_BASE, common::MEDIUM_TIMEOUT);
    }

    assert_eq!(
        common::wait_for_tohost(runtime.as_mut(), common::MEDIUM_TIMEOUT),
        SUCCESS_CODE
    );
}

#[test]
fn test_host_bus_works_after_halt() {
    let instructions = vec![0, 0, 0, 0];
    let mut runtime = common::create_test_runtime();
    let program_bytes = common::instructions_to_bytes(&instructions);
    common::load_and_boot(runtime.as_mut(), common::DEFAULT_BOOT_PC, &program_bytes);

    let status_addr = sysctrl_status_addr();
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut status_value = None;

    while Instant::now() < deadline {
        runtime
            .send_host_request(BusRequest::read(status_addr, AccessSize::Word))
            .expect("Should queue status read request");
        let status = common::wait_for_host_read_response(
            runtime.as_mut(),
            status_addr,
            common::SHORT_TIMEOUT,
        );
        if (status & SYSCTRL_STATUS_CPU_HALTED) != 0 {
            status_value = Some(status);
            break;
        }
    }

    let status = status_value.expect("CPU never reported halted state through host bus");
    assert!(
        (status & SYSCTRL_STATUS_CPU_HALTED) != 0,
        "STATUS register should show cpu_halted=1"
    );
}
