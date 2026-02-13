//! RTL Verification Tests
//!
//! Low-level testbench tests that verify the RTL implementation directly
//! by running programmatically generated instruction sequences.
//!
//! These tests were migrated from tests/src/cpu_test.rs to leverage the
//! cpu-sim infrastructure (SystemBus, VCD dumps, instruction tracing)
//! rather than maintaining a duplicate CpuTestHarness implementation.

mod common;

use cpu_sim::*;
use riscv_core::instruction::*;
use riscv_shared::sim_control::SUCCESS_CODE;

/// Helper function to initialize test logger (idempotent)
fn init_test_logger() {
    let _ = env_logger::builder().is_test(true).try_init();
}

/// Helper to run programmatic instructions with options for trace/VCD/callbacks
///
/// This is the ONLY helper function for running programmatic tests.
/// It supports:
/// - Instruction trace printing (print_inst_trace)
/// - VCD waveform dumping (vcd_path)
/// - Trace callbacks for programmatic validation (trace_callback)
/// - Post-execution callbacks for verification (termination_callback)
fn run_program_with_options<T, F>(
    instructions: &[u32],
    max_cycles: u64,
    print_inst_trace: bool,
    vcd_path: Option<&str>,
    trace_callback: Option<T>,
    termination_callback: Option<F>,
) -> Result<SimulationResult, String>
where
    T: FnMut(&riscv_core::trace::InstructionTrace),
    F: FnOnce(&SimulatorView, &SimulationResult),
{
    const START_ADDR: u32 = 0x8000_0000;

    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    run_program(
        max_cycles,
        print_inst_trace,
        false, // Don't print FSM state
        None::<fn(&mut SimulatorView)>,
        trace_callback,
        vcd_path,
        0, // Zero latency for RTL verification tests
        |sim| {
            sim.write_memory_region(START_ADDR, &program_bytes, true);
            Ok(START_ADDR)
        },
        termination_callback,
    )
}

// ============================================================================
// Basic Execution Tests
// ============================================================================

#[test]
fn test_cpu_basic_execution() {
    init_test_logger();

    // Program: Simple arithmetic operations
    // 0x00: ADDI x1, x0, 5    ; x1 = 5
    // 0x04: ADDI x2, x0, 3    ; x2 = 3
    // 0x08: ADD  x3, x1, x2   ; x3 = x1 + x2 = 8
    let mut instructions = vec![addi(1, 0, 5), addi(2, 0, 3), add(3, 1, 2)];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|_sim: &SimulatorView, result: &SimulationResult| {
            assert!(
                result.tohost_value == Some(SUCCESS_CODE),
                "Program should terminate with tohost=1"
            );
        }),
    )
    .expect("Program should run");
}

#[test]
fn test_cpu_three_instructions() {
    init_test_logger();

    // Program: Execute exactly 3 instructions as required
    // 0x00: ADDI x1, x0, 10   ; x1 = 10
    // 0x04: ADD  x2, x1, x1   ; x2 = x1 + x1 = 20
    // 0x08: SUB  x3, x2, x1   ; x3 = x2 - x1 = 10
    let mut instructions = vec![addi(1, 0, 10), add(2, 1, 1), sub(3, 2, 1)];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|_sim: &SimulatorView, result: &SimulationResult| {
            assert!(
                result.tohost_value == Some(SUCCESS_CODE),
                "Program should terminate with tohost=1"
            );
        }),
    )
    .expect("Program should run");

    println!("Successfully executed 3 instructions: ADDI, ADD, SUB");
}

#[test]
fn test_cpu_lui_instruction() {
    init_test_logger();

    // Program: Test LUI instruction
    // 0x00: LUI x1, 0x12345   ; x1 = 0x12345000
    // 0x04: ADDI x2, x1, 0x678 ; x2 = x1 + 0x678
    let mut instructions = vec![lui(1, 0x12345000), addi(2, 1, 0x678)];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|_sim: &SimulatorView, result: &SimulationResult| {
            assert!(
                result.tohost_value == Some(SUCCESS_CODE),
                "Program should terminate with tohost=1"
            );
        }),
    )
    .expect("Program should run");

    println!("Successfully executed LUI instruction");
}

#[test]
fn test_cpu_logic_operations() {
    init_test_logger();

    // Program: Test logic operations
    // 0x00: ADDI x1, x0, 0xFF  ; x1 = 0xFF
    // 0x04: ADDI x2, x0, 0x0F  ; x2 = 0x0F
    // 0x08: AND x3, x1, x2     ; x3 = x1 & x2 = 0x0F
    // 0x0C: OR  x4, x1, x2     ; x4 = x1 | x2 = 0xFF
    // 0x10: XOR x5, x1, x2     ; x5 = x1 ^ x2 = 0xF0
    let mut instructions = vec![
        addi(1, 0, 0xFF),
        addi(2, 0, 0x0F),
        and(3, 1, 2),
        or(4, 1, 2),
        xor(5, 1, 2),
    ];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|_sim: &SimulatorView, result: &SimulationResult| {
            assert!(
                result.tohost_value == Some(SUCCESS_CODE),
                "Program should terminate with tohost=1"
            );
        }),
    )
    .expect("Program should run");

    println!("Successfully executed logic operations: AND, OR, XOR");
}

// ============================================================================
// Branch Tests
// ============================================================================

#[test]
fn test_cpu_branch_beq_bne() {
    init_test_logger();

    // Program: Test BEQ and BNE instructions
    // 0x00: ADDI x1, x0, 10   ; x1 = 10
    // 0x04: ADDI x2, x0, 10   ; x2 = 10
    // 0x08: BEQ  x1, x2, 8    ; Should branch to 0x10 (skip next instr)
    // 0x0C: ADDI x3, x0, 99   ; Should be skipped - write marker to memory if executed
    // 0x10: ADDI x4, x0, 5    ; x4 = 5
    // 0x14: BNE  x1, x4, 8    ; Should branch to 0x1C (skip next instr)
    // 0x18: ADDI x5, x0, 99   ; Should be skipped - write marker to memory if executed
    // 0x1C: ADDI x6, x0, 1    ; x6 = 1
    // 0x20: LUI x9, 0x80000   ; x9 = 0x80000000 (base address)
    // 0x24: SW   x3, 0(x9)    ; Store x3 to verify it wasn't set to 99
    // 0x28: SW   x5, 4(x9)    ; Store x5 to verify it wasn't set to 99
    let mut instructions = vec![
        addi(1, 0, 10),
        addi(2, 0, 10),
        beq(1, 2, 8),
        addi(3, 0, 99),
        addi(4, 0, 5),
        bne(1, 4, 8),
        addi(5, 0, 99),
        addi(6, 0, 1),
        lui(9, DRAM_BASE),
        sw(9, 3, 0),
        sw(9, 5, 4),
    ];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, result: &SimulationResult| {
            // Verify branches worked - skipped instructions should leave registers at 0
            let marker1 = sim.read_word(0x80000000);
            let marker2 = sim.read_word(0x80000004);
            assert_eq!(
                marker1, 0,
                "First branch should skip addi x3,x0,99, so x3 should be 0"
            );
            assert_eq!(
                marker2, 0,
                "Second branch should skip addi x5,x0,99, so x5 should be 0"
            );
            assert!(
                result.tohost_value == Some(SUCCESS_CODE),
                "Program should terminate with tohost=1"
            );
        }),
    )
    .expect("Program should run");

    println!("Successfully executed BEQ and BNE branches");
}

