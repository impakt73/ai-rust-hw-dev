# Testing Strategy

## Overview

This document outlines the comprehensive testing strategy for the multi-cycle CPU implementation. Testing is organized in layers, from unit tests to full system tests.

## Test Organization

```
tests/
├── src/
│   ├── lib.rs              # Module declarations
│   ├── alu_test.rs         # ALU tests (no changes needed)
│   ├── regfile_test.rs     # RegFile tests (no changes needed)
│   ├── cpu_test.rs         # CPU tests (update for multi-cycle)
│   └── multicycle_test.rs  # NEW: Multi-cycle specific tests
```

## Testing Layers

### Layer 1: Unit Tests (Unchanged)

#### ALU Tests (`alu_test.rs`)
**No changes required.** The ALU remains purely combinational.

Existing tests cover:
- ADD, SUB, AND, OR, XOR
- SLL, SRL, SRA
- SLT, SLTU
- MUL, MULH, MULHSU, MULHU
- DIV, DIVU, REM, REMU

#### Register File Tests (`regfile_test.rs`)
**No changes required.** The register file behavior is unchanged.

Existing tests cover:
- Read/write operations
- x0 hardwired to zero
- Simultaneous read of rs1 and rs2

### Layer 2: FSM State Tests (NEW)

Create new tests to verify the FSM behavior:

