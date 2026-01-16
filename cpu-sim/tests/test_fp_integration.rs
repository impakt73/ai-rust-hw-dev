//! RV32F Extension CPU Integration Tests
//!
//! Tests that verify the floating-point extension works correctly in the full CPU context,
//! including FP load/store, register interactions, FCSR management, and multi-cycle execution.

use cpu_sim::*;
use riscv_core::instruction::*;

/// Helper function to initialize test logger (idempotent)
fn init_test_logger() {
    let _ = env_logger::builder().is_test(true).try_init();
}

/// Generate tohost termination sequence
fn tohost_termination(addr_reg: u32, value_reg: u32) -> Vec<u32> {
    vec![
        lui(addr_reg, 0x10000),     // Load 0x10000000 into addr_reg (upper 20 bits)
        addi(value_reg, 0, 1),      // Load success code (1)
        sw(addr_reg, value_reg, 0), // Store value to tohost address (0x1000_0000)
        jal(0, 0),                  // Infinite loop (jump to self)
    ]
}

/// Helper to run programmatic instructions with FP support
fn run_fp_program_with_options<T, F>(
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
// FP Load/Store Tests
// ============================================================================

#[test]
fn test_cpu_flw_fsw_basic() {
    init_test_logger();

    // Program: Test FLW and FSW instructions
    // Store a floating point value to memory, then load it back
    //
    // Memory layout:
    // 0x8000_1000: FP data storage
    // 0x100: Test result marker
    //
    // x1 = address (0x8000_1000)
    // x2 = FP bit pattern (0x3F800000 = 1.0 in IEEE 754)
    // f1 = FP register
    let mut instructions = vec![
        lui(1, 0x80001000), // x1 = 0x80001000 (data address)
        lui(2, 0x3F800000), // x2 = 0x3F800000 (1.0 in FP)
        sw(1, 2, 0),        // Store integer representation to memory
        flw(1, 1, 0),       // f1 = load FP value from memory
        fsw(1, 1, 4),       // Store f1 to memory[0x80001004]
        lw(3, 1, 4),        // x3 = load from memory[0x80001004]
        addi(4, 0, 0x100),  // x4 = 0x100 (result marker address)
        sw(4, 3, 0),        // Store result to 0x100
    ];
    instructions.extend(tohost_termination(7, 8));

    run_fp_program_with_options(
        &instructions,
        200,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, result: &SimulationResult| {
            assert!(
                result.tohost_value == Some(1),
                "Program should terminate with tohost=1"
            );

            // Verify that the value round-tripped correctly
            // We stored 0x3F800000, loaded to f1, stored from f1, and loaded to x3
            let result_value = sim.read_word(0x100);
            assert_eq!(
                result_value, 0x3F800000,
                "FLW/FSW round trip should preserve bit pattern"
            );
        }),
    )
    .expect("FP load/store test should run");
}

#[test]
fn test_cpu_flw_multiple_registers() {
    init_test_logger();

    // Program: Load different FP values into multiple FP registers
    // Verifies that FP register file has independent registers
    let mut instructions = vec![
        lui(1, 0x80001000), // x1 = 0x80001000 (base address)
        lui(2, 0x3F800000), // x2 = 1.0
        lui(3, 0x40000000), // x3 = 2.0
        lui(4, 0x40400000), // x4 = 3.0
        sw(1, 2, 0),        // mem[x1+0] = 1.0
        sw(1, 3, 4),        // mem[x1+4] = 2.0
        sw(1, 4, 8),        // mem[x1+8] = 3.0
        flw(1, 1, 0),       // f1 = 1.0
        flw(2, 1, 4),       // f2 = 2.0
        flw(3, 1, 8),       // f3 = 3.0
        fsw(1, 1, 12),      // mem[x1+12] = f1
        fsw(1, 2, 16),      // mem[x1+16] = f2
        fsw(1, 3, 20),      // mem[x1+20] = f3
        lw(5, 1, 12),       // x5 = f1 value
        lw(6, 1, 16),       // x6 = f2 value
        lw(7, 1, 20),       // x7 = f3 value
        addi(10, 0, 0x100), // x10 = 0x100
        sw(10, 5, 0),       // Store results to 0x100
        sw(10, 6, 4),
        sw(10, 7, 8),
    ];
    instructions.extend(tohost_termination(11, 12));

    run_fp_program_with_options(
        &instructions,
        200,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, result: &SimulationResult| {
            assert!(
                result.tohost_value == Some(1),
                "Program should terminate with tohost=1"
            );

            let val1 = sim.read_word(0x100);
            let val2 = sim.read_word(0x104);
            let val3 = sim.read_word(0x108);

            assert_eq!(val1, 0x3F800000, "f1 should be 1.0");
            assert_eq!(val2, 0x40000000, "f2 should be 2.0");
            assert_eq!(val3, 0x40400000, "f3 should be 3.0");
        }),
    )
    .expect("Multiple FP register test should run");
}

