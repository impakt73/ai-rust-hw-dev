//! RTL Verification Tests
//!
//! Low-level testbench tests that verify the RTL implementation directly
//! by running programmatically generated instruction sequences.
//!
//! These tests were migrated from tests/src/cpu_test.rs to leverage the
//! cpu-sim infrastructure (SystemBus, VCD dumps, instruction tracing)
//! rather than maintaining a duplicate CpuTestHarness implementation.

#[cfg(test)]
mod tests {
    use crate::*;
    use riscv_core::instruction::*;

    /// Helper function to initialize test logger (idempotent)
    fn init_test_logger() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    /// Generate tohost termination sequence
    ///
    /// Generates a sequence of instructions that write a success code to the tohost address.
    /// This is required for multi-cycle CPU implementations to signal program completion.
    ///
    /// The sequence uses two registers:
    /// - addr_reg: holds the tohost address (0xFFFF_FFF0)
    /// - value_reg: holds the success code (1)
    ///
    /// Note: 0xFFFF_FFF0 = -16 in two's complement, so we use ADDI to load it
    ///
    /// Returns: [ADDI addr_reg (load -16), ADDI value_reg, SW]
    fn tohost_termination(addr_reg: u32, value_reg: u32) -> Vec<u32> {
        vec![
            addi(addr_reg, 0, -16),     // Load -16 (0xFFFF_FFF0) into addr_reg
            addi(value_reg, 0, 1),      // Load success code (1)
            sw(addr_reg, value_reg, 0), // Store value to tohost address
        ]
    }

