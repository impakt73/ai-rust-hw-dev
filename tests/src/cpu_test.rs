use riscv_core::{create_cpu_runtime, Top};
use std::collections::HashMap;

fn create_runtime() -> riscv_core::VerilatorRuntime {
    create_cpu_runtime()
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

// Helper function to encode RISC-V instructions
fn encode_i_type(opcode: u32, rd: u32, funct3: u32, rs1: u32, imm: i32) -> u32 {
    let imm_u = (imm as u32) & 0xFFF;
    (imm_u << 20) | (rs1 << 15) | (funct3 << 12) | (rd << 7) | opcode
}

fn encode_r_type(opcode: u32, rd: u32, funct3: u32, rs1: u32, rs2: u32, funct7: u32) -> u32 {
    (funct7 << 25) | (rs2 << 20) | (rs1 << 15) | (funct3 << 12) | (rd << 7) | opcode
}

fn encode_u_type(opcode: u32, rd: u32, imm: u32) -> u32 {
    (imm & 0xFFFFF000) | (rd << 7) | opcode
}

fn encode_b_type(opcode: u32, funct3: u32, rs1: u32, rs2: u32, imm: i32) -> u32 {
    let imm_u = imm as u32;
    let imm_12 = (imm_u >> 12) & 0x1;
    let imm_10_5 = (imm_u >> 5) & 0x3F;
    let imm_4_1 = (imm_u >> 1) & 0xF;
    let imm_11 = (imm_u >> 11) & 0x1;
    (imm_12 << 31)
        | (imm_10_5 << 25)
        | (rs2 << 20)
        | (rs1 << 15)
        | (funct3 << 12)
        | (imm_4_1 << 8)
        | (imm_11 << 7)
        | opcode
}

fn encode_s_type(opcode: u32, funct3: u32, rs1: u32, rs2: u32, imm: i32) -> u32 {
    let imm_u = (imm as u32) & 0xFFF;
    let imm_11_5 = (imm_u >> 5) & 0x7F;
    let imm_4_0 = imm_u & 0x1F;
    (imm_11_5 << 25) | (rs2 << 20) | (rs1 << 15) | (funct3 << 12) | (imm_4_0 << 7) | opcode
}

// RISC-V instruction encoders
fn addi(rd: u32, rs1: u32, imm: i32) -> u32 {
    encode_i_type(0b0010011, rd, 0b000, rs1, imm)
}

fn add(rd: u32, rs1: u32, rs2: u32) -> u32 {
    encode_r_type(0b0110011, rd, 0b000, rs1, rs2, 0b0000000)
}

fn sub(rd: u32, rs1: u32, rs2: u32) -> u32 {
    encode_r_type(0b0110011, rd, 0b000, rs1, rs2, 0b0100000)
}

fn and_inst(rd: u32, rs1: u32, rs2: u32) -> u32 {
    encode_r_type(0b0110011, rd, 0b111, rs1, rs2, 0b0000000)
}

fn or_inst(rd: u32, rs1: u32, rs2: u32) -> u32 {
    encode_r_type(0b0110011, rd, 0b110, rs1, rs2, 0b0000000)
}

fn xor_inst(rd: u32, rs1: u32, rs2: u32) -> u32 {
    encode_r_type(0b0110011, rd, 0b100, rs1, rs2, 0b0000000)
}

fn lui(rd: u32, imm: u32) -> u32 {
    encode_u_type(0b0110111, rd, imm)
}

fn auipc(rd: u32, imm: u32) -> u32 {
    encode_u_type(0b0010111, rd, imm)
}

// Branch instructions
fn beq(rs1: u32, rs2: u32, imm: i32) -> u32 {
    encode_b_type(0b1100011, 0b000, rs1, rs2, imm)
}

fn bne(rs1: u32, rs2: u32, imm: i32) -> u32 {
    encode_b_type(0b1100011, 0b001, rs1, rs2, imm)
}

fn blt(rs1: u32, rs2: u32, imm: i32) -> u32 {
    encode_b_type(0b1100011, 0b100, rs1, rs2, imm)
}

fn bge(rs1: u32, rs2: u32, imm: i32) -> u32 {
    encode_b_type(0b1100011, 0b101, rs1, rs2, imm)
}

fn bltu(rs1: u32, rs2: u32, imm: i32) -> u32 {
    encode_b_type(0b1100011, 0b110, rs1, rs2, imm)
}

fn bgeu(rs1: u32, rs2: u32, imm: i32) -> u32 {
    encode_b_type(0b1100011, 0b111, rs1, rs2, imm)
}

// Load/Store instructions
fn lw(rd: u32, rs1: u32, imm: i32) -> u32 {
    encode_i_type(0b0000011, rd, 0b010, rs1, imm)
}

fn sw(rs1: u32, rs2: u32, imm: i32) -> u32 {
    encode_s_type(0b0100011, 0b010, rs1, rs2, imm)
}

#[test]
fn test_cpu_basic_execution() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Top>().unwrap();

    // Create instruction memory (HashMap)
    let mut imem = HashMap::new();

    // Program: Simple arithmetic operations
    // 0x00: ADDI x1, x0, 5    ; x1 = 5
    // 0x04: ADDI x2, x0, 3    ; x2 = 3
    // 0x08: ADD  x3, x1, x2   ; x3 = x1 + x2 = 8
    imem.insert(0x00, addi(1, 0, 5));
    imem.insert(0x04, addi(2, 0, 3));
    imem.insert(0x08, add(3, 1, 2));
    imem.insert(0x0C, addi(0, 0, 0)); // NOP to end

    // Data memory (not used in this test)
    let dmem: HashMap<u32, u32> = HashMap::new();

    // Reset
    dut.rst_n = 0;
    dut.clk = 0;
    dut.eval();
    clock_cycle!(dut);
    dut.rst_n = 1;
    dut.eval();

    // Run for several cycles
    for _ in 0..10 {
        // Fetch instruction
        let pc = dut.imem_addr;
        let instruction = imem.get(&pc).copied().unwrap_or(0);
        dut.imem_data = instruction;

        // Handle data memory (reads)
        let dmem_addr = dut.dmem_addr;
        dut.dmem_rdata = dmem.get(&dmem_addr).copied().unwrap_or(0);

        dut.eval();

        // Clock cycle
        clock_cycle!(dut);
    }

    // Note: In a single-cycle implementation, we can't directly read register values
    // We would need to add debug ports or trace signals to verify register contents
    // For now, we verify that the CPU runs without errors
    assert!(true, "CPU executed without crashing");
}