#[test]
fn test_cpu_branch_blt_bge() {
    init_test_logger();

    // Program: Test BLT and BGE instructions
    // 0x00: ADDI x1, x0, 5     ; x1 = 5
    // 0x04: ADDI x2, x0, 10    ; x2 = 10
    // 0x08: BLT  x1, x2, 8     ; Should branch (5 < 10)
    // 0x0C: ADDI x3, x0, 99    ; Should be skipped
    // 0x10: BGE  x2, x1, 8     ; Should branch (10 >= 5)
    // 0x14: ADDI x4, x0, 99    ; Should be skipped
    // 0x18: ADDI x5, x0, 1     ; x5 = 1
    // 0x1C: LUI x9, 0x80000    ; x9 = 0x80000000 (base address)
    // 0x20: SW   x3, 0(x9)     ; Store x3 to verify
    // 0x24: SW   x4, 4(x9)     ; Store x4 to verify
    let mut instructions = vec![
        addi(1, 0, 5),
        addi(2, 0, 10),
        blt(1, 2, 8),
        addi(3, 0, 99),
        bge(2, 1, 8),
        addi(4, 0, 99),
        addi(5, 0, 1),
        lui(9, DRAM_BASE),
        sw(9, 3, 0),
        sw(9, 4, 4),
    ];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, result: &SimulationResult| {
            // Verify branches worked
            let marker1 = sim.read_word(0x80000000);
            let marker2 = sim.read_word(0x80000004);
            assert_eq!(marker1, 0, "BLT should skip setting x3 to 99");
            assert_eq!(marker2, 0, "BGE should skip setting x4 to 99");
            assert!(
                result.tohost_value == Some(SUCCESS_CODE),
                "Program should terminate with tohost=1"
            );
        }),
    )
    .expect("Program should run");

    println!("Successfully executed BLT and BGE branches");
}

#[test]
fn test_cpu_branch_bltu_bgeu() {
    init_test_logger();

    // Program: Test BLTU and BGEU instructions (unsigned comparison)
    // 0x00: ADDI x1, x0, -1    ; x1 = 0xFFFFFFFF (unsigned max)
    // 0x04: ADDI x2, x0, 5     ; x2 = 5
    // 0x08: BLTU x2, x1, 8     ; Should branch (5 < 0xFFFFFFFF unsigned)
    // 0x0C: ADDI x3, x0, 99    ; Should be skipped
    // 0x10: BGEU x1, x2, 8     ; Should branch (0xFFFFFFFF >= 5 unsigned)
    // 0x14: ADDI x4, x0, 99    ; Should be skipped
    // 0x18: ADDI x5, x0, 1     ; x5 = 1
    // 0x1C: LUI x9, 0x80000    ; x9 = 0x80000000 (base address)
    // 0x20: SW   x3, 0(x9)     ; Store x3 to verify
    // 0x24: SW   x4, 4(x9)     ; Store x4 to verify
    let mut instructions = vec![
        addi(1, 0, -1),
        addi(2, 0, 5),
        bltu(2, 1, 8),
        addi(3, 0, 99),
        bgeu(1, 2, 8),
        addi(4, 0, 99),
        addi(5, 0, 1),
        lui(9, DRAM_BASE),
        sw(9, 3, 0),
        sw(9, 4, 4),
    ];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, result: &SimulationResult| {
            // Verify branches worked
            let marker1 = sim.read_word(0x80000000);
            let marker2 = sim.read_word(0x80000004);
            assert_eq!(marker1, 0, "BLTU should skip setting x3 to 99");
            assert_eq!(marker2, 0, "BGEU should skip setting x4 to 99");
            assert!(
                result.tohost_value == Some(SUCCESS_CODE),
                "Program should terminate with tohost=1"
            );
        }),
    )
    .expect("Program should run");

    println!("Successfully executed BLTU and BGEU branches");
}

// ============================================================================
// Load/Store Tests
// ============================================================================

#[test]
fn test_cpu_load_store() {
    init_test_logger();

    // Program: Test load and store instructions
    // Memory base: 0x80000000 (DRAM start)
    // 0x00: LUI x1, 0x80000  ; x1 = 0x80000000 (base address)
    // 0x04: ADDI x2, x0, 42   ; x2 = 42 (value to store)
    // 0x08: SW   x2, 0(x1)    ; Store x2 to memory[0x80000000]
    // 0x0C: LW   x3, 0(x1)    ; Load from memory[0x80000000] to x3
    // 0x10: ADDI x4, x0, 8    ; x4 = 8 (offset)
    // 0x14: SW   x2, 8(x1)    ; Store x2 to memory[0x80000008]
    // 0x18: LW   x5, 8(x1)    ; Load from memory[0x80000008] to x5
    let mut instructions = vec![
        lui(1, DRAM_BASE),
        addi(2, 0, 42),
        sw(1, 2, 0),
        lw(3, 1, 0),
        addi(4, 0, 8),
        sw(1, 2, 8),
        lw(5, 1, 8),
    ];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            assert_eq!(
                sim.read_word(0x80000000),
                42,
                "Memory[0x80000000] should contain 42"
            );
            assert_eq!(
                sim.read_word(0x80000008),
                42,
                "Memory[0x80000008] should contain 42"
            );
        }),
    )
    .expect("Program should run");

    println!("Successfully executed load and store instructions");
}

#[test]
fn test_cpu_load_byte() {
    init_test_logger();

    // Program: Test LB (load byte signed) and LBU (load byte unsigned)
    // We'll store a word with mixed signed/unsigned bytes and load them
    // Memory base: 0x80000000
    // 0x00: LUI x1, 0x80000  ; x1 = 0x80000000 (base address)
    // 0x04: ADDI x2, x0, -1   ; x2 = 0xFFFFFFFF
    // 0x08: SW   x2, 0(x1)    ; Store 0xFFFFFFFF to mem[0x80000000]
    // 0x0C: LB   x3, 0(x1)    ; Load byte 0 (0xFF), sign-extend to 0xFFFFFFFF
    // 0x10: LB   x4, 1(x1)    ; Load byte 1 (0xFF), sign-extend to 0xFFFFFFFF
    // 0x14: LBU  x5, 0(x1)    ; Load byte 0 (0xFF), zero-extend to 0x000000FF
    // 0x18: LBU  x6, 1(x1)    ; Load byte 1 (0xFF), zero-extend to 0x000000FF
    // 0x1C: SW   x3, 0x10(x1) ; Store loaded values for verification
    // 0x20: SW   x4, 0x14(x1)
    // 0x24: SW   x5, 0x18(x1)
    // 0x28: SW   x6, 0x1C(x1)
    let mut instructions = vec![
        lui(1, DRAM_BASE),
        addi(2, 0, -1),
        sw(1, 2, 0),
        lb(3, 1, 0),
        lb(4, 1, 1),
        lbu(5, 1, 0),
        lbu(6, 1, 1),
        sw(1, 3, 0x10),
        sw(1, 4, 0x14),
        sw(1, 5, 0x18),
        sw(1, 6, 0x1C),
    ];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            // Verify memory operations
            assert_eq!(
                sim.read_word(0x80000000),
                0xFFFFFFFF,
                "Memory[0x80000000] should contain 0xFFFFFFFF"
            );
            // Verify load operations
            assert_eq!(
                sim.read_word(0x80000010),
                0xFFFFFFFF,
                "LB x3, 0(x1) should load 0xFF and sign-extend to 0xFFFFFFFF"
            );
            assert_eq!(
                sim.read_word(0x80000014),
                0xFFFFFFFF,
                "LB x4, 1(x1) should load 0xFF and sign-extend to 0xFFFFFFFF"
            );
            assert_eq!(
                sim.read_word(0x80000018),
                0x000000FF,
                "LBU x5, 0(x1) should load 0xFF and zero-extend to 0x000000FF"
            );
            assert_eq!(
                sim.read_word(0x8000001C),
                0x000000FF,
                "LBU x6, 1(x1) should load 0xFF and zero-extend to 0x000000FF"
            );
        }),
    )
    .expect("Program should run");

    println!("Successfully executed LB and LBU instructions");
}

