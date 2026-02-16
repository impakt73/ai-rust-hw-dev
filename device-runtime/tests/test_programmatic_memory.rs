mod common;

use common::{
    create_test_runtime, instructions_to_bytes, load_and_boot, read_word_with_timeout,
    wait_for_cpu_halt, LONG_TIMEOUT, SHORT_TIMEOUT, TEST_BOOT_PC,
};
use riscv_core::instruction::{addi, beq, ebreak, jal, lbu, lui, sw};
use riscv_shared::bus::{DRAM_BASE, SIM_CONTROL_BASE, SRAM_BASE};

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
        lui(12, DRAM_BASE),  // x12 = DRAM base
        addi(13, 12, 0x400), // x13 = DRAM_BASE + 0x400
        addi(13, 13, 0x400), // x13 = DRAM_BASE + 0x800
        addi(13, 13, 0x400), // x13 = DRAM_BASE + 0xC00
        addi(13, 13, 0x400), // x13 = DRAM_BASE + 0x1000
        addi(10, 0, 0x12),
        sw(13, 10, 0), // seed pattern at +0x1000
        addi(10, 0, 0x34),
        sw(13, 10, 4), // independent region write at +0x1004
        addi(10, 0, 0xFF),
        sw(13, 10, 0),  // overwrite first region
        lbu(10, 13, 0), // read overwritten byte
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

#[test]
fn test_runtime_write_and_read_memory_region_sram_without_cpu_program() {
    let mut runtime = create_test_runtime();
    let addr = SRAM_BASE + 0x100;
    let payload = vec![0xAB, 0xCD, 0x12, 0x34, 0x56];

    runtime
        .load_program(TEST_BOOT_PC, &[])
        .expect("Failed to prepare runtime for host SRAM access");

    runtime
        .write_memory_region(addr, &payload)
        .expect("Failed to write SRAM via runtime");
    let read_back = runtime
        .read_memory_region(addr, payload.len() as u32)
        .expect("Failed to read SRAM via runtime");

    assert_eq!(read_back, payload);
}

#[test]
fn test_runtime_write_sram_then_cpu_reads_it() {
    let mut runtime = create_test_runtime();
    let payload = [0x78];
    let checker_program = instructions_to_bytes(&[
        lui(12, SRAM_BASE),
        lbu(10, 12, 0),
        beq(10, 0, -4),
        lui(11, DRAM_BASE),
        sw(11, 10, 0),
        0,
        0,
    ]);
    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &checker_program);

    runtime
        .write_memory_region(SRAM_BASE, &payload)
        .expect("Failed to write SRAM via runtime");

    let _ = wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT);
    assert_eq!(
        read_word_with_timeout(runtime.as_mut(), DRAM_BASE, SHORT_TIMEOUT),
        0x78
    );
}

#[test]
fn test_cpu_writes_sram_then_runtime_reads_it() {
    let mut runtime = create_test_runtime();

    let writer_program = instructions_to_bytes(&[
        lui(12, SRAM_BASE),
        addi(10, 0, 0x7E),
        sw(12, 10, 0),
        lui(11, SIM_CONTROL_BASE),
        addi(10, 0, 1),
        sw(11, 10, 0),
        ebreak(),
        jal(0, 0),
    ]);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &writer_program);
    assert_eq!(wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT), Some(1));

    let read_back = runtime
        .read_memory_region(SRAM_BASE, 4)
        .expect("Failed to read SRAM via runtime");
    assert_eq!(read_back, vec![0x7E, 0x00, 0x00, 0x00]);
}