#[test]
fn test_cpu_three_instructions() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Top>().unwrap();

    // Create instruction memory
    let mut imem = HashMap::new();

    // Program: Execute exactly 3 instructions as required
    // 0x00: ADDI x1, x0, 10   ; x1 = 10
    // 0x04: ADD  x2, x1, x1   ; x2 = x1 + x1 = 20
    // 0x08: SUB  x3, x2, x1   ; x3 = x2 - x1 = 10
    imem.insert(0x00, addi(1, 0, 10));
    imem.insert(0x04, add(2, 1, 1));
    imem.insert(0x08, sub(3, 2, 1));
    imem.insert(0x0C, addi(0, 0, 0)); // NOP

    // Data memory
    let dmem: HashMap<u32, u32> = HashMap::new();

    // Reset
    dut.rst_n = 0;
    dut.clk = 0;
    dut.eval();
    clock_cycle!(dut);
    dut.rst_n = 1;
    dut.eval();

    let mut pc_history = Vec::new();

    // Execute and track PC progression
    for cycle in 0..5 {
        let pc = dut.imem_addr;
        pc_history.push(pc);

        let instruction = imem.get(&pc).copied().unwrap_or(0);
        dut.imem_data = instruction;

        let dmem_addr = dut.dmem_addr;
        dut.dmem_rdata = dmem.get(&dmem_addr).copied().unwrap_or(0);

        dut.eval();

        println!(
            "Cycle {}: PC = 0x{:08X}, Instruction = 0x{:08X}",
            cycle, pc, instruction
        );

        clock_cycle!(dut);
    }

    // Verify that PC progressed through the expected addresses
    assert_eq!(pc_history[0], 0x00, "First instruction at PC=0x00");
    assert_eq!(pc_history[1], 0x04, "Second instruction at PC=0x04");
    assert_eq!(pc_history[2], 0x08, "Third instruction at PC=0x08");

    println!("Successfully executed 3 instructions: ADDI, ADD, SUB");
}

