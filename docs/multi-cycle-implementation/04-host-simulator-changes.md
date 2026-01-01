# Host-Side Simulator Changes

## Overview

This document describes the minimal changes required to the host-side Rust simulator code to support the multi-cycle CPU implementation. The changes are designed to be backward-compatible where possible.

## Key Design Goal

**Minimal impact on host-side code.** The multi-cycle changes are primarily in the RTL. The simulator should only need to:

1. Wait for instruction completion (instead of assuming 1 cycle = 1 instruction)
2. Track cycle counts accurately
3. Handle the new `instr_complete` signal

## Changes Required

### 1. riscv_core/src/lib.rs

**No changes required** to the core library. The `Top` struct definition uses the `#[verilog]` macro which automatically picks up new ports.

The `instr_complete` signal will be automatically exposed through the marlin interface:

```rust
// The #[verilog] macro will automatically expose:
// dut.instr_complete  (readable as u8 or bool)
```

### 2. cpu-sim/src/sim.rs

The simulator needs to be updated to wait for instruction completion.

#### Current Implementation (Single-Cycle)

```rust
pub fn step(&mut self) -> Option<u32> {
    // ... fetch, decode, memory setup ...
    
    // Clock tick - assumes instruction completes in one cycle
    self.cpu.clk = 0;
    self.cpu.eval();
    self.cpu.clk = 1;
    self.cpu.eval();
    
    self.cycle_count += 1;
    
    // ... check for halt ...
}
```

#### Updated Implementation (Multi-Cycle)

```rust
/// Execute a single simulation step (one instruction)
/// Returns Some(tohost_value) if halt detected, None otherwise
pub fn step(&mut self) -> Option<u32> {
    const TOHOST_ADDR: u32 = 0xFFFF_FFF0;
    const MAX_CYCLES_PER_INSTR: u64 = 100; // Safety limit
    
    let mut cycles_this_instr = 0;
    let mut halt_value = None;
    
    // Loop until instruction completes
    loop {
        cycles_this_instr += 1;
        if cycles_this_instr > MAX_CYCLES_PER_INSTR {
            log::error!("Instruction taking too long (>{} cycles)", MAX_CYCLES_PER_INSTR);
            break;
        }
        
        // Sample signals before clock edge
        let pc = self.cpu.imem_addr;
        
        // Instruction Fetch (only meaningful in FETCH state, but harmless otherwise)
        let instruction = self.bus.read_word(pc);
        self.cpu.imem_data = instruction;
        
        // First evaluation: update combinational logic
        self.cpu.eval();
        
        // Data Memory Read (handle if dmem_re is asserted)
        if self.cpu.dmem_re != 0 {
            let dmem_addr = self.cpu.dmem_addr;
            let dmem_size = self.cpu.dmem_size;
            let rdata = match dmem_size {
                0b00 => self.bus.read_byte(dmem_addr) as u32,
                0b01 => self.bus.read_halfword(dmem_addr) as u32,
                _ => self.bus.read_word(dmem_addr),
            };
            self.cpu.dmem_rdata = rdata;
            self.cpu.eval();
        }
        
        // Data Memory Write (handle if dmem_we is asserted)
        if self.cpu.dmem_we != 0 {
            let dmem_addr = self.cpu.dmem_addr;
            let dmem_size = self.cpu.dmem_size;
            let wdata = self.cpu.dmem_wdata;
            
            match dmem_size {
                0b00 => self.bus.write_byte(dmem_addr, wdata as u8),
                0b01 => self.bus.write_halfword(dmem_addr, wdata as u16),
                _ => self.bus.write_word(dmem_addr, wdata),
            }
            
            // Check for halt signal
            if dmem_addr == TOHOST_ADDR {
                halt_value = Some(wdata);
            }
        }
        
        // Clock tick
        self.cpu.clk = 0;
        self.cpu.eval();
        self.cpu.clk = 1;
        self.cpu.eval();
        
        self.cycle_count += 1;
        
        // VCD dump
        if let Some(ref mut vcd) = self.vcd {
            vcd.dump(self.cycle_count + 3);
        }
        
        // Check if instruction completed
        if self.cpu.instr_complete != 0 {
            break;
        }
        
        // Check if halted
        if self.cpu.halted != 0 {
            break;
        }
    }
    
    // Process FIFO, trace callbacks, etc. (unchanged)
    // ...
    
    halt_value
}
```

#### Key Changes Summary

| Aspect | Single-Cycle | Multi-Cycle |
|--------|--------------|-------------|
| Cycles per step() | Always 1 | Variable (3-5+) |
| Loop structure | No loop | Loop until instr_complete |
| Memory handling | Once per step | Every cycle |
| Cycle counting | +1 per step | +N per step |

### 3. tests/src/cpu_test.rs

The test harness needs updates to work with multi-cycle execution.

#### Current clock_cycle! Macro

```rust
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
```

#### Updated Macros

Add a new macro for multi-cycle execution:

