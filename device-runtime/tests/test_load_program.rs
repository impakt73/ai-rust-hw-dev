use riscv_core::instruction::{addi, ebreak, lui, sw};
use riscv_shared::bus::{sysctrl_halt_addr, sysctrl_status_addr, SYSCTRL_STATUS_CPU_HALTED};
use std::time::Duration;

mod common;

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
    let instructions = [
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
    let mut runtime = common::create_test_runtime();

    // Load the program bytes at DRAM_BASE
    let boot_pc = common::DEFAULT_BOOT_PC;
    let program = build_tohost_program();
    common::load_and_boot(runtime.as_mut(), boot_pc, &program);

    // Verify tohost value matches expected success code
    assert_eq!(
        common::wait_for_tohost(runtime.as_mut(), common::MEDIUM_TIMEOUT),
        1,
        "Expected tohost termination with value 1"
    );

    // Confirm CPU has halted by reading the system controller STATUS register
    let status_addr = sysctrl_status_addr();
    let status =
        common::read_word_with_timeout(runtime.as_mut(), status_addr, common::MEDIUM_TIMEOUT);
    let cpu_halted = (status & SYSCTRL_STATUS_CPU_HALTED) != 0;

    assert!(cpu_halted, "CPU did not halt after EBREAK");
}

#[test]
fn test_load_program_halt_register_termination_code() {
    let mut runtime = common::create_test_runtime();

    let boot_pc = common::DEFAULT_BOOT_PC;
    let halt_code: u32 = 0x5A5;
    let sysctrl_base: u32 = 0x5300_0000;
    let halt_offset: i32 = 0x0C;

    // Program:
    //   LUI  x15, SYSCTRL_BASE
    //   ADDI x14, x0, halt_code
    //   SW   x14, 0x0C(x15)   ; write HALT register, requesting CPU halt
    let program: Vec<u8> = [
        lui(15, sysctrl_base),
        addi(14, 0, halt_code as i32),
        sw(15, 14, halt_offset),
    ]
    .iter()
    .flat_map(|inst| inst.to_le_bytes())
    .collect();

    common::load_and_boot(runtime.as_mut(), boot_pc, &program);

    let status_addr = sysctrl_status_addr();
    let timeout = Duration::from_secs(10);
    let status = common::read_word_with_timeout(runtime.as_mut(), status_addr, timeout);
    let cpu_halted = (status & SYSCTRL_STATUS_CPU_HALTED) != 0;

    assert!(
        cpu_halted,
        "CPU did not enter halted state via HALT register"
    );

    let read_halt_code =
        common::read_word_with_timeout(runtime.as_mut(), sysctrl_halt_addr(), timeout);
    assert_eq!(
        read_halt_code, halt_code,
        "HALT register should retain termination code for host retrieval"
    );
}
