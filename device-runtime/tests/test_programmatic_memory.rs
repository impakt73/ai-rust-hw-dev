mod common;

use common::{
    create_test_runtime, instructions_to_bytes, load_and_boot, wait_for_cpu_halt, LONG_TIMEOUT,
    TEST_BOOT_PC,
};
use riscv_core::instruction::{addi, ebreak, jal, lbu, lui, sw};
use riscv_shared::bus::{DRAM_BASE, SIM_CONTROL_BASE};

/// Test that demonstrates loading and executing programmatic instructions without an ELF file.
#[test]
fn test_programmatic_instruction_loading() {
    let mut runtime = create_test_runtime();

    let instructions = vec![
        addi(10, 0, 42),
        lui(11, SIM_CONTROL_BASE),
        sw(11, 10, 0),
        ebreak(),
        jal(0, 0),
    ];
    let program = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program);
    assert_eq!(wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT), Some(42));
}

/// Test programmatic memory write/overwrite patterns using CPU-side DRAM accesses.
#[test]
fn test_write_memory_region_patterns() {
    let mut runtime = create_test_runtime();
    let checker_program = instructions_to_bytes(&[
        lui(12, DRAM_BASE), // x12 = DRAM base
        addi(10, 0, 0x12),
        sw(12, 10, 0x1000), // seed pattern at +0x1000
        addi(10, 0, 0x34),
        sw(12, 10, 0x1004), // independent region write
        addi(10, 0, 0xFF),
        sw(12, 10, 0x1000),  // overwrite first region
        lbu(10, 12, 0x1000), // read overwritten byte
        lui(11, SIM_CONTROL_BASE),
        sw(11, 10, 0), // tohost = 0xFF
        ebreak(),
        jal(0, 0),
    ]);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &checker_program);
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(0xFF)
    );
}
