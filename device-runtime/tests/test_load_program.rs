mod common;

use common::{
    create_test_runtime, instructions_to_bytes, load_and_boot, read_word_with_timeout,
    wait_for_cpu_halt, LONG_TIMEOUT,
};
use riscv_core::instruction::{addi, ebreak, lui, sw};
use riscv_shared::bus::{
    sysctrl_halt_addr, sysctrl_status_addr, DRAM_BASE, SIM_CONTROL_BASE, SRAM_BASE, SYSCTRL_BASE,
    SYSCTRL_HALT_OFFSET, SYSCTRL_STATUS_CPU_HALTED,
};

/// Build a simple program that writes a success code to the tohost address
/// and then halts via EBREAK.
///
/// The program:
///   LUI  x15, SIM_CONTROL_BASE   ; load tohost base address into x15
///   ADDI x14, x0, 1              ; load success code (1) into x14
///   SW   x14, 0(x15)             ; store x14 to tohost address
///   EBREAK                       ; halt execution
fn build_tohost_program() -> Vec<u8> {
    let instructions = vec![
        lui(15, SIM_CONTROL_BASE), // Load SIM_CONTROL_BASE into x15
        addi(14, 0, 1),            // Load success code (1) into x14
        sw(15, 14, 0),             // Store x14 to address in x15 (tohost)
        ebreak(),                  // Halt execution
    ];
    instructions_to_bytes(&instructions)
}

#[test]
fn test_load_program_and_tohost_termination() {
    let mut runtime = create_test_runtime();

    // Load the program bytes at DRAM_BASE
    let boot_pc: u32 = DRAM_BASE;
    let program = build_tohost_program();
    load_and_boot(runtime.as_mut(), boot_pc, &program);
    assert_eq!(wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT), Some(1));

    // Confirm CPU has halted by reading the system controller STATUS register
    let status = read_word_with_timeout(runtime.as_mut(), sysctrl_status_addr(), LONG_TIMEOUT);
    let cpu_halted = (status & SYSCTRL_STATUS_CPU_HALTED) != 0;

    assert!(cpu_halted, "CPU did not halt after EBREAK");
}

#[test]
fn test_load_program_halt_register_termination_code() {
    let mut runtime = create_test_runtime();

    let boot_pc: u32 = DRAM_BASE;
    let halt_code: u32 = 0x5A5;
    let halt_offset: i32 = i32::try_from(SYSCTRL_HALT_OFFSET).expect("HALT offset must fit i32");

    // Program:
    //   LUI  x15, SYSCTRL_BASE
    //   ADDI x14, x0, halt_code
    //   SW   x14, 0x0C(x15)   ; write HALT register, requesting CPU halt
    let instructions = vec![
        lui(15, SYSCTRL_BASE),
        addi(14, 0, halt_code as i32),
        sw(15, 14, halt_offset),
    ];
    let program = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), boot_pc, &program);

    assert_eq!(wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT), None);

    let read_halt_code =
        read_word_with_timeout(runtime.as_mut(), sysctrl_halt_addr(), LONG_TIMEOUT);
    assert_eq!(
        read_halt_code, halt_code,
        "HALT register should retain termination code for host retrieval"
    );
}

#[test]
fn test_load_program_runs_from_sram_and_reports_tohost() {
    let mut runtime = create_test_runtime();
    let boot_pc: u32 = SRAM_BASE;
    let tohost_value: i32 = 0x2A;
    let program = instructions_to_bytes(&[
        lui(15, SIM_CONTROL_BASE),
        addi(14, 0, tohost_value),
        sw(15, 14, 0),
        ebreak(),
    ]);

    runtime
        .load_program(boot_pc, &program)
        .expect("Failed to load SRAM program");
    runtime.boot_cpu(boot_pc).expect("Failed to boot from SRAM");

    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(tohost_value as u32)
    );
}