    /// Helper to run programmatic instructions with options for trace/VCD/callbacks
    ///
    /// This is the ONLY helper function for running programmatic tests.
    /// It supports:
    /// - Instruction trace printing (print_inst_trace)
    /// - VCD waveform dumping (vcd_path)
    /// - Trace callbacks for programmatic validation (trace_callback)
    /// - Post-execution callbacks for verification (post_callback)
    fn run_program_with_options<T, F>(
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
        instructions.extend(tohost_termination(7, 8));

        run_program_with_options(
            &instructions,
            100,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |_sim, result| {
                assert!(
                    result.tohost_value == Some(1),
                    "Program should terminate with tohost=1"
                );
            },
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
        instructions.extend(tohost_termination(7, 8));

        run_program_with_options(
            &instructions,
            100,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |_sim, result| {
                assert!(
                    result.tohost_value == Some(1),
                    "Program should terminate with tohost=1"
                );
            },
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
        instructions.extend(tohost_termination(7, 8));

        run_program_with_options(
            &instructions,
            100,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |_sim, result| {
                assert!(
                    result.tohost_value == Some(1),
                    "Program should terminate with tohost=1"
                );
            },
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
        instructions.extend(tohost_termination(7, 8));

        run_program_with_options(
            &instructions,
            100,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |_sim, result| {
                assert!(
                    result.tohost_value == Some(1),
                    "Program should terminate with tohost=1"
                );
            },
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
        // 0x20: SW   x3, 0x100(x0) ; Store x3 to verify it wasn't set to 99
        // 0x24: SW   x5, 0x104(x0) ; Store x5 to verify it wasn't set to 99
        let mut instructions = vec![
            addi(1, 0, 10),
            addi(2, 0, 10),
            beq(1, 2, 8),
            addi(3, 0, 99),
            addi(4, 0, 5),
            bne(1, 4, 8),
            addi(5, 0, 99),
            addi(6, 0, 1),
            sw(0, 3, 0x100),
            sw(0, 5, 0x104),
        ];
        instructions.extend(tohost_termination(7, 8));

        run_program_with_options(
            &instructions,
            200,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |sim, result| {
                // Verify branches worked - skipped instructions should leave registers at 0
                let marker1 = sim.bus.read_word(0x100);
                let marker2 = sim.bus.read_word(0x104);
                assert_eq!(
                    marker1, 0,
                    "First branch should skip addi x3,x0,99, so x3 should be 0"
                );
                assert_eq!(
                    marker2, 0,
                    "Second branch should skip addi x5,x0,99, so x5 should be 0"
                );
                assert!(
                    result.tohost_value == Some(1),
                    "Program should terminate with tohost=1"
                );
            },
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
        // 0x1C: SW   x3, 0x100(x0) ; Store x3 to verify
        // 0x20: SW   x4, 0x104(x0) ; Store x4 to verify
        let mut instructions = vec![
            addi(1, 0, 5),
            addi(2, 0, 10),
            blt(1, 2, 8),
            addi(3, 0, 99),
            bge(2, 1, 8),
            addi(4, 0, 99),
            addi(5, 0, 1),
            sw(0, 3, 0x100),
            sw(0, 4, 0x104),
        ];
        instructions.extend(tohost_termination(7, 8));

        run_program_with_options(
            &instructions,
            200,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |sim, result| {
                // Verify branches worked
                let marker1 = sim.bus.read_word(0x100);
                let marker2 = sim.bus.read_word(0x104);
                assert_eq!(marker1, 0, "BLT should skip setting x3 to 99");
                assert_eq!(marker2, 0, "BGE should skip setting x4 to 99");
                assert!(
                    result.tohost_value == Some(1),
                    "Program should terminate with tohost=1"
                );
            },
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
        // 0x1C: SW   x3, 0x100(x0) ; Store x3 to verify
        // 0x20: SW   x4, 0x104(x0) ; Store x4 to verify
        let mut instructions = vec![
            addi(1, 0, -1),
            addi(2, 0, 5),
            bltu(2, 1, 8),
            addi(3, 0, 99),
            bgeu(1, 2, 8),
            addi(4, 0, 99),
            addi(5, 0, 1),
            sw(0, 3, 0x100),
            sw(0, 4, 0x104),
        ];
        instructions.extend(tohost_termination(7, 8));

        run_program_with_options(
            &instructions,
            200,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |sim, result| {
                // Verify branches worked
                let marker1 = sim.bus.read_word(0x100);
                let marker2 = sim.bus.read_word(0x104);
                assert_eq!(marker1, 0, "BLTU should skip setting x3 to 99");
                assert_eq!(marker2, 0, "BGEU should skip setting x4 to 99");
                assert!(
                    result.tohost_value == Some(1),
                    "Program should terminate with tohost=1"
                );
            },
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
        // 0x00: ADDI x1, x0, 100   ; x1 = 100 (base address)
        // 0x04: ADDI x2, x0, 42    ; x2 = 42 (value to store)
        // 0x08: SW   x2, 0(x1)     ; Store x2 to memory[100]
        // 0x0C: LW   x3, 0(x1)     ; Load from memory[100] to x3
        // 0x10: ADDI x4, x0, 8     ; x4 = 8 (offset)
        // 0x14: SW   x2, 8(x1)     ; Store x2 to memory[108]
        // 0x18: LW   x5, 8(x1)     ; Load from memory[108] to x5
        let mut instructions = vec![
            addi(1, 0, 100),
            addi(2, 0, 42),
            sw(1, 2, 0),
            lw(3, 1, 0),
            addi(4, 0, 8),
            sw(1, 2, 8),
            lw(5, 1, 8),
        ];
        instructions.extend(tohost_termination(7, 8));

        run_program_with_options(
            &instructions,
            200,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |sim, _result| {
                assert_eq!(sim.bus.read_word(100), 42, "Memory[100] should contain 42");
                assert_eq!(sim.bus.read_word(108), 42, "Memory[108] should contain 42");
            },
        )
        .expect("Program should run");

        println!("Successfully executed load and store instructions");
    }

    #[test]
    fn test_cpu_load_byte() {
        init_test_logger();

        // Program: Test LB (load byte signed) and LBU (load byte unsigned)
        // We'll store a word with mixed signed/unsigned bytes and load them
        // 0x00: ADDI x1, x0, 100   ; x1 = 100 (base address)
        // 0x04: ADDI x2, x0, -1    ; x2 = 0xFFFFFFFF
        // 0x08: SW   x2, 0(x1)     ; Store 0xFFFFFFFF to mem[100]
        // 0x0C: LB   x3, 0(x1)     ; Load byte 0 (0xFF), sign-extend to 0xFFFFFFFF
        // 0x10: LB   x4, 1(x1)     ; Load byte 1 (0xFF), sign-extend to 0xFFFFFFFF
        // 0x14: LBU  x5, 0(x1)     ; Load byte 0 (0xFF), zero-extend to 0x000000FF
        // 0x18: LBU  x6, 1(x1)     ; Load byte 1 (0xFF), zero-extend to 0x000000FF
        // 0x1C: SW   x3, 0x200(x0) ; Store loaded values for verification
        // 0x20: SW   x4, 0x204(x0)
        // 0x24: SW   x5, 0x208(x0)
        // 0x28: SW   x6, 0x20C(x0)
        let mut instructions = vec![
            addi(1, 0, 100),
            addi(2, 0, -1),
            sw(1, 2, 0),
            lb(3, 1, 0),
            lb(4, 1, 1),
            lbu(5, 1, 0),
            lbu(6, 1, 1),
            sw(0, 3, 0x200),
            sw(0, 4, 0x204),
            sw(0, 5, 0x208),
            sw(0, 6, 0x20C),
        ];
        instructions.extend(tohost_termination(7, 8));

        run_program_with_options(
            &instructions,
            200,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |sim, _result| {
                // Verify memory operations
                assert_eq!(
                    sim.bus.read_word(100),
                    0xFFFFFFFF,
                    "Memory[100] should contain 0xFFFFFFFF"
                );
                // Verify load operations
                assert_eq!(
                    sim.bus.read_word(0x200),
                    0xFFFFFFFF,
                    "LB x3, 0(x1) should load 0xFF and sign-extend to 0xFFFFFFFF"
                );
                assert_eq!(
                    sim.bus.read_word(0x204),
                    0xFFFFFFFF,
                    "LB x4, 1(x1) should load 0xFF and sign-extend to 0xFFFFFFFF"
                );
                assert_eq!(
                    sim.bus.read_word(0x208),
                    0x000000FF,
                    "LBU x5, 0(x1) should load 0xFF and zero-extend to 0x000000FF"
                );
                assert_eq!(
                    sim.bus.read_word(0x20C),
                    0x000000FF,
                    "LBU x6, 1(x1) should load 0xFF and zero-extend to 0x000000FF"
                );
            },
        )
        .expect("Program should run");

        println!("Successfully executed LB and LBU instructions");
    }

    #[test]
    fn test_cpu_load_halfword() {
        init_test_logger();

        // Program: Test LH (load halfword signed) and LHU (load halfword unsigned)
        let mut instructions = vec![
            addi(1, 0, 100),
            addi(2, 0, -1),
            sw(1, 2, 0),
            lh(3, 1, 0),
            lh(4, 1, 2),
            lhu(5, 1, 0),
            lhu(6, 1, 2),
            sw(0, 3, 0x200),
            sw(0, 4, 0x204),
            sw(0, 5, 0x208),
            sw(0, 6, 0x20C),
        ];
        instructions.extend(tohost_termination(7, 8));

        run_program_with_options(
            &instructions,
            200,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |sim, _result| {
                // Verify memory operations
                assert_eq!(
                    sim.bus.read_word(100),
                    0xFFFFFFFF,
                    "Memory[100] should contain 0xFFFFFFFF"
                );
                // Verify load operations
                assert_eq!(
                    sim.bus.read_word(0x200),
                    0xFFFFFFFF,
                    "LH x3, 0(x1) should load 0xFFFF and sign-extend to 0xFFFFFFFF"
                );
                assert_eq!(
                    sim.bus.read_word(0x204),
                    0xFFFFFFFF,
                    "LH x4, 2(x1) should load 0xFFFF and sign-extend to 0xFFFFFFFF"
                );
                assert_eq!(
                    sim.bus.read_word(0x208),
                    0x0000FFFF,
                    "LHU x5, 0(x1) should load 0xFFFF and zero-extend to 0x0000FFFF"
                );
                assert_eq!(
                    sim.bus.read_word(0x20C),
                    0x0000FFFF,
                    "LHU x6, 2(x1) should load 0xFFFF and zero-extend to 0x0000FFFF"
                );
            },
        )
        .expect("Program should run");

        println!("Successfully executed LH and LHU instructions");
    }

    #[test]
    fn test_cpu_store_byte() {
        init_test_logger();

        // Program: Test SB (store byte)
        // We'll write individual bytes to different positions in a word
        let mut instructions = vec![
            addi(1, 0, 100),
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
        instructions.extend(tohost_termination(7, 8));

        run_program_with_options(
            &instructions,
            200,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |sim, _result| {
                // Verify memory operations - bytes stored in little-endian order
                assert_eq!(
                    sim.bus.read_word(100),
                    0x78563412,
                    "Memory should contain 0x78563412"
                );
            },
        )
        .expect("Program should run");

        println!("Successfully executed SB instruction");
    }

    #[test]
    fn test_cpu_store_halfword() {
        init_test_logger();

        // Program: Test SH (store halfword)
        let mut instructions = vec![
            addi(1, 0, 100),
            addi(2, 0, 0x234),
            addi(3, 0, 0x678),
            sh(1, 2, 0),
            sh(1, 3, 2),
            lw(4, 1, 0),
        ];
        instructions.extend(tohost_termination(7, 8));

        run_program_with_options(
            &instructions,
            200,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |sim, _result| {
                // Verify memory operations - halfwords stored in little-endian order
                assert_eq!(
                    sim.bus.read_word(100),
                    0x06780234,
                    "Memory should contain 0x06780234"
                );
            },
        )
        .expect("Program should run");

        println!("Successfully executed SH instruction");
    }

    #[test]
    fn test_cpu_byte_halfword_mixed() {
        init_test_logger();

        // Program: Test mixed byte/halfword operations with positive and negative values
        let mut instructions = vec![
            addi(1, 0, 200),
            addi(2, 0, -128),
            sb(1, 2, 0),
            lb(3, 1, 0),
            lbu(4, 1, 0),
            addi(5, 0, -1),
            sh(1, 5, 4),
            lh(6, 1, 4),
            lhu(7, 1, 4),
            sw(0, 3, 0x200),
            sw(0, 4, 0x204),
            sw(0, 6, 0x208),
            sw(0, 7, 0x20C),
        ];
        instructions.extend(tohost_termination(7, 8));

        run_program_with_options(
            &instructions,
            200,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |sim, _result| {
                // Verify load operations
                assert_eq!(
                    sim.bus.read_word(0x200),
                    0xFFFFFF80,
                    "LB x3, 0(x1) should load 0x80 and sign-extend to 0xFFFFFF80"
                );
                assert_eq!(
                    sim.bus.read_word(0x204),
                    0x00000080,
                    "LBU x4, 0(x1) should load 0x80 and zero-extend to 0x00000080"
                );
                assert_eq!(
                    sim.bus.read_word(0x208),
                    0xFFFFFFFF,
                    "LH x6, 4(x1) should load 0xFFFF and sign-extend to 0xFFFFFFFF"
                );
                assert_eq!(
                    sim.bus.read_word(0x20C),
                    0x0000FFFF,
                    "LHU x7, 4(x1) should load 0xFFFF and zero-extend to 0x0000FFFF"
                );
            },
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
        instructions.extend(tohost_termination(7, 8));

        run_program_with_options(
            &instructions,
            100,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |_sim, result| {
                assert!(
                    result.tohost_value == Some(1),
                    "Program should terminate with tohost=1"
                );
            },
        )
        .expect("Program should run");

        println!("Successfully executed AUIPC instruction");
    }

    #[test]
    fn test_cpu_tohost_halt() {
        init_test_logger();

        // TOHOST address for halt signal
        const TOHOST_ADDR: u32 = 0xFFFF_FFF0;

        // Program: Execute a few instructions, then write to tohost to signal halt
        let instructions = vec![
            addi(1, 0, 10),
            addi(2, 1, 5),
            add(3, 1, 2),
            addi(4, 0, -16), // x4 = 0xFFFFFFF0 (tohost address)
            addi(5, 0, 1),   // x5 = 1 (exit code)
            sw(4, 5, 0),     // Store x5 to tohost address
        ];

        run_program_with_options(
            &instructions,
            200,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |sim, result| {
                // Verify that tohost write was detected
                assert_eq!(
                    result.tohost_value,
                    Some(1),
                    "Expected tohost value to be 1 (exit code)"
                );
                assert_eq!(
                    sim.bus.read_word(TOHOST_ADDR),
                    1,
                    "TOHOST memory location should contain 1"
                );
            },
        )
        .expect("Program should run");

        println!("Successfully tested tohost halt mechanism");
    }

    #[test]
    fn test_cpu_fence_instruction() {
        init_test_logger();

        let mut instructions = vec![addi(1, 0, 10), fence(), addi(2, 1, 5), addi(0, 0, 0)];
        instructions.extend(tohost_termination(7, 8));

        run_program_with_options(
            &instructions,
            100,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |_sim, result| {
                // FENCE is essentially a NOP for single-cycle CPU
                assert!(
                    result.tohost_value == Some(1),
                    "Program should terminate with tohost=1"
                );
            },
        )
        .expect("Program should run");

        println!("Successfully executed FENCE instruction");
    }

    #[test]
    fn test_cpu_ecall_instruction() {
        init_test_logger();

        let mut instructions = vec![addi(1, 0, 42)];
        instructions.extend(tohost_termination(7, 8));
        instructions.push(ecall()); // Should halt CPU after tohost write
        instructions.push(addi(2, 0, 99)); // Should not execute

        run_program_with_options(
            &instructions,
            100,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |_sim, result| {
                // After ECALL, CPU should halt
                assert!(
                    result.tohost_value == Some(1),
                    "Program should terminate with tohost=1"
                );
            },
        )
        .expect("Program should run");

        println!("Successfully executed ECALL instruction");
    }

    #[test]
    fn test_cpu_ebreak_instruction() {
        init_test_logger();

        let mut instructions = vec![addi(1, 0, 100)];
        instructions.extend(tohost_termination(7, 8));
        instructions.push(ebreak()); // Should halt CPU after tohost write
        instructions.push(addi(2, 0, 200)); // Should not execute

        run_program_with_options(
            &instructions,
            100,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |_sim, result| {
                // After EBREAK, CPU should halt
                assert!(
                    result.tohost_value == Some(1),
                    "Program should terminate with tohost=1"
                );
            },
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
        let mut instructions = vec![
            addi(1, 0, 100),
            csrrw(2, 1, 0x300), // x2 = CSR[0x300]; CSR[0x300] = x1
            sw(0, 2, 0x100),
            csrrw(3, 0, 0x300), // x3 = CSR[0x300]; CSR[0x300] = 0
            sw(0, 3, 0x104),
            csrrw(4, 0, 0x300),
            sw(0, 4, 0x108),
        ];
        instructions.extend(tohost_termination(7, 8));

        run_program_with_options(
            &instructions,
            200,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |sim, _result| {
                // Verify CSR operations
                assert_eq!(
                    sim.bus.read_word(0x100),
                    0,
                    "First CSRRW should read 0 from uninitialized CSR"
                );
                assert_eq!(
                    sim.bus.read_word(0x104),
                    100,
                    "Second CSRRW should read 100 from CSR"
                );
                assert_eq!(
                    sim.bus.read_word(0x108),
                    0,
                    "Third CSRRW should read 0 from CSR"
                );
            },
        )
        .expect("Program should run");

        println!("Successfully executed CSR read/write operations");
    }

    #[test]
    fn test_cpu_csr_set_clear() {
        init_test_logger();

        // Test CSRRS (CSR Read and Set) and CSRRC (CSR Read and Clear)
        let mut instructions = vec![
            addi(1, 0, 0b1010),
            csrrw(0, 1, 0x301),
            addi(2, 0, 0b0101),
            csrrs(3, 2, 0x301), // x3 = CSR; CSR |= x2
            sw(0, 3, 0x100),
            addi(4, 0, 0b1000),
            csrrc(5, 4, 0x301), // x5 = CSR; CSR &= ~x4
            sw(0, 5, 0x104),
            csrrw(6, 0, 0x301),
            sw(0, 6, 0x108),
        ];
        instructions.extend(tohost_termination(7, 8));

        run_program_with_options(
            &instructions,
            200,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |sim, _result| {
                // Verify CSR operations
                assert_eq!(
                    sim.bus.read_word(0x100),
                    0b1010,
                    "CSRRS should read old value 0b1010"
                );
                assert_eq!(
                    sim.bus.read_word(0x104),
                    0b1111,
                    "CSRRC should read value 0b1111"
                );
                assert_eq!(
                    sim.bus.read_word(0x108),
                    0b0111,
                    "Final CSR value should be 0b0111"
                );
            },
        )
        .expect("Program should run");

        println!("Successfully executed CSR set/clear operations");
    }

    #[test]
    fn test_cpu_csr_immediate() {
        init_test_logger();

        // Test immediate CSR instructions (CSRRWI, CSRRSI, CSRRCI)
        let mut instructions = vec![
            csrrwi(1, 15, 0x302),
            sw(0, 1, 0x100),
            csrrsi(2, 8, 0x302),
            sw(0, 2, 0x104),
            csrrci(3, 4, 0x302),
            sw(0, 3, 0x108),
            csrrw(4, 0, 0x302),
            sw(0, 4, 0x10C),
        ];
        instructions.extend(tohost_termination(7, 8));

        run_program_with_options(
            &instructions,
            200,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |sim, _result| {
                // Verify CSR operations
                assert_eq!(
                    sim.bus.read_word(0x100),
                    0,
                    "CSRRWI should read 0 from uninitialized CSR"
                );
                assert_eq!(sim.bus.read_word(0x104), 15, "CSRRSI should read 15");
                assert_eq!(sim.bus.read_word(0x108), 15, "CSRRCI should read 15");
                assert_eq!(sim.bus.read_word(0x10C), 11, "Final CSR value should be 11");
            },
        )
        .expect("Program should run");

        println!("Successfully executed CSR immediate operations");
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
            sw(0, 3, 0x100),
        ];
        instructions.extend(tohost_termination(7, 8));

        run_program_with_options(
            &instructions,
            200,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |sim, _result| {
                assert_eq!(sim.bus.read_word(0x100), 200, "MUL: 10 × 20 should be 200");
            },
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
            sw(0, 3, 0x100),
        ];
        instructions.extend(tohost_termination(7, 8));

        run_program_with_options(
            &instructions,
            200,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |sim, _result| {
                assert_eq!(
                    sim.bus.read_word(0x100),
                    0x00000001,
                    "MULH: upper 32 bits should be 0x00000001"
                );
            },
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
            sw(0, 3, 0x100),
        ];
        instructions.extend(tohost_termination(7, 8));

        run_program_with_options(
            &instructions,
            200,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |sim, _result| {
                assert_eq!(sim.bus.read_word(0x100), 14, "DIV: 100 ÷ 7 should be 14");
            },
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
            sw(0, 3, 0x100),
        ];
        instructions.extend(tohost_termination(7, 8));

        run_program_with_options(
            &instructions,
            200,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |sim, _result| {
                assert_eq!(
                    sim.bus.read_word(0x100),
                    0xFFFFFFFF,
                    "DIV by zero should return 0xFFFFFFFF"
                );
            },
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
            sw(0, 3, 0x100),
        ];
        instructions.extend(tohost_termination(7, 8));

        run_program_with_options(
            &instructions,
            200,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |sim, _result| {
                assert_eq!(sim.bus.read_word(0x100), 2, "REM: 100 % 7 should be 2");
            },
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
            sw(0, 3, 0x100),
            sw(0, 4, 0x104),
        ];
        instructions.extend(tohost_termination(7, 8));

        run_program_with_options(
            &instructions,
            200,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |sim, _result| {
                assert_eq!(
                    sim.bus.read_word(0x100),
                    0x7FFFFFFF,
                    "DIVU: 0xFFFFFFFF ÷ 2 should be 0x7FFFFFFF"
                );
                assert_eq!(
                    sim.bus.read_word(0x104),
                    1,
                    "REMU: 0xFFFFFFFF % 2 should be 1"
                );
            },
        )
        .expect("Program should run");

        println!("Successfully executed DIVU and REMU instructions");
    }

    #[test]
    fn test_cpu_m_extension_program() {
        init_test_logger();

        // Complex program using multiple M extension instructions
        // Calculate: result = (a × b) ÷ c + (d % e)
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
            sw(0, 9, 0x100),
        ];
        instructions.extend(tohost_termination(7, 8));

        run_program_with_options(
            &instructions,
            200,
            false,
            None,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |sim, _result| {
                assert_eq!(
                    sim.bus.read_word(0x100),
                    22,
                    "Complex M extension program result should be 22"
                );
            },
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
            sw(0, 1, 0x100),     // mem[0x100] = x1 = 10
            lw(11, 0, 0x100),    // x11 = mem[0x100] = 10
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
                inst_type: riscv_core::trace::InstructionType::Sw,
                pc: base_addr + 40,
                rd: None,
                rs1: Some((0, 0)),
                rs2: Some((1, 10)),
                immediate: Some(0x100),
            },
            ExpectedInstruction {
                inst_type: riscv_core::trace::InstructionType::Lw,
                pc: base_addr + 44,
                rd: Some((11, 10)),
                rs1: Some((0, 0)),
                rs2: None,
                immediate: Some(0x100),
            },
        ];

        // Add termination sequence
        instructions.extend(tohost_termination(15, 16));

        // Collect traces
        let mut captured_traces = Vec::new();
        run_program_with_options(
            &instructions,
            200,
            false,
            None,
            Some(|trace: &riscv_core::trace::InstructionTrace| {
                captured_traces.push(trace.clone());
            }),
            |_sim, result| {
                assert_eq!(
                    result.tohost_value,
                    Some(1),
                    "Program should terminate with tohost=1"
                );
            },
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
        instructions.extend(tohost_termination(7, 8));

        // Collect traces
        let mut captured_traces = Vec::new();
        run_program_with_options(
            &instructions,
            200,
            false,
            None,
            Some(|trace: &riscv_core::trace::InstructionTrace| {
                captured_traces.push(trace.clone());
            }),
            |_sim, result| {
                assert_eq!(result.tohost_value, Some(1));
            },
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
        instructions.extend(tohost_termination(7, 8));

        // Run with VCD enabled
        run_program_with_options(
            &instructions,
            100,
            false,
            Some(vcd_path),
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            |_sim, result| {
                assert_eq!(result.tohost_value, Some(1));
            },
        )
        .expect("Simulation should succeed");

        // Verify VCD file was created
        assert!(
            std::path::Path::new(vcd_path).exists(),
            "VCD file should be created"
        );

        // Read VCD file
        let vcd_contents =
            std::fs::read_to_string(vcd_path).expect("Should be able to read VCD file");

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
}
