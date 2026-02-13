use device_runtime::{create_device_runtime, BusEvent, BusRequest, DeviceRuntimeType};
use host_bus_handler::AccessSize;
use riscv_core::instruction::{addi, ebreak, lui, sw};
use riscv_shared::bus::{sysctrl_halt_addr, sysctrl_status_addr, SYSCTRL_STATUS_CPU_HALTED};
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

fn read_word_with_timeout(
    runtime: &mut dyn device_runtime::DeviceRuntime,
    addr: u32,
    timeout: Duration,
) -> u32 {
    runtime
        .send_host_request(BusRequest::read(addr, AccessSize::Word))
        .expect("Failed to send host read request");

    let start = Instant::now();
    while start.elapsed() < timeout {
        match runtime.poll() {
            Ok(Some(BusEvent::HostReadResponse {
                addr: resp_addr,
                data,
                ..
            })) if resp_addr == addr && !runtime.has_pending_host_request() => {
                return data;
            }
            Ok(Some(_)) => {}
            Ok(None) => std::thread::sleep(Duration::from_millis(1)),
            Err(e) => panic!("Poll error while reading 0x{addr:08X}: {e}"),
        }
    }

    panic!("Timed out waiting for read response at 0x{addr:08X}");
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

    // Confirm CPU has halted by reading the system controller STATUS register
    let status_addr = sysctrl_status_addr();
    let request = BusRequest::read(status_addr, AccessSize::Word);
    runtime
        .send_host_request(request)
        .expect("Failed to send STATUS read request");

    let mut cpu_halted = false;
    let halt_start = Instant::now();
    while halt_start.elapsed() < timeout {
        match runtime.poll() {
            Ok(Some(BusEvent::HostReadResponse { addr, data, .. }))
                if addr == status_addr && !runtime.has_pending_host_request() =>
            {
                cpu_halted = (data & SYSCTRL_STATUS_CPU_HALTED) != 0;
                break;
            }
            Ok(Some(_)) => {}
            Ok(None) => {
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(e) => {
                panic!("Poll error while checking CPU halt: {}", e);
            }
        }
    }

    assert!(cpu_halted, "CPU did not halt after EBREAK");
}

#[test]
fn test_load_program_halt_register_termination_code() {
    let mut runtime = create_test_runtime();

    let boot_pc: u32 = 0x8000_0000;
    let halt_code: u32 = 0x5A5;
    let sysctrl_base: u32 = 0x5300_0000;
    let halt_offset: i32 = 0x0C;

    // Program:
    //   LUI  x15, SYSCTRL_BASE
    //   ADDI x14, x0, halt_code
    //   SW   x14, 0x0C(x15)   ; write HALT register, requesting CPU halt
    let program: Vec<u8> = vec![
        lui(15, sysctrl_base),
        addi(14, 0, halt_code as i32),
        sw(15, 14, halt_offset),
    ]
    .iter()
    .flat_map(|inst| inst.to_le_bytes())
    .collect();

    runtime
        .load_program(boot_pc, &program)
        .expect("Failed to load HALT register test program");
    runtime.boot_cpu(boot_pc).expect("Failed to boot CPU");

    let status_addr = sysctrl_status_addr();
    let timeout = Duration::from_secs(10);
    let start = Instant::now();
    let mut cpu_halted = false;

    while start.elapsed() < timeout {
        let status = read_word_with_timeout(runtime.as_mut(), status_addr, timeout);
        if (status & SYSCTRL_STATUS_CPU_HALTED) != 0 {
            cpu_halted = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }

    assert!(
        cpu_halted,
        "CPU did not enter halted state via HALT register"
    );

    let read_halt_code = read_word_with_timeout(runtime.as_mut(), sysctrl_halt_addr(), timeout);
    assert_eq!(
        read_halt_code, halt_code,
        "HALT register should retain termination code for host retrieval"
    );
}