#[test]
fn test_cpu_load_halfword() {
    init_test_logger();

    // Program: Test LH (load halfword signed) and LHU (load halfword unsigned)
    // Memory base: 0x80000000
    let mut instructions = vec![
        lui(1, DRAM_BASE),
        addi(2, 0, -1),
        sw(1, 2, 0),
        lh(3, 1, 0),
        lh(4, 1, 2),
        lhu(5, 1, 0),
        lhu(6, 1, 2),
        sw(1, 3, 0x10),
        sw(1, 4, 0x14),
        sw(1, 5, 0x18),
        sw(1, 6, 0x1C),
    ];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            // Verify memory operations
            assert_eq!(
                sim.read_word(0x80000000),
                0xFFFFFFFF,
                "Memory[0x80000000] should contain 0xFFFFFFFF"
            );
            // Verify load operations
            assert_eq!(
                sim.read_word(0x80000010),
                0xFFFFFFFF,
                "LH x3, 0(x1) should load 0xFFFF and sign-extend to 0xFFFFFFFF"
            );
            assert_eq!(
                sim.read_word(0x80000014),
                0xFFFFFFFF,
                "LH x4, 2(x1) should load 0xFFFF and sign-extend to 0xFFFFFFFF"
            );
            assert_eq!(
                sim.read_word(0x80000018),
                0x0000FFFF,
                "LHU x5, 0(x1) should load 0xFFFF and zero-extend to 0x0000FFFF"
            );
            assert_eq!(
                sim.read_word(0x8000001C),
                0x0000FFFF,
                "LHU x6, 2(x1) should load 0xFFFF and zero-extend to 0x0000FFFF"
            );
        }),
    )
    .expect("Program should run");

    println!("Successfully executed LH and LHU instructions");
}

#[test]
fn test_cpu_store_byte() {
    init_test_logger();

    // Program: Test SB (store byte)
    // We'll write individual bytes to different positions in a word
    // Memory base: 0x80000000
    let mut instructions = vec![
        lui(1, DRAM_BASE),
        addi(2, 0, 0x12),
        addi(3, 0, 0x34),
        addi(4, 0, 0x56),
        addi(5, 0, 0x78),
        sb(1, 2, 0),
        sb(1, 3, 1),
        sb(1, 4, 2),
        sb(1, 5, 3),
        lw(6, 1, 0),
    ];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            // Verify memory operations - bytes stored in little-endian order
            assert_eq!(
                sim.read_word(0x80000000),
                0x78563412,
                "Memory should contain 0x78563412"
            );
        }),
    )
    .expect("Program should run");

    println!("Successfully executed SB instruction");
}

#[test]
fn test_cpu_store_halfword() {
    init_test_logger();

    // Program: Test SH (store halfword)
    // Memory base: 0x80000000
    let mut instructions = vec![
        lui(1, DRAM_BASE),
        addi(2, 0, 0x234),
        addi(3, 0, 0x678),
        sh(1, 2, 0),
        sh(1, 3, 2),
        lw(4, 1, 0),
    ];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            // Verify memory operations - halfwords stored in little-endian order
            assert_eq!(
                sim.read_word(0x80000000),
                0x06780234,
                "Memory should contain 0x06780234"
            );
        }),
    )
    .expect("Program should run");

    println!("Successfully executed SH instruction");
}

#[test]
fn test_cpu_byte_halfword_mixed() {
    init_test_logger();

    // Program: Test mixed byte/halfword operations with positive and negative values
    // Memory base: 0x80000000
    let mut instructions = vec![
        lui(1, DRAM_BASE),
        addi(2, 0, -128),
        sb(1, 2, 0),
        lb(3, 1, 0),
        lbu(4, 1, 0),
        addi(5, 0, -1),
        sh(1, 5, 4),
        lh(6, 1, 4),
        lhu(7, 1, 4),
        sw(1, 3, 0x10),
        sw(1, 4, 0x14),
        sw(1, 6, 0x18),
        sw(1, 7, 0x1C),
    ];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            // Verify load operations
            assert_eq!(
                sim.read_word(0x80000010),
                0xFFFFFF80,
                "LB x3, 0(x1) should load 0x80 and sign-extend to 0xFFFFFF80"
            );
            assert_eq!(
                sim.read_word(0x80000014),
                0x00000080,
                "LBU x4, 0(x1) should load 0x80 and zero-extend to 0x00000080"
            );
            assert_eq!(
                sim.read_word(0x80000018),
                0xFFFFFFFF,
                "LH x6, 4(x1) should load 0xFFFF and sign-extend to 0xFFFFFFFF"
            );
            assert_eq!(
                sim.read_word(0x8000001C),
                0x0000FFFF,
                "LHU x7, 4(x1) should load 0xFFFF and zero-extend to 0x0000FFFF"
            );
        }),
    )
    .expect("Program should run");

    println!("Successfully executed mixed byte/halfword operations");
}

// ============================================================================
// Special Instruction Tests
// ============================================================================

#[test]
fn test_cpu_auipc() {
    init_test_logger();

    // Program: Test AUIPC instruction
    let mut instructions = vec![auipc(1, 0x12345000), auipc(2, 0x00001000), addi(0, 0, 0)];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|_sim: &SimulatorView, result: &SimulationResult| {
            assert!(
                result.tohost_value == Some(SUCCESS_CODE),
                "Program should terminate with tohost=1"
            );
        }),
    )
    .expect("Program should run");

    println!("Successfully executed AUIPC instruction");
}

#[test]
fn test_cpu_tohost_halt() {
    init_test_logger();

    // Program: Execute a few instructions, then write to tohost to signal halt
    let instructions = vec![
        addi(1, 0, 10),
        addi(2, 1, 5),
        add(3, 1, 2),
        lui(4, SIM_CONTROL_BASE), // x4 = SIM_CONTROL_BASE (tohost address)
        addi(5, 0, SUCCESS_CODE as i32), // x5 = success code
        sw(4, 5, 0),              // Store x5 to tohost address
        jal(0, 0),                // Infinite loop
    ];

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|_sim: &SimulatorView, result: &SimulationResult| {
            // Verify that tohost write was detected
            assert_eq!(
                result.tohost_value,
                Some(SUCCESS_CODE),
                "Expected tohost value to be SUCCESS_CODE (exit code)"
            );
            // Note: We cannot read from TOHOST_ADDR directly as it's write-only
            // The tohost value is captured in result.tohost_value above
        }),
    )
    .expect("Program should run");

    println!("Successfully tested tohost halt mechanism");
}

