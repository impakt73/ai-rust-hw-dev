use device_runtime::ResetKind;
use riscv_shared::bus::{sysctrl_status_addr, SYSCTRL_STATUS_CPU_BOOTING};

mod common;

#[test]
fn test_reset_cpu_path() {
    let mut runtime = common::create_test_runtime();
    runtime
        .load_program(common::DEFAULT_BOOT_PC, &[])
        .expect("Failed to initialize runtime");
    runtime.reset(ResetKind::Cpu).expect("CPU reset failed");
    let status = common::read_word_with_timeout(
        runtime.as_mut(),
        sysctrl_status_addr(),
        common::SHORT_TIMEOUT,
    );
    assert_ne!(status & SYSCTRL_STATUS_CPU_BOOTING, 0);
}

#[test]
fn test_reset_system_path() {
    let mut runtime = common::create_test_runtime();
    runtime
        .load_program(common::DEFAULT_BOOT_PC, &[])
        .expect("Failed to initialize runtime");
    runtime
        .reset(ResetKind::System)
        .expect("System reset failed");
    let status = common::read_word_with_timeout(
        runtime.as_mut(),
        sysctrl_status_addr(),
        common::SHORT_TIMEOUT,
    );
    assert_ne!(status & SYSCTRL_STATUS_CPU_BOOTING, 0);
}
