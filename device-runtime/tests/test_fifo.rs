mod common;

use bus_shared::{Fifo, FifoDataSource};
use common::{
    append_register_tohost_termination, create_test_runtime_with_registrations,
    instructions_to_bytes, load_and_boot, wait_for_cpu_halt, TEST_BOOT_PC,
};
use device_runtime::BusDeviceRegistration;
use riscv_core::instruction::{addi, andi, beq, jal, lbu, lui, lw, sb};
use riscv_shared::bus::FIFO_BASE;
use riscv_shared::sim_control::SUCCESS_CODE;
use riscv_shared::FIFO_DATA;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn fifo_echo_program() -> Vec<u8> {
    let mut instructions = vec![
        lui(1, FIFO_DATA),
        addi(2, 1, 4),
        lw(3, 2, 0),
        andi(4, 3, 1),
        beq(4, 0, 20),
        lbu(5, 1, 0),
        beq(5, 0, 12),
        sb(1, 5, 0),
        jal(0, -24),
        lui(6, riscv_shared::bus::SIM_CONTROL_BASE),
        addi(
            7,
            0,
            i32::try_from(SUCCESS_CODE).expect("SUCCESS_CODE fits immediate"),
        ),
    ];
    append_register_tohost_termination(&mut instructions, 6, 7);

    instructions_to_bytes(&instructions)
}

#[test]
fn test_fifo_echo_via_runtime_api() {
    let captured = Arc::new(Mutex::new(Vec::<u8>::new()));
    let captured_clone = Arc::clone(&captured);

    let fifo_source = Arc::new(Mutex::new(FifoDataSource::new()));
    let fifo_source_for_test = Arc::clone(&fifo_source);
    let test_string = "Qu1ck_Br0wn-F0x!Jump5*0v3r@Lazy#D0g$2024%";
    let fifo = Fifo::new_with_callback(
        fifo_source,
        Box::new(move |byte| {
            captured_clone
                .lock()
                .expect("captured lock poisoned")
                .push(byte);
        }),
    );

    let mut runtime = create_test_runtime_with_registrations(Some(vec![BusDeviceRegistration {
        base_addr: FIFO_BASE,
        device: Box::new(fifo),
    }]));

    // Runtime initialization resets devices, so seed FIFO after runtime creation.
    fifo_source_for_test
        .lock()
        .expect("fifo source lock poisoned")
        .push_string_to_fifo_rx(test_string);

    let program = fifo_echo_program();
    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program);

    let tohost = wait_for_cpu_halt(runtime.as_mut(), Duration::from_secs(10));
    assert_eq!(tohost, Some(SUCCESS_CODE));

    let echoed = String::from_utf8(
        captured
            .lock()
            .expect("captured lock poisoned")
            .iter()
            .copied()
            .filter(|byte| *byte != 0)
            .collect::<Vec<u8>>(),
    )
    .expect("echoed bytes should be utf-8");

    assert_eq!(echoed, test_string);
}
