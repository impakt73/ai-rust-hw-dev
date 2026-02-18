//! Common utilities for cpu-sim integration tests

#![allow(dead_code)]

use bus_shared::Fifo;
use cpu_sim::{FifoReceiveCallback, SimulationResult};
use riscv_core::instruction::*;
use riscv_shared::bus::SIM_CONTROL_BASE;
use riscv_shared::sim_control::SUCCESS_CODE;
use riscv_shared::FIFO_DATA;
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
pub fn create_fifo_collector() -> (Arc<Mutex<Vec<u8>>>, FifoReceiveCallback) {
    let fifo_data = Arc::new(Mutex::new(Vec::new()));
    let fifo_data_clone = Arc::clone(&fifo_data);

    let callback = move |word: u32| {
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
    };

    (fifo_data, Box::new(callback))
}

/// Convert raw FIFO bytes to a UTF-8 string, stripping trailing null bytes.
pub fn fifo_data_to_string(data: &[u8]) -> String {
    let trimmed = match data.iter().rposition(|&b| b != 0) {
        Some(idx) => &data[..=idx],
        None => &[],
    };
    String::from_utf8(trimmed.to_vec()).expect("FIFO data should be valid UTF-8")
}

/// Push a word into a FIFO device's RX queue.
pub fn preload_fifo_rx_word(fifo: &mut Fifo, word: u32) {
    fifo.rx.push_back(word);
}

/// Write a string to a FIFO RX queue with word packing and optional null terminator.
pub fn preload_fifo_rx_string(fifo: &mut Fifo, s: &str) {
    let bytes = s.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let mut word: u32 = 0;
        for j in 0..4 {
            if i + j < bytes.len() {
                word |= (bytes[i + j] as u32) << (j * 8);
            }
        }
        preload_fifo_rx_word(fifo, word);
        i += 4;
    }

    if bytes.len().is_multiple_of(4) {
        preload_fifo_rx_word(fifo, 0);
    }
}

/// Serialize a packet and preload it into a FIFO RX queue.
pub fn preload_packet_to_fifo_rx<T: serde::Serialize>(fifo: &mut Fifo, packet: &T) {
    let bytes = postcard::to_allocvec(packet).expect("Packet serialization should succeed");
    for chunk in bytes.chunks(4) {
        let mut word: u32 = 0;
        for (idx, byte) in chunk.iter().enumerate() {
            word |= (*byte as u32) << (idx * 8);
        }
        preload_fifo_rx_word(fifo, word);
    }
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

/// Create a minimal program that immediately exits with SUCCESS_CODE via tohost.
///
/// This is the raw-instruction equivalent of `simple_test.rs`.
pub fn create_simple_exit_program() -> Vec<u8> {
    let mut instructions = Vec::new();
    append_tohost_termination(&mut instructions, 1, 2, SUCCESS_CODE);
    instructions_to_bytes(&instructions)
}

/// Create a loop program that executes `iterations` NOP-like cycles then exits.
///
/// Useful for measuring cycle counts when a non-trivial program is required.
pub fn create_loop_program(iterations: u32) -> Vec<u8> {
    // Instruction layout (program base = 0x8000_0000):
    //   0: addi x1, x0, N     (x1 = iterations)
    // loop [PC = base+4]:
    //   1: addi x1, x1, -1    (x1--)
    //   2: bne  x1, x0, -4    (if x1 != 0 goto instr 1)
    // tohost termination (instrs 3..7):
    //   append_tohost_termination(&mut instructions, 2, 3, SUCCESS_CODE)
    let count =
        i32::try_from(iterations).expect("iterations must fit in a 12-bit signed immediate");
    let mut instructions = vec![
        addi(1, 0, count), // x1 = iterations
        // loop:
        addi(1, 1, -1), // x1--
        bne(1, 0, -4),  // branch back to addi(1,1,-1) if x1 != 0
    ];
    append_tohost_termination(&mut instructions, 2, 3, SUCCESS_CODE);
    instructions_to_bytes(&instructions)
}

/// Create a FIFO echo program (raw-instruction equivalent of `hello_world.rs`).
///
/// Reads words from the FIFO RX queue and echoes them to TX until the queue is
/// empty or a zero word is received, then terminates with SUCCESS_CODE.
///
/// Instruction layout (program base assumed to be 0x8000_0000):
///
/// ```text
///   0: lui  x1, 0x4000_3000    // x1 = FIFO_DATA address
///   1: addi x2, x1, 4          // x2 = FIFO_STATUS address
/// loop [PC = base+8]:
///   2: lw   x3, 0(x2)          // x3 = FIFO_STATUS
///   3: andi x4, x3, 1          // x4 = RX_VALID bit
///   4: beq  x4, x0, +20        // if RX empty  → done
///   5: lw   x5, 0(x1)          // x5 = word from FIFO_DATA
///   6: beq  x5, x0, +12        // if zero word → done
///   7: sw   x5, 0(x1)          // echo: write x5 to FIFO TX
///   8: jal  x0, -24            // loop back to instr 2
/// done [PC = base+36]:
///   9: lui  x6, SIM_CONTROL_BASE
///  10: addi x7, x0, SUCCESS_CODE
///  11: sw   x7, 0(x6)
///  12: ebreak
///  13: jal  x0, 0              // infinite loop (halt)
/// ```
pub fn create_fifo_echo_program() -> Vec<u8> {
    // FIFO_DATA     = 0x4000_3000  (lower 12 bits == 0, safe for LUI)
    // FIFO_STATUS   = FIFO_DATA + 4
    // SIM_CONTROL_BASE = 0x4000_0000

    // Branch / jump offsets (all relative to the *branch/jump instruction* PC):
    //   beq at instr 4 (PC = base+16): target = instr 9 (PC = base+36) → offset = +20
    //   beq at instr 6 (PC = base+24): target = instr 9 (PC = base+36) → offset = +12
    //   jal at instr 8 (PC = base+32): target = instr 2 (PC = base+8)  → offset = -24
    let instructions = vec![
        lui(1, FIFO_DATA), // x1 = FIFO_DATA (0x4000_3000)
        addi(2, 1, 4),     // x2 = FIFO_STATUS
        // loop:
        lw(3, 2, 0),   // x3 = FIFO_STATUS
        andi(4, 3, 1), // x4 = RX_VALID
        beq(4, 0, 20), // RX empty → done
        lw(5, 1, 0),   // x5 = FIFO word
        beq(5, 0, 12), // zero word → done
        sw(1, 5, 0),   // echo to TX
        jal(0, -24),   // loop
        // done:
        lui(6, SIM_CONTROL_BASE), // x6 = SIM_CONTROL_BASE
        addi(
            7,
            0,
            i32::try_from(SUCCESS_CODE).expect("SUCCESS_CODE fits i32"),
        ),
        sw(6, 7, 0), // tohost = SUCCESS_CODE
        ebreak(),
        jal(0, 0),
    ];
    instructions_to_bytes(&instructions)
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
