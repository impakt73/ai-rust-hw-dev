//! RV32F Extension CPU Integration Tests
//!
//! Tests that verify the floating-point extension works correctly in the full CPU context,
//! including FP load/store, register interactions, FCSR management, and multi-cycle execution.

#[cfg(test)]
mod tests {
    use crate::*;
    use riscv_core::instruction::*;

    /// Helper function to initialize test logger (idempotent)
    fn init_test_logger() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    /// Generate tohost termination sequence
    fn tohost_termination(addr_reg: u32, value_reg: u32) -> Vec<u32> {
        vec![
            addi(addr_reg, 0, -16),     // Load -16 (0xFFFF_FFF0) into addr_reg
            addi(value_reg, 0, 1),      // Load success code (1)
            sw(addr_reg, value_reg, 0), // Store value to tohost address
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
        post_callback: F,
    ) -> Result<SimulationResult, String>
    where
        T: FnMut(&riscv_core::trace::InstructionTrace),
        F: for<'a> FnOnce(&mut Simulator<'a, fn(u32), T>, &SimulationResult),
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
            None::<fn(u32)>,
            trace_callback,
            vcd_path,
            0, // Zero latency for RTL verification tests
            |sim| {
                sim.write_memory_region(START_ADDR, &program_bytes, true);
                Ok(START_ADDR)
            },
            post_callback,
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
            lui(1, 0x80001), // x1 = 0x80001000 (data address)
            lui(2, 0x3F800), // x2 = 0x3F800000 (1.0 in FP)
            sw(1, 2, 0),     // Store integer representation to memory
            flw(1, 1, 0),    // f1 = load FP value from memory
            fsw(1, 1, 4),    // Store f1 to memory[0x80001004]
            lw(3, 1, 4),     // x3 = load from memory[0x80001004]
            addi(4, 0, 0x100), // x4 = 0x100 (result marker address)
            sw(4, 3, 0),     // Store result to 0x100
        ];
        instructions.extend(tohost_termination(7, 8));

        run_fp_program_with_options(
            &instructions,
            200,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |sim, result| {
                assert!(
                    result.tohost_value == Some(1),
                    "Program should terminate with tohost=1"
                );
                
                // Verify that the value round-tripped correctly
                // We stored 0x3F800000, loaded to f1, stored from f1, and loaded to x3
                let result_value = sim.bus.read_word(0x100);
                assert_eq!(
                    result_value, 0x3F800000,
                    "FLW/FSW round trip should preserve bit pattern"
                );
            },
        )
        .expect("FP load/store test should run");
    }

    #[test]
    fn test_cpu_flw_multiple_registers() {
        init_test_logger();

        // Program: Load different FP values into multiple FP registers
        // Verifies that FP register file has independent registers
        let mut instructions = vec![
            lui(1, 0x80001),  // x1 = 0x80001000 (base address)
            lui(2, 0x3F800),  // x2 = 1.0
            lui(3, 0x40000),  // x3 = 2.0
            lui(4, 0x40400),  // x4 = 3.0
            sw(1, 2, 0),      // mem[x1+0] = 1.0
            sw(1, 3, 4),      // mem[x1+4] = 2.0
            sw(1, 4, 8),      // mem[x1+8] = 3.0
            flw(1, 1, 0),     // f1 = 1.0
            flw(2, 1, 4),     // f2 = 2.0
            flw(3, 1, 8),     // f3 = 3.0
            fsw(1, 1, 12),    // mem[x1+12] = f1
            fsw(1, 2, 16),    // mem[x1+16] = f2
            fsw(1, 3, 20),    // mem[x1+20] = f3
            lw(5, 1, 12),     // x5 = f1 value
            lw(6, 1, 16),     // x6 = f2 value
            lw(7, 1, 20),     // x7 = f3 value
            addi(10, 0, 0x100), // x10 = 0x100
            sw(10, 5, 0),     // Store results to 0x100
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
            |sim, result| {
                assert!(
                    result.tohost_value == Some(1),
                    "Program should terminate with tohost=1"
                );
                
                let val1 = sim.bus.read_word(0x100);
                let val2 = sim.bus.read_word(0x104);
                let val3 = sim.bus.read_word(0x108);
                
                assert_eq!(val1, 0x3F800000, "f1 should be 1.0");
                assert_eq!(val2, 0x40000000, "f2 should be 2.0");
                assert_eq!(val3, 0x40400000, "f3 should be 3.0");
            },
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
            lui(1, 0x80001),     // x1 = 0x80001000 (base address)
            lui(2, 0x3F800),     // x2 = 1.0
            lui(3, 0x40000),     // x3 = 2.0
            sw(1, 2, 0),         // mem[x1+0] = 1.0
            sw(1, 3, 4),         // mem[x1+4] = 2.0
            flw(1, 1, 0),        // f1 = 1.0
            flw(2, 1, 4),        // f2 = 2.0
            fadd_s(3, 1, 2),     // f3 = f1 + f2 = 3.0
            fsw(1, 3, 8),        // mem[x1+8] = f3
            lw(4, 1, 8),         // x4 = result
            addi(5, 0, 0x100),   // x5 = 0x100
            sw(5, 4, 0),         // Store result to 0x100
        ];
        instructions.extend(tohost_termination(7, 8));

        run_fp_program_with_options(
            &instructions,
            200,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |sim, result| {
                assert!(
                    result.tohost_value == Some(1),
                    "Program should terminate with tohost=1"
                );
                
                let result_value = sim.bus.read_word(0x100);
                assert_eq!(result_value, 0x40400000, "1.0 + 2.0 should equal 3.0");
            },
        )
        .expect("FADD test should run");
    }

    #[test]
    fn test_cpu_fmul_basic() {
        init_test_logger();

        // Program: Test FMUL.S instruction
        let mut instructions = vec![
            lui(1, 0x80001),     // x1 = 0x80001000
            lui(2, 0x40000),     // x2 = 2.0
            lui(3, 0x40400),     // x3 = 3.0
            sw(1, 2, 0),         // mem[x1+0] = 2.0
            sw(1, 3, 4),         // mem[x1+4] = 3.0
            flw(1, 1, 0),        // f1 = 2.0
            flw(2, 1, 4),        // f2 = 3.0
            fmul_s(3, 1, 2),     // f3 = f1 * f2 = 6.0
            fsw(1, 3, 8),        // mem[x1+8] = f3
            lw(4, 1, 8),         // x4 = result
            addi(5, 0, 0x100),   // x5 = 0x100
            sw(5, 4, 0),         // Store result to 0x100
        ];
        instructions.extend(tohost_termination(7, 8));

        run_fp_program_with_options(
            &instructions,
            200,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |sim, result| {
                assert!(
                    result.tohost_value == Some(1),
                    "Program should terminate with tohost=1"
                );
                
                let result_value = sim.bus.read_word(0x100);
                assert_eq!(result_value, 0x40C00000, "2.0 * 3.0 should equal 6.0");
            },
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
            addi(1, 0, 42),      // x1 = 42 (integer)
            fcvt_s_w(1, 1),      // f1 = (float)42
            lui(2, 0x80001),     // x2 = 0x80001000
            fsw(2, 1, 0),        // mem[x2] = f1
            lw(3, 2, 0),         // x3 = result
            addi(4, 0, 0x100),   // x4 = 0x100
            sw(4, 3, 0),         // Store result to 0x100
        ];
        instructions.extend(tohost_termination(7, 8));

        run_fp_program_with_options(
            &instructions,
            200,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |sim, result| {
                assert!(
                    result.tohost_value == Some(1),
                    "Program should terminate with tohost=1"
                );
                
                let result_value = sim.bus.read_word(0x100);
                assert_eq!(result_value, 0x42280000, "42 as float should be 0x42280000");
            },
        )
        .expect("FCVT.S.W test should run");
    }

    #[test]
    fn test_cpu_fcvt_w_s() {
        init_test_logger();

        // Program: Test FCVT.W.S (FP to integer conversion)
        let mut instructions = vec![
            lui(1, 0x80001),     // x1 = 0x80001000
            lui(2, 0x42280),     // x2 = 42.0 in FP
            sw(1, 2, 0),         // mem[x1] = 42.0
            flw(1, 1, 0),        // f1 = 42.0
            fcvt_w_s(3, 1),      // x3 = (int)f1 = 42
            addi(4, 0, 0x100),   // x4 = 0x100
            sw(4, 3, 0),         // Store result to 0x100
        ];
        instructions.extend(tohost_termination(7, 8));

        run_fp_program_with_options(
            &instructions,
            200,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |sim, result| {
                assert!(
                    result.tohost_value == Some(1),
                    "Program should terminate with tohost=1"
                );
                
                let result_value = sim.bus.read_word(0x100);
                assert_eq!(result_value, 42, "42.0 as int should be 42");
            },
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
            lui(1, 0x80001),     // x1 = 0x80001000
            lui(2, 0x3F800),     // x2 = 1.0
            lui(3, 0x40000),     // x3 = 2.0
            sw(1, 2, 0),         // mem[x1+0] = 1.0
            sw(1, 3, 4),         // mem[x1+4] = 2.0
            flw(1, 1, 0),        // f1 = 1.0
            flw(2, 1, 4),        // f2 = 2.0
            feq_s(4, 1, 1),      // x4 = (f1 == f1) = 1
            feq_s(5, 1, 2),      // x5 = (f1 == f2) = 0
            flt_s(6, 1, 2),      // x6 = (f1 < f2) = 1
            flt_s(7, 2, 1),      // x7 = (f2 < f1) = 0
            addi(10, 0, 0x100),  // x10 = 0x100
            sw(10, 4, 0),        // Store results to 0x100-0x10C
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
            |sim, result| {
                assert!(
                    result.tohost_value == Some(1),
                    "Program should terminate with tohost=1"
                );
                
                let eq_same = sim.bus.read_word(0x100);
                let eq_diff = sim.bus.read_word(0x104);
                let lt_true = sim.bus.read_word(0x108);
                let lt_false = sim.bus.read_word(0x10C);
                
                assert_eq!(eq_same, 1, "1.0 == 1.0 should be true");
                assert_eq!(eq_diff, 0, "1.0 == 2.0 should be false");
                assert_eq!(lt_true, 1, "1.0 < 2.0 should be true");
                assert_eq!(lt_false, 0, "2.0 < 1.0 should be false");
            },
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
            lui(1, 0x3F800),     // x1 = 0x3F800000 (1.0 in FP)
            fmv_w_x(1, 1),       // f1 = x1 (bitwise move)
            fmv_x_w(2, 1),       // x2 = f1 (bitwise move back)
            addi(3, 0, 0x100),   // x3 = 0x100
            sw(3, 2, 0),         // Store result to 0x100
        ];
        instructions.extend(tohost_termination(7, 8));

        run_fp_program_with_options(
            &instructions,
            200,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |sim, result| {
                assert!(
                    result.tohost_value == Some(1),
                    "Program should terminate with tohost=1"
                );
                
                let result_value = sim.bus.read_word(0x100);
                assert_eq!(result_value, 0x3F800000, "FMV round trip should preserve bits");
            },
        )
        .expect("FMV test should run");
    }
}