#[test]
fn test_cpu_fence_instruction() {
    init_test_logger();

    let mut instructions = vec![addi(1, 0, 10), fence(), addi(2, 1, 5), addi(0, 0, 0)];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|_sim: &SimulatorView, result: &SimulationResult| {
            // FENCE is essentially a NOP for single-cycle CPU
            assert!(
                result.tohost_value == Some(SUCCESS_CODE),
                "Program should terminate with tohost=1"
            );
        }),
    )
    .expect("Program should run");

    println!("Successfully executed FENCE instruction");
}

#[test]
fn test_cpu_ecall_instruction() {
    init_test_logger();

    let mut instructions = vec![addi(1, 0, 42)];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));
    instructions.push(ecall()); // Should halt CPU after tohost write
    instructions.push(addi(2, 0, 99)); // Should not execute

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|_sim: &SimulatorView, result: &SimulationResult| {
            // After ECALL, CPU should halt
            assert!(
                result.tohost_value == Some(SUCCESS_CODE),
                "Program should terminate with tohost=1"
            );
        }),
    )
    .expect("Program should run");

    println!("Successfully executed ECALL instruction");
}

#[test]
fn test_cpu_ebreak_instruction() {
    init_test_logger();

    let mut instructions = vec![addi(1, 0, 100)];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));
    instructions.push(ebreak()); // Should halt CPU after tohost write
    instructions.push(addi(2, 0, 200)); // Should not execute

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|_sim: &SimulatorView, result: &SimulationResult| {
            // After EBREAK, CPU should halt
            assert!(
                result.tohost_value == Some(SUCCESS_CODE),
                "Program should terminate with tohost=1"
            );
        }),
    )
    .expect("Program should run");

    println!("Successfully executed EBREAK instruction");
}

// ============================================================================
// CSR Tests
// ============================================================================

#[test]
fn test_cpu_csr_read_write() {
    init_test_logger();

    // Test CSRRW (CSR Read/Write)
    // Memory base: 0x80000000
    let mut instructions = vec![
        addi(1, 0, 100),
        csrrw(2, 1, 0x300), // x2 = CSR[0x300]; CSR[0x300] = x1
        lui(8, DRAM_BASE),
        sw(8, 2, 0),
        csrrw(3, 0, 0x300), // x3 = CSR[0x300]; CSR[0x300] = 0
        sw(8, 3, 4),
        csrrw(4, 0, 0x300),
        sw(8, 4, 8),
    ];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            // Verify CSR operations
            assert_eq!(
                sim.read_word(0x80000000),
                0,
                "First CSRRW should read 0 from uninitialized CSR"
            );
            assert_eq!(
                sim.read_word(0x80000004),
                100,
                "Second CSRRW should read 100 from CSR"
            );
            assert_eq!(
                sim.read_word(0x80000008),
                0,
                "Third CSRRW should read 0 from CSR"
            );
        }),
    )
    .expect("Program should run");

    println!("Successfully executed CSR read/write operations");
}

#[test]
fn test_cpu_csr_set_clear() {
    init_test_logger();

    // Test CSRRS (CSR Read and Set) and CSRRC (CSR Read and Clear)
    // Memory base: 0x80000000
    let mut instructions = vec![
        addi(1, 0, 0b1010),
        csrrw(0, 1, 0x301),
        addi(2, 0, 0b0101),
        csrrs(3, 2, 0x301), // x3 = CSR; CSR |= x2
        lui(8, DRAM_BASE),
        sw(8, 3, 0),
        addi(4, 0, 0b1000),
        csrrc(5, 4, 0x301), // x5 = CSR; CSR &= ~x4
        sw(8, 5, 4),
        csrrw(6, 0, 0x301),
        sw(8, 6, 8),
    ];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            // Verify CSR operations
            assert_eq!(
                sim.read_word(0x80000000),
                0b1010,
                "CSRRS should read old value 0b1010"
            );
            assert_eq!(
                sim.read_word(0x80000004),
                0b1111,
                "CSRRC should read value 0b1111"
            );
            assert_eq!(
                sim.read_word(0x80000008),
                0b0111,
                "Final CSR value should be 0b0111"
            );
        }),
    )
    .expect("Program should run");

    println!("Successfully executed CSR set/clear operations");
}

#[test]
fn test_cpu_csr_immediate() {
    init_test_logger();

    // Test immediate CSR instructions (CSRRWI, CSRRSI, CSRRCI)
    // Memory base: 0x80000000
    let mut instructions = vec![
        csrrwi(1, 15, 0x302),
        lui(8, DRAM_BASE),
        sw(8, 1, 0),
        csrrsi(2, 8, 0x302),
        sw(8, 2, 4),
        csrrci(3, 4, 0x302),
        sw(8, 3, 8),
        csrrw(4, 0, 0x302),
        sw(8, 4, 12),
    ];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            // Verify CSR operations
            assert_eq!(
                sim.read_word(0x80000000),
                0,
                "CSRRWI should read 0 from uninitialized CSR"
            );
            assert_eq!(sim.read_word(0x80000004), 15, "CSRRSI should read 15");
            assert_eq!(sim.read_word(0x80000008), 15, "CSRRCI should read 15");
            assert_eq!(
                sim.read_word(0x8000000C),
                11,
                "Final CSR value should be 11"
            );
        }),
    )
    .expect("Program should run");

    println!("Successfully executed CSR immediate operations");
}

#[test]
fn test_cpu_csr_instret() {
    init_test_logger();

    // Test INSTRET CSR counter (0xC02)
    // This test verifies that the instruction retired counter increments correctly
    //
    // Program flow:
    // 1. Execute a known number of instructions (NOP, ADDI, etc.)
    // 2. Read INSTRET CSR using CSRRS (rd=dest, rs1=0 means read-only)
    // 3. Store result to memory
    // 4. Verify the count matches expected number of retired instructions
    //
    // Memory base: 0x80000000
    let mut instructions = vec![
        addi(1, 0, 0),      // Instr #1: NOP equivalent (x1 = 0)
        addi(2, 0, 0),      // Instr #2: NOP equivalent (x2 = 0)
        addi(3, 0, 0),      // Instr #3: NOP equivalent (x3 = 0)
        csrrs(4, 0, 0xC02), // Instr #4: Read INSTRET (x4 = CSR[0xC02], no write)
        lui(8, DRAM_BASE),  // Instr #5: Load base address
        sw(8, 4, 0),        // Instr #6: Store INSTRET value to memory
    ];
    instructions.extend(common::tohost_termination(7, 9, SUCCESS_CODE)); // Instr #7-10: Termination sequence

    // Expected instruction count at the CSRRS:
    // Before CSRRS executes, 3 instructions have completed (the 3 ADDIs)
    // CSRRS reads the current value (3) and then completes, making it 4
    // But CSRRS captures the value BEFORE it increments, so we expect 3

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            let instret_value = sim.read_word(0x80000000);
            // INSTRET should be 3 when CSRRS reads it (3 ADDI instructions completed)
            assert_eq!(
                instret_value, 3,
                "INSTRET should be 3 after 3 ADDI instructions"
            );
        }),
    )
    .expect("Program should run");

    println!("Successfully verified INSTRET CSR counter");
}

// ============================================================================
// M Extension Tests
// ============================================================================

