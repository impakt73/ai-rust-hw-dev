use device_runtime::{create_device_runtime, BusEvent, DeviceRuntimeType};
use riscv_core::instruction::{addi, ebreak, lui, sw};
use std::time::{Duration, Instant};

/// Create a device runtime based on environment variables.
///
/// If `FPGA_DEVICE_PATH` and `FPGA_BAUD_RATE` are set, the FPGA backend is used.
/// Otherwise, the simulation backend is used by default.
fn create_test_runtime() -> Box<dyn device_runtime::DeviceRuntime> {
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

/// Build a simple program that writes a success code to the tohost address
/// and then halts via EBREAK.
///
/// The program:
///   LUI  x15, SIM_CONTROL_BASE   ; load tohost base address into x15
///   ADDI x14, x0, 1              ; load success code (1) into x14
///   SW   x14, 0(x15)             ; store x14 to tohost address
///   EBREAK                       ; halt execution
fn build_tohost_program() -> Vec<u8> {
    let sim_control_base: u32 = 0x4000_0000;
    let instructions = vec![
        lui(15, sim_control_base), // Load SIM_CONTROL_BASE into x15
        addi(14, 0, 1),            // Load success code (1) into x14
        sw(15, 14, 0),             // Store x14 to address in x15 (tohost)
        ebreak(),                  // Halt execution
    ];
    instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect()
}

#[test]
fn test_load_program_and_tohost_termination() {
    let mut runtime = create_test_runtime();

    // Load the program bytes at DRAM_BASE
    let boot_pc: u32 = 0x8000_0000;
    let program = build_tohost_program();
    runtime
        .load_program(boot_pc, &program)
        .expect("Failed to load program");

    // Boot the CPU from the same address used for load_program
    runtime.boot_cpu(boot_pc).expect("Failed to boot CPU");

    // Poll for tohost termination with a timeout
    let timeout = Duration::from_secs(10);
    let start = Instant::now();
    let mut tohost_value = None;

    while start.elapsed() < timeout {
        match runtime.poll() {
            Ok(Some(BusEvent::TohostTermination { value })) => {
                tohost_value = Some(value);
                break;
            }
            Ok(Some(_)) => {
                // Ignore other events, keep polling
            }
            Ok(None) => {
                // No event ready; yield briefly
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(e) => {
                panic!("Poll error: {}", e);
            }
        }
    }

    // Verify tohost value matches expected success code
    assert_eq!(
        tohost_value,
        Some(1),
        "Expected tohost termination with value 1"
    );
}