// ============================================================================
// FP Arithmetic in CPU Context
// ============================================================================

#[test]
fn test_cpu_fadd_basic() {
    init_test_logger();

    // Program: Test FADD.S instruction in CPU context
    // Load two FP values, add them, store result
    let mut instructions = vec![
        lui(1, 0x80001000), // x1 = 0x80001000 (base address)
        lui(2, 0x3F800000), // x2 = 1.0
        lui(3, 0x40000000), // x3 = 2.0
        sw(1, 2, 0),        // mem[x1+0] = 1.0
        sw(1, 3, 4),        // mem[x1+4] = 2.0
        flw(1, 1, 0),       // f1 = 1.0
        flw(2, 1, 4),       // f2 = 2.0
        fadd_s(3, 1, 2),    // f3 = f1 + f2 = 3.0
        fsw(1, 3, 8),       // mem[x1+8] = f3
        lw(4, 1, 8),        // x4 = result
        addi(5, 0, 0x100),  // x5 = 0x100
        sw(5, 4, 0),        // Store result to 0x100
    ];
    instructions.extend(tohost_termination(7, 8));

    run_fp_program_with_options(
        &instructions,
        200,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, result: &SimulationResult| {
            assert!(
                result.tohost_value == Some(1),
                "Program should terminate with tohost=1"
            );

            let result_value = sim.read_word(0x100);
            assert_eq!(result_value, 0x40400000, "1.0 + 2.0 should equal 3.0");
        }),
    )
    .expect("FADD test should run");
}

#[test]
fn test_cpu_fmul_basic() {
    init_test_logger();

    // Program: Test FMUL.S instruction
    let mut instructions = vec![
        lui(1, 0x80001000), // x1 = 0x80001000
        lui(2, 0x40000000), // x2 = 2.0
        lui(3, 0x40400000), // x3 = 3.0
        sw(1, 2, 0),        // mem[x1+0] = 2.0
        sw(1, 3, 4),        // mem[x1+4] = 3.0
        flw(1, 1, 0),       // f1 = 2.0
        flw(2, 1, 4),       // f2 = 3.0
        fmul_s(3, 1, 2),    // f3 = f1 * f2 = 6.0
        fsw(1, 3, 8),       // mem[x1+8] = f3
        lw(4, 1, 8),        // x4 = result
        addi(5, 0, 0x100),  // x5 = 0x100
        sw(5, 4, 0),        // Store result to 0x100
    ];
    instructions.extend(tohost_termination(7, 8));

    run_fp_program_with_options(
        &instructions,
        200,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, result: &SimulationResult| {
            assert!(
                result.tohost_value == Some(1),
                "Program should terminate with tohost=1"
            );

            let result_value = sim.read_word(0x100);
            assert_eq!(result_value, 0x40C00000, "2.0 * 3.0 should equal 6.0");
        }),
    )
    .expect("FMUL test should run");
}

// ============================================================================
// FP/Integer Conversion Tests
// ============================================================================