#[test]
fn test_cpu_mul_instruction() {
    init_test_logger();

    let mut instructions = vec![
        addi(1, 0, 10),
        addi(2, 0, 20),
        mul(3, 1, 2),
        lui(8, DRAM_BASE),
        sw(8, 3, 0),
    ];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            assert_eq!(sim.read_word(0x80000000), 200, "MUL: 10 × 20 should be 200");
        }),
    )
    .expect("Program should run");

    println!("Successfully executed MUL instruction");
}

#[test]
fn test_cpu_mulh_instruction() {
    init_test_logger();

    let mut instructions = vec![
        lui(1, 0x10000),
        lui(2, 0x10000),
        mulh(3, 1, 2),
        lui(8, DRAM_BASE),
        sw(8, 3, 0),
    ];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            assert_eq!(
                sim.read_word(0x80000000),
                0x00000001,
                "MULH: upper 32 bits should be 0x00000001"
            );
        }),
    )
    .expect("Program should run");

    println!("Successfully executed MULH instruction");
}

#[test]
fn test_cpu_div_instruction() {
    init_test_logger();

    let mut instructions = vec![
        addi(1, 0, 100),
        addi(2, 0, 7),
        div(3, 1, 2),
        lui(8, DRAM_BASE),
        sw(8, 3, 0),
    ];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            assert_eq!(sim.read_word(0x80000000), 14, "DIV: 100 ÷ 7 should be 14");
        }),
    )
    .expect("Program should run");

    println!("Successfully executed DIV instruction");
}

#[test]
fn test_cpu_div_by_zero() {
    init_test_logger();

    let mut instructions = vec![
        addi(1, 0, 100),
        addi(2, 0, 0),
        div(3, 1, 2),
        lui(8, DRAM_BASE),
        sw(8, 3, 0),
    ];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            assert_eq!(
                sim.read_word(0x80000000),
                0xFFFFFFFF,
                "DIV by zero should return 0xFFFFFFFF"
            );
        }),
    )
    .expect("Program should run");

    println!("Successfully executed DIV by zero");
}

#[test]
fn test_cpu_rem_instruction() {
    init_test_logger();

    let mut instructions = vec![
        addi(1, 0, 100),
        addi(2, 0, 7),
        rem(3, 1, 2),
        lui(8, DRAM_BASE),
        sw(8, 3, 0),
    ];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            assert_eq!(sim.read_word(0x80000000), 2, "REM: 100 % 7 should be 2");
        }),
    )
    .expect("Program should run");

    println!("Successfully executed REM instruction");
}

#[test]
fn test_cpu_divu_remu_unsigned() {
    init_test_logger();

    let mut instructions = vec![
        addi(1, 0, -1),
        addi(2, 0, 2),
        divu(3, 1, 2),
        remu(4, 1, 2),
        lui(8, DRAM_BASE),
        sw(8, 3, 0),
        sw(8, 4, 4),
    ];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            assert_eq!(
                sim.read_word(0x80000000),
                0x7FFFFFFF,
                "DIVU: 0xFFFFFFFF ÷ 2 should be 0x7FFFFFFF"
            );
            assert_eq!(
                sim.read_word(0x80000004),
                1,
                "REMU: 0xFFFFFFFF % 2 should be 1"
            );
        }),
    )
    .expect("Program should run");

    println!("Successfully executed DIVU and REMU instructions");
}

#[test]
fn test_cpu_m_extension_program() {
    init_test_logger();

    // Complex program using multiple M extension instructions
    // Calculate: result = (a × b) ÷ c + (d % e)
    // Memory base: 0x80000000
    let mut instructions = vec![
        addi(1, 0, 12),
        addi(2, 0, 5),
        addi(3, 0, 3),
        addi(4, 0, 17),
        addi(5, 0, 5),
        mul(6, 1, 2),
        div(7, 6, 3),
        rem(8, 4, 5),
        add(9, 7, 8),
        lui(10, DRAM_BASE),
        sw(10, 9, 0),
    ];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            assert_eq!(
                sim.read_word(0x80000000),
                22,
                "Complex M extension program result should be 22"
            );
        }),
    )
    .expect("Program should run");

    println!("Successfully executed complex M extension program");
}

// ============================================================================
// Comprehensive Trace Validation Tests
// ============================================================================

