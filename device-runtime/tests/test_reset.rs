use device_runtime::{
    create_device_runtime, BusEvent, BusRequest, DeviceRuntime, DeviceRuntimeType, ResetKind,
};
use host_bus_handler::AccessSize;
use riscv_shared::bus::{sysctrl_status_addr, SYSCTRL_STATUS_CPU_BOOTING};
use std::time::{Duration, Instant};

fn create_sim_runtime() -> Box<dyn DeviceRuntime> {
    create_device_runtime(DeviceRuntimeType::Sim).expect("Failed to create sim runtime")
}

fn initialize_runtime(runtime: &mut dyn DeviceRuntime) {
    runtime
        .load_program(0x8000_0000, &[])
        .expect("Failed to initialize runtime");
}

fn read_status(runtime: &mut dyn DeviceRuntime) -> u32 {
    let status_addr = sysctrl_status_addr();
    runtime
        .send_host_request(BusRequest::read(status_addr, AccessSize::Word))
        .expect("Failed to send STATUS read request");

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        match runtime.poll() {
            Ok(Some(BusEvent::HostReadResponse { addr, data, .. })) if addr == status_addr => {
                return data;
            }
            Ok(Some(_)) => {}
            Ok(None) => std::thread::sleep(Duration::from_millis(1)),
            Err(e) => panic!("Poll failed while reading STATUS: {}", e),
        }
    }

    panic!("Timed out waiting for STATUS read response");
}

#[test]
fn test_reset_cpu_path() {
    let mut runtime = create_sim_runtime();
    initialize_runtime(runtime.as_mut());
    runtime.reset(ResetKind::Cpu).expect("CPU reset failed");
    let status = read_status(runtime.as_mut());
    assert_ne!(status & SYSCTRL_STATUS_CPU_BOOTING, 0);
}

#[test]
fn test_reset_system_path() {
    let mut runtime = create_sim_runtime();
    initialize_runtime(runtime.as_mut());
    runtime
        .reset(ResetKind::System)
        .expect("System reset failed");
    let status = read_status(runtime.as_mut());
    assert_ne!(status & SYSCTRL_STATUS_CPU_BOOTING, 0);
}