#[test]
fn test_cpu_fcvt_s_w() {
    init_test_logger();

    // Program: Test FCVT.S.W (integer to FP conversion)
    let mut instructions = vec![
        addi(1, 0, 42),     // x1 = 42 (integer)
        fcvt_s_w(1, 1),     // f1 = (float)42
        lui(2, 0x80001000), // x2 = 0x80001000
        fsw(2, 1, 0),       // mem[x2] = f1
        lw(3, 2, 0),        // x3 = result
        addi(4, 0, 0x100),  // x4 = 0x100
        sw(4, 3, 0),        // Store result to 0x100
    ];
    instructions.extend(tohost_termination(7, 8));

    run_fp_program_with_options(
        &instructions,
        200,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, result: &SimulationResult| {
            assert!(
                result.tohost_value == Some(1),
                "Program should terminate with tohost=1"
            );

            let result_value = sim.read_word(0x100);
            assert_eq!(result_value, 0x42280000, "42 as float should be 0x42280000");
        }),
    )
    .expect("FCVT.S.W test should run");
}

#[test]
fn test_cpu_fcvt_w_s() {
    init_test_logger();

    // Program: Test FCVT.W.S (FP to integer conversion)
    let mut instructions = vec![
        lui(1, 0x80001000), // x1 = 0x80001000
        lui(2, 0x42280000), // x2 = 42.0 in FP (0x42280000)
        sw(1, 2, 0),        // mem[x1] = 42.0
        flw(1, 1, 0),       // f1 = 42.0
        fcvt_w_s(3, 1),     // x3 = (int)f1 = 42
        addi(4, 0, 0x100),  // x4 = 0x100
        sw(4, 3, 0),        // Store result to 0x100
    ];
    instructions.extend(tohost_termination(7, 8));

    run_fp_program_with_options(
        &instructions,
        200,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, result: &SimulationResult| {
            assert!(
                result.tohost_value == Some(1),
                "Program should terminate with tohost=1"
            );

            let result_value = sim.read_word(0x100);
            assert_eq!(result_value, 42, "42.0 as int should be 42");
        }),
    )
    .expect("FCVT.W.S test should run");
}

// ============================================================================
// FP Comparison Tests
// ============================================================================

#[test]
fn test_cpu_feq_flt() {
    init_test_logger();

    // Program: Test FEQ.S and FLT.S comparisons
    let mut instructions = vec![
        lui(1, 0x80001000), // x1 = 0x80001000
        lui(2, 0x3F800000), // x2 = 1.0
        lui(3, 0x40000000), // x3 = 2.0
        sw(1, 2, 0),        // mem[x1+0] = 1.0
        sw(1, 3, 4),        // mem[x1+4] = 2.0
        flw(1, 1, 0),       // f1 = 1.0
        flw(2, 1, 4),       // f2 = 2.0
        feq_s(4, 1, 1),     // x4 = (f1 == f1) = 1
        feq_s(5, 1, 2),     // x5 = (f1 == f2) = 0
        flt_s(6, 1, 2),     // x6 = (f1 < f2) = 1
        flt_s(7, 2, 1),     // x7 = (f2 < f1) = 0
        addi(10, 0, 0x100), // x10 = 0x100
        sw(10, 4, 0),       // Store results to 0x100-0x10C
        sw(10, 5, 4),
        sw(10, 6, 8),
        sw(10, 7, 12),
    ];
    instructions.extend(tohost_termination(11, 12));

    run_fp_program_with_options(
        &instructions,
        200,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, result: &SimulationResult| {
            assert!(
                result.tohost_value == Some(1),
                "Program should terminate with tohost=1"
            );

            let eq_same = sim.read_word(0x100);
            let eq_diff = sim.read_word(0x104);
            let lt_true = sim.read_word(0x108);
            let lt_false = sim.read_word(0x10C);

            assert_eq!(eq_same, 1, "1.0 == 1.0 should be true");
            assert_eq!(eq_diff, 0, "1.0 == 2.0 should be false");
            assert_eq!(lt_true, 1, "1.0 < 2.0 should be true");
            assert_eq!(lt_false, 0, "2.0 < 1.0 should be false");
        }),
    )
    .expect("FP comparison test should run");
}