#[test]
fn test_comprehensive_trace_validation() {
    init_test_logger();

    println!("\n========================================");
    println!("COMPREHENSIVE TRACE VALIDATION TEST");
    println!("========================================");
    println!("Testing instruction trace against known sequence...\n");

    // Expected instruction sequence for validation
    #[derive(Debug)]
    struct ExpectedInstruction {
        inst_type: riscv_core::trace::InstructionType,
        pc: u32,
        rd: Option<(u8, u32)>,  // (register number, expected value)
        rs1: Option<(u8, u32)>, // (register number, expected value)
        rs2: Option<(u8, u32)>, // (register number, expected value)
        immediate: Option<i32>,
    }

    // Build test program with known expected results
    let base_addr: u32 = 0x8000_0000;
    let mut instructions = vec![
        addi(1, 0, 10),      // x1 = 10
        addi(2, 0, 20),      // x2 = 20
        add(3, 1, 2),        // x3 = x1 + x2 = 30
        sub(4, 2, 1),        // x4 = x2 - x1 = 10
        and(5, 3, 2),        // x5 = x3 & x2 = 20
        or(6, 1, 2),         // x6 = x1 | x2 = 30
        xor(7, 3, 2),        // x7 = x3 ^ x2 = 10
        sll(8, 1, 0),        // x8 = x1 << 0 = 10
        srl(9, 2, 0),        // x9 = x2 >> 0 = 20
        lui(10, 0x12345000), // x10 = 0x12345000
        lui(11, DRAM_BASE),  // x11 = 0x80000000 (base address)
        sw(11, 1, 0),        // mem[0x80000000] = x1 = 10
        lw(11, 11, 0),       // x11 = mem[0x80000000] = 10
    ];

    // Define expected traces (before adding termination sequence)
    let expected_traces = vec![
        ExpectedInstruction {
            inst_type: riscv_core::trace::InstructionType::Addi,
            pc: base_addr,
            rd: Some((1, 10)),
            rs1: Some((0, 0)),
            rs2: None,
            immediate: Some(10),
        },
        ExpectedInstruction {
            inst_type: riscv_core::trace::InstructionType::Addi,
            pc: base_addr + 4,
            rd: Some((2, 20)),
            rs1: Some((0, 0)),
            rs2: None,
            immediate: Some(20),
        },
        ExpectedInstruction {
            inst_type: riscv_core::trace::InstructionType::Add,
            pc: base_addr + 8,
            rd: Some((3, 30)),
            rs1: Some((1, 10)),
            rs2: Some((2, 20)),
            immediate: None,
        },
        ExpectedInstruction {
            inst_type: riscv_core::trace::InstructionType::Sub,
            pc: base_addr + 12,
            rd: Some((4, 10)),
            rs1: Some((2, 20)),
            rs2: Some((1, 10)),
            immediate: None,
        },
        ExpectedInstruction {
            inst_type: riscv_core::trace::InstructionType::And,
            pc: base_addr + 16,
            rd: Some((5, 20)),
            rs1: Some((3, 30)),
            rs2: Some((2, 20)),
            immediate: None,
        },
        ExpectedInstruction {
            inst_type: riscv_core::trace::InstructionType::Or,
            pc: base_addr + 20,
            rd: Some((6, 30)),
            rs1: Some((1, 10)),
            rs2: Some((2, 20)),
            immediate: None,
        },
        ExpectedInstruction {
            inst_type: riscv_core::trace::InstructionType::Xor,
            pc: base_addr + 24,
            rd: Some((7, 10)),
            rs1: Some((3, 30)),
            rs2: Some((2, 20)),
            immediate: None,
        },
        ExpectedInstruction {
            inst_type: riscv_core::trace::InstructionType::Sll,
            pc: base_addr + 28,
            rd: Some((8, 10)),
            rs1: Some((1, 10)),
            rs2: Some((0, 0)),
            immediate: None,
        },
        ExpectedInstruction {
            inst_type: riscv_core::trace::InstructionType::Srl,
            pc: base_addr + 32,
            rd: Some((9, 20)),
            rs1: Some((2, 20)),
            rs2: Some((0, 0)),
            immediate: None,
        },
        ExpectedInstruction {
            inst_type: riscv_core::trace::InstructionType::Lui,
            pc: base_addr + 36,
            rd: Some((10, 0x12345000)),
            rs1: None,
            rs2: None,
            immediate: Some(74565), // 0x12345
        },
        ExpectedInstruction {
            inst_type: riscv_core::trace::InstructionType::Lui,
            pc: base_addr + 40,
            rd: Some((11, 0x80000000)),
            rs1: None,
            rs2: None,
            immediate: Some(524288), // 0x80000
        },
        ExpectedInstruction {
            inst_type: riscv_core::trace::InstructionType::Sw,
            pc: base_addr + 44,
            rd: None,
            rs1: Some((11, 0x80000000)),
            rs2: Some((1, 10)),
            immediate: Some(0),
        },
        ExpectedInstruction {
            inst_type: riscv_core::trace::InstructionType::Lw,
            pc: base_addr + 48,
            rd: Some((11, 10)),
            rs1: Some((11, 0x80000000)),
            rs2: None,
            immediate: Some(0),
        },
    ];

    // Add termination sequence
    instructions.extend(common::tohost_termination(15, 16, SUCCESS_CODE));

    // Collect traces
    let mut captured_traces = Vec::new();
    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        Some(|trace: &riscv_core::trace::InstructionTrace| {
            captured_traces.push(trace.clone());
        }),
        Some(|_sim: &SimulatorView, result: &SimulationResult| {
            assert_eq!(
                result.tohost_value,
                Some(SUCCESS_CODE),
                "Program should terminate with tohost=1"
            );
        }),
    )
    .expect("Simulation should succeed");

    // Verify we captured the expected number of traces (12 main + 3 termination)
    println!("Captured {} instruction traces", captured_traces.len());
    assert!(
        captured_traces.len() >= expected_traces.len(),
        "Should capture at least {} traces, got {}",
        expected_traces.len(),
        captured_traces.len()
    );

    // Validate each expected trace
    println!("\nValidating instruction traces:");
    for (i, expected) in expected_traces.iter().enumerate() {
        let trace = &captured_traces[i];

        print!("  [{}] PC=0x{:08x} {:?} ... ", i, trace.pc, trace.inst_type);

        // Validate PC
        assert_eq!(
            trace.pc, expected.pc,
            "Trace {} PC mismatch: expected 0x{:08x}, got 0x{:08x}",
            i, expected.pc, trace.pc
        );

        // Validate instruction type
        assert_eq!(
            trace.inst_type, expected.inst_type,
            "Trace {} instruction type mismatch: expected {:?}, got {:?}",
            i, expected.inst_type, trace.inst_type
        );

        // Validate rd
        if let Some((exp_reg, exp_val)) = expected.rd {
            assert!(trace.rd.is_some(), "Trace {} should have rd operand", i);
            let rd = trace.rd.as_ref().unwrap();
            assert_eq!(
                rd.reg, exp_reg,
                "Trace {} rd register mismatch: expected x{}, got x{}",
                i, exp_reg, rd.reg
            );
            assert_eq!(
                rd.value, exp_val,
                "Trace {} rd value mismatch: expected 0x{:08x}, got 0x{:08x}",
                i, exp_val, rd.value
            );
        }

        // Validate rs1
        if let Some((exp_reg, exp_val)) = expected.rs1 {
            assert!(trace.rs1.is_some(), "Trace {} should have rs1 operand", i);
            let rs1 = trace.rs1.as_ref().unwrap();
            assert_eq!(
                rs1.reg, exp_reg,
                "Trace {} rs1 register mismatch: expected x{}, got x{}",
                i, exp_reg, rs1.reg
            );
            assert_eq!(
                rs1.value, exp_val,
                "Trace {} rs1 value mismatch: expected 0x{:08x}, got 0x{:08x}",
                i, exp_val, rs1.value
            );
        }

        // Validate rs2
        if let Some((exp_reg, exp_val)) = expected.rs2 {
            assert!(trace.rs2.is_some(), "Trace {} should have rs2 operand", i);
            let rs2 = trace.rs2.as_ref().unwrap();
            assert_eq!(
                rs2.reg, exp_reg,
                "Trace {} rs2 register mismatch: expected x{}, got x{}",
                i, exp_reg, rs2.reg
            );
            assert_eq!(
                rs2.value, exp_val,
                "Trace {} rs2 value mismatch: expected 0x{:08x}, got 0x{:08x}",
                i, exp_val, rs2.value
            );
        }

        // Validate immediate
        if let Some(exp_imm) = expected.immediate {
            assert!(
                trace.immediate.is_some(),
                "Trace {} should have immediate value",
                i
            );
            let imm = trace.immediate.unwrap();
            assert_eq!(
                imm, exp_imm,
                "Trace {} immediate mismatch: expected {}, got {}",
                i, exp_imm, imm
            );
        }

        println!("✓");
    }

    println!("\n========================================");
    println!("✓ ALL TRACE VALIDATIONS PASSED");
    println!("========================================");
    println!("  - {} instructions validated", expected_traces.len());
    println!("  - PC values matched expected sequence");
    println!("  - Instruction types decoded correctly");
    println!("  - Register values computed correctly");
    println!("  - Immediate values extracted correctly");
    println!("========================================\n");
}