```rust
/// Execute one clock cycle (for low-level testing)
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

/// Execute cycles until instruction completes (for multi-cycle CPU)
/// Returns the number of cycles taken
macro_rules! execute_instruction {
    ($dut:expr, $imem:expr, $dmem:expr) => {{
        const MAX_CYCLES: u32 = 100;
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
                let addr = $dut.dmem_addr;
                let wdata = $dut.dmem_wdata;
                let size = $dut.dmem_size;
                
                match size {
                    0b00 => {
                        // Byte store
                        let word_addr = addr & !0x3;
                        let byte_offset = (addr & 0x3) as usize;
                        let current = $dmem.get(&word_addr).copied().unwrap_or(0);
                        let mut bytes = current.to_le_bytes();
                        bytes[byte_offset] = wdata as u8;
                        $dmem.insert(word_addr, u32::from_le_bytes(bytes));
                    }
                    0b01 => {
                        // Halfword store
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
                        // Word store
                        $dmem.insert(addr, wdata);
                    }
                }
            }
            
            clock_cycle!($dut);
            
            // Check for completion
            if $dut.instr_complete != 0 || $dut.halted != 0 {
                break;
            }
        }
        
        cycles
    }};
}
```

#### Updated Test Example

```rust
#[test]
fn test_cpu_basic_execution() {
    let runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Top>().unwrap();

    let mut imem = HashMap::new();
    let mut dmem: HashMap<u32, u32> = HashMap::new();

    // Program: ADDI x1, x0, 5
    imem.insert(0x00, addi(1, 0, 5));
    imem.insert(0x04, addi(2, 0, 3));
    imem.insert(0x08, add(3, 1, 2));
    imem.insert(0x0C, addi(0, 0, 0)); // NOP

    // Reset
    dut.rst_n = 0;
    dut.clk = 0;
    dut.eval();
    clock_cycle!(dut);
    dut.rst_n = 1;
    dut.eval();

    // Execute first instruction
    let cycles1 = execute_instruction!(dut, imem, dmem);
    assert_eq!(dut.imem_addr, 0x04, "PC should be at 0x04 after first instruction");
    println!("First instruction took {} cycles", cycles1);

    // Execute second instruction
    let cycles2 = execute_instruction!(dut, imem, dmem);
    assert_eq!(dut.imem_addr, 0x08, "PC should be at 0x08 after second instruction");
    println!("Second instruction took {} cycles", cycles2);

    // Execute third instruction
    let cycles3 = execute_instruction!(dut, imem, dmem);
    assert_eq!(dut.imem_addr, 0x0C, "PC should be at 0x0C after third instruction");
    println!("Third instruction took {} cycles", cycles3);
}
```

### 4. Expected Cycle Counts

Tests should be updated to expect correct cycle counts:

```rust
#[test]
fn test_cpu_instruction_cycle_counts() {
    // ... setup ...

    // R-type: 4 cycles (FETCH, DECODE, EXECUTE, WRITEBACK)
    let cycles = execute_instruction!(dut, imem, dmem);
    assert_eq!(cycles, 4, "R-type should take 4 cycles");

    // Load: 5 cycles (FETCH, DECODE, MEM_ADDR, MEM_READ, WRITEBACK)
    let cycles = execute_instruction!(dut, imem, dmem);
    assert_eq!(cycles, 5, "Load should take 5 cycles");

    // Store: 4 cycles (FETCH, DECODE, MEM_ADDR, MEM_WRITE)
    let cycles = execute_instruction!(dut, imem, dmem);
    assert_eq!(cycles, 4, "Store should take 4 cycles");

    // Branch: 3 cycles (FETCH, DECODE, BRANCH)
    let cycles = execute_instruction!(dut, imem, dmem);
    assert_eq!(cycles, 3, "Branch should take 3 cycles");
}
```

## Backward Compatibility

### Option: Conditional Compilation

If needed, the simulator can support both single-cycle and multi-cycle CPUs:

```rust
#[cfg(feature = "multicycle")]
pub fn step(&mut self) -> Option<u32> {
    // Multi-cycle implementation
    loop {
        // ... multi-cycle logic ...
        if self.cpu.instr_complete != 0 {
            break;
        }
    }
}

#[cfg(not(feature = "multicycle"))]
pub fn step(&mut self) -> Option<u32> {
    // Single-cycle implementation (current)
    // ... single clock cycle ...
}
```

### Option: Runtime Detection

Alternatively, detect at runtime if the CPU has the `instr_complete` signal:

```rust
pub fn step(&mut self) -> Option<u32> {
    // Always run at least one cycle
    self.run_clock_cycle();
    
    // If instr_complete exists and is not asserted, keep running
    #[cfg(feature = "multicycle")]
    while self.cpu.instr_complete == 0 && self.cpu.halted == 0 {
        self.run_clock_cycle();
    }
    
    // ... rest of step logic ...
}
```

## Summary of Host-Side Changes

| File | Change Type | Lines Affected | Description |
|------|-------------|----------------|-------------|
| `riscv_core/src/lib.rs` | None | 0 | Auto-picks up new signals |
| `cpu-sim/src/sim.rs` | Minor | ~50 | Add loop waiting for instr_complete |
| `tests/src/cpu_test.rs` | Minor | ~100 | Add execute_instruction! macro |
| `tests/src/lib.rs` | None | 0 | No changes |

**Total estimated changes: ~150 lines**

## Test Updates Required

Existing tests will need modifications:

| Test File | Change Required |
|-----------|-----------------|
| `alu_test.rs` | **None** - ALU is unchanged |
| `regfile_test.rs` | **None** - RegFile is unchanged |
| `cpu_test.rs` | **Minor** - Use execute_instruction! macro |

---

**Next Document:** [05-testing-strategy.md](05-testing-strategy.md)