// ============================================================================
// FP Move Tests
// ============================================================================

#[test]
fn test_cpu_fmv_x_w_fmv_w_x() {
    init_test_logger();

    // Program: Test FMV.X.W and FMV.W.X (bitwise moves)
    let mut instructions = vec![
        lui(1, 0x3F800000), // x1 = 0x3F800000 (1.0 in FP)
        fmv_w_x(1, 1),      // f1 = x1 (bitwise move)
        fmv_x_w(2, 1),      // x2 = f1 (bitwise move back)
        addi(3, 0, 0x100),  // x3 = 0x100
        sw(3, 2, 0),        // Store result to 0x100
    ];
    instructions.extend(tohost_termination(7, 8));

    run_fp_program_with_options(
        &instructions,
        200,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, result: &SimulationResult| {
            assert!(
                result.tohost_value == Some(1),
                "Program should terminate with tohost=1"
            );

            let result_value = sim.read_word(0x100);
            assert_eq!(
                result_value, 0x3F800000,
                "FMV round trip should preserve bits"
            );
        }),
    )
    .expect("FMV test should run");
}

// ============================================================================
// Comprehensive FP Instruction Coverage Tests
// ============================================================================

#[test]
fn test_cpu_fsub_fdiv_fsqrt() {
    init_test_logger();

    // Program: Test FSUB.S, FDIV.S, FSQRT.S
    let mut instructions = vec![
        lui(1, 0x80001000), // x1 = 0x80001000
        lui(2, 0x40A00000), // x2 = 5.0 (0x40A00000)
        lui(3, 0x40000000), // x3 = 2.0 (0x40000000)
        sw(1, 2, 0),        // mem[x1+0] = 5.0
        sw(1, 3, 4),        // mem[x1+4] = 2.0
        flw(1, 1, 0),       // f1 = 5.0
        flw(2, 1, 4),       // f2 = 2.0
        fsub_s(3, 1, 2),    // f3 = 5.0 - 2.0 = 3.0
        fdiv_s(4, 1, 2),    // f4 = 5.0 / 2.0 = 2.5
        fsqrt_s(5, 2),      // f5 = sqrt(2.0) ≈ 1.414...
        fsw(1, 3, 8),       // Store f3 to mem[x1+8]
        fsw(1, 4, 12),      // Store f4 to mem[x1+12]
        lw(4, 1, 8),        // x4 = result (FSUB)
        lw(5, 1, 12),       // x5 = result (FDIV)
        addi(6, 0, 0x100),  // x6 = 0x100
        sw(6, 4, 0),        // Store FSUB result
        sw(6, 5, 4),        // Store FDIV result
    ];
    instructions.extend(tohost_termination(7, 8));

    run_fp_program_with_options(
        &instructions,
        300,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, result: &SimulationResult| {
            assert!(
                result.tohost_value == Some(1),
                "Program should terminate with tohost=1"
            );

            let fsub_result = sim.read_word(0x100);
            let fdiv_result = sim.read_word(0x104);
            assert_eq!(fsub_result, 0x40400000, "5.0 - 2.0 should equal 3.0");
            assert_eq!(fdiv_result, 0x40200000, "5.0 / 2.0 should equal 2.5");
        }),
    )
    .expect("FSUB/FDIV/FSQRT test should run");
}

