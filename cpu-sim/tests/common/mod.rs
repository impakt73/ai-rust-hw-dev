//! Common utilities for cpu-sim integration tests

#![allow(dead_code)]

use riscv_core::instruction::*;
use riscv_shared::bus::SIM_CONTROL_BASE;

/// Helper to convert instructions to bytes
pub fn instructions_to_bytes(instructions: &[u32]) -> Vec<u8> {
    instructions
        .iter()
        .flat_map(|instr| instr.to_le_bytes())
        .collect()
}

/// Create a simple test program (equivalent to test.s)
pub fn create_test_program() -> Vec<u8> {
    let instructions = vec![
        addi(1, 0, 10),            // x1 = 10
        addi(2, 0, 20),            // x2 = 20
        add(3, 1, 2),              // x3 = 30
        sub(4, 2, 1),              // x4 = 10
        lui(5, 0x80001000),        // x5 = 0x80001000
        sw(5, 1, 0),               // mem[x5] = x1
        lw(6, 5, 0),               // x6 = mem[x5]
        addi(10, 0, 42),           // x10 = 42
        lui(11, SIM_CONTROL_BASE), // x11 = tohost address
        sw(11, 10, 0),             // tohost = 42
        jal(0, 0),                 // halt
    ];

    instructions_to_bytes(&instructions)
}

/// Create a trace test program (equivalent to trace_test.s)
pub fn create_trace_test_program() -> Vec<u8> {
    let instructions = vec![
        addi(1, 0, 10),            // x1 = 10
        addi(2, 0, 20),            // x2 = 20
        addi(3, 0, 5),             // x3 = 5
        add(4, 1, 2),              // x4 = 30
        sub(5, 2, 3),              // x5 = 15
        andi(6, 1, 0xFF),          // x6 = 10
        ori(7, 2, 0x1),            // x7 = 21
        lui(8, 0x12345000),        // x8 = 0x12345000
        sw(0, 1, 0),               // mem[0] = x1
        lw(9, 0, 0),               // x9 = mem[0]
        addi(10, 0, 42),           // x10 = 42
        lui(11, SIM_CONTROL_BASE), // x11 = tohost address
        sw(11, 10, 0),             // tohost = 42
        jal(0, 0),                 // halt
    ];

    instructions_to_bytes(&instructions)
}

/// Create a register trace audit program (equivalent to register_trace_audit.s)
pub fn create_register_trace_program() -> Vec<u8> {
    let instructions = vec![
        // Fibonacci-like sequence
        addi(1, 0, 1), // x1 = 1
        addi(2, 0, 2), // x2 = 2
        add(3, 1, 2),  // x3 = 3
        add(4, 2, 3),  // x4 = 5
        add(5, 3, 4),  // x5 = 8
        add(6, 4, 5),  // x6 = 13
        add(7, 5, 6),  // x7 = 21
        // Round numbers
        addi(8, 0, 10),  // x8 = 10
        addi(9, 0, 20),  // x9 = 20
        add(10, 8, 9),   // x10 = 30
        addi(11, 0, 50), // x11 = 50
        add(12, 10, 11), // x12 = 80
        add(13, 12, 9),  // x13 = 100
        // Powers of 2
        addi(14, 0, 1),  // x14 = 1
        add(15, 14, 14), // x15 = 2
        add(16, 15, 15), // x16 = 4
        add(17, 16, 16), // x17 = 8
        add(18, 17, 17), // x18 = 16
        add(19, 18, 18), // x19 = 32
        add(20, 19, 19), // x20 = 64
        add(21, 20, 20), // x21 = 128
        add(22, 21, 21), // x22 = 256
        // Subtraction
        addi(23, 0, 100), // x23 = 100
        addi(24, 0, 40),  // x24 = 40
        sub(25, 23, 24),  // x25 = 60
        sub(26, 25, 24),  // x26 = 20
        // Load/Store
        lui(27, 0x80001000), // x27 = 0x80001000
        addi(28, 0, 123),    // x28 = 123
        sw(27, 28, 0),       // mem[0x80001000] = 123
        lw(29, 27, 0),       // x29 = 123
        add(30, 29, 1),      // x30 = 124
        // Success
        addi(30, 0, 42),           // x30 = 42
        lui(31, SIM_CONTROL_BASE), // x31 = tohost address
        sw(31, 30, 0),             // tohost = 42
        jal(0, 0),                 // halt
    ];

    instructions_to_bytes(&instructions)
}
