# Multi-Cycle Non-Pipelined CPU Implementation Plan

**Date:** 2026-01-01  
**Status:** Planning Document  
**Target:** Convert single-cycle RISC-V RV32IM CPU to multi-cycle non-pipelined design

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Current Architecture Analysis](#current-architecture-analysis)
3. [Multi-Cycle Architecture Design](#multi-cycle-architecture-design)
4. [RTL Module Changes](#rtl-module-changes)
5. [Control Signal Timing](#control-signal-timing)
6. [Memory Interface Protocol](#memory-interface-protocol)
7. [Host-Side Simulator Adaptations](#host-side-simulator-adaptations)
8. [Migration Strategy](#migration-strategy)
9. [Testing Plan](#testing-plan)
10. [References](#references)

---

## Executive Summary

This document outlines a comprehensive plan to convert the current single-cycle RISC-V RV32IM CPU implementation to a multi-cycle non-pipelined architecture. The primary motivation is to enable FPGA synthesis by reducing critical path length through converting combinational logic to sequential (registered) logic.

**Key Goals:**
- Reduce combinational logic depth for better timing closure on FPGAs
- Maintain full RV32IM instruction set support (54 instructions)
- Minimize changes to host-side simulator (marlin/Verilator test framework)
- Preserve existing test infrastructure (84 tests)
- Enable practical FPGA deployment

**Design Approach:**
- 4-5 stage Finite State Machine (FSM) per instruction
- Sequential execution with registered intermediate values
- Unchanged memory interface semantics (handshake protocol)
- Backward-compatible testbench interface

---

## Current Architecture Analysis

### Single-Cycle Design Overview

The existing implementation completes every instruction in one clock cycle:

**RTL Modules:**
1. **`top.sv`** (286 lines)
   - Top-level CPU with PC, instruction decoder, control logic
   - Single-cycle execution: all operations complete in one `clk` edge
   - Exposed instruction memory (`imem_*`) and data memory (`dmem_*`) interfaces
   - Branch decision logic (combinational comparisons)
   - CSR register file (4096 entries)
   - Write-back multiplexer (combinational)

2. **`decoder.sv`** (240 lines)
   - Purely combinational instruction decoder
   - Generates control signals: `alu_op`, `alu_src`, `reg_write`, `mem_write`, `mem_read`, `branch`, `jump`, etc.
   - Immediate extraction for all formats (I, S, B, U, J)

3. **`alu.sv`** (128 lines)
   - Combinational ALU supporting RV32I + M extension
   - Operations: ADD, SUB, AND, OR, XOR, SLL, SRL, SRA, SLT, SLTU
   - M extension: MUL, MULH, MULHSU, MULHU, DIV, DIVU, REM, REMU
   - 64-bit multiply intermediate (combinational)
   - Division/remainder operations (combinational)

4. **`regfile.sv`** (45 lines)
   - 32x32-bit register file
   - Combinational reads (asynchronous)
   - Synchronous writes on `posedge clk`
   - x0 hardwired to zero

**Critical Path:**
```
imem_data → decoder → regfile read → ALU → write-back mux → regfile write
```

For M extension:
```
imem_data → decoder → regfile read → 64-bit multiply/divide → write-back mux → regfile write
```

**Timing Challenges for FPGA:**
- Long combinational path (especially for division operations)
- 64-bit multiplier synthesis creates deep logic
- Branch comparators add to critical path
- PC calculation logic in same cycle as instruction execution

### Host-Side Simulator Architecture

**Test Framework (`tests/src/cpu_test.rs`, `alu_test.rs`, `regfile_test.rs`):**
- Uses `marlin` crate with Verilator backend
- 84 comprehensive tests (50 in cpu_verifier package)
- Instruction memory: `HashMap<u32, u32>` managed by testbench
- Data memory: `HashMap<u32, u32>` managed by testbench

**Current Test Loop Pattern:**
```rust
for _ in 0..N {
    // 1. Fetch instruction from imem based on PC
    let pc = dut.imem_addr;
    let instruction = imem.get(&pc).copied().unwrap_or(0);
    dut.imem_data = instruction;
    
    // 2. Provide data memory read data
    let dmem_addr = dut.dmem_addr;
    dut.dmem_rdata = dmem.get(&dmem_addr).copied().unwrap_or(0);
    
    // 3. Evaluate combinational logic
    dut.eval();
    
    // 4. Handle data memory writes (if dmem_we is high)
    if dut.dmem_we == 1 {
        let addr = dut.dmem_addr;
        let data = dut.dmem_wdata;
        dmem.insert(addr, data);
    }
    
    // 5. Clock cycle (posedge clk)
    clock_cycle!(dut);  // clk=0→1→0
}
```

**Key Observation:** The testbench assumes one instruction completes per clock cycle. This must be adapted for multi-cycle execution.

---

## Multi-Cycle Architecture Design

### State Machine Definition

We'll implement a 5-state FSM that executes instructions across multiple clock cycles:

**States:**
```systemverilog
typedef enum logic [2:0] {
    FETCH      = 3'b000,  // Fetch instruction from memory
    DECODE     = 3'b001,  // Decode instruction and read registers
    EXECUTE    = 3'b010,  // Execute ALU/branch/jump operation
    MEMORY     = 3'b011,  // Access data memory (load/store)
    WRITEBACK  = 3'b100   // Write result to register file
} state_t;
```

**State Transitions:**

```
FETCH → DECODE → EXECUTE → [MEMORY] → WRITEBACK → FETCH
                    ↓
                 (branches/jumps skip MEMORY/WRITEBACK)
```

**Cycle Breakdown by Instruction Type:**

| Instruction Type | States Required | Cycle Count |
|-----------------|----------------|-------------|
| R-type (ADD, SUB, AND, etc.) | FETCH → DECODE → EXECUTE → WRITEBACK | 4 |
| I-type ALU (ADDI, ANDI, etc.) | FETCH → DECODE → EXECUTE → WRITEBACK | 4 |
| Load (LW, LH, LB, etc.) | FETCH → DECODE → EXECUTE → MEMORY → WRITEBACK | 5 |
| Store (SW, SH, SB) | FETCH → DECODE → EXECUTE → MEMORY | 4 |
| Branch (BEQ, BNE, etc.) | FETCH → DECODE → EXECUTE | 3 |
| Jump (JAL, JALR) | FETCH → DECODE → EXECUTE → WRITEBACK | 4 |
| U-type (LUI, AUIPC) | FETCH → DECODE → EXECUTE → WRITEBACK | 4 |
| System (ECALL, EBREAK, FENCE) | FETCH → DECODE | 2 |
| CSR | FETCH → DECODE → EXECUTE → WRITEBACK | 4 |
| M-ext (MUL, DIV, etc.) | FETCH → DECODE → EXECUTE → WRITEBACK | 4* |

*Note: M extension operations may require additional EXECUTE cycles for iterative multiply/divide implementations. Initial implementation uses combinational M-ext operations for 4 cycles; future optimization could add multi-cycle multiply/divide.


### Registered Intermediate Values

To break up combinational paths, we introduce pipeline registers between stages:

**FETCH → DECODE Registers:**
```systemverilog
logic [31:0] ir;  // Instruction Register (fetched instruction)
logic [31:0] pc_reg;  // Current PC (for AUIPC, JAL, JALR)
```

**DECODE → EXECUTE Registers:**
```systemverilog
logic [31:0] a_reg;     // ALU operand A (rs1 data)
logic [31:0] b_reg;     // ALU operand B (rs2 data or immediate)
logic [31:0] imm_reg;   // Immediate value for memory/branches
logic [4:0]  rd_reg;    // Destination register address
logic [4:0]  alu_op_reg; // ALU operation
logic        reg_write_reg;
logic        mem_write_reg;
logic        mem_read_reg;
logic [2:0]  funct3_reg;  // For load/store/branch types
logic [6:0]  opcode_reg;  // Opcode for state machine decisions
```

**EXECUTE → MEMORY Registers:**
```systemverilog
logic [31:0] alu_result_reg;  // ALU output (address for load/store)
logic [31:0] mem_wdata_reg;   // Data to write (for stores)
```

**MEMORY → WRITEBACK Registers:**
```systemverilog
logic [31:0] mem_rdata_reg;  // Data read from memory (for loads)
logic [31:0] result_reg;     // Final result to write back
```

### Control FSM Logic

**State Transition Logic:**
```systemverilog
always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        state <= FETCH;
    end else if (halted) begin
        state <= state;  // Stay in current state when halted
    end else begin
        case (state)
            FETCH: begin
                state <= DECODE;
            end
            
            DECODE: begin
                // Determine next state based on instruction type
                if (is_ecall || is_ebreak || is_fence) begin
                    state <= FETCH;  // System instructions complete early
                end else begin
                    state <= EXECUTE;
                end
            end
            
            EXECUTE: begin
                // Branch and jump to FETCH (PC already updated)
                if (branch || jump) begin
                    state <= FETCH;
                end
                // Load/store instructions need MEMORY stage
                else if (mem_read || mem_write) begin
                    state <= MEMORY;
                end
                // Other instructions skip MEMORY
                else begin
                    state <= WRITEBACK;
                end
            end
            
            MEMORY: begin
                // After memory access, always writeback (loads) or fetch (stores)
                if (mem_read) begin
                    state <= WRITEBACK;
                end else begin
                    state <= FETCH;  // Stores don't write back
                end
            end
            
            WRITEBACK: begin
                state <= FETCH;
            end
            
            default: state <= FETCH;
        endcase
    end
end
```

---

## RTL Module Changes

### 1. `top.sv` Changes

**Major Modifications:**

**A. Add State Machine:**
- Add `typedef enum` for state machine states (FETCH, DECODE, EXECUTE, MEMORY, WRITEBACK)
- Add state register and next_state logic
- Estimated: +30 lines

**B. Add Pipeline Registers:**
- FETCH → DECODE: ir, pc_reg
- DECODE → EXECUTE: a_reg, b_reg, imm_reg, rd_reg, control signals
- EXECUTE → MEMORY: alu_result_reg, mem_wdata_reg
- MEMORY → WRITEBACK: mem_rdata_reg, result_reg
- Estimated: +25 lines of declarations

**C. Modify PC Logic:**
- Change from combinational to sequential PC updates
- PC increments in WRITEBACK state
- Branches/jumps update PC in EXECUTE state
- Estimated: +40 lines

**D. Stage-Specific Logic:**
- FETCH: Capture instruction into IR
- DECODE: Capture operands and control signals
- EXECUTE: Perform ALU operation, branch decision
- MEMORY: Access data memory
- WRITEBACK: Format and write result
- Estimated: +120 lines

**E. CSR Updates:**
- Move CSR writes to EXECUTE stage (sequential)
- Estimated: +10 lines modification

**F. Modify Memory and Register File Interfaces:**
- Qualify signals with state checks
- Estimated: +20 lines

**Total Estimated Changes:** ~400-450 lines (from 286 to ~700 lines)

---

### 2. `decoder.sv` Changes

**Minimal Changes Required:**

The decoder remains purely combinational. No internal changes needed.

**Interface Update in top.sv:**
- Connect decoder to `ir` instead of `imem_data`

**Estimated Line Count Change:** 0 lines (no change to decoder.sv itself)

---

### 3. `alu.sv` Changes

**No Changes Required:**

The ALU remains purely combinational. It will operate on registered inputs in the multi-cycle design.

**Estimated Line Count Change:** 0 lines

---

### 4. `regfile.sv` Changes

**Minor Changes:**

Option 1: Qualify write enable externally (in top.sv)
Option 2: Add state input to regfile and qualify internally

**Recommended: Option 1** (minimal changes to regfile.sv)

**Estimated Line Count Change:** 0 lines (changes in top.sv only)

---

## Control Signal Timing

### Memory Interface Timing

**Data Memory Read (Load):**

```
        EXECUTE         MEMORY        WRITEBACK
           │               │               │
CLK    ────┐   ┌───────┐   ┌───────┐   ┌──
           └───┘       └───┘       └───┘
           
dmem_addr   <invalid> <address> <address> <invalid>
dmem_re     ────────┐ ┌────────────────┐ ┌─────
                     └─┘                └─┘
dmem_rdata  <xxxxxxx> <xxxxxxx> <data> <data>
                                   ↑
                              Captured here
```

**Data Memory Write (Store):**

```
        EXECUTE         MEMORY         FETCH
           │               │              │
CLK    ────┐   ┌───────┐   ┌───────┐   ┌──
           └───┘       └───┘       └───┘
           
dmem_addr   <invalid> <address> <address> <invalid>
dmem_wdata  <invalid> <data>    <data>    <invalid>
dmem_we     ────────┐ ┌────────────────┐ ┌─────
                     └─┘                └─┘
                                   ↑
                            Write occurs here
```

**Key Timing Points:**
1. **MEMORY state**: `dmem_re` or `dmem_we` asserted
2. Memory responds **within the same cycle** (combinational memory model in testbench)
3. For reads: `dmem_rdata` captured on clock edge exiting MEMORY
4. For writes: `dmem_we` high during MEMORY state triggers write

---

## Memory Interface Protocol

### External Memory Semantics

**Instruction Memory (`imem_*`):**

**Current (Single-Cycle):**
- `imem_addr = pc` (combinational)
- Testbench provides `imem_data` based on `imem_addr`
- No handshake protocol

**Multi-Cycle (Proposed):**
- `imem_addr = pc` (sequential, updated during PC updates)
- Testbench provides `imem_data` based on `imem_addr`
- Instruction captured in `ir` during FETCH → DECODE transition
- **No protocol change required** from testbench perspective

**Data Memory (`dmem_*`):**

**Current (Single-Cycle):**
- `dmem_addr`, `dmem_wdata`, `dmem_we`, `dmem_re`, `dmem_size` driven combinationally
- Testbench provides `dmem_rdata` combinationally based on `dmem_addr`
- Write occurs when `dmem_we` is high on clock edge

**Multi-Cycle (Proposed):**
- `dmem_addr`, `dmem_wdata`, `dmem_we`, `dmem_re`, `dmem_size` driven from registered signals
- Asserted during MEMORY state
- Testbench provides `dmem_rdata` combinationally based on `dmem_addr`
- **Protocol unchanged:** Memory still responds within same cycle

**Critical Design Decision:** We maintain **single-cycle memory response** to minimize testbench changes.

---

## Host-Side Simulator Adaptations

### Required Testbench Changes

**Multi-Cycle Test Loop (Recommended Approach):**
```rust
// Execute one clock cycle at a time, independent of instruction timing
loop {
    // Instruction memory interface
    let pc = dut.imem_addr;
    let instruction = imem.get(&pc).copied().unwrap_or(0);
    dut.imem_data = instruction;
    
    // Data memory interface
    let dmem_addr = dut.dmem_addr;
    dut.dmem_rdata = dmem.get(&dmem_addr).copied().unwrap_or(0);
    
    dut.eval();
    
    // Handle memory writes
    if dut.dmem_we == 1 {
        dmem.insert(dut.dmem_addr, dut.dmem_wdata);
    }
    
    clock_cycle!(dut);
    
    // Exit when halted or max cycles reached
    if dut.halted == 1 {
        break;
    }
    
    // Safety limit to prevent infinite loops
    cycle_count += 1;
    if cycle_count >= max_cycles {
        panic!("Test exceeded maximum cycle count");
    }
}
```

**Changes Summary:**
- Execute one cycle at a time without assumptions about instruction timing
- Memory interface logic remains unchanged and works correctly cycle-by-cycle
- Tests specify maximum cycle budget instead of instruction count
- Early exit on `halted` signal
- Safety limit prevents infinite loops in case of CPU bugs

### Debug Signal Additions

**Add to `top.sv` interface:**
```systemverilog
module top (
    // ... existing ports ...
    
    // Debug outputs for multi-cycle state
    output logic [2:0]  debug_state,       // Current FSM state
    output logic [31:0] debug_ir,          // Instruction register
    output logic        debug_instr_complete  // Pulse when instruction completes
);

assign debug_state = state;
assign debug_ir = ir;
assign debug_instr_complete = (state == WRITEBACK || 
                                (state == EXECUTE && (branch || jump)) ||
                                (state == DECODE && (is_ecall || is_ebreak || is_fence)));
```

### Test Migration Strategy

**Phase 1: Update test loop structure**
- Replace instruction-count-based loops with cycle-based loops
- Set appropriate maximum cycle budgets for each test
- Verify memory interface continues to work correctly

**Phase 2: Add state awareness (optional)**
- Expose `debug_state` signal for debugging
- Can be used to verify state transitions during development
- Not required for basic functionality

**Phase 3: Update test expectations**
- Adjust cycle count expectations based on actual execution
- Add tests for state transitions (if debug signals added)
- Verify intermediate register values

**Estimated Test Change Scope:**
- **`alu_test.rs`:** No changes (ALU remains combinational, tests are self-contained)
- **`regfile_test.rs`:** Minor changes (adjust write timing)
- **`cpu_test.rs`:** Moderate changes (update loop structure, set cycle budgets)

---

## Migration Strategy

### Step-by-Step Implementation Plan

**Phase 1: Preparation (1-2 days)**
1. Create feature branch `multi-cycle-implementation`
2. Add `debug_state` output to `top.sv`
3. Modify testbench to print state transitions
4. Run baseline tests to confirm current behavior

**Phase 2: Core FSM Implementation (3-5 days)**
1. Add state machine type and state register to `top.sv`
2. Implement state transition logic
3. Add pipeline registers (IR, a_reg, b_reg, etc.)
4. Add state-based control for PC updates
5. Test: Verify FSM state transitions in simulation

**Phase 3: Stage-by-Stage Data Path (5-7 days)**
1. **FETCH stage:** Connect instruction register `ir`
2. **DECODE stage:** Capture decoded signals into pipeline registers
3. **EXECUTE stage:** Connect ALU inputs to registered values
4. **MEMORY stage:** Connect memory interface to registered signals
5. **WRITEBACK stage:** Connect register file write to result register

**Phase 4: Testbench Updates (2-3 days)**
1. Update all CPU tests to run sufficient cycles
2. Adjust assertions for multi-cycle timing
3. Add instruction completion detection

**Phase 5: Validation and Debugging (3-5 days)**
1. Run full test suite (84 tests)
2. Debug failures
3. Verify all instruction types

**Phase 6: Optimization and Documentation (2-3 days)**
1. Optimize state transitions
2. Update documentation
3. Code review and cleanup

**Total Estimated Time:** 17-28 days (3-4 weeks)

---

## Testing Plan

### Test Categories

**1. Unit Tests (Module-Level):**
- ALU: No changes needed (remains combinational)
- RegFile: Test write timing with state qualification
- Decoder: No changes needed

**2. Integration Tests (CPU-Level):**
- Basic Execution: Arithmetic, logic operations
- Control Flow: Branches, jumps
- Memory Operations: Loads, stores
- System Instructions: ECALL, EBREAK, FENCE
- CSR Instructions: All CSR variants
- M Extension: Multiply, divide, remainder

**3. State Transition Tests (New):**
- Verify correct state progression
- Test early termination (ECALL, EBREAK)
- Test state reset behavior

**4. Timing Tests (New):**
- Verify cycle count for each instruction type
- Test pipeline register updates

### Success Criteria

- **100% of existing tests pass** after adaptation
- **No regressions** in instruction functionality
- **Correct state transitions** for all instruction types
- **Correct cycle counts** for representative instructions
- **Verilator lint clean** (no warnings)

---

## References

### RISC-V Specifications
- **RISC-V ISA Specification:** https://riscv.org/technical/specifications/

### CPU Design Resources
- **Patterson & Hennessy: Computer Organization and Design (RISC-V Edition)**
  - Chapter 4: The Processor (Multi-Cycle Implementation)
- **Harris & Harris: Digital Design and Computer Architecture (RISC-V Edition)**
  - Chapter 7: Microarchitecture (Multi-Cycle Datapath)

### Verilator and Marlin
- **Verilator Manual:** https://verilator.org/guide/latest/
- **Marlin Crate Documentation:** https://docs.rs/marlin/

---

## Appendix: Instruction Cycle Counts Table

| Instruction | Format | Stages | Cycles | Notes |
|-------------|--------|--------|--------|-------|
| ADD, SUB, AND, OR, XOR, SLL, SRL, SRA, SLT, SLTU | R | F→D→E→W | 4 | Standard ALU ops |
| ADDI, ANDI, ORI, XORI, SLLI, SRLI, SRAI, SLTI, SLTIU | I | F→D→E→W | 4 | Immediate ALU ops |
| LW, LH, LB, LHU, LBU | I | F→D→E→M→W | 5 | Load instructions |
| SW, SH, SB | S | F→D→E→M | 4 | Store instructions |
| BEQ, BNE, BLT, BGE, BLTU, BGEU | B | F→D→E | 3 | Branch instructions |
| JAL | J | F→D→E→W | 4 | Jump and link |
| JALR | I | F→D→E→W | 4 | Jump and link register |
| LUI | U | F→D→E→W | 4 | Load upper immediate |
| AUIPC | U | F→D→E→W | 4 | Add upper immediate to PC |
| ECALL, EBREAK | I | F→D | 2 | System calls (halt) |
| FENCE | I | F→D | 2 | Memory fence (NOP) |
| CSRRW, CSRRS, CSRRC, CSRRWI, CSRRSI, CSRRCI | I | F→D→E→W | 4 | CSR instructions |
| MUL, MULH, MULHSU, MULHU | R | F→D→E→W | 4 | M extension multiply |
| DIV, DIVU, REM, REMU | R | F→D→E→W | 4 | M extension divide |

**Stage Abbreviations:**
- F = FETCH
- D = DECODE
- E = EXECUTE
- M = MEMORY
- W = WRITEBACK

---

**End of Document**
