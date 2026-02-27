mod common;

use common::{append_tohost_termination, instructions_to_bytes, wait_for_cpu_halt, TEST_BOOT_PC};
use device_runtime::{create_device_runtime, DeviceRuntimeType, SimDeviceRuntimeArgs};
use riscv_core::instruction::{add, addi};
use riscv_core::trace::{InstructionTrace, InstructionType};
use std::sync::{Mutex, OnceLock};

static TRACE_LOG: OnceLock<Mutex<Vec<InstructionTrace>>> = OnceLock::new();

fn trace_collector(trace: &InstructionTrace) {
    TRACE_LOG
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .expect("trace log lock poisoned")
        .push(trace.clone());
}

#[test]
fn test_trace_callback_with_public_runtime_api() {
    TRACE_LOG
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .expect("trace log lock poisoned")
        .clear();

    let mut runtime = create_device_runtime(
        DeviceRuntimeType::Sim {
            args: SimDeviceRuntimeArgs {
                instruction_trace_callback: Some(trace_collector),
                ..Default::default()
            },
        },
        None,
    )
    .expect("Failed to create simulator runtime");

    let mut instructions = vec![addi(1, 0, 10), addi(2, 0, 20), add(3, 1, 2)];
    append_tohost_termination(&mut instructions, 15, 16, 42);
    let program = instructions_to_bytes(&instructions);

    runtime
        .load_program(TEST_BOOT_PC, &program)
        .expect("Failed to load program");
    runtime.boot_cpu(TEST_BOOT_PC).expect("Failed to boot cpu");

    let tohost = wait_for_cpu_halt(runtime.as_mut(), std::time::Duration::from_secs(10));
    assert_eq!(tohost, Some(42));

    let traces = TRACE_LOG
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .expect("trace log lock poisoned");
    assert!(
        !traces.is_empty(),
        "trace callback should capture instructions"
    );

    assert!(
        traces
            .iter()
            .any(|t| t.inst_type == InstructionType::Addi
                && t.rd.as_ref().map(|rd| rd.reg) == Some(1)),
        "expected addi x1 trace entry"
    );
    assert!(
        traces
            .iter()
            .any(|t| t.inst_type == InstructionType::Add
                && t.rd.as_ref().map(|rd| rd.reg) == Some(3)),
        "expected add x3 trace entry"
    );
}

#[test]
fn test_vcd_generation_with_public_runtime_api() {
    // Keep this test scoped to simulator backend where VCD is supported.
    if std::env::var("FPGA_DEVICE_PATH").is_ok() {
        return;
    }

    let vcd_path = std::env::temp_dir().join(format!(
        "device_runtime_trace_test_{}_{}.vcd",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos()
    ));

    let mut runtime = create_device_runtime(
        DeviceRuntimeType::Sim {
            args: SimDeviceRuntimeArgs {
                vcd_path: Some(vcd_path.to_string_lossy().into_owned()),
                ..Default::default()
            },
        },
        None,
    )
    .expect("Failed to create simulator runtime with VCD");

    let mut instructions = vec![addi(1, 0, 42)];
    append_tohost_termination(&mut instructions, 15, 16, 42);
    let program = instructions_to_bytes(&instructions);

    runtime
        .load_program(TEST_BOOT_PC, &program)
        .expect("Failed to load program");
    runtime.boot_cpu(TEST_BOOT_PC).expect("Failed to boot cpu");

    let tohost = wait_for_cpu_halt(runtime.as_mut(), std::time::Duration::from_secs(10));
    assert_eq!(tohost, Some(42));

    let vcd = std::fs::read_to_string(&vcd_path).expect("Expected VCD file to be generated");
    assert!(vcd.contains("$version"));
    assert!(vcd.contains("$timescale"));
    assert!(vcd.contains("clk"));

    let _ = std::fs::remove_file(&vcd_path);
}