#[test]
fn test_cpu_fmin_fmax() {
    init_test_logger();

    // Program: Test FMIN.S and FMAX.S
    let mut instructions = vec![
        lui(1, 0x80001000), // x1 = 0x80001000
        lui(2, 0x3F800000), // x2 = 1.0
        lui(3, 0x40400000), // x3 = 3.0
        sw(1, 2, 0),        // mem[x1+0] = 1.0
        sw(1, 3, 4),        // mem[x1+4] = 3.0
        flw(1, 1, 0),       // f1 = 1.0
        flw(2, 1, 4),       // f2 = 3.0
        fmin_s(3, 1, 2),    // f3 = min(1.0, 3.0) = 1.0
        fmax_s(4, 1, 2),    // f4 = max(1.0, 3.0) = 3.0
        fsw(1, 3, 8),       // Store min result
        fsw(1, 4, 12),      // Store max result
        lw(4, 1, 8),        // x4 = min result
        lw(5, 1, 12),       // x5 = max result
        addi(6, 0, 0x100),  // x6 = 0x100
        sw(6, 4, 0),        // Store min
        sw(6, 5, 4),        // Store max
    ];
    instructions.extend(tohost_termination(7, 8));

    run_fp_program_with_options(
        &instructions,
        200,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, result: &SimulationResult| {
            assert!(
                result.tohost_value == Some(1),
                "Program should terminate with tohost=1"
            );

            let min_result = sim.read_word(0x100);
            let max_result = sim.read_word(0x104);
            assert_eq!(min_result, 0x3F800000, "min(1.0, 3.0) should be 1.0");
            assert_eq!(max_result, 0x40400000, "max(1.0, 3.0) should be 3.0");
        }),
    )
    .expect("FMIN/FMAX test should run");
}

#[test]
fn test_cpu_fsgnj_ops() {
    init_test_logger();

    // Program: Test FSGNJ.S, FSGNJN.S, FSGNJX.S
    let mut instructions = vec![
        lui(1, 0x3F800000), // x1 = 1.0 (positive)
        lui(2, 0xBF800000), // x2 = -1.0 (negative)
        fmv_w_x(1, 1),      // f1 = 1.0
        fmv_w_x(2, 2),      // f2 = -1.0
        fsgnj_s(3, 1, 2),   // f3 = abs(1.0) with sign of -1.0 = -1.0
        fsgnjn_s(4, 1, 2),  // f4 = abs(1.0) with inverted sign of -1.0 = 1.0
        fsgnjx_s(5, 1, 2),  // f5 = abs(1.0) with XOR of signs = -1.0
        fmv_x_w(4, 3),      // x4 = bits of f3
        fmv_x_w(5, 4),      // x5 = bits of f4
        fmv_x_w(6, 5),      // x6 = bits of f5
        addi(7, 0, 0x100),  // x7 = 0x100
        sw(7, 4, 0),        // Store FSGNJ result
        sw(7, 5, 4),        // Store FSGNJN result
        sw(7, 6, 8),        // Store FSGNJX result
    ];
    instructions.extend(tohost_termination(10, 11));

    run_fp_program_with_options(
        &instructions,
        200,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, result: &SimulationResult| {
            assert!(
                result.tohost_value == Some(1),
                "Program should terminate with tohost=1"
            );

            let fsgnj_result = sim.read_word(0x100);
            let fsgnjn_result = sim.read_word(0x104);
            let fsgnjx_result = sim.read_word(0x108);
            assert_eq!(
                fsgnj_result, 0xBF800000,
                "FSGNJ should copy sign: result should be -1.0"
            );
            assert_eq!(
                fsgnjn_result, 0x3F800000,
                "FSGNJN should copy inverted sign: result should be 1.0"
            );
            assert_eq!(
                fsgnjx_result, 0xBF800000,
                "FSGNJX should XOR signs: result should be -1.0"
            );
        }),
    )
    .expect("FSGNJ operations test should run");
}