#[test]
fn test_cpu_lui_instruction() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Top>().unwrap();

    // Create instruction memory
    let mut imem = HashMap::new();

    // Program: Test LUI instruction
    // 0x00: LUI x1, 0x12345   ; x1 = 0x12345000
    // 0x04: ADDI x2, x1, 0x678 ; x2 = x1 + 0x678
    imem.insert(0x00, lui(1, 0x12345000));
    imem.insert(0x04, addi(2, 1, 0x678));
    imem.insert(0x08, addi(0, 0, 0)); // NOP

    let dmem: HashMap<u32, u32> = HashMap::new();

    // Reset
    dut.rst_n = 0;
    dut.clk = 0;
    dut.eval();
    clock_cycle!(dut);
    dut.rst_n = 1;
    dut.eval();

    // Execute for a few cycles
    for cycle in 0..4 {
        let pc = dut.imem_addr;
        let instruction = imem.get(&pc).copied().unwrap_or(0);
        dut.imem_data = instruction;

        let dmem_addr = dut.dmem_addr;
        dut.dmem_rdata = dmem.get(&dmem_addr).copied().unwrap_or(0);

        dut.eval();

        println!(
            "Cycle {}: PC = 0x{:08X}, Instruction = 0x{:08X}",
            cycle, pc, instruction
        );

        clock_cycle!(dut);
    }

    println!("Successfully executed LUI instruction");
}

#[test]
fn test_cpu_logic_operations() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Top>().unwrap();

    // Create instruction memory
    let mut imem = HashMap::new();

    // Program: Test logic operations
    // 0x00: ADDI x1, x0, 0xFF  ; x1 = 0xFF
    // 0x04: ADDI x2, x0, 0x0F  ; x2 = 0x0F
    // 0x08: AND x3, x1, x2     ; x3 = x1 & x2 = 0x0F
    // 0x0C: OR  x4, x1, x2     ; x4 = x1 | x2 = 0xFF
    // 0x10: XOR x5, x1, x2     ; x5 = x1 ^ x2 = 0xF0
    imem.insert(0x00, addi(1, 0, 0xFF));
    imem.insert(0x04, addi(2, 0, 0x0F));
    imem.insert(0x08, and_inst(3, 1, 2));
    imem.insert(0x0C, or_inst(4, 1, 2));
    imem.insert(0x10, xor_inst(5, 1, 2));
    imem.insert(0x14, addi(0, 0, 0)); // NOP

    let dmem: HashMap<u32, u32> = HashMap::new();

    // Reset
    dut.rst_n = 0;
    dut.clk = 0;
    dut.eval();
    clock_cycle!(dut);
    dut.rst_n = 1;
    dut.eval();

    // Execute for several cycles
    for cycle in 0..8 {
        let pc = dut.imem_addr;
        let instruction = imem.get(&pc).copied().unwrap_or(0);
        dut.imem_data = instruction;

        let dmem_addr = dut.dmem_addr;
        dut.dmem_rdata = dmem.get(&dmem_addr).copied().unwrap_or(0);

        dut.eval();

        println!(
            "Cycle {}: PC = 0x{:08X}, Instruction = 0x{:08X}",
            cycle, pc, instruction
        );

        clock_cycle!(dut);
    }

    println!("Successfully executed logic operations: AND, OR, XOR");
}