```rust
// tests/src/multicycle_test.rs

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

#[test]
fn test_fsm_initial_state() {
    let runtime = create_cpu_runtime().expect("Failed to create runtime");
    let mut dut = runtime.create_model_simple::<Top>().unwrap();
    
    // After reset, should be in IDLE then transition to FETCH
    dut.rst_n = 0;
    dut.boot_addr = 0x1000;
    clock_cycle!(dut);
    
    dut.rst_n = 1;
    dut.eval();
    
    // PC should be at boot address
    assert_eq!(dut.imem_addr, 0x1000, "PC should be boot_addr after reset");
    
    // Should not be halted
    assert_eq!(dut.halted, 0, "Should not be halted initially");
}

#[test]
fn test_fsm_r_type_cycle_count() {
    let runtime = create_cpu_runtime().expect("Failed to create runtime");
    let mut dut = runtime.create_model_simple::<Top>().unwrap();
    
    let mut imem = HashMap::new();
    
    // ADD x1, x2, x3 (R-type: should take 4 cycles)
    imem.insert(0x0000, 0x003100b3); // add x1, x2, x3
    
    // Reset
    dut.rst_n = 0;
    dut.boot_addr = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    dut.eval();
    
    let mut cycles = 0;
    
    // Run until instruction completes
    loop {
        cycles += 1;
        
        let pc = dut.imem_addr;
        dut.imem_data = imem.get(&pc).copied().unwrap_or(0);
        dut.dmem_rdata = 0;
        dut.eval();
        
        clock_cycle!(dut);
        
        if dut.instr_complete != 0 {
            break;
        }
        
        assert!(cycles < 10, "R-type instruction taking too long");
    }
    
    assert_eq!(cycles, 4, "R-type instruction should take 4 cycles");
}

#[test]
fn test_fsm_load_cycle_count() {
    let runtime = create_cpu_runtime().expect("Failed to create runtime");
    let mut dut = runtime.create_model_simple::<Top>().unwrap();
    
    let mut imem = HashMap::new();
    let mut dmem = HashMap::new();
    
    // Set up base address in x1
    // LUI x1, 0x10000 (load 0x10000000 to x1)
    imem.insert(0x0000, 0x100000b7);
    
    // LW x2, 0(x1) (Load: should take 5 cycles)
    imem.insert(0x0004, 0x0000a103);
    
    // Memory content at 0x10000000
    dmem.insert(0x10000000, 0xDEADBEEF);
    
    // Reset
    dut.rst_n = 0;
    dut.boot_addr = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    dut.eval();
    
    // Execute LUI first (skip cycle count check)
    let mut cycles_lui = 0;
    loop {
        cycles_lui += 1;
        let pc = dut.imem_addr;
        dut.imem_data = imem.get(&pc).copied().unwrap_or(0);
        dut.eval();
        clock_cycle!(dut);
        if dut.instr_complete != 0 { break; }
    }
    
    // Now execute LW and count cycles
    let mut cycles_lw = 0;
    loop {
        cycles_lw += 1;
        
        let pc = dut.imem_addr;
        dut.imem_data = imem.get(&pc).copied().unwrap_or(0);
        dut.eval();
        
        // Handle memory read
        if dut.dmem_re != 0 {
            let addr = dut.dmem_addr;
            dut.dmem_rdata = dmem.get(&addr).copied().unwrap_or(0);
            dut.eval();
        }
        
        clock_cycle!(dut);
        
        if dut.instr_complete != 0 {
            break;
        }
        
        assert!(cycles_lw < 10, "Load instruction taking too long");
    }
    
    assert_eq!(cycles_lw, 5, "Load instruction should take 5 cycles");
}

#[test]
fn test_fsm_store_cycle_count() {
    let runtime = create_cpu_runtime().expect("Failed to create runtime");
    let mut dut = runtime.create_model_simple::<Top>().unwrap();
    
    let mut imem = HashMap::new();
    let mut dmem = HashMap::new();
    
    // ADDI x1, x0, 100 (base address)
    imem.insert(0x0000, 0x06400093);
    
    // ADDI x2, x0, 42 (value)
    imem.insert(0x0004, 0x02a00113);
    
    // SW x2, 0(x1) (Store: should take 4 cycles)
    imem.insert(0x0008, 0x0020a023);
    
    // Reset
    dut.rst_n = 0;
    dut.boot_addr = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    dut.eval();
    
    // Execute first two instructions (ADDI x1 and ADDI x2)
    for _ in 0..2 {
        loop {
            let pc = dut.imem_addr;
            dut.imem_data = imem.get(&pc).copied().unwrap_or(0);
            dut.eval();
            
            // Handle memory operations (for consistency with other loops)
            if dut.dmem_re != 0 {
                let addr = dut.dmem_addr;
                dut.dmem_rdata = dmem.get(&addr).copied().unwrap_or(0);
                dut.eval();
            }
            if dut.dmem_we != 0 {
                let addr = dut.dmem_addr;
                dmem.insert(addr, dut.dmem_wdata);
            }
            
            clock_cycle!(dut);
            if dut.instr_complete != 0 { break; }
        }
    }
    
    // Now execute SW and count cycles
    let mut cycles_sw = 0;
    loop {
        cycles_sw += 1;
        
        let pc = dut.imem_addr;
        dut.imem_data = imem.get(&pc).copied().unwrap_or(0);
        dut.eval();
        
        // Handle memory write
        if dut.dmem_we != 0 {
            let addr = dut.dmem_addr;
            dmem.insert(addr, dut.dmem_wdata);
        }
        
        clock_cycle!(dut);
        
        if dut.instr_complete != 0 {
            break;
        }
        
        assert!(cycles_sw < 10, "Store instruction taking too long");
    }
    
    assert_eq!(cycles_sw, 4, "Store instruction should take 4 cycles");
    assert_eq!(dmem.get(&100), Some(&42), "Store should write value to memory");
}

#[test]
fn test_fsm_branch_cycle_count() {
    let runtime = create_cpu_runtime().expect("Failed to create runtime");
    let mut dut = runtime.create_model_simple::<Top>().unwrap();
    
    let mut imem = HashMap::new();
    
    // ADDI x1, x0, 5
    imem.insert(0x0000, 0x00500093);
    
    // ADDI x2, x0, 5
    imem.insert(0x0004, 0x00500113);
    
    // BEQ x1, x2, 8 (Branch: should take 3 cycles)
    imem.insert(0x0008, 0x00208463);
    
    // Reset
    dut.rst_n = 0;
    dut.boot_addr = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    dut.eval();
    
    // Execute first two instructions (ADDI x1 and ADDI x2)
    for _ in 0..2 {
        loop {
            let pc = dut.imem_addr;
            dut.imem_data = imem.get(&pc).copied().unwrap_or(0);
            dut.eval();
            
            // Consistent memory handling
            if dut.dmem_re != 0 {
                dut.dmem_rdata = 0;
                dut.eval();
            }
            
            clock_cycle!(dut);
            if dut.instr_complete != 0 { break; }
        }
    }
    
    // Now execute BEQ and count cycles
    let mut cycles_br = 0;
    loop {
        cycles_br += 1;
        
        let pc = dut.imem_addr;
        dut.imem_data = imem.get(&pc).copied().unwrap_or(0);
        dut.eval();
        
        clock_cycle!(dut);
        
        if dut.instr_complete != 0 {
            break;
        }
        
        assert!(cycles_br < 10, "Branch instruction taking too long");
    }
    
    assert_eq!(cycles_br, 3, "Branch instruction should take 3 cycles");
}
```