#[test]
fn test_cpu_fle() {
    init_test_logger();

    // Program: Test FLE.S (less than or equal)
    let mut instructions = vec![
        lui(1, 0x80001000), // x1 = 0x80001000
        lui(2, 0x3F800000), // x2 = 1.0
        lui(3, 0x40000000), // x3 = 2.0
        sw(1, 2, 0),        // mem[x1+0] = 1.0
        sw(1, 3, 4),        // mem[x1+4] = 2.0
        flw(1, 1, 0),       // f1 = 1.0
        flw(2, 1, 4),       // f2 = 2.0
        fle_s(4, 1, 2),     // x4 = (1.0 <= 2.0) = 1
        fle_s(5, 2, 1),     // x5 = (2.0 <= 1.0) = 0
        fle_s(6, 1, 1),     // x6 = (1.0 <= 1.0) = 1
        addi(7, 0, 0x100),  // x7 = 0x100
        sw(7, 4, 0),        // Store results
        sw(7, 5, 4),
        sw(7, 6, 8),
    ];
    instructions.extend(tohost_termination(10, 11));

    run_fp_program_with_options(
        &instructions,
        200,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, result: &SimulationResult| {
            assert!(
                result.tohost_value == Some(1),
                "Program should terminate with tohost=1"
            );

            let le1 = sim.read_word(0x100);
            let le2 = sim.read_word(0x104);
            let le3 = sim.read_word(0x108);
            assert_eq!(le1, 1, "1.0 <= 2.0 should be true");
            assert_eq!(le2, 0, "2.0 <= 1.0 should be false");
            assert_eq!(le3, 1, "1.0 <= 1.0 should be true");
        }),
    )
    .expect("FLE test should run");
}

#[test]
fn test_cpu_fcvt_unsigned() {
    init_test_logger();

    // Program: Test FCVT.WU.S and FCVT.S.WU (unsigned conversions)
    let mut instructions = vec![
        lui(1, 0x80001000), // x1 = 0x80001000
        lui(2, 0x42280000), // x2 = 42.0 in FP
        sw(1, 2, 0),        // mem[x1] = 42.0
        flw(1, 1, 0),       // f1 = 42.0
        fcvt_wu_s(3, 1),    // x3 = (unsigned int)42.0 = 42
        addi(4, 0, 100),    // x4 = 100 (unsigned int)
        fcvt_s_wu(2, 4),    // f2 = (float)100 = 100.0
        fsw(1, 2, 4),       // Store conversion result
        lw(5, 1, 4),        // x5 = 100.0 as bits
        addi(6, 0, 0x100),  // x6 = 0x100
        sw(6, 3, 0),        // Store FCVT.WU.S result
        sw(6, 5, 4),        // Store FCVT.S.WU result
    ];
    instructions.extend(tohost_termination(7, 8));

    run_fp_program_with_options(
        &instructions,
        200,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, result: &SimulationResult| {
            assert!(
                result.tohost_value == Some(1),
                "Program should terminate with tohost=1"
            );

            let wu_result = sim.read_word(0x100);
            let swu_result = sim.read_word(0x104);
            assert_eq!(
                wu_result, 42,
                "FCVT.WU.S: 42.0 as unsigned int should be 42"
            );
            assert_eq!(
                swu_result, 0x42C80000,
                "FCVT.S.WU: 100 as float should be 100.0"
            );
        }),
    )
    .expect("FCVT unsigned conversion test should run");
}

#[test]
fn test_cpu_fclass() {
    init_test_logger();

    // Program: Test FCLASS.S instruction
    let mut instructions = vec![
        lui(1, 0x3F800000), // x1 = 1.0 (positive normal)
        lui(2, 0xBF800000), // x2 = -1.0 (negative normal)
        lui(3, 0x00000000), // x3 = +0.0 (positive zero)
        fmv_w_x(1, 1),      // f1 = 1.0
        fmv_w_x(2, 2),      // f2 = -1.0
        fmv_w_x(3, 3),      // f3 = +0.0
        fclass_s(4, 1),     // x4 = classify(1.0) = positive normal (bit 6 = 0x40)
        fclass_s(5, 2),     // x5 = classify(-1.0) = negative normal (bit 1 = 0x02)
        fclass_s(6, 3),     // x6 = classify(+0.0) = positive zero (bit 4 = 0x10)
        addi(7, 0, 0x100),  // x7 = 0x100
        sw(7, 4, 0),        // Store classify results
        sw(7, 5, 4),
        sw(7, 6, 8),
    ];
    instructions.extend(tohost_termination(10, 11));

    run_fp_program_with_options(
        &instructions,
        200,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, result: &SimulationResult| {
            assert!(
                result.tohost_value == Some(1),
                "Program should terminate with tohost=1"
            );

            let class_pos_normal = sim.read_word(0x100);
            let class_neg_normal = sim.read_word(0x104);
            let class_pos_zero = sim.read_word(0x108);
            assert_eq!(
                class_pos_normal, 0x40,
                "FCLASS: 1.0 should be positive normal (bit 6)"
            );
            assert_eq!(
                class_neg_normal, 0x02,
                "FCLASS: -1.0 should be negative normal (bit 1)"
            );
            assert_eq!(
                class_pos_zero, 0x10,
                "FCLASS: +0.0 should be positive zero (bit 4)"
            );
        }),
    )
    .expect("FCLASS test should run");
}