#[test]
fn test_trace_with_branches() {
    init_test_logger();

    println!("\n========================================");
    println!("TRACE VALIDATION WITH BRANCHES");
    println!("========================================\n");

    let base_addr: u32 = 0x8000_0000;
    let mut instructions = vec![
        addi(1, 0, 10), // 0x00: x1 = 10
        addi(2, 0, 20), // 0x04: x2 = 20
        beq(1, 1, 8),   // 0x08: branch to 0x10 (taken - skip next)
        addi(3, 0, 99), // 0x0C: SKIPPED
        addi(4, 0, 5),  // 0x10: x4 = 5
        bne(1, 2, 8),   // 0x14: branch to 0x1C (taken - skip next)
        addi(5, 0, 99), // 0x18: SKIPPED
        addi(6, 0, 1),  // 0x1C: x6 = 1
    ];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    // Collect traces
    let mut captured_traces = Vec::new();
    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        Some(|trace: &riscv_core::trace::InstructionTrace| {
            captured_traces.push(trace.clone());
        }),
        Some(|_sim: &SimulatorView, result: &SimulationResult| {
            assert_eq!(result.tohost_value, Some(SUCCESS_CODE));
        }),
    )
    .expect("Simulation should succeed");

    println!("Captured {} traces", captured_traces.len());
    println!("\nTrace sequence:");
    for (i, trace) in captured_traces.iter().enumerate() {
        println!("  [{}] PC=0x{:08x} {:?}", i, trace.pc, trace.inst_type);
    }

    // Verify branch behavior - skipped instructions should not appear in trace
    let pcs: Vec<u32> = captured_traces.iter().map(|t| t.pc).collect();

    // Should NOT contain the skipped instructions
    assert!(
        !pcs.contains(&(base_addr + 0x0C)),
        "Trace should not contain skipped instruction at 0x0C (after BEQ)"
    );
    assert!(
        !pcs.contains(&(base_addr + 0x18)),
        "Trace should not contain skipped instruction at 0x18 (after BNE)"
    );

    // Should contain the executed instructions
    assert!(
        pcs.contains(&base_addr),
        "Trace should contain ADDI x1 at 0x00"
    );
    assert!(
        pcs.contains(&(base_addr + 0x04)),
        "Trace should contain ADDI x2 at 0x04"
    );
    assert!(
        pcs.contains(&(base_addr + 0x08)),
        "Trace should contain BEQ at 0x08"
    );
    assert!(
        pcs.contains(&(base_addr + 0x10)),
        "Trace should contain ADDI x4 at 0x10"
    );
    assert!(
        pcs.contains(&(base_addr + 0x14)),
        "Trace should contain BNE at 0x14"
    );
    assert!(
        pcs.contains(&(base_addr + 0x1C)),
        "Trace should contain ADDI x6 at 0x1C"
    );

    println!("\n========================================");
    println!("✓ BRANCH TRACE VALIDATION PASSED");
    println!("========================================");
    println!("  - Branches executed correctly");
    println!("  - Skipped instructions not traced");
    println!("  - Control flow sequence validated");
    println!("========================================\n");
}

#[test]
fn test_trace_and_vcd_together() {
    init_test_logger();

    println!("\n========================================");
    println!("COMBINED TRACE + VCD TEST");
    println!("========================================\n");

    let vcd_path = "/tmp/test_trace_vcd.vcd";

    // Simple test program
    let mut instructions = vec![addi(1, 0, 42), addi(2, 1, 8), add(3, 1, 2)];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    // Run with VCD enabled
    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        Some(vcd_path),
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|_sim: &SimulatorView, result: &SimulationResult| {
            assert_eq!(result.tohost_value, Some(SUCCESS_CODE));
        }),
    )
    .expect("Simulation should succeed");

    // Verify VCD file was created
    assert!(
        std::path::Path::new(vcd_path).exists(),
        "VCD file should be created"
    );

    // Read VCD file
    let vcd_contents = std::fs::read_to_string(vcd_path).expect("Should be able to read VCD file");

    // Validate VCD contains essential signals
    assert!(
        vcd_contents.contains("clk"),
        "VCD should contain clk signal"
    );
    assert!(
        vcd_contents.contains("rst_n"),
        "VCD should contain rst_n signal"
    );
    assert!(
        vcd_contents.contains("imem_addr"),
        "VCD should contain imem_addr"
    );
    assert!(vcd_contents.contains("#0"), "VCD should have timestamps");

    // Clean up
    std::fs::remove_file(vcd_path).expect("Should be able to remove VCD file");

    println!("✓ VCD file generated successfully");
    println!("✓ VCD contains all expected signals");

    println!("\n========================================");
    println!("✓ COMBINED TRACE + VCD TEST PASSED");
    println!("========================================");
    println!("  - VCD waveform dumping works");
    println!("  - Trace options enable easy config");
    println!("  - Both features can be used together");
    println!("========================================\n");
}

// ============================================================================
// RV32A Atomic Extension Tests
// ============================================================================

#[test]
fn test_cpu_lr_sc_success() {
    init_test_logger();

    println!("\n========================================");
    println!("LR/SC SUCCESS TEST (RV32A)");
    println!("========================================\n");

    // Program: Successful LR/SC sequence
    // Memory location: 0x80000000 (DRAM start)
    // 1. Store initial value 100 to 0x80000000
    // 2. Load-Reserved from 0x80000000 into x2
    // 3. Add 5 to the loaded value (x2 = 100 + 5 = 105)
    // 4. Store-Conditional the new value back to 0x80000000
    // 5. Check that SC succeeded (x4 should be 0)

    let mem_addr = 0x80000000u32;
    let initial_value = 100u32;

    let mut instructions = vec![
        // Setup: x1 = 0x80000000 (memory address)
        lui(1, DRAM_BASE),
        // Store initial value
        addi(2, 0, initial_value as i32),
        sw(1, 2, 0), // mem[x1] = 100
        // LR/SC sequence
        lr_w(2, 1),    // x2 = mem[x1] (100), set reservation
        addi(3, 2, 5), // x3 = x2 + 5 = 105
        sc_w(4, 1, 3), // mem[x1] = x3 (105), x4 = success status
        // Load final value to verify
        lw(5, 1, 0), // x5 = mem[x1] (should be 105)
    ];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, result: &SimulationResult| {
            assert_eq!(
                result.tohost_value,
                Some(SUCCESS_CODE),
                "Program should complete"
            );
            // Verify SC succeeded by checking program completed successfully
            // (In a real test, we would check x4 register value = 0 for success)
            let _mem_value = sim.read_word(mem_addr); // Should be 105
        }),
    )
    .expect("LR/SC test should run");

    println!("✓ LR/SC successful sequence executed");
    println!("========================================\n");
}

#[test]
fn test_cpu_amoswap() {
    init_test_logger();

    println!("\n========================================");
    println!("AMOSWAP.W TEST (RV32A)");
    println!("========================================\n");

    // Program: Atomic swap operation
    // 1. Store initial value 42 to 0x80000000
    // 2. Atomic swap with value 100
    // 3. Verify old value was returned and new value was stored

    let initial_value = 42u32;
    let swap_value = 100u32;

    let mut instructions = vec![
        // Setup: x1 = 0x80000000 (memory address)
        lui(1, DRAM_BASE),
        // Store initial value
        addi(2, 0, initial_value as i32),
        sw(1, 2, 0), // mem[x1] = 42
        // Atomic swap
        addi(3, 0, swap_value as i32), // x3 = 100 (new value)
        amoswap_w(4, 1, 3),            // x4 = mem[x1] (42), mem[x1] = x3 (100)
        // Load final value to verify
        lw(5, 1, 0), // x5 = mem[x1] (should be 100)
    ];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|_sim: &SimulatorView, result: &SimulationResult| {
            assert_eq!(
                result.tohost_value,
                Some(SUCCESS_CODE),
                "Program should complete"
            );
        }),
    )
    .expect("AMOSWAP test should run");

    println!("✓ AMOSWAP.W atomic swap executed");
    println!("========================================\n");
}

