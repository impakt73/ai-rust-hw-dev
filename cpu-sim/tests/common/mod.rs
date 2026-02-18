//! Common utilities for cpu-sim integration tests

#![allow(dead_code)]

use cpu_sim::{SimulationResult, SimulatorView};
use riscv_core::instruction::*;
use riscv_shared::bus::SIM_CONTROL_BASE;
use riscv_shared::sim_control::SUCCESS_CODE;
use std::sync::{Arc, Mutex};

/// Initialize the test logger (idempotent – safe to call from multiple tests).
pub fn init_test_logger() {
    let _ = env_logger::builder().is_test(true).try_init();
}

/// Assert that a simulation result has the expected tohost value.
pub fn assert_tohost(result: &SimulationResult, expected: u32, test_name: &str) {
    assert_eq!(
        result.tohost_value,
        Some(expected),
        "Expected tohost value 0x{:x} ({}) from {}",
        expected,
        expected,
        test_name
    );
}

/// Create a FIFO data collector callback and the shared buffer it writes into.
///
/// The callback drains the simulator's TX FIFO on each call and appends the
/// bytes (little-endian) to the returned `Arc<Mutex<Vec<u8>>>`.
pub fn create_fifo_collector() -> (Arc<Mutex<Vec<u8>>>, impl FnMut(&mut SimulatorView)) {
    let fifo_data = Arc::new(Mutex::new(Vec::new()));
    let fifo_data_clone = Arc::clone(&fifo_data);

    let callback = move |view: &mut SimulatorView| {
        while let Some(word) = view.fifo_read_tx() {
            let bytes = [
                (word & 0xFF) as u8,
                ((word >> 8) & 0xFF) as u8,
                ((word >> 16) & 0xFF) as u8,
                ((word >> 24) & 0xFF) as u8,
            ];
            fifo_data_clone
                .lock()
                .expect("Failed to lock FIFO data mutex in create_fifo_collector callback")
                .extend_from_slice(&bytes);
        }
    };

    (fifo_data, callback)
}

/// Convert raw FIFO bytes to a UTF-8 string, stripping trailing null bytes.
pub fn fifo_data_to_string(data: &[u8]) -> String {
    let trimmed = match data.iter().rposition(|&b| b != 0) {
        Some(idx) => &data[..=idx],
        None => &[],
    };
    String::from_utf8(trimmed.to_vec()).expect("FIFO data should be valid UTF-8")
}

/// Helper to convert instructions to bytes
pub fn instructions_to_bytes(instructions: &[u32]) -> Vec<u8> {
    instructions
        .iter()
        .flat_map(|instr| instr.to_le_bytes())
        .collect()
}

/// Build a standard tohost termination sequence.
pub fn tohost_termination(addr_reg: u32, value_reg: u32, tohost_value: u32) -> [u32; 5] {
    [
        lui(addr_reg, SIM_CONTROL_BASE),
        addi(
            value_reg,
            0,
            i32::try_from(tohost_value).expect("tohost value must fit in i32 immediate"),
        ),
        sw(addr_reg, value_reg, 0),
        ebreak(),
        jal(0, 0),
    ]
}

/// Append a standard tohost termination sequence to an instruction vector.
pub fn append_tohost_termination(
    instructions: &mut Vec<u32>,
    addr_reg: u32,
    value_reg: u32,
    tohost_value: u32,
) {
    instructions.extend(tohost_termination(addr_reg, value_reg, tohost_value));
}

/// Create a simple test program (equivalent to test.s)
pub fn create_test_program() -> Vec<u8> {
    let mut instructions = vec![
        addi(1, 0, 10),     // x1 = 10
        addi(2, 0, 20),     // x2 = 20
        add(3, 1, 2),       // x3 = 30
        sub(4, 2, 1),       // x4 = 10
        lui(5, 0x80001000), // x5 = 0x80001000
        sw(5, 1, 0),        // mem[x5] = x1
        lw(6, 5, 0),        // x6 = mem[x5]
    ];
    append_tohost_termination(&mut instructions, 11, 10, SUCCESS_CODE);

    instructions_to_bytes(&instructions)
}

/// Create a trace test program (equivalent to trace_test.s)
pub fn create_trace_test_program() -> Vec<u8> {
    let mut instructions = vec![
        addi(1, 0, 10),     // x1 = 10
        addi(2, 0, 20),     // x2 = 20
        addi(3, 0, 5),      // x3 = 5
        add(4, 1, 2),       // x4 = 30
        sub(5, 2, 3),       // x5 = 15
        andi(6, 1, 0xFF),   // x6 = 10
        ori(7, 2, 0x1),     // x7 = 21
        lui(8, 0x12345000), // x8 = 0x12345000
        sw(0, 1, 0),        // mem[0] = x1
        lw(9, 0, 0),        // x9 = mem[0]
    ];
    append_tohost_termination(&mut instructions, 11, 10, SUCCESS_CODE);

    instructions_to_bytes(&instructions)
}

/// Create a register trace audit program (equivalent to register_trace_audit.s)
pub fn create_register_trace_program() -> Vec<u8> {
    let mut instructions = vec![
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
    ];
    append_tohost_termination(&mut instructions, 31, 30, SUCCESS_CODE);

    instructions_to_bytes(&instructions)
}
