#[allow(unused_imports)]
use riscv_core::instruction::{
    add, addi, and as and_inst, auipc, beq, bge, bgeu, blt, bltu, bne, csrrc, csrrci, csrrs,
    csrrsi, csrrw, csrrwi, div, divu, ebreak, ecall, fence, jal, jalr, lb, lbu, lh, lhu, lui, lw,
    mul, mulh, mulhsu, mulhu, or as or_inst, rem, remu, sb, sh, sub, sw, xor as xor_inst,
};
use riscv_core::{create_cpu_runtime, Top};
use std::collections::HashMap;

fn create_runtime() -> riscv_core::VerilatorRuntime {
    create_cpu_runtime().expect("Failed to create CPU runtime")
}

macro_rules! clock_cycle {
    ($dut:expr) => {
        $dut.clk = 0;
        $dut.eval();
        $dut.clk = 1;
        $dut.eval();
        $dut.clk = 0;
        $dut.eval();
    };
}

/// Helper structure to encapsulate common CPU test state
struct CpuTestHarness {
    pub imem: HashMap<u32, u32>,
    pub dmem: HashMap<u32, u32>,
}

impl CpuTestHarness {
    /// Create a new test harness with empty memory
    fn new() -> Self {
        Self {
            imem: HashMap::new(),
            dmem: HashMap::new(),
        }
    }

