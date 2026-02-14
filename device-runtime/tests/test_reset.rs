mod common;

use common::{create_test_runtime, read_word_with_timeout, MEDIUM_TIMEOUT};
use device_runtime::ResetKind;
use riscv_shared::bus::{sysctrl_status_addr, SYSCTRL_STATUS_CPU_BOOTING};

fn initialize_runtime(runtime: &mut dyn device_runtime::DeviceRuntime) {
    runtime
        .load_program(0x8000_0000, &[])
        .expect("Failed to initialize runtime");
}

#[test]
fn test_reset_cpu_path() {
    let mut runtime = create_test_runtime();
    initialize_runtime(runtime.as_mut());
    runtime.reset(ResetKind::Cpu).expect("CPU reset failed");
    let status = read_word_with_timeout(runtime.as_mut(), sysctrl_status_addr(), MEDIUM_TIMEOUT);
    assert_ne!(status & SYSCTRL_STATUS_CPU_BOOTING, 0);
}

#[test]
fn test_reset_system_path() {
    let mut runtime = create_test_runtime();
    initialize_runtime(runtime.as_mut());
    runtime
        .reset(ResetKind::System)
        .expect("System reset failed");
    let status = read_word_with_timeout(runtime.as_mut(), sysctrl_status_addr(), MEDIUM_TIMEOUT);
    assert_ne!(status & SYSCTRL_STATUS_CPU_BOOTING, 0);
}