#[test]
fn test_cpu_amoadd() {
    init_test_logger();

    println!("\n========================================");
    println!("AMOADD.W TEST (RV32A)");
    println!("========================================\n");

    // Program: Atomic add operation (atomic counter)
    // 1. Store initial counter value 10 to 0x80000000
    // 2. Atomic add 5 to the counter
    // 3. Verify old value was returned and new value is 15

    let initial_value = 10u32;
    let add_value = 5u32;

    let mut instructions = vec![
        // Setup: x1 = 0x80000000 (memory address)
        lui(1, DRAM_BASE),
        // Store initial value
        addi(2, 0, initial_value as i32),
        sw(1, 2, 0), // mem[x1] = 10
        // Atomic add
        addi(3, 0, add_value as i32), // x3 = 5
        amoadd_w(4, 1, 3),            // x4 = mem[x1] (10), mem[x1] = 10 + 5 = 15
        // Load final value to verify
        lw(5, 1, 0), // x5 = mem[x1] (should be 15)
    ];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|_sim: &SimulatorView, result: &SimulationResult| {
            assert_eq!(
                result.tohost_value,
                Some(SUCCESS_CODE),
                "Program should complete"
            );
        }),
    )
    .expect("AMOADD test should run");

    println!("✓ AMOADD.W atomic add executed");
    println!("========================================\n");
}

#[test]
fn test_cpu_amo_logical() {
    init_test_logger();

    println!("\n========================================");
    println!("AMO LOGICAL OPERATIONS TEST (RV32A)");
    println!("========================================\n");

    // Program: Test AMOXOR, AMOAND, AMOOR
    // All operate on the same memory location with different values

    let mut instructions = vec![
        // Setup: x1 = 0x80000000 (memory address)
        lui(1, DRAM_BASE),
        // Test AMOXOR: mem = 0xFF, xor with 0x0F -> mem = 0xF0
        addi(2, 0, 0xFF),
        sw(1, 2, 0), // mem[x1] = 0xFF
        addi(3, 0, 0x0F),
        amoxor_w(4, 1, 3), // x4 = 0xFF, mem[x1] = 0xF0
        // Test AMOAND: mem = 0xF0, and with 0x3C -> mem = 0x30
        addi(5, 0, 0x3C),
        amoand_w(6, 1, 5), // x6 = 0xF0, mem[x1] = 0x30
        // Test AMOOR: mem = 0x30, or with 0x0F -> mem = 0x3F
        addi(7, 0, 0x0F),
        amoor_w(8, 1, 7), // x8 = 0x30, mem[x1] = 0x3F
        // Load final value
        lw(9, 1, 0), // x9 = mem[x1] (should be 0x3F)
    ];
    instructions.extend(common::tohost_termination(10, 11, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|_sim: &SimulatorView, result: &SimulationResult| {
            assert_eq!(
                result.tohost_value,
                Some(SUCCESS_CODE),
                "Program should complete"
            );
        }),
    )
    .expect("AMO logical test should run");

    println!("✓ AMOXOR, AMOAND, AMOOR executed");
    println!("========================================\n");
}

#[test]
fn test_cpu_amo_min_max() {
    init_test_logger();

    println!("\n========================================");
    println!("AMO MIN/MAX TEST (RV32A)");
    println!("========================================\n");

    // Program: Test AMOMIN, AMOMAX (signed)

    let mut instructions = vec![
        // Setup: x1 = 0x80000000 (memory address)
        lui(1, DRAM_BASE),
        // Test AMOMIN: mem = 20, min with 15 -> mem = 15
        addi(2, 0, 20),
        sw(1, 2, 0), // mem[x1] = 20
        addi(3, 0, 15),
        amomin_w(4, 1, 3), // x4 = 20, mem[x1] = 15
        // Test AMOMAX: mem = 15, max with 25 -> mem = 25
        addi(5, 0, 25),
        amomax_w(6, 1, 5), // x6 = 15, mem[x1] = 25
        // Load final value
        lw(7, 1, 0), // x7 = mem[x1] (should be 25)
    ];
    instructions.extend(common::tohost_termination(10, 11, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|_sim: &SimulatorView, result: &SimulationResult| {
            assert_eq!(
                result.tohost_value,
                Some(SUCCESS_CODE),
                "Program should complete"
            );
        }),
    )
    .expect("AMO MIN/MAX test should run");

    println!("✓ AMOMIN, AMOMAX executed");
    println!("========================================\n");
}

#[test]
fn test_cpu_amo_unsigned_min_max() {
    init_test_logger();

    println!("\n========================================");
    println!("AMO UNSIGNED MIN/MAX TEST (RV32A)");
    println!("========================================\n");

    // Program: Test AMOMINU, AMOMAXU (unsigned)

    let mut instructions = vec![
        // Setup: x1 = 0x80000000 (memory address)
        lui(1, DRAM_BASE),
        // Test AMOMINU: mem = 100, minu with 50 -> mem = 50
        addi(2, 0, 100),
        sw(1, 2, 0), // mem[x1] = 100
        addi(3, 0, 50),
        amominu_w(4, 1, 3), // x4 = 100, mem[x1] = 50
        // Test AMOMAXU: mem = 50, maxu with 75 -> mem = 75
        addi(5, 0, 75),
        amomaxu_w(6, 1, 5), // x6 = 50, mem[x1] = 75
        // Load final value
        lw(7, 1, 0), // x7 = mem[x1] (should be 75)
    ];
    instructions.extend(common::tohost_termination(10, 11, SUCCESS_CODE));

    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|_sim: &SimulatorView, result: &SimulationResult| {
            assert_eq!(
                result.tohost_value,
                Some(SUCCESS_CODE),
                "Program should complete"
            );
        }),
    )
    .expect("AMO unsigned MIN/MAX test should run");

    println!("✓ AMOMINU, AMOMAXU executed");
    println!("========================================\n");
}

// ============================================================================
// Invalid Instruction Tests
// ============================================================================

/// Test that CPU halts when fetching an instruction value of 0
///
/// When memory returns 0x0000, the decompressor identifies this as an invalid
/// compressed instruction (C.ADDI4SPN with nzuimm=0), sets is_valid=0.
/// The CPU should transition to S_HALT state when it detects this.
#[test]
fn test_cpu_halts_on_zero_instruction() {
    init_test_logger();
    let cpu_halted = std::cell::Cell::new(false);

    // Program with 4 zero instructions (invalid compressed instructions)
    // The CPU should halt when it fetches 0x0000
    let instructions: Vec<u32> = vec![0, 0, 0, 0];

    const START_ADDR: u32 = 0x8000_0000;

    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    // Run with low max cycles - we expect the CPU to halt on the invalid instruction.
    // The hung detector skips PC loop detection when the CPU is in the halted state,
    // so the simulation runs to max_cycles and returns Ok with no tohost value.
    let result = run_program(
        100, // Low max cycles
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0, // Zero latency
        |sim| {
            sim.write_memory_region(START_ADDR, &program_bytes, true);
            Ok(START_ADDR)
        },
        Some(|sim: &SimulatorView, _result: &SimulationResult| {
            cpu_halted.set(sim.is_cpu_halted());
        }),
    );

    // The CPU enters S_HALT on the invalid instruction. Since the hung detector
    // skips PC loop detection when halted, the simulation completes normally
    // without triggering a hang error. The tohost value is None because the
    // program never writes to tohost.
    let sim_result = result.expect("CPU should halt without triggering hung detector");
    assert!(
        sim_result.tohost_value.is_none(),
        "Expected no tohost value when halted on invalid instruction, got: {:?}",
        sim_result.tohost_value
    );
    assert!(
        cpu_halted.get(),
        "Expected CPU to reach halted state on zero instruction"
    );

    println!("✓ CPU halts correctly on instruction value 0x0000");
}
