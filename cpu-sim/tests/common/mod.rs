//! Common utilities for cpu-sim integration tests

#![allow(dead_code)]

use cpu_sim::{SimulationResult, SimulatorView};
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

/// Create a DMA copy-and-verify program (raw-instruction equivalent of `test_dma_copy.rs`).
///
/// Writes a 64-byte sequential pattern to `SRC_BASE`, clears `DST_BASE`, triggers a
/// DMA transfer, polls for completion, verifies every byte, and exits via tohost
/// (`SUCCESS_CODE` on pass, `1` on mismatch).
///
/// Memory layout used:
/// * SRC_BASE  = 0x8000_1000
/// * DST_BASE  = 0x8000_2000
/// * DMA_BASE  = 0x4000_4000 (SRC+0, DST+4, SIZE+8, STATUS+12, DISPATCH+16)
pub fn create_dma_copy_program() -> Vec<u8> {
    // Register allocation:
    //   x1 = SRC_BASE,  x2 = DST_BASE,  x3 = counter,
    //   x4 = temp,      x5 = DMA_BASE,  x6 = verify tmp,
    //   x7 = 64 (loop limit),  x8 = TOHOST_BASE
    //
    // Branch offset computation (each instruction = 4 bytes):
    //
    //  Instr  PC-offset  Description
    //  -----  ---------  -----------
    //    0      +0       lui x1, SRC_BASE
    //    1      +4       lui x2, DST_BASE
    //    2      +8       lui x5, DMA_BASE
    //    3     +12       lui x8, TOHOST_BASE
    //    4     +16       addi x7, x0, 64
    //    5     +20       addi x3, x0, 0           ← start of loop1 setup
    // [loop1 start]
    //    6     +24       add  x4, x1, x3
    //    7     +28       sb   x4, x3, 0
    //    8     +32       addi x3, x3, 1
    //    9     +36       bne  x3, x7, -12  → instr 6 (+24); offset = 24-36 = -12 ✓
    //   10     +40       addi x3, x0, 0           ← start of loop2 setup
    // [loop2 start]
    //   11     +44       add  x4, x2, x3
    //   12     +48       sb   x4, x0, 0
    //   13     +52       addi x3, x3, 1
    //   14     +56       bne  x3, x7, -12  → instr 11 (+44); offset = 44-56 = -12 ✓
    // [DMA config]
    //   15     +60       sw   x5, x1, 0           DMA_SRC_ADDR = SRC_BASE
    //   16     +64       sw   x5, x2, 4           DMA_DST_ADDR = DST_BASE
    //   17     +68       addi x4, x0, 64
    //   18     +72       sw   x5, x4, 8           DMA_SIZE = 64
    //   19     +76       addi x4, x0, 1
    //   20     +80       sw   x5, x4, 16          DMA_DISPATCH = 1
    // [DMA poll]
    //   21     +84       lw   x4, x5, 12          ← poll start (DMA_STATUS)
    //   22     +88       andi x4, x4, 1
    //   23     +92       bne  x4, x0, -8   → instr 21 (+84); offset = 84-92 = -8 ✓
    // [verify]
    //   24     +96       addi x3, x0, 0
    // [verify start]
    //   25    +100       add  x4, x2, x3
    //   26    +104       lbu  x6, x4, 0
    //   27    +108       bne  x6, x3, +28  → instr 34 (+136); offset = 136-108 = +28 ✓
    //   28    +112       addi x3, x3, 1
    //   29    +116       bne  x3, x7, -16  → instr 25 (+100); offset = 100-116 = -16 ✓
    // [success]
    //   30    +120       addi x4, x0, 42
    //   31    +124       sw   x8, x4, 0
    //   32    +128       ebreak
    //   33    +132       jal  x0, 0
    // [fail]
    //   34    +136       addi x4, x0, 1
    //   35    +140       sw   x8, x4, 0
    //   36    +144       ebreak
    //   37    +148       jal  x0, 0
    let instructions = vec![
        // Setup
        lui(1, 0x8000_1000u32),   // x1 = SRC_BASE
        lui(2, 0x8000_2000u32),   // x2 = DST_BASE
        lui(5, 0x4000_4000u32),   // x5 = DMA_BASE
        lui(8, SIM_CONTROL_BASE), // x8 = TOHOST_BASE
        addi(7, 0, 64),           // x7 = 64 (loop limit)
        // Loop 1: write pattern to SRC
        addi(3, 0, 0),
        add(4, 1, 3), // x4 = SRC_BASE + counter
        sb(4, 3, 0),  // mem[x4] = counter (byte)
        addi(3, 3, 1),
        bne(3, 7, -12), // loop back to instr 6
        // Loop 2: clear DST
        addi(3, 0, 0),
        add(4, 2, 3), // x4 = DST_BASE + counter
        sb(4, 0, 0),  // mem[x4] = 0
        addi(3, 3, 1),
        bne(3, 7, -12), // loop back to instr 11
        // DMA configuration
        sw(5, 1, 0), // DMA_SRC_ADDR = SRC_BASE
        sw(5, 2, 4), // DMA_DST_ADDR = DST_BASE
        addi(4, 0, 64),
        sw(5, 4, 8), // DMA_SIZE = 64
        addi(4, 0, 1),
        sw(5, 4, 16), // DMA_DISPATCH = 1
        // Poll DMA completion
        lw(4, 5, 12),  // x4 = DMA_STATUS
        andi(4, 4, 1), // x4 = BUSY bit
        bne(4, 0, -8), // loop back to instr 21
        // Verify loop
        addi(3, 0, 0),
        add(4, 2, 3),  // x4 = DST_BASE + counter
        lbu(6, 4, 0),  // x6 = mem[x4] (byte, zero-extended)
        bne(6, 3, 28), // mismatch → fail (instr 34)
        addi(3, 3, 1),
        bne(3, 7, -16), // loop back to instr 25
        // Success
        addi(
            4,
            0,
            i32::try_from(SUCCESS_CODE).expect("SUCCESS_CODE fits i32"),
        ),
        sw(8, 4, 0),
        ebreak(),
        jal(0, 0),
        // Fail
        addi(4, 0, 1), // FAILURE code
        sw(8, 4, 0),
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