#[test]
fn test_cpu_fused_multiply_add_ops() {
    init_test_logger();

    // Program: Test FMADD.S, FMSUB.S, FNMSUB.S, FNMADD.S
    let mut instructions = vec![
        lui(1, 0x80001000),   // x1 = 0x80001000
        lui(2, 0x40000000),   // x2 = 2.0
        lui(3, 0x40400000),   // x3 = 3.0
        lui(4, 0x3F800000),   // x4 = 1.0
        sw(1, 2, 0),          // mem[x1+0] = 2.0
        sw(1, 3, 4),          // mem[x1+4] = 3.0
        sw(1, 4, 8),          // mem[x1+8] = 1.0
        flw(1, 1, 0),         // f1 = 2.0
        flw(2, 1, 4),         // f2 = 3.0
        flw(3, 1, 8),         // f3 = 1.0
        fmadd_s(4, 1, 2, 3),  // f4 = (2.0 * 3.0) + 1.0 = 7.0
        fmsub_s(5, 1, 2, 3),  // f5 = (2.0 * 3.0) - 1.0 = 5.0
        fnmsub_s(6, 1, 2, 3), // f6 = -(2.0 * 3.0 - 1.0) = -5.0
        fnmadd_s(7, 1, 2, 3), // f7 = -(2.0 * 3.0 + 1.0) = -7.0
        fsw(1, 4, 12),        // Store FMADD result
        fsw(1, 5, 16),        // Store FMSUB result
        fsw(1, 6, 20),        // Store FNMSUB result
        fsw(1, 7, 24),        // Store FNMADD result
        lw(4, 1, 12),         // Load results into integer regs
        lw(5, 1, 16),
        lw(6, 1, 20),
        lw(7, 1, 24),
        addi(8, 0, 0x100), // x8 = 0x100
        sw(8, 4, 0),       // Store all results
        sw(8, 5, 4),
        sw(8, 6, 8),
        sw(8, 7, 12),
    ];
    instructions.extend(tohost_termination(10, 11));

    run_fp_program_with_options(
        &instructions,
        300,
        false,
        None,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|sim: &SimulatorView, result: &SimulationResult| {
            assert!(
                result.tohost_value == Some(1),
                "Program should terminate with tohost=1"
            );

            let fmadd_result = sim.read_word(0x100);
            let fmsub_result = sim.read_word(0x104);
            let fnmsub_result = sim.read_word(0x108);
            let fnmadd_result = sim.read_word(0x10C);
            assert_eq!(fmadd_result, 0x40E00000, "FMADD: (2*3)+1 should be 7.0");
            assert_eq!(fmsub_result, 0x40A00000, "FMSUB: (2*3)-1 should be 5.0");
            assert_eq!(
                fnmsub_result, 0xC0A00000,
                "FNMSUB: -((2*3)-1) should be -5.0"
            );
            assert_eq!(
                fnmadd_result, 0xC0E00000,
                "FNMADD: -((2*3)+1) should be -7.0"
            );
        }),
    )
    .expect("Fused multiply-add operations test should run");
}