### Layer 3: Instruction Execution Tests (Updated)

Update existing CPU tests to work with multi-cycle execution:

```rust
// tests/src/cpu_test.rs - Updated helper macro

/// Execute cycles until instruction completes
/// Returns the number of cycles taken
macro_rules! execute_instruction {
    ($dut:expr, $imem:expr, $dmem:expr) => {{
        const MAX_CYCLES: u32 = 20;
        let mut cycles = 0;
        
        loop {
            cycles += 1;
            if cycles > MAX_CYCLES {
                panic!("Instruction timeout after {} cycles", MAX_CYCLES);
            }
            
            // Instruction fetch
            let pc = $dut.imem_addr;
            let instruction = $imem.get(&pc).copied().unwrap_or(0);
            $dut.imem_data = instruction;
            
            $dut.eval();
            
            // Memory read
            if $dut.dmem_re != 0 {
                let addr = $dut.dmem_addr & !0x3;
                $dut.dmem_rdata = $dmem.get(&addr).copied().unwrap_or(0);
                $dut.eval();
            }
            
            // Memory write
            if $dut.dmem_we != 0 {
                handle_memory_write($dut, $dmem);
            }
            
            clock_cycle!($dut);
            
            if $dut.instr_complete != 0 || $dut.halted != 0 {
                break;
            }
        }
        
        cycles
    }};
}

fn handle_memory_write(dut: &mut Top, dmem: &mut HashMap<u32, u32>) {
    let addr = dut.dmem_addr;
    let wdata = dut.dmem_wdata;
    let size = dut.dmem_size;
    
    match size {
        0b00 => {
            let word_addr = addr & !0x3;
            let byte_offset = (addr & 0x3) as usize;
            let current = dmem.get(&word_addr).copied().unwrap_or(0);
            let mut bytes = current.to_le_bytes();
            bytes[byte_offset] = wdata as u8;
            dmem.insert(word_addr, u32::from_le_bytes(bytes));
        }
        0b01 => {
            let word_addr = addr & !0x3;
            let hw_offset = ((addr & 0x2) >> 1) as usize;
            let current = dmem.get(&word_addr).copied().unwrap_or(0);
            let mut bytes = current.to_le_bytes();
            let hw_bytes = (wdata as u16).to_le_bytes();
            bytes[hw_offset * 2] = hw_bytes[0];
            bytes[hw_offset * 2 + 1] = hw_bytes[1];
            dmem.insert(word_addr, u32::from_le_bytes(bytes));
        }
        _ => {
            dmem.insert(addr, wdata);
        }
    }
}
```

### Layer 4: Integration Tests

Full program execution tests with cycle count verification:

```rust
#[test]
fn test_program_execution_multi_cycle() {
    let runtime = create_cpu_runtime().expect("Failed to create runtime");
    let mut dut = runtime.create_model_simple::<Top>().unwrap();
    
    let mut imem = HashMap::new();
    let mut dmem: HashMap<u32, u32> = HashMap::new();
    
    // Program:
    // 0x00: ADDI x1, x0, 10    (4 cycles)
    // 0x04: ADDI x2, x0, 20    (4 cycles)
    // 0x08: ADD  x3, x1, x2    (4 cycles)
    // 0x0C: SW   x3, 100(x0)   (4 cycles)
    // 0x10: LW   x4, 100(x0)   (5 cycles)
    // Total: 21 cycles
    
    imem.insert(0x00, addi(1, 0, 10));
    imem.insert(0x04, addi(2, 0, 20));
    imem.insert(0x08, add(3, 1, 2));
    imem.insert(0x0C, sw(0, 3, 100));
    imem.insert(0x10, lw(4, 0, 100));
    imem.insert(0x14, addi(0, 0, 0)); // NOP
    
    // Reset
    dut.rst_n = 0;
    dut.boot_addr = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    dut.eval();
    
    let mut total_cycles = 0;
    let mut instr_count = 0;
    
    // Execute 5 instructions
    for _ in 0..5 {
        let cycles = execute_instruction!(dut, imem, dmem);
        total_cycles += cycles;
        instr_count += 1;
        
        println!("Instruction {} took {} cycles", instr_count, cycles);
    }
    
    // Verify results
    assert_eq!(dmem.get(&100), Some(&30), "Memory[100] should contain 30");
    assert_eq!(total_cycles, 21, "Total cycles should be 21");
    
    println!("Program executed: {} instructions in {} cycles", instr_count, total_cycles);
}
```