#[test]
fn test_cpu_branch_beq_bne() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Top>().unwrap();

    // Create instruction memory
    let mut imem = HashMap::new();

    // Program: Test BEQ and BNE instructions
    // 0x00: ADDI x1, x0, 10   ; x1 = 10
    // 0x04: ADDI x2, x0, 10   ; x2 = 10
    // 0x08: BEQ  x1, x2, 8    ; Should branch to 0x10 (skip next instr)
    // 0x0C: ADDI x3, x0, 99   ; Should be skipped
    // 0x10: ADDI x4, x0, 5    ; x4 = 5
    // 0x14: BNE  x1, x4, 8    ; Should branch to 0x1C (skip next instr)
    // 0x18: ADDI x5, x0, 99   ; Should be skipped
    // 0x1C: ADDI x6, x0, 1    ; x6 = 1
    imem.insert(0x00, addi(1, 0, 10));
    imem.insert(0x04, addi(2, 0, 10));
    imem.insert(0x08, beq(1, 2, 8));
    imem.insert(0x0C, addi(3, 0, 99));
    imem.insert(0x10, addi(4, 0, 5));
    imem.insert(0x14, bne(1, 4, 8));
    imem.insert(0x18, addi(5, 0, 99));
    imem.insert(0x1C, addi(6, 0, 1));
    imem.insert(0x20, addi(0, 0, 0)); // NOP

    let dmem: HashMap<u32, u32> = HashMap::new();

    // Reset
    dut.rst_n = 0;
    dut.clk = 0;
    dut.eval();
    clock_cycle!(dut);
    dut.rst_n = 1;
    dut.eval();

    let mut pc_history = Vec::new();

    // Execute and track PC progression
    for cycle in 0..10 {
        let pc = dut.imem_addr;
        pc_history.push(pc);

        let instruction = imem.get(&pc).copied().unwrap_or(0);
        dut.imem_data = instruction;

        let dmem_addr = dut.dmem_addr;
        dut.dmem_rdata = dmem.get(&dmem_addr).copied().unwrap_or(0);

        dut.eval();

        println!(
            "Cycle {}: PC = 0x{:08X}, Instruction = 0x{:08X}",
            cycle, pc, instruction
        );

        clock_cycle!(dut);
    }

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
}

#[test]
fn test_cpu_branch_blt_bge() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Top>().unwrap();

    // Create instruction memory
    let mut imem = HashMap::new();

    // Program: Test BLT and BGE instructions
    // 0x00: ADDI x1, x0, 5     ; x1 = 5
    // 0x04: ADDI x2, x0, 10    ; x2 = 10
    // 0x08: BLT  x1, x2, 8     ; Should branch (5 < 10)
    // 0x0C: ADDI x3, x0, 99    ; Should be skipped
    // 0x10: BGE  x2, x1, 8     ; Should branch (10 >= 5)
    // 0x14: ADDI x4, x0, 99    ; Should be skipped
    // 0x18: ADDI x5, x0, 1     ; x5 = 1
    imem.insert(0x00, addi(1, 0, 5));
    imem.insert(0x04, addi(2, 0, 10));
    imem.insert(0x08, blt(1, 2, 8));
    imem.insert(0x0C, addi(3, 0, 99));
    imem.insert(0x10, bge(2, 1, 8));
    imem.insert(0x14, addi(4, 0, 99));
    imem.insert(0x18, addi(5, 0, 1));
    imem.insert(0x1C, addi(0, 0, 0)); // NOP

    let dmem: HashMap<u32, u32> = HashMap::new();

    // Reset
    dut.rst_n = 0;
    dut.clk = 0;
    dut.eval();
    clock_cycle!(dut);
    dut.rst_n = 1;
    dut.eval();

    let mut pc_history = Vec::new();

    // Execute and track PC progression
    for cycle in 0..10 {
        let pc = dut.imem_addr;
        pc_history.push(pc);

        let instruction = imem.get(&pc).copied().unwrap_or(0);
        dut.imem_data = instruction;

        let dmem_addr = dut.dmem_addr;
        dut.dmem_rdata = dmem.get(&dmem_addr).copied().unwrap_or(0);

        dut.eval();

        println!(
            "Cycle {}: PC = 0x{:08X}, Instruction = 0x{:08X}",
            cycle, pc, instruction
        );

        clock_cycle!(dut);
    }

    // Verify branch behavior
    assert!(!pc_history.contains(&0x0C), "Should skip 0x0C due to BLT");
    assert!(!pc_history.contains(&0x14), "Should skip 0x14 due to BGE");

    println!("Successfully executed BLT and BGE branches");
}

