use riscv_core::{create_cpu_runtime, Top};
use std::collections::HashMap;

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

/// Execute cycles until instruction completes. Returns cycle count.
macro_rules! execute_instruction {
    ($dut:expr, $imem:expr, $dmem:expr) => {{
        const MAX_CYCLES: u32 = 100;
        let mut cycles = 0;
        loop {
            cycles += 1;
            assert!(cycles <= MAX_CYCLES, "Instruction timeout");

            let pc = $dut.imem_addr;
            $dut.imem_data = $imem.get(&pc).copied().unwrap_or(0);

            $dut.eval();

            if $dut.dmem_re != 0 {
                let addr = $dut.dmem_addr & !0x3;
                $dut.dmem_rdata = $dmem.get(&addr).copied().unwrap_or(0);
                $dut.eval();
            }

            if $dut.dmem_we != 0 {
                let addr = $dut.dmem_addr;
                let wdata = $dut.dmem_wdata;
                let size = $dut.dmem_size;
                println!(
                    "cycle {}: dmem_we=1 addr=0x{:08x} data=0x{:08x} size={}",
                    cycles, addr, wdata, size
                );
                match size {
                    0b00 => {
                        let word_addr = addr & !0x3;
                        let byte_offset = (addr & 0x3) as usize;
                        let current = $dmem.get(&word_addr).copied().unwrap_or(0);
                        let mut bytes = current.to_le_bytes();
                        bytes[byte_offset] = wdata as u8;
                        $dmem.insert(word_addr, u32::from_le_bytes(bytes));
                    }
                    0b01 => {
                        let word_addr = addr & !0x3;
                        let hw_offset = ((addr & 0x2) >> 1) as usize;
                        let current = $dmem.get(&word_addr).copied().unwrap_or(0);
                        let mut bytes = current.to_le_bytes();
                        let hw_bytes = (wdata as u16).to_le_bytes();
                        bytes[hw_offset * 2] = hw_bytes[0];
                        bytes[hw_offset * 2 + 1] = hw_bytes[1];
                        $dmem.insert(word_addr, u32::from_le_bytes(bytes));
                    }
                    _ => {
                        $dmem.insert(addr, wdata);
                    }
                }
            }

            let done = $dut.instr_complete != 0 || $dut.halted != 0;

            clock_cycle!($dut);

            if done || $dut.instr_complete != 0 || $dut.halted != 0 {
                break;
            }
        }
        cycles
    }};
}

fn create_runtime() -> riscv_core::VerilatorRuntime {
    create_cpu_runtime().expect("Failed to create runtime")
}

fn addi(rd: u32, rs1: u32, imm: i32) -> u32 {
    let imm_u = (imm as u32) & 0xFFF;
    (imm_u << 20) | (rs1 << 15) | (0b000 << 12) | (rd << 7) | 0b0010011
}

fn add(rd: u32, rs1: u32, rs2: u32) -> u32 {
    (0b0000000 << 25) | (rs2 << 20) | (rs1 << 15) | (0b000 << 12) | (rd << 7) | 0b0110011
}

fn lw(rd: u32, rs1: u32, imm: i32) -> u32 {
    let imm_u = (imm as u32) & 0xFFF;
    (imm_u << 20) | (rs1 << 15) | (0b010 << 12) | (rd << 7) | 0b0000011
}

fn sw(rs1: u32, rs2: u32, imm: i32) -> u32 {
    let imm_u = (imm as u32) & 0xFFF;
    let imm_11_5 = (imm_u >> 5) & 0x7F;
    let imm_4_0 = imm_u & 0x1F;
    (imm_11_5 << 25) | (rs2 << 20) | (rs1 << 15) | (0b010 << 12) | (imm_4_0 << 7) | 0b0100011
}