### Layer 5: Edge Case Tests

Test edge cases specific to multi-cycle operation:

```rust
#[test]
fn test_back_to_back_branches() {
    // Test that branches work correctly in sequence
    // Each branch should take 3 cycles regardless of taken/not-taken
}

#[test]
fn test_load_followed_by_use() {
    // Test that load result is available for next instruction
    // Load (5 cycles) -> ADD using loaded value (4 cycles)
}

#[test]
fn test_memory_write_read_sequence() {
    // Store (4 cycles) -> Load same address (5 cycles)
    // Verify loaded value matches stored value
}

#[test]
fn test_csr_multi_cycle() {
    // CSR operations should take 4 cycles
}

#[test]
fn test_halt_states() {
    // ECALL/EBREAK should transition to HALT state
    // CPU should stay halted, not advance PC
}
```

## Test Execution Matrix

### Cycle Count Verification

| Instruction Type | Expected Cycles | Test |
|------------------|-----------------|------|
| R-type (ADD, etc.) | 4 | test_fsm_r_type_cycle_count |
| I-type arithmetic | 4 | test_fsm_i_type_cycle_count |
| Load (LW, LH, LB) | 5 | test_fsm_load_cycle_count |
| Store (SW, SH, SB) | 4 | test_fsm_store_cycle_count |
| Branch | 3 | test_fsm_branch_cycle_count |
| JAL | 4 | test_fsm_jal_cycle_count |
| JALR | 4 | test_fsm_jalr_cycle_count |
| LUI | 4 | test_fsm_lui_cycle_count |
| AUIPC | 4 | test_fsm_auipc_cycle_count |
| CSR | 4 | test_fsm_csr_cycle_count |
| FENCE | 2 | test_fsm_fence_cycle_count |
| ECALL/EBREAK | 2+ | test_fsm_halt_cycle_count |

### Regression Testing

All existing tests must continue to pass with multi-cycle execution:

| Test Category | Tests | Expected Outcome |
|---------------|-------|------------------|
| ALU tests | 16 | All pass (unchanged) |
| RegFile tests | 6 | All pass (unchanged) |
| CPU basic | 10+ | All pass (updated macros) |
| Branch tests | 6 | All pass (updated macros) |
| Load/Store tests | 10+ | All pass (updated macros) |
| CSR tests | 3 | All pass (updated macros) |
| M-extension tests | 8+ | All pass (updated macros) |

## Test Commands

```bash
# Run all tests
cargo test --verbose

# Run only multi-cycle FSM tests
cargo test --package cpu_verifier -- multicycle_test

# Run only CPU tests
cargo test --package cpu_verifier -- cpu_test

# Run with output
cargo test --package cpu_verifier -- --nocapture

# Lint RTL
verilator --lint-only rtl/*.sv
```

## VCD Debugging

For debugging FSM behavior, use VCD waveform dumps:

```rust
#[test]
fn test_fsm_with_vcd() {
    let runtime = create_cpu_runtime().expect("Failed to create runtime");
    
    // Create model with tracing
    let config = VerilatedModelConfig {
        enable_tracing: true,
        ..Default::default()
    };
    
    let mut dut = runtime.create_model::<Top>(&config).unwrap();
    let mut vcd = dut.open_vcd("debug_fsm.vcd");
    
    // ... run test ...
    
    // View with: gtkwave debug_fsm.vcd
}
```

Key signals to monitor in VCD:
- `current_state` - FSM state
- `next_state` - Next FSM state
- `pc` - Program counter
- `ir` - Instruction register
- `instr_complete` - Instruction completion signal
- `a_reg`, `b_reg`, `alu_out_reg`, `mdr` - Latching registers

---

**Next Document:** [06-implementation-phases.md](06-implementation-phases.md)