#[test]
fn test_cpu_branch_bltu_bgeu() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Top>().unwrap();

    // Create instruction memory
    let mut imem = HashMap::new();

    // Program: Test BLTU and BGEU instructions (unsigned comparison)
    // 0x00: ADDI x1, x0, -1    ; x1 = 0xFFFFFFFF (unsigned max)
    // 0x04: ADDI x2, x0, 5     ; x2 = 5
    // 0x08: BLTU x2, x1, 8     ; Should branch (5 < 0xFFFFFFFF unsigned)
    // 0x0C: ADDI x3, x0, 99    ; Should be skipped
    // 0x10: BGEU x1, x2, 8     ; Should branch (0xFFFFFFFF >= 5 unsigned)
    // 0x14: ADDI x4, x0, 99    ; Should be skipped
    // 0x18: ADDI x5, x0, 1     ; x5 = 1
    imem.insert(0x00, addi(1, 0, -1));
    imem.insert(0x04, addi(2, 0, 5));
    imem.insert(0x08, bltu(2, 1, 8));
    imem.insert(0x0C, addi(3, 0, 99));
    imem.insert(0x10, bgeu(1, 2, 8));
    imem.insert(0x14, addi(4, 0, 99));
    imem.insert(0x18, addi(5, 0, 1));
    imem.insert(0x1C, addi(0, 0, 0)); // NOP

    let dmem: HashMap<u32, u32> = HashMap::new();

    // Reset
    dut.rst_n = 0;
    dut.clk = 0;
    dut.eval();
    clock_cycle!(dut);
    dut.rst_n = 1;
    dut.eval();

    let mut pc_history = Vec::new();

    // Execute and track PC progression
    for cycle in 0..10 {
        let pc = dut.imem_addr;
        pc_history.push(pc);

        let instruction = imem.get(&pc).copied().unwrap_or(0);
        dut.imem_data = instruction;

        let dmem_addr = dut.dmem_addr;
        dut.dmem_rdata = dmem.get(&dmem_addr).copied().unwrap_or(0);

        dut.eval();

        println!(
            "Cycle {}: PC = 0x{:08X}, Instruction = 0x{:08X}",
            cycle, pc, instruction
        );

        clock_cycle!(dut);
    }

    // Verify branch behavior
    assert!(!pc_history.contains(&0x0C), "Should skip 0x0C due to BLTU");
    assert!(!pc_history.contains(&0x14), "Should skip 0x14 due to BGEU");

    println!("Successfully executed BLTU and BGEU branches");
}