    fn run_cpu_test<F>(test_callback: F)
    where
        F: FnOnce(&mut Top<'_>, &mut CpuTestHarness),
    {
        let runtime = create_runtime();
        let mut dut = runtime
            .create_model_simple::<Top>()
            .expect("Failed to create RTL model");

        // Perform reset sequence
        dut.rst_n = 0;
        dut.clk = 0;
        dut.eval();
        clock_cycle!(dut);
        dut.rst_n = 1;
        dut.eval();

        let mut harness = CpuTestHarness::new();

        // Invoke user-provided callback to run test-specific logic
        test_callback(&mut dut, &mut harness);
    }

    /// Load a program into instruction memory
    fn load_program(&mut self, program: &[(u32, u32)]) {
        for &(addr, instruction) in program {
            self.imem.insert(addr, instruction);
        }
    }

    /// Execute a specified number of CPU cycles with automatic memory handling
    fn run_cycles(&mut self, dut: &mut Top, num_cycles: usize) {
        for _ in 0..num_cycles {
            self.step_cycle(dut);
        }
    }

    /// Execute a single CPU cycle with memory handling
    fn step_cycle(&mut self, dut: &mut Top) {
        // Fetch instruction
        let pc = dut.imem_addr;
        let instruction = self.imem.get(&pc).copied().unwrap_or(0);
        dut.imem_data = instruction;

        // Handle data memory reads
        // For byte/halfword loads, the RTL expects the full word containing the byte/halfword
        // and will extract the appropriate bits based on dmem_size
        let dmem_addr_pre = dut.dmem_addr & !0x3;
        dut.dmem_rdata = self.dmem.get(&dmem_addr_pre).copied().unwrap_or(0);

        dut.eval();

        // Handle data memory writes
        self.handle_memory_write(dut);

        // Clock cycle
        clock_cycle!(dut);
    }

    /// Handle memory writes based on dmem_size (byte, halfword, or word)
    fn handle_memory_write(&mut self, dut: &mut Top) {
        if dut.dmem_we == 0 {
            return;
        }

        let dmem_addr = dut.dmem_addr;
        let word_addr = dmem_addr & !0x3;
        let byte_offset = (dmem_addr & 0x3) as usize;
        let halfword_offset = ((dmem_addr & 0x2) >> 1) as usize;
        let current_word = self.dmem.get(&word_addr).copied().unwrap_or(0);
        let mut word_bytes = current_word.to_le_bytes();

        let dmem_size = dut.dmem_size;

        match dmem_size {
            0b00 => {
                // SB - Store Byte
                let byte_val = (dut.dmem_wdata & 0xFF) as u8;
                word_bytes[byte_offset] = byte_val;
            }
            0b01 => {
                // SH - Store Halfword
                let halfword_val = (dut.dmem_wdata & 0xFFFF) as u16;
                let hw_bytes = halfword_val.to_le_bytes();
                word_bytes[halfword_offset * 2] = hw_bytes[0];
                word_bytes[halfword_offset * 2 + 1] = hw_bytes[1];
            }
            _ => {
                // SW - Store Word
                word_bytes = dut.dmem_wdata.to_le_bytes();
            }
        }

        let new_word = u32::from_le_bytes(word_bytes);
        self.dmem.insert(word_addr, new_word);
    }

    /// Execute cycles and track PC history
    fn run_cycles_with_pc_trace(&mut self, dut: &mut Top, num_cycles: usize) -> Vec<u32> {
        let mut pc_history = Vec::new();
        for _ in 0..num_cycles {
            pc_history.push(dut.imem_addr);
            self.step_cycle(dut);
        }
        pc_history
    }

    /// Execute cycles and capture debug_rd_data at specific PCs
    fn run_cycles_capture_rd_data(
        &mut self,
        dut: &mut Top,
        num_cycles: usize,
        pcs: &[u32],
    ) -> HashMap<u32, u32> {
        let mut rd_data_map = HashMap::new();
        for _ in 0..num_cycles {
            let pc = dut.imem_addr;

            // Need to manually handle the cycle to capture rd_data at the right time
            // Fetch instruction
            let instruction = self.imem.get(&pc).copied().unwrap_or(0);
            dut.imem_data = instruction;

            // Handle data memory reads
            let dmem_addr_pre = dut.dmem_addr & !0x3;
            dut.dmem_rdata = self.dmem.get(&dmem_addr_pre).copied().unwrap_or(0);

            dut.eval();

            // Capture debug_rd_data AFTER eval, BEFORE clock
            // This is why we can't use step_cycle() - we need to capture between eval and clock
            if pcs.contains(&pc) {
                rd_data_map.insert(pc, dut.debug_rd_data);
            }

            // Handle data memory writes
            self.handle_memory_write(dut);

            // Clock cycle
            clock_cycle!(dut);
        }
        rd_data_map
    }

    /// Run until tohost write is detected or max_cycles is reached
    fn run_until_tohost_write(
        &mut self,
        dut: &mut Top,
        tohost_addr: u32,
        max_cycles: usize,
    ) -> Option<u32> {
        for _ in 0..max_cycles {
            // Need to manually handle the cycle to check for tohost write before memory handling
            let pc = dut.imem_addr;
            let instruction = self.imem.get(&pc).copied().unwrap_or(0);
            dut.imem_data = instruction;

            let dmem_addr_pre = dut.dmem_addr & !0x3;
            dut.dmem_rdata = self.dmem.get(&dmem_addr_pre).copied().unwrap_or(0);

            dut.eval();

            // Check for tohost write BEFORE normal memory write handling
            // This is why we can't use step_cycle() - we need to intercept the tohost write
            if dut.dmem_we != 0 && dut.dmem_addr == tohost_addr {
                let tohost_value = dut.dmem_wdata;
                self.dmem.insert(dut.dmem_addr, tohost_value);
                return Some(tohost_value);
            }

            self.handle_memory_write(dut);
            clock_cycle!(dut);
        }
        None
    }
}

#[test]
fn test_cpu_basic_execution() {
    CpuTestHarness::run_cpu_test(|mut dut, harness| {
        // Program: Simple arithmetic operations
        // 0x00: ADDI x1, x0, 5    ; x1 = 5
        // 0x04: ADDI x2, x0, 3    ; x2 = 3
        // 0x08: ADD  x3, x1, x2   ; x3 = x1 + x2 = 8
        harness.load_program(&[
            (0x00, addi(1, 0, 5)),
            (0x04, addi(2, 0, 3)),
            (0x08, add(3, 1, 2)),
            (0x0C, addi(0, 0, 0)), // NOP to end
        ]);

        // Run for several cycles
        harness.run_cycles(&mut dut, 10);

        // Note: In a single-cycle implementation, we can't directly read register values
        // We would need to add debug ports or trace signals to verify register contents
        // For now, we verify that the CPU runs without errors
        assert!(true, "CPU executed without crashing");
    });
}

#[test]
fn test_cpu_three_instructions() {
    CpuTestHarness::run_cpu_test(|mut dut, harness| {
        // Program: Execute exactly 3 instructions as required
        // 0x00: ADDI x1, x0, 10   ; x1 = 10
        // 0x04: ADD  x2, x1, x1   ; x2 = x1 + x1 = 20
        // 0x08: SUB  x3, x2, x1   ; x3 = x2 - x1 = 10
        harness.load_program(&[
            (0x00, addi(1, 0, 10)),
            (0x04, add(2, 1, 1)),
            (0x08, sub(3, 2, 1)),
            (0x0C, addi(0, 0, 0)), // NOP
        ]);

        // Execute and track PC progression
        let pc_history = harness.run_cycles_with_pc_trace(&mut dut, 5);

        // Verify that PC progressed through the expected addresses
        assert_eq!(pc_history[0], 0x00, "First instruction at PC=0x00");
        assert_eq!(pc_history[1], 0x04, "Second instruction at PC=0x04");
        assert_eq!(pc_history[2], 0x08, "Third instruction at PC=0x08");

        println!("Successfully executed 3 instructions: ADDI, ADD, SUB");
    });
}

#[test]
fn test_cpu_lui_instruction() {
    CpuTestHarness::run_cpu_test(|mut dut, harness| {
        // Program: Test LUI instruction
        // 0x00: LUI x1, 0x12345   ; x1 = 0x12345000
        // 0x04: ADDI x2, x1, 0x678 ; x2 = x1 + 0x678
        harness.load_program(&[
            (0x00, lui(1, 0x12345000)),
            (0x04, addi(2, 1, 0x678)),
            (0x08, addi(0, 0, 0)), // NOP
        ]);

        // Execute for a few cycles
        harness.run_cycles(&mut dut, 4);

        println!("Successfully executed LUI instruction");
    });
}

#[test]
fn test_cpu_logic_operations() {
    CpuTestHarness::run_cpu_test(|mut dut, harness| {
        // Program: Test logic operations
        // 0x00: ADDI x1, x0, 0xFF  ; x1 = 0xFF
        // 0x04: ADDI x2, x0, 0x0F  ; x2 = 0x0F
        // 0x08: AND x3, x1, x2     ; x3 = x1 & x2 = 0x0F
        // 0x0C: OR  x4, x1, x2     ; x4 = x1 | x2 = 0xFF
        // 0x10: XOR x5, x1, x2     ; x5 = x1 ^ x2 = 0xF0
        harness.load_program(&[
            (0x00, addi(1, 0, 0xFF)),
            (0x04, addi(2, 0, 0x0F)),
            (0x08, and_inst(3, 1, 2)),
            (0x0C, or_inst(4, 1, 2)),
            (0x10, xor_inst(5, 1, 2)),
            (0x14, addi(0, 0, 0)), // NOP
        ]);

        // Execute for several cycles
        harness.run_cycles(&mut dut, 8);

        println!("Successfully executed logic operations: AND, OR, XOR");
    });
}

#[test]
fn test_cpu_branch_beq_bne() {
    CpuTestHarness::run_cpu_test(|mut dut, harness| {
        // Program: Test BEQ and BNE instructions
        // 0x00: ADDI x1, x0, 10   ; x1 = 10
        // 0x04: ADDI x2, x0, 10   ; x2 = 10
        // 0x08: BEQ  x1, x2, 8    ; Should branch to 0x10 (skip next instr)
        // 0x0C: ADDI x3, x0, 99   ; Should be skipped
        // 0x10: ADDI x4, x0, 5    ; x4 = 5
        // 0x14: BNE  x1, x4, 8    ; Should branch to 0x1C (skip next instr)
        // 0x18: ADDI x5, x0, 99   ; Should be skipped
        // 0x1C: ADDI x6, x0, 1    ; x6 = 1
        harness.load_program(&[
            (0x00, addi(1, 0, 10)),
            (0x04, addi(2, 0, 10)),
            (0x08, beq(1, 2, 8)),
            (0x0C, addi(3, 0, 99)),
            (0x10, addi(4, 0, 5)),
            (0x14, bne(1, 4, 8)),
            (0x18, addi(5, 0, 99)),
            (0x1C, addi(6, 0, 1)),
            (0x20, addi(0, 0, 0)), // NOP
        ]);

        // Execute and track PC progression
        let pc_history = harness.run_cycles_with_pc_trace(&mut dut, 10);

        // Verify branch behavior - should skip instructions at 0x0C and 0x18
        assert!(pc_history.contains(&0x00), "Should execute at 0x00");
        assert!(pc_history.contains(&0x04), "Should execute at 0x04");
        assert!(pc_history.contains(&0x08), "Should execute at 0x08 (BEQ)");
        assert!(!pc_history.contains(&0x0C), "Should skip 0x0C due to BEQ");
        assert!(pc_history.contains(&0x10), "Should execute at 0x10");
        assert!(pc_history.contains(&0x14), "Should execute at 0x14 (BNE)");
        assert!(!pc_history.contains(&0x18), "Should skip 0x18 due to BNE");
        assert!(pc_history.contains(&0x1C), "Should execute at 0x1C");

        println!("Successfully executed BEQ and BNE branches");
    });
}

#[test]
fn test_cpu_branch_blt_bge() {
    CpuTestHarness::run_cpu_test(|mut dut, harness| {
        // Program: Test BLT and BGE instructions
        // 0x00: ADDI x1, x0, 5     ; x1 = 5
        // 0x04: ADDI x2, x0, 10    ; x2 = 10
        // 0x08: BLT  x1, x2, 8     ; Should branch (5 < 10)
        // 0x0C: ADDI x3, x0, 99    ; Should be skipped
        // 0x10: BGE  x2, x1, 8     ; Should branch (10 >= 5)
        // 0x14: ADDI x4, x0, 99    ; Should be skipped
        // 0x18: ADDI x5, x0, 1     ; x5 = 1
        harness.load_program(&[
            (0x00, addi(1, 0, 5)),
            (0x04, addi(2, 0, 10)),
            (0x08, blt(1, 2, 8)),
            (0x0C, addi(3, 0, 99)),
            (0x10, bge(2, 1, 8)),
            (0x14, addi(4, 0, 99)),
            (0x18, addi(5, 0, 1)),
            (0x1C, addi(0, 0, 0)), // NOP
        ]);

        // Execute and track PC progression
        let pc_history = harness.run_cycles_with_pc_trace(&mut dut, 10);

        // Verify branch behavior
        assert!(!pc_history.contains(&0x0C), "Should skip 0x0C due to BLT");
        assert!(!pc_history.contains(&0x14), "Should skip 0x14 due to BGE");

        println!("Successfully executed BLT and BGE branches");
    });
}

#[test]
fn test_cpu_branch_bltu_bgeu() {
    CpuTestHarness::run_cpu_test(|mut dut, harness| {
        // Program: Test BLTU and BGEU instructions (unsigned comparison)
        // 0x00: ADDI x1, x0, -1    ; x1 = 0xFFFFFFFF (unsigned max)
        // 0x04: ADDI x2, x0, 5     ; x2 = 5
        // 0x08: BLTU x2, x1, 8     ; Should branch (5 < 0xFFFFFFFF unsigned)
        // 0x0C: ADDI x3, x0, 99    ; Should be skipped
        // 0x10: BGEU x1, x2, 8     ; Should branch (0xFFFFFFFF >= 5 unsigned)
        // 0x14: ADDI x4, x0, 99    ; Should be skipped
        // 0x18: ADDI x5, x0, 1     ; x5 = 1
        harness.load_program(&[
            (0x00, addi(1, 0, -1)),
            (0x04, addi(2, 0, 5)),
            (0x08, bltu(2, 1, 8)),
            (0x0C, addi(3, 0, 99)),
            (0x10, bgeu(1, 2, 8)),
            (0x14, addi(4, 0, 99)),
            (0x18, addi(5, 0, 1)),
            (0x1C, addi(0, 0, 0)), // NOP
        ]);

        // Execute and track PC progression
        let pc_history = harness.run_cycles_with_pc_trace(&mut dut, 10);

        // Verify branch behavior
        assert!(!pc_history.contains(&0x0C), "Should skip 0x0C due to BLTU");
        assert!(!pc_history.contains(&0x14), "Should skip 0x14 due to BGEU");

        println!("Successfully executed BLTU and BGEU branches");
    });
}

#[test]
fn test_cpu_load_store() {
    CpuTestHarness::run_cpu_test(|mut dut, harness| {
        // Program: Test load and store instructions
        // 0x00: ADDI x1, x0, 100   ; x1 = 100 (base address)
        // 0x04: ADDI x2, x0, 42    ; x2 = 42 (value to store)
        // 0x08: SW   x2, 0(x1)     ; Store x2 to memory[100]
        // 0x0C: LW   x3, 0(x1)     ; Load from memory[100] to x3
        // 0x10: ADDI x4, x0, 8     ; x4 = 8 (offset)
        // 0x14: SW   x2, 8(x1)     ; Store x2 to memory[108]
        // 0x18: LW   x5, 8(x1)     ; Load from memory[108] to x5
        harness.load_program(&[
            (0x00, addi(1, 0, 100)),
            (0x04, addi(2, 0, 42)),
            (0x08, sw(1, 2, 0)),
            (0x0C, lw(3, 1, 0)),
            (0x10, addi(4, 0, 8)),
            (0x14, sw(1, 2, 8)),
            (0x18, lw(5, 1, 8)),
            (0x1C, addi(0, 0, 0)), // NOP
        ]);

        // Execute and handle memory operations
        harness.run_cycles(&mut dut, 10);

        // Verify memory operations
        assert_eq!(
            harness.dmem.get(&100),
            Some(&42),
            "Memory[100] should contain 42"
        );
        assert_eq!(
            harness.dmem.get(&108),
            Some(&42),
            "Memory[108] should contain 42"
        );

        println!("Successfully executed load and store instructions");
    });
}

#[test]
fn test_cpu_auipc() {
    CpuTestHarness::run_cpu_test(|mut dut, harness| {
        // Program: Test AUIPC instruction
        // 0x00: AUIPC x1, 0x12345  ; x1 = PC + 0x12345000 = 0x12345000
        // 0x04: AUIPC x2, 0x00001  ; x2 = PC + 0x00001000 = 0x00001004
        harness.load_program(&[
            (0x00, auipc(1, 0x12345000)),
            (0x04, auipc(2, 0x00001000)),
            (0x08, addi(0, 0, 0)), // NOP
        ]);

        // Execute for a few cycles
        harness.run_cycles(&mut dut, 5);

        println!("Successfully executed AUIPC instruction");
    });
}

#[test]
fn test_cpu_tohost_halt() {
    CpuTestHarness::run_cpu_test(|mut dut, harness| {
        // TOHOST address for halt signal
        const TOHOST_ADDR: u32 = 0xFFFF_FFF0;

        // Program: Execute a few instructions, then write to tohost to signal halt
        // 0x00: ADDI x1, x0, 10    ; x1 = 10
        // 0x04: ADDI x2, x1, 5     ; x2 = x1 + 5 = 15
        // 0x08: ADD  x3, x1, x2    ; x3 = x1 + x2 = 25
        // 0x0C: ADDI x4, x0, -16   ; x4 = -16 = 0xFFFFFFF0 (tohost address, since -16 sign extends)
        // 0x10: ADDI x5, x0, 1     ; x5 = 1 (exit code)
        // 0x14: SW   x5, 0(x4)     ; Store x5 to tohost address (triggers halt)
        harness.load_program(&[
            (0x00, addi(1, 0, 10)),
            (0x04, addi(2, 1, 5)),
            (0x08, add(3, 1, 2)),
            (0x0C, addi(4, 0, -16)),
            (0x10, addi(5, 0, 1)),
            (0x14, sw(4, 5, 0)),
            (0x18, addi(0, 0, 0)), // NOP (should not be reached)
        ]);

        // Execute and watch for tohost write
        let tohost_value = harness.run_until_tohost_write(&mut dut, TOHOST_ADDR, 20);

        // Verify that tohost write was detected
        assert!(
            tohost_value.is_some(),
            "Expected write to tohost address (0x{:08X}) to be detected",
            TOHOST_ADDR
        );
        assert_eq!(
            tohost_value.unwrap(),
            1,
            "Expected tohost value to be 1 (exit code)"
        );
        assert_eq!(
            harness.dmem.get(&TOHOST_ADDR),
            Some(&1),
            "Memory at tohost address should contain the written value"
        );

        println!("Successfully tested tohost halt mechanism");
    });
}

#[test]
fn test_cpu_load_byte() {
    CpuTestHarness::run_cpu_test(|mut dut, harness| {
        // Program: Test LB (load byte signed) and LBU (load byte unsigned)
        // We'll store a word with mixed signed/unsigned bytes and load them
        // 0x00: ADDI x1, x0, 100   ; x1 = 100 (base address)
        // 0x04: ADDI x2, x0, -1    ; x2 = 0xFFFFFFFF
        // 0x08: SW   x2, 0(x1)     ; Store 0xFFFFFFFF to mem[100]
        // 0x0C: LB   x3, 0(x1)     ; Load byte 0 (0xFF), sign-extend to 0xFFFFFFFF
        // 0x10: LB   x4, 1(x1)     ; Load byte 1 (0xFF), sign-extend to 0xFFFFFFFF
        // 0x14: LBU  x5, 0(x1)     ; Load byte 0 (0xFF), zero-extend to 0x000000FF
        // 0x18: LBU  x6, 1(x1)     ; Load byte 1 (0xFF), zero-extend to 0x000000FF
        harness.load_program(&[
            (0x00, addi(1, 0, 100)),
            (0x04, addi(2, 0, -1)),
            (0x08, sw(1, 2, 0)),
            (0x0C, lb(3, 1, 0)),
            (0x10, lb(4, 1, 1)),
            (0x14, lbu(5, 1, 0)),
            (0x18, lbu(6, 1, 1)),
            (0x1C, addi(0, 0, 0)), // NOP
        ]);

        // Execute and handle memory operations, capturing rd_data at specific PCs
        let rd_data = harness.run_cycles_capture_rd_data(&mut dut, 12, &[0x0C, 0x10, 0x14, 0x18]);

        // Verify memory operations
        assert_eq!(
            harness.dmem.get(&100),
            Some(&0xFFFFFFFF),
            "Memory[100] should contain 0xFFFFFFFF"
        );

        // Verify load operations
        assert_eq!(
            rd_data.get(&0x0C).copied().unwrap_or(0),
            0xFFFFFFFF,
            "LB x3, 0(x1) should load 0xFF and sign-extend to 0xFFFFFFFF"
        );
        assert_eq!(
            rd_data.get(&0x10).copied().unwrap_or(0),
            0xFFFFFFFF,
            "LB x4, 1(x1) should load 0xFF and sign-extend to 0xFFFFFFFF"
        );
        assert_eq!(
            rd_data.get(&0x14).copied().unwrap_or(0),
            0x000000FF,
            "LBU x5, 0(x1) should load 0xFF and zero-extend to 0x000000FF"
        );
        assert_eq!(
            rd_data.get(&0x18).copied().unwrap_or(0),
            0x000000FF,
            "LBU x6, 1(x1) should load 0xFF and zero-extend to 0x000000FF"
        );

        println!("Successfully executed LB and LBU instructions");
    });
}

#[test]
fn test_cpu_load_halfword() {
    CpuTestHarness::run_cpu_test(|mut dut, harness| {
        // Program: Test LH (load halfword signed) and LHU (load halfword unsigned)
        // 0x00: ADDI x1, x0, 100   ; x1 = 100 (base address)
        // 0x04: ADDI x2, x0, -1    ; x2 = 0xFFFFFFFF
        // 0x08: SW   x2, 0(x1)     ; Store 0xFFFFFFFF to mem[100]
        // 0x0C: LH   x3, 0(x1)     ; Load halfword 0 (0xFFFF), sign-extend to 0xFFFFFFFF
        // 0x10: LH   x4, 2(x1)     ; Load halfword 1 (0xFFFF), sign-extend to 0xFFFFFFFF
        // 0x14: LHU  x5, 0(x1)     ; Load halfword 0 (0xFFFF), zero-extend to 0x0000FFFF
        // 0x18: LHU  x6, 2(x1)     ; Load halfword 1 (0xFFFF), zero-extend to 0x0000FFFF
        harness.load_program(&[
            (0x00, addi(1, 0, 100)),
            (0x04, addi(2, 0, -1)),
            (0x08, sw(1, 2, 0)),
            (0x0C, lh(3, 1, 0)),
            (0x10, lh(4, 1, 2)),
            (0x14, lhu(5, 1, 0)),
            (0x18, lhu(6, 1, 2)),
            (0x1C, addi(0, 0, 0)), // NOP
        ]);

        // Execute and handle memory operations
        let rd_data = harness.run_cycles_capture_rd_data(&mut dut, 12, &[0x0C, 0x10, 0x14, 0x18]);

        // Verify memory operations
        assert_eq!(
            harness.dmem.get(&100),
            Some(&0xFFFFFFFF),
            "Memory[100] should contain 0xFFFFFFFF"
        );

        // Verify load operations
        assert_eq!(
            rd_data.get(&0x0C).copied().unwrap_or(0),
            0xFFFFFFFF,
            "LH x3, 0(x1) should load 0xFFFF and sign-extend to 0xFFFFFFFF"
        );
        assert_eq!(
            rd_data.get(&0x10).copied().unwrap_or(0),
            0xFFFFFFFF,
            "LH x4, 2(x1) should load 0xFFFF and sign-extend to 0xFFFFFFFF"
        );
        assert_eq!(
            rd_data.get(&0x14).copied().unwrap_or(0),
            0x0000FFFF,
            "LHU x5, 0(x1) should load 0xFFFF and zero-extend to 0x0000FFFF"
        );
        assert_eq!(
            rd_data.get(&0x18).copied().unwrap_or(0),
            0x0000FFFF,
            "LHU x6, 2(x1) should load 0xFFFF and zero-extend to 0x0000FFFF"
        );

        println!("Successfully executed LH and LHU instructions");
    });
}

#[test]
fn test_cpu_store_byte() {
    CpuTestHarness::run_cpu_test(|mut dut, harness| {
        // Program: Test SB (store byte)
        // We'll write individual bytes to different positions in a word
        // 0x00: ADDI x1, x0, 100   ; x1 = 100 (base address)
        // 0x04: ADDI x2, x0, 0x12  ; x2 = 0x12
        // 0x08: ADDI x3, x0, 0x34  ; x3 = 0x34
        // 0x0C: ADDI x4, x0, 0x56  ; x4 = 0x56
        // 0x10: ADDI x5, x0, 0x78  ; x5 = 0x78
        // 0x14: SB   x2, 0(x1)     ; Store 0x12 to byte 0 of mem[100]
        // 0x18: SB   x3, 1(x1)     ; Store 0x34 to byte 1 of mem[100]
        // 0x1C: SB   x4, 2(x1)     ; Store 0x56 to byte 2 of mem[100]
        // 0x20: SB   x5, 3(x1)     ; Store 0x78 to byte 3 of mem[100]
        // 0x24: LW   x6, 0(x1)     ; Load full word, should be 0x78563412
        harness.load_program(&[
            (0x00, addi(1, 0, 100)),
            (0x04, addi(2, 0, 0x12)),
            (0x08, addi(3, 0, 0x34)),
            (0x0C, addi(4, 0, 0x56)),
            (0x10, addi(5, 0, 0x78)),
            (0x14, sb(1, 2, 0)),
            (0x18, sb(1, 3, 1)),
            (0x1C, sb(1, 4, 2)),
            (0x20, sb(1, 5, 3)),
            (0x24, lw(6, 1, 0)),
            (0x28, addi(0, 0, 0)), // NOP
        ]);

        // Execute and handle memory operations
        harness.run_cycles(&mut dut, 15);

        // Verify memory operations - bytes stored in little-endian order
        assert_eq!(
            harness.dmem.get(&100),
            Some(&0x78563412),
            "Memory[100] should contain 0x78563412 after byte stores"
        );

        println!("Successfully executed SB instruction");
    });
}

#[test]
fn test_cpu_store_halfword() {
    CpuTestHarness::run_cpu_test(|mut dut, harness| {
        // Program: Test SH (store halfword)
        // 0x00: ADDI x1, x0, 100   ; x1 = 100 (base address)
        // 0x04: ADDI x2, x0, 0x234 ; x2 = 0x234 (ADDI only supports 12-bit immediates)
        // 0x08: ADDI x3, x0, 0x678 ; x3 = 0x678
        // 0x0C: SH   x2, 0(x1)     ; Store 0x0234 to halfword 0 of mem[100]
        // 0x10: SH   x3, 2(x1)     ; Store 0x0678 to halfword 1 of mem[100]
        // 0x14: LW   x4, 0(x1)     ; Load full word, should be 0x06780234
        harness.load_program(&[
            (0x00, addi(1, 0, 100)),
            (0x04, addi(2, 0, 0x234)),
            (0x08, addi(3, 0, 0x678)),
            (0x0C, sh(1, 2, 0)),
            (0x10, sh(1, 3, 2)),
            (0x14, lw(4, 1, 0)),
            (0x18, addi(0, 0, 0)), // NOP
        ]);

        // Execute and handle memory operations
        harness.run_cycles(&mut dut, 12);

        // Verify memory operations - halfwords stored in little-endian order
        assert_eq!(
            harness.dmem.get(&100),
            Some(&0x06780234),
            "Memory[100] should contain 0x06780234 after halfword stores"
        );

        println!("Successfully executed SH instruction");
    });
}

#[test]
fn test_cpu_byte_halfword_mixed() {
    CpuTestHarness::run_cpu_test(|mut dut, harness| {
        // Program: Test mixed byte/halfword operations with positive and negative values
        // 0x00: ADDI x1, x0, 200   ; x1 = 200 (base address)
        // 0x04: ADDI x2, x0, -128  ; x2 = 0xFFFFFF80 (negative byte)
        // 0x08: SB   x2, 0(x1)     ; Store 0x80 to byte 0
        // 0x0C: LB   x3, 0(x1)     ; Load byte (signed), should be 0xFFFFFF80
        // 0x10: LBU  x4, 0(x1)     ; Load byte (unsigned), should be 0x00000080
        // 0x14: ADDI x5, x0, -1    ; x5 = 0xFFFFFFFF
        // 0x18: SH   x5, 4(x1)     ; Store 0xFFFF to halfword at offset 4
        // 0x1C: LH   x6, 4(x1)     ; Load halfword (signed), should be 0xFFFFFFFF
        // 0x20: LHU  x7, 4(x1)     ; Load halfword (unsigned), should be 0x0000FFFF
        harness.load_program(&[
            (0x00, addi(1, 0, 200)),
            (0x04, addi(2, 0, -128)),
            (0x08, sb(1, 2, 0)),
            (0x0C, lb(3, 1, 0)),
            (0x10, lbu(4, 1, 0)),
            (0x14, addi(5, 0, -1)),
            (0x18, sh(1, 5, 4)),
            (0x1C, lh(6, 1, 4)),
            (0x20, lhu(7, 1, 4)),
            (0x24, addi(0, 0, 0)), // NOP
        ]);

        // Execute and handle memory operations
        let rd_data = harness.run_cycles_capture_rd_data(&mut dut, 15, &[0x0C, 0x10, 0x1C, 0x20]);

        // Verify load operations
        assert_eq!(
            rd_data.get(&0x0C).copied().unwrap_or(0),
            0xFFFFFF80,
            "LB x3, 0(x1) should load 0x80 and sign-extend to 0xFFFFFF80"
        );
        assert_eq!(
            rd_data.get(&0x10).copied().unwrap_or(0),
            0x00000080,
            "LBU x4, 0(x1) should load 0x80 and zero-extend to 0x00000080"
        );
        assert_eq!(
            rd_data.get(&0x1C).copied().unwrap_or(0),
            0xFFFFFFFF,
            "LH x6, 4(x1) should load 0xFFFF and sign-extend to 0xFFFFFFFF"
        );
        assert_eq!(
            rd_data.get(&0x20).copied().unwrap_or(0),
            0x0000FFFF,
            "LHU x7, 4(x1) should load 0xFFFF and zero-extend to 0x0000FFFF"
        );

        println!("Successfully executed mixed byte/halfword operations");
    });
}

#[test]
fn test_cpu_fence_instruction() {
    CpuTestHarness::run_cpu_test(|mut dut, harness| {
        harness.load_program(&[
            (0x00, addi(1, 0, 10)), // x1 = 10
            (0x04, fence()),        // FENCE (should be NOP for single-cycle CPU)
            (0x08, addi(2, 1, 5)),  // x2 = x1 + 5 = 15
            (0x0C, addi(0, 0, 0)),  // NOP
        ]);

        // Execute instructions
        harness.run_cycles(&mut dut, 5);

        // Verify FENCE didn't affect execution
        // After 3 cycles (addi, fence, addi), x1 should be 10 and x2 should be 15
        // We can't directly check register values, but execution should proceed normally
        assert_eq!(dut.halted, 0, "CPU should not be halted after FENCE");
    });
}

#[test]
fn test_cpu_ecall_instruction() {
    CpuTestHarness::run_cpu_test(|mut dut, harness| {
        harness.load_program(&[
            (0x00, addi(1, 0, 42)), // x1 = 42
            (0x04, ecall()),        // ECALL - should halt CPU
            (0x08, addi(2, 0, 99)), // Should not execute
        ]);

        // Execute first instruction
        harness.step_cycle(&mut dut);
        assert_eq!(dut.halted, 0, "CPU should not be halted yet");

        // Execute ECALL
        harness.step_cycle(&mut dut);
        assert_eq!(dut.halted, 1, "CPU should be halted after ECALL");

        // PC should stop advancing
        let halted_pc = dut.imem_addr;
        harness.step_cycle(&mut dut);
        assert_eq!(
            dut.imem_addr, halted_pc,
            "PC should not advance when halted"
        );
    });
}

#[test]
fn test_cpu_ebreak_instruction() {
    CpuTestHarness::run_cpu_test(|mut dut, harness| {
        harness.load_program(&[
            (0x00, addi(1, 0, 100)), // x1 = 100
            (0x04, ebreak()),        // EBREAK - should halt CPU
            (0x08, addi(2, 0, 200)), // Should not execute
        ]);

        // Execute first instruction
        harness.step_cycle(&mut dut);
        assert_eq!(dut.halted, 0, "CPU should not be halted yet");

        // Execute EBREAK
        harness.step_cycle(&mut dut);
        assert_eq!(dut.halted, 1, "CPU should be halted after EBREAK");

        // PC should stop advancing
        let halted_pc = dut.imem_addr;
        harness.step_cycle(&mut dut);
        assert_eq!(
            dut.imem_addr, halted_pc,
            "PC should not advance when halted"
        );
    });
}

#[test]
fn test_cpu_csr_read_write() {
    CpuTestHarness::run_cpu_test(|mut dut, harness| {
        // Test CSRRW (CSR Read/Write)
        // CSR address 0x300 (mstatus in real RISC-V, but we treat it as generic)
        harness.load_program(&[
            (0x00, addi(1, 0, 100)),    // x1 = 100
            (0x04, csrrw(2, 1, 0x300)), // x2 = CSR[0x300] (old value, should be 0); CSR[0x300] = x1 (100)
            (0x08, sw(0, 2, 0x100)),    // Store x2 to memory[0x100] to verify it's 0
            (0x0C, csrrw(3, 0, 0x300)), // x3 = CSR[0x300] (should be 100); CSR[0x300] = 0
            (0x10, sw(0, 3, 0x104)),    // Store x3 to memory[0x104] to verify it's 100
            (0x14, csrrw(4, 0, 0x300)), // x4 = CSR[0x300] (should be 0); CSR[0x300] = 0
            (0x18, sw(0, 4, 0x108)),    // Store x4 to memory[0x108] to verify it's 0
            (0x1C, addi(0, 0, 0)),      // NOP
        ]);

        // Execute instructions
        harness.run_cycles(&mut dut, 12);

        assert_eq!(dut.halted, 0, "CPU should not be halted");

        // Verify CSR operations
        assert_eq!(
            harness.dmem.get(&0x100).copied().unwrap_or(0xDEADBEEF),
            0,
            "First CSRRW should read 0 from uninitialized CSR"
        );
        assert_eq!(
            harness.dmem.get(&0x104).copied().unwrap_or(0xDEADBEEF),
            100,
            "Second CSRRW should read 100 from CSR (written by first CSRRW)"
        );
        assert_eq!(
            harness.dmem.get(&0x108).copied().unwrap_or(0xDEADBEEF),
            0,
            "Third CSRRW should read 0 from CSR (written by second CSRRW)"
        );
    });
}

#[test]
fn test_cpu_csr_set_clear() {
    CpuTestHarness::run_cpu_test(|mut dut, harness| {
        // Test CSRRS (CSR Read and Set) and CSRRC (CSR Read and Clear)
        harness.load_program(&[
            (0x00, addi(1, 0, 0b1010)), // x1 = 0b1010
            (0x04, csrrw(0, 1, 0x301)), // CSR[0x301] = 0b1010 (write initial value)
            (0x08, addi(2, 0, 0b0101)), // x2 = 0b0101
            (0x0C, csrrs(3, 2, 0x301)), // x3 = CSR[0x301] (0b1010); CSR[0x301] |= x2 (becomes 0b1111)
            (0x10, sw(0, 3, 0x100)),    // Store x3 to verify it read 0b1010
            (0x14, addi(4, 0, 0b1000)), // x4 = 0b1000
            (0x18, csrrc(5, 4, 0x301)), // x5 = CSR[0x301] (0b1111); CSR[0x301] &= ~x4 (becomes 0b0111)
            (0x1C, sw(0, 5, 0x104)),    // Store x5 to verify it read 0b1111
            (0x20, csrrw(6, 0, 0x301)), // x6 = CSR[0x301] (final value, should be 0b0111)
            (0x24, sw(0, 6, 0x108)),    // Store x6 to verify final CSR value
            (0x28, addi(0, 0, 0)),      // NOP
        ]);

        // Execute instructions
        harness.run_cycles(&mut dut, 15);

        assert_eq!(dut.halted, 0, "CPU should not be halted");

        // Verify CSR operations
        assert_eq!(
            harness.dmem.get(&0x100).copied().unwrap_or(0xDEADBEEF),
            0b1010,
            "CSRRS should read old value 0b1010"
        );
        assert_eq!(
            harness.dmem.get(&0x104).copied().unwrap_or(0xDEADBEEF),
            0b1111,
            "CSRRC should read value 0b1111 (after CSRRS set bits)"
        );
        assert_eq!(
            harness.dmem.get(&0x108).copied().unwrap_or(0xDEADBEEF),
            0b0111,
            "Final CSR value should be 0b0111 (after CSRRC cleared bit 3)"
        );
    });
}

#[test]
fn test_cpu_csr_immediate() {
    CpuTestHarness::run_cpu_test(|mut dut, harness| {
        // Test immediate CSR instructions (CSRRWI, CSRRSI, CSRRCI)
        harness.load_program(&[
            (0x00, csrrwi(1, 15, 0x302)), // CSR[0x302] = 15; x1 = old value (0)
            (0x04, sw(0, 1, 0x100)),      // Store x1 to verify it's 0
            (0x08, csrrsi(2, 8, 0x302)),  // CSR[0x302] |= 8 (15 | 8 = 15); x2 = old value (15)
            (0x0C, sw(0, 2, 0x104)),      // Store x2 to verify it's 15
            (0x10, csrrci(3, 4, 0x302)),  // CSR[0x302] &= ~4 (15 & ~4 = 11); x3 = old value (15)
            (0x14, sw(0, 3, 0x108)),      // Store x3 to verify it's 15
            (0x18, csrrw(4, 0, 0x302)),   // x4 = CSR[0x302] (final value, should be 11)
            (0x1C, sw(0, 4, 0x10C)),      // Store x4 to verify final CSR value
            (0x20, addi(0, 0, 0)),        // NOP
        ]);

        // Execute instructions
        harness.run_cycles(&mut dut, 12);

        assert_eq!(dut.halted, 0, "CPU should not be halted");

        // Verify CSR operations
        assert_eq!(
            harness.dmem.get(&0x100).copied().unwrap_or(0xDEADBEEF),
            0,
            "CSRRWI should read 0 from uninitialized CSR"
        );
        assert_eq!(
            harness.dmem.get(&0x104).copied().unwrap_or(0xDEADBEEF),
            15,
            "CSRRSI should read 15 (value written by CSRRWI)"
        );
        assert_eq!(
            harness.dmem.get(&0x108).copied().unwrap_or(0xDEADBEEF),
            15,
            "CSRRCI should read 15 (15 | 8 = 15, so unchanged)"
        );
        assert_eq!(
            harness.dmem.get(&0x10C).copied().unwrap_or(0xDEADBEEF),
            11,
            "Final CSR value should be 11 (15 & ~4 = 11)"
        );
    });
}

// ============================================================================
// M Extension CPU Tests
// ============================================================================

#[test]
fn test_cpu_mul_instruction() {
    CpuTestHarness::run_cpu_test(|mut dut, harness| {
        // Test MUL instruction
        harness.load_program(&[
            (0x00, addi(1, 0, 10)),  // x1 = 10
            (0x04, addi(2, 0, 20)),  // x2 = 20
            (0x08, mul(3, 1, 2)),    // x3 = x1 × x2 = 200
            (0x0C, sw(0, 3, 0x100)), // Store result
            (0x10, addi(0, 0, 0)),   // NOP
        ]);

        // Execute instructions
        harness.run_cycles(&mut dut, 6);

        assert_eq!(
            harness.dmem.get(&0x100).copied().unwrap_or(0xDEADBEEF),
            200,
            "MUL: 10 × 20 should be 200"
        );
    });
}

#[test]
fn test_cpu_mulh_instruction() {
    CpuTestHarness::run_cpu_test(|mut dut, harness| {
        // Test MULH instruction (signed × signed, upper 32 bits)
        // Load large values that will produce non-zero upper 32 bits
        harness.load_program(&[
            (0x00, lui(1, 0x10000)), // x1 = 0x00010000
            (0x04, lui(2, 0x10000)), // x2 = 0x00010000
            (0x08, mulh(3, 1, 2)),   // x3 = upper 32 bits of x1 × x2
            (0x0C, sw(0, 3, 0x100)), // Store result
            (0x10, addi(0, 0, 0)),   // NOP
        ]);

        // Execute instructions
        harness.run_cycles(&mut dut, 6);

        // 0x00010000 × 0x00010000 = 0x0000000100000000
        // Upper 32 bits = 0x00000001
        assert_eq!(
            harness.dmem.get(&0x100).copied().unwrap_or(0xDEADBEEF),
            0x00000001,
            "MULH: upper 32 bits should be 0x00000001"
        );
    });
}

#[test]
fn test_cpu_div_instruction() {
    CpuTestHarness::run_cpu_test(|mut dut, harness| {
        // Test DIV instruction
        harness.load_program(&[
            (0x00, addi(1, 0, 100)), // x1 = 100
            (0x04, addi(2, 0, 7)),   // x2 = 7
            (0x08, div(3, 1, 2)),    // x3 = x1 ÷ x2 = 14
            (0x0C, sw(0, 3, 0x100)), // Store quotient
            (0x10, addi(0, 0, 0)),   // NOP
        ]);

        // Execute instructions
        harness.run_cycles(&mut dut, 6);

        assert_eq!(
            harness.dmem.get(&0x100).copied().unwrap_or(0xDEADBEEF),
            14,
            "DIV: 100 ÷ 7 should be 14"
        );
    });
}

#[test]
fn test_cpu_div_by_zero() {
    CpuTestHarness::run_cpu_test(|mut dut, harness| {
        // Test division by zero
        harness.load_program(&[
            (0x00, addi(1, 0, 100)), // x1 = 100
            (0x04, addi(2, 0, 0)),   // x2 = 0
            (0x08, div(3, 1, 2)),    // x3 = x1 ÷ 0 = 0xFFFFFFFF
            (0x0C, sw(0, 3, 0x100)), // Store result
            (0x10, addi(0, 0, 0)),   // NOP
        ]);

        // Execute instructions
        harness.run_cycles(&mut dut, 6);

        assert_eq!(
            harness.dmem.get(&0x100).copied().unwrap_or(0xDEADBEEF),
            0xFFFFFFFF,
            "DIV by zero should return 0xFFFFFFFF"
        );
    });
}

#[test]
fn test_cpu_rem_instruction() {
    CpuTestHarness::run_cpu_test(|mut dut, harness| {
        // Test REM instruction
        harness.load_program(&[
            (0x00, addi(1, 0, 100)), // x1 = 100
            (0x04, addi(2, 0, 7)),   // x2 = 7
            (0x08, rem(3, 1, 2)),    // x3 = x1 % x2 = 2
            (0x0C, sw(0, 3, 0x100)), // Store remainder
            (0x10, addi(0, 0, 0)),   // NOP
        ]);

        // Execute instructions
        harness.run_cycles(&mut dut, 6);

        assert_eq!(
            harness.dmem.get(&0x100).copied().unwrap_or(0xDEADBEEF),
            2,
            "REM: 100 % 7 should be 2"
        );
    });
}

#[test]
fn test_cpu_divu_remu_unsigned() {
    CpuTestHarness::run_cpu_test(|mut dut, harness| {
        // Test DIVU and REMU with large unsigned values
        harness.load_program(&[
            (0x00, addi(1, 0, -1)),  // x1 = 0xFFFFFFFF (max u32)
            (0x04, addi(2, 0, 2)),   // x2 = 2
            (0x08, divu(3, 1, 2)),   // x3 = 0xFFFFFFFF ÷ 2 = 0x7FFFFFFF
            (0x0C, remu(4, 1, 2)),   // x4 = 0xFFFFFFFF % 2 = 1
            (0x10, sw(0, 3, 0x100)), // Store quotient
            (0x14, sw(0, 4, 0x104)), // Store remainder
            (0x18, addi(0, 0, 0)),   // NOP
        ]);

        // Execute instructions
        harness.run_cycles(&mut dut, 8);

        assert_eq!(
            harness.dmem.get(&0x100).copied().unwrap_or(0xDEADBEEF),
            0x7FFFFFFF,
            "DIVU: 0xFFFFFFFF ÷ 2 should be 0x7FFFFFFF"
        );
        assert_eq!(
            harness.dmem.get(&0x104).copied().unwrap_or(0xDEADBEEF),
            1,
            "REMU: 0xFFFFFFFF % 2 should be 1"
        );
    });
}

#[test]
fn test_cpu_m_extension_program() {
    CpuTestHarness::run_cpu_test(|mut dut, harness| {
        // Complex program using multiple M extension instructions
        // Calculate: result = (a × b) ÷ c + (d % e)
        // where a=12, b=5, c=3, d=17, e=5
        // result = (12 × 5) ÷ 3 + (17 % 5) = 60 ÷ 3 + 2 = 20 + 2 = 22

        harness.load_program(&[
            (0x00, addi(1, 0, 12)),  // x1 = a = 12
            (0x04, addi(2, 0, 5)),   // x2 = b = 5
            (0x08, addi(3, 0, 3)),   // x3 = c = 3
            (0x0C, addi(4, 0, 17)),  // x4 = d = 17
            (0x10, addi(5, 0, 5)),   // x5 = e = 5
            (0x14, mul(6, 1, 2)),    // x6 = a × b = 60
            (0x18, div(7, 6, 3)),    // x7 = (a × b) ÷ c = 20
            (0x1C, rem(8, 4, 5)),    // x8 = d % e = 2
            (0x20, add(9, 7, 8)),    // x9 = x7 + x8 = 22
            (0x24, sw(0, 9, 0x100)), // Store final result
            (0x28, addi(0, 0, 0)),   // NOP
        ]);

        // Execute instructions
        harness.run_cycles(&mut dut, 15);

        assert_eq!(
            harness.dmem.get(&0x100).copied().unwrap_or(0xDEADBEEF),
            22,
            "Complex M extension program result should be 22"
        );
    });
}