fn beq(rs1: u32, rs2: u32, imm: i32) -> u32 {
    let imm_u = imm as u32;
    let imm_12 = (imm_u >> 12) & 0x1;
    let imm_10_5 = (imm_u >> 5) & 0x3F;
    let imm_4_1 = (imm_u >> 1) & 0xF;
    let imm_11 = (imm_u >> 11) & 0x1;
    (imm_12 << 31)
        | (imm_10_5 << 25)
        | (rs2 << 20)
        | (rs1 << 15)
        | (0b000 << 12)
        | (imm_4_1 << 8)
        | (imm_11 << 7)
        | 0b1100011
}

#[test]
fn test_fsm_initial_state() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Top>().unwrap();

    dut.rst_n = 0;
    dut.boot_addr = 0x1000;
    clock_cycle!(dut);

    dut.rst_n = 1;
    dut.eval();

    assert_eq!(dut.imem_addr, 0x1000, "PC should be boot_addr after reset");
    assert_eq!(dut.halted, 0, "CPU should not be halted after reset");
}

#[test]
fn test_fsm_r_type_cycle_count() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Top>().unwrap();

    let mut imem = HashMap::new();
    let mut dmem: HashMap<u32, u32> = HashMap::new();

    imem.insert(0x0000, add(1, 2, 3)); // ADD x1, x2, x3

    dut.rst_n = 0;
    dut.boot_addr = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    dut.eval();

    let cycles = execute_instruction!(dut, imem, dmem);
    assert_eq!(cycles, 4, "R-type should take 4 cycles");
}

#[test]
fn test_fsm_load_cycle_count() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Top>().unwrap();

    let mut imem = HashMap::new();
    let mut dmem = HashMap::new();

    imem.insert(0x0000, addi(1, 0, 16)); // x1 = 16
    imem.insert(0x0004, lw(2, 1, 0)); // LW x2, 0(x1)
    dmem.insert(16, 0xDEADBEEF);

    dut.rst_n = 0;
    dut.boot_addr = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    dut.eval();

    // Execute ADDI (ignore cycles)
    execute_instruction!(dut, imem, dmem);

    // Execute LW and measure
    let cycles = execute_instruction!(dut, imem, dmem);
    assert_eq!(cycles, 5, "Load should take 5 cycles");
}

#[test]
fn test_fsm_store_cycle_count() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Top>().unwrap();

    let mut imem = HashMap::new();
    let mut dmem = HashMap::new();

    imem.insert(0x0000, addi(1, 0, 100)); // base
    imem.insert(0x0004, addi(2, 0, 42)); // value
    imem.insert(0x0008, sw(1, 2, 0)); // store

    dut.rst_n = 0;
    dut.boot_addr = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    dut.eval();

    execute_instruction!(dut, imem, dmem); // ADDI base
    execute_instruction!(dut, imem, dmem); // ADDI value
    println!("PC before store = 0x{:08x}", dut.imem_addr);
    let cycles = execute_instruction!(dut, imem, dmem); // SW

    println!("store cycles = {}", cycles);
    println!("dmem contents: {:?}", dmem);
    assert_eq!(cycles, 4, "Store should take 4 cycles");
    assert_eq!(dmem.get(&100), Some(&42), "Store should write value");
}

#[test]
fn test_fsm_branch_cycle_count() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Top>().unwrap();

    let mut imem = HashMap::new();
    let mut dmem: HashMap<u32, u32> = HashMap::new();

    imem.insert(0x0000, addi(1, 0, 5));
    imem.insert(0x0004, addi(2, 0, 5));
    imem.insert(0x0008, beq(1, 2, 8)); // branch to 0x10

    dut.rst_n = 0;
    dut.boot_addr = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    dut.eval();

    execute_instruction!(dut, imem, dmem); // addi
    execute_instruction!(dut, imem, dmem); // addi
    let cycles = execute_instruction!(dut, imem, dmem); // branch

    assert_eq!(cycles, 3, "Branch should take 3 cycles");
    assert_eq!(dut.imem_addr, 0x0010, "PC should branch to target");
}