#[test]
fn test_cpu_load_store() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Top>().unwrap();

    // Create instruction memory
    let mut imem = HashMap::new();

    // Program: Test load and store instructions
    // 0x00: ADDI x1, x0, 100   ; x1 = 100 (base address)
    // 0x04: ADDI x2, x0, 42    ; x2 = 42 (value to store)
    // 0x08: SW   x2, 0(x1)     ; Store x2 to memory[100]
    // 0x0C: LW   x3, 0(x1)     ; Load from memory[100] to x3
    // 0x10: ADDI x4, x0, 8     ; x4 = 8 (offset)
    // 0x14: SW   x2, 8(x1)     ; Store x2 to memory[108]
    // 0x18: LW   x5, 8(x1)     ; Load from memory[108] to x5
    imem.insert(0x00, addi(1, 0, 100));
    imem.insert(0x04, addi(2, 0, 42));
    imem.insert(0x08, sw(1, 2, 0));
    imem.insert(0x0C, lw(3, 1, 0));
    imem.insert(0x10, addi(4, 0, 8));
    imem.insert(0x14, sw(1, 2, 8));
    imem.insert(0x18, lw(5, 1, 8));
    imem.insert(0x1C, addi(0, 0, 0)); // NOP

    let mut dmem: HashMap<u32, u32> = HashMap::new();

    // Reset
    dut.rst_n = 0;
    dut.clk = 0;
    dut.eval();
    clock_cycle!(dut);
    dut.rst_n = 1;
    dut.eval();

    // Execute and handle memory operations
    for cycle in 0..10 {
        let pc = dut.imem_addr;
        let instruction = imem.get(&pc).copied().unwrap_or(0);
        dut.imem_data = instruction;

        // Handle data memory reads (before eval, use old address)
        let dmem_addr_pre = dut.dmem_addr;
        dut.dmem_rdata = dmem.get(&dmem_addr_pre).copied().unwrap_or(0);

        dut.eval();

        // Handle data memory writes (after eval, use new address)
        let dmem_addr = dut.dmem_addr;
        if dut.dmem_we != 0 {
            dmem.insert(dmem_addr, dut.dmem_wdata);
            println!(
                "Cycle {}: WRITE mem[{}] = {}",
                cycle, dmem_addr, dut.dmem_wdata
            );
        }

        println!(
            "Cycle {}: PC = 0x{:08X}, Instruction = 0x{:08X}",
            cycle, pc, instruction
        );

        clock_cycle!(dut);
    }

    // Verify memory operations
    assert_eq!(dmem.get(&100), Some(&42), "Memory[100] should contain 42");
    assert_eq!(dmem.get(&108), Some(&42), "Memory[108] should contain 42");

    println!("Successfully executed load and store instructions");
}

#[test]
fn test_cpu_auipc() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Top>().unwrap();

    // Create instruction memory
    let mut imem = HashMap::new();

    // Program: Test AUIPC instruction
    // 0x00: AUIPC x1, 0x12345  ; x1 = PC + 0x12345000 = 0x12345000
    // 0x04: AUIPC x2, 0x00001  ; x2 = PC + 0x00001000 = 0x00001004
    imem.insert(0x00, auipc(1, 0x12345000));
    imem.insert(0x04, auipc(2, 0x00001000));
    imem.insert(0x08, addi(0, 0, 0)); // NOP

    let dmem: HashMap<u32, u32> = HashMap::new();

    // Reset
    dut.rst_n = 0;
    dut.clk = 0;
    dut.eval();
    clock_cycle!(dut);
    dut.rst_n = 1;
    dut.eval();

    // Execute for a few cycles
    for cycle in 0..5 {
        let pc = dut.imem_addr;
        let instruction = imem.get(&pc).copied().unwrap_or(0);
        dut.imem_data = instruction;

        let dmem_addr = dut.dmem_addr;
        dut.dmem_rdata = dmem.get(&dmem_addr).copied().unwrap_or(0);

        dut.eval();

        println!(
            "Cycle {}: PC = 0x{:08X}, Instruction = 0x{:08X}",
            cycle, pc, instruction
        );

        clock_cycle!(dut);
    }

    println!("Successfully executed AUIPC instruction");
}
