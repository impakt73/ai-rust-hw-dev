mod common;

use common::{append_tohost_termination, instructions_to_bytes, wait_for_cpu_halt, TEST_BOOT_PC};
use device_runtime::{create_device_runtime, DeviceRuntimeType, SimDeviceRuntimeArgs};
use riscv_core::instruction::addi;

fn run_with_latency(memory_latency_cycles: u32) {
    let mut runtime = create_device_runtime(
        DeviceRuntimeType::Sim {
            args: SimDeviceRuntimeArgs {
                memory_latency_cycles,
                ..Default::default()
            },
        },
        None,
    )
    .expect("Failed to create simulator runtime");

    let mut instructions = vec![addi(1, 0, 42)];
    append_tohost_termination(&mut instructions, 15, 16, 42);
    let program = instructions_to_bytes(&instructions);

    runtime
        .load_program(TEST_BOOT_PC, &program)
        .expect("Failed to load program");
    runtime.boot_cpu(TEST_BOOT_PC).expect("Failed to boot cpu");

    let tohost = wait_for_cpu_halt(runtime.as_mut(), std::time::Duration::from_secs(10));
    assert_eq!(tohost, Some(42));
}

#[test]
fn test_zero_latency_runtime_execution() {
    run_with_latency(0);
}

#[test]
fn test_nonzero_latency_runtime_execution() {
    run_with_latency(3);
}
