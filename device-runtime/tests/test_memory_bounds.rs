mod common;

use common::{instructions_to_bytes, load_and_boot, wait_for_cpu_halt, LONG_TIMEOUT, TEST_BOOT_PC};
use riscv_core::instruction::{addi, ebreak, jal, lbu, lui, lw, sb, sw};
use riscv_shared::bus::{DRAM_BASE, SIM_CONTROL_BASE};

fn run_and_expect(program: &[u32], expected_tohost: u32) {
    let mut runtime = common::create_test_runtime();
    let program_bytes = instructions_to_bytes(program);
    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program_bytes);
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(expected_tohost)
    );
}

#[test]
fn test_read_word_outside_dram_range() {
    let program = [
        addi(11, 0, 0),            // x11 = 0x00000000 (outside DRAM)
        lw(10, 11, 0),             // x10 = read word (expected 0)
        lui(12, SIM_CONTROL_BASE), // x12 = tohost base
        sw(12, 10, 0),             // tohost = x10
        ebreak(),
        jal(0, 0),
    ];
    run_and_expect(&program, 0);
}

#[test]
fn test_valid_dram_accesses() {
    let mut runtime = common::create_test_runtime();
    let program = instructions_to_bytes(&[
        lui(11, DRAM_BASE), // x11 = 0x80000000 (DRAM base)
        addi(10, 0, 0xAA),
        sb(11, 10, 0x1000), // [0x80001000] = 0xAA
        addi(10, 0, 0xBB),
        sb(11, 10, 0x1001), // [0x80001001] = 0xBB
        addi(10, 0, 0xCC),
        sb(11, 10, 0x1002), // [0x80001002] = 0xCC
        addi(10, 0, 0xDD),
        sb(11, 10, 0x1003),        // [0x80001003] = 0xDD
        lw(10, 11, 0x1000),        // x10 = 0xDDCCBBAA
        lui(12, SIM_CONTROL_BASE), // x12 = tohost base
        sw(12, 10, 0),             // tohost = x10
        ebreak(),
        jal(0, 0),
    ]);
    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program);
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(0xDDCC_BBAA)
    );
}

#[test]
fn test_boundary_at_dram_end_byte_read() {
    let program = [
        addi(10, 0, 0x42),         // x10 = test byte
        addi(11, 0, -1),           // x11 = 0xFFFF_FFFF (upper DRAM boundary address)
        sb(11, 10, 0),             // byte write attempt at upper boundary
        lbu(10, 11, 0),            // current implementation reads back 0 at this boundary
        lui(12, SIM_CONTROL_BASE), // x12 = tohost base
        sw(12, 10, 0),             // tohost = x10
        ebreak(),
        jal(0, 0),
    ];
    run_and_expect(&program, 0);
}

#[test]
fn test_boundary_at_dram_end_word_read_out_of_bounds() {
    let program = [
        addi(11, 0, -1),           // x11 = 0xFFFF_FFFF (upper DRAM boundary address)
        lw(10, 11, 0),             // word access spans beyond boundary, expected 0
        lui(12, SIM_CONTROL_BASE), // x12 = tohost base
        sw(12, 10, 0),             // tohost = x10
        ebreak(),
        jal(0, 0),
    ];
    run_and_expect(&program, 0);
}
