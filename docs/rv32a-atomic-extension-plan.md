# RV32A Atomic Extension Implementation Plan

## Executive Summary

This document outlines a comprehensive plan to add the **RV32A (Atomic Instructions)** extension to the current **RV32IM** single-cycle RISC-V CPU implementation. The A extension provides atomic read-modify-write operations and load-reserved/store-conditional primitives essential for lock-free synchronization and multi-processor systems. This upgrade will transform the CPU from **RV32IM** to **RV32IMA**, adding 11 new atomic instructions.

## Table of Contents

1. [Overview of RV32A Extension](#overview-of-rv32a-extension)
2. [Current Architecture Analysis](#current-architecture-analysis)
3. [RTL Modifications Required](#rtl-modifications-required)
4. [Memory Interface Changes](#memory-interface-changes)
5. [Testing Strategy](#testing-strategy)
6. [Build Configuration Updates](#build-configuration-updates)
7. [Implementation Phases](#implementation-phases)
8. [Risk Assessment](#risk-assessment)
9. [Validation Criteria](#validation-criteria)
10. [Appendices](#appendices)

---

## Overview of RV32A Extension

### What is RV32A?

RV32A = Atomic Instructions extension for RV32I

The A extension provides:
- **Atomic Memory Operations (AMO)**: Read-modify-write operations that execute atomically
- **Load-Reserved/Store-Conditional (LR/SC)**: Primitives for implementing lock-free algorithms and synchronization

### RV32A Instructions (11 Total)

The A extension adds **11 new instructions** for atomic operations:

#### Load-Reserved and Store-Conditional

| Instruction | Encoding | Description |
|-------------|----------|-------------|
| **LR.W**    | R-type, funct5=00010 | Load-Reserved Word |
| **SC.W**    | R-type, funct5=00011 | Store-Conditional Word |

#### Atomic Memory Operations (AMO)

| Instruction | Encoding | Description |
|-------------|----------|-------------|
| **AMOSWAP.W** | R-type, funct5=00001 | Atomic Swap |
| **AMOADD.W**  | R-type, funct5=00000 | Atomic Add |
| **AMOXOR.W**  | R-type, funct5=00100 | Atomic XOR |
| **AMOAND.W**  | R-type, funct5=01100 | Atomic AND |
| **AMOOR.W**   | R-type, funct5=01000 | Atomic OR |
| **AMOMIN.W**  | R-type, funct5=10000 | Atomic Minimum (signed) |
| **AMOMAX.W**  | R-type, funct5=10100 | Atomic Maximum (signed) |
| **AMOMINU.W** | R-type, funct5=11000 | Atomic Minimum (unsigned) |
| **AMOMAXU.W** | R-type, funct5=11100 | Atomic Maximum (unsigned) |

**Key Characteristics:**
- All A instructions use **R-type encoding** with opcode `0101111` (AMO)
- Use `funct3 = 010` for word (32-bit) operations
- `funct5` field (bits [31:27]) identifies the specific operation
- Support optional **acquire** and **release** semantics via `aq` and `rl` bits
- All operations are **word-aligned** (32-bit) on RV32

### Instruction Format

```
31    27 26 25 24    20 19    15 14   12 11     7 6      0
┌───────┬──┬──┬────────┬────────┬───────┬────────┬────────┐
│funct5 │aq│rl│  rs2   │  rs1   │funct3 │   rd   │ opcode │
└───────┴──┴──┴────────┴────────┴───────┴────────┴────────┘
```

- **opcode**: `0101111` (AMO opcode)
- **funct3**: `010` (word operations)
- **funct5**: Specifies the atomic operation
- **aq** (acquire): Memory ordering - acquire semantics
- **rl** (release): Memory ordering - release semantics
- **rs1**: Address register (base address)
- **rs2**: Source data register (for AMO) or 0 (for LR)
- **rd**: Destination register

### Atomic Operation Semantics

#### Load-Reserved / Store-Conditional

**LR.W (Load-Reserved Word):**
```c
// Atomically:
rd = mem[rs1]           // Load word from address in rs1
reservation = rs1       // Record reservation on this address
```

**SC.W (Store-Conditional Word):**
```c
// Atomically:
if (reservation == rs1 && reservation_valid) {
    mem[rs1] = rs2      // Store word if reservation is still valid
    rd = 0              // Success
    clear_reservation()
} else {
    rd = 1              // Failure (non-zero)
}
```

**LR/SC Usage Pattern:**
```assembly
retry:
    lr.w    t0, (a0)        # Load-reserved from address in a0
    add     t0, t0, t1      # Modify the value
    sc.w    t2, t0, (a0)    # Store-conditional
    bnez    t2, retry       # Retry if store failed
```

#### Atomic Memory Operations (AMO)

All AMO instructions follow this pattern:
```c
// Atomically (indivisible):
rd = mem[rs1]              // Read original value
mem[rs1] = rd OP rs2       // Perform operation and write back
```

**Examples:**

**AMOSWAP.W:**
```c
temp = mem[rs1]
mem[rs1] = rs2
rd = temp
```

**AMOADD.W:**
```c
temp = mem[rs1]
mem[rs1] = temp + rs2
rd = temp
```

**AMOMIN.W (signed):**
```c
temp = mem[rs1]
mem[rs1] = min(signed(temp), signed(rs2))
rd = temp
```

### Memory Ordering (Acquire/Release)

- **aq=1 (acquire)**: No subsequent memory operations can be observed before this operation
- **rl=1 (release)**: This operation cannot be observed before any prior memory operations
- **aq=1, rl=1**: Both acquire and release (sequential consistency)
- **aq=0, rl=0**: No ordering guarantees (relaxed)

**For single-cycle, single-hart implementation:**
- Memory ordering is naturally satisfied (all operations are sequential)
- aq/rl bits can be ignored in RTL (but must be preserved in encoding)
- Important for future multi-hart or pipelined implementations

### Special Cases and Edge Conditions

#### Alignment Requirements
- All atomic operations must be **word-aligned** (address[1:0] = 00)
- Misaligned accesses should raise an exception (or be implementation-defined)

#### Reservation Management (LR/SC)
- **Reservation invalidation**: Reservation is cleared on:
  - Any SC.W to any address
  - Any write to the reserved address (from any source)
  - Context switches, exceptions, interrupts
  - Implementation-specific events
- **Forward progress guarantee**: LR/SC sequence must eventually succeed if no conflicting accesses occur

#### LR/SC Granularity
- RISC-V spec allows implementation to reserve a region (not just exact word)
- Minimum granularity: one word (4 bytes)
- Can be larger (e.g., cache line) for efficiency
- **Recommended for single-cycle**: Exact word matching for simplicity

---

## Current Architecture Analysis

### Existing RTL Modules

```
top.sv (CPU top-level)
├── decoder.sv (Instruction decoder)
├── alu.sv (ALU - arithmetic/logic/shift/multiply/divide operations)
└── regfile.sv (32×32-bit register file)
```

### Current Memory Interface

**Instruction Memory (Read-Only):**
```systemverilog
output logic [31:0] imem_addr,    // Instruction address
input  logic [31:0] imem_rdata    // Instruction data
```

**Data Memory (Read/Write):**
```systemverilog
output logic [31:0] dmem_addr,    // Data address
output logic        dmem_write,   // Write enable
output logic [3:0]  dmem_wmask,   // Byte write mask
output logic [31:0] dmem_wdata,   // Write data
input  logic [31:0] dmem_rdata    // Read data
```

### Current Decoder Logic

The decoder currently handles:
- RV32I base instructions (arithmetic, logic, loads, stores, branches, jumps)
- M extension (multiplication and division)
- Zicsr extension (CSR operations)

**Current opcode support:**
- `0110011`: R-type (ALU, M extension)
- `0010011`: I-type immediate
- `0000011`: Load
- `0100011`: Store
- `1100011`: Branch
- `1101111`: JAL
- `1100111`: JALR
- `0110111`: LUI
- `0010111`: AUIPC
- `1110011`: System/CSR

**Gap:** No support for opcode `0101111` (AMO - atomic operations)

### Current ALU Operations

The ALU currently uses 5-bit encoding and supports:
- RV32I: ADD, SUB, AND, OR, XOR, SLL, SRL, SRA, SLT, SLTU (10 ops)
- M extension: MUL, MULH, MULHSU, MULHU, DIV, DIVU, REM, REMU (8 ops)
- **Total: 18 operations** (uses codes 0-17 in 5-bit space)

**Available encoding space:** 5 bits support up to 32 operations, so we have room for 14 more.

---

## RTL Modifications Required

### 1. Top Module (`rtl/top.sv`)

The top module requires **significant changes** to support atomic operations due to the read-modify-write nature of AMO instructions.

#### A. Add Reservation Station for LR/SC

```systemverilog
// LR/SC reservation tracking
logic        reservation_valid;
logic [31:0] reservation_addr;

// Clear reservation on SC or any write to reserved address
always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        reservation_valid <= 1'b0;
        reservation_addr <= 32'd0;
    end else begin
        if (is_lr_instr) begin
            // Set reservation on LR.W
            reservation_valid <= 1'b1;
            reservation_addr <= dmem_addr;
        end else if (is_sc_instr || (dmem_write && reservation_valid && dmem_addr == reservation_addr)) begin
            // Clear reservation on SC or write to reserved address
            reservation_valid <= 1'b0;
        end
    end
end
```

#### B. Modify Data Memory Interface Signals

**Add new control signals:**
```systemverilog
// Atomic operation signals
logic        is_atomic_instr;   // Indicates AMO or LR/SC instruction
logic        is_lr_instr;       // Load-Reserved
logic        is_sc_instr;       // Store-Conditional
logic        is_amo_instr;      // Atomic Memory Operation
logic [31:0] amo_result;        // Result of AMO operation (to write back)
logic [31:0] atomic_wdata;      // Data to write for atomic operations
```

#### C. Implement Read-Modify-Write Logic for AMO

AMO instructions require reading memory, performing an operation, and writing back in a single instruction. For a single-cycle implementation:

**Option 1: Two-Phase Single Cycle (Recommended)**
```systemverilog
// Phase 1: Read from memory
// Phase 2: Compute and write back
// Both happen within the same clock cycle using combinational logic

always_comb begin
    if (is_amo_instr) begin
        // Read phase: dmem_rdata contains original value
        // Compute phase: ALU computes operation
        // Write phase: write amo_result back to same address
        atomic_wdata = amo_result;
        dmem_write = 1'b1;
        dmem_addr = rs1_data;  // Address from rs1
    end
end
```

**Option 2: Multi-Cycle State Machine (More Realistic)**
For better realism, AMO could use a 2-cycle implementation:
- Cycle 1: Read from memory
- Cycle 2: Compute and write back

However, this violates the single-cycle design principle. For consistency, we'll use **Option 1** with the understanding that this is a simplified model.

#### D. Update Write Data Multiplexer

```systemverilog
// Data memory write data selection
always_comb begin
    if (is_amo_instr) begin
        dmem_wdata = atomic_wdata;  // AMO computed result
    end else if (is_sc_instr) begin
        dmem_wdata = rs2_data;       // SC.W stores rs2
    end else begin
        dmem_wdata = store_data;     // Normal store data
    end
end
```

#### E. Update Result Selection for Register Write

```systemverilog
// Write-back data selection
always_comb begin
    case (wb_sel)
        WB_ALU: wr_data = alu_result;
        WB_MEM: wr_data = dmem_rdata;
        WB_PC4: wr_data = pc + 4;
        WB_IMM: wr_data = imm_u;
        WB_CSR: wr_data = csr_rdata;
        WB_AMO: wr_data = dmem_rdata;  // AMO returns original value
        WB_SC:  wr_data = sc_result;   // SC returns 0 (success) or 1 (failure)
        default: wr_data = 32'd0;
    endcase
end
```

### 2. Decoder Module (`rtl/decoder.sv`)

#### A. Add AMO Opcode Recognition

```systemverilog
// Opcode definitions
localparam logic [6:0] OP_AMO = 7'b0101111;  // Atomic operations

// Inside decoder main logic
case (opcode)
    // ... existing opcodes ...
    
    OP_AMO: begin
        // Atomic operations
        alu_src = 1'b0;           // Use rs2 (though address from rs1)
        reg_write = 1'b1;         // Write result to rd
        mem_read = 1'b1;          // Read from memory
        is_atomic = 1'b1;         // Flag as atomic
        
        // Decode specific atomic operation
        case (funct5)
            5'b00010: begin  // LR.W
                is_lr = 1'b1;
                atomic_op = ATOMIC_LR;
            end
            5'b00011: begin  // SC.W
                is_sc = 1'b1;
                atomic_op = ATOMIC_SC;
            end
            5'b00001: atomic_op = ATOMIC_SWAP;   // AMOSWAP.W
            5'b00000: atomic_op = ATOMIC_ADD;    // AMOADD.W
            5'b00100: atomic_op = ATOMIC_XOR;    // AMOXOR.W
            5'b01100: atomic_op = ATOMIC_AND;    // AMOAND.W
            5'b01000: atomic_op = ATOMIC_OR;     // AMOOR.W
            5'b10000: atomic_op = ATOMIC_MIN;    // AMOMIN.W
            5'b10100: atomic_op = ATOMIC_MAX;    // AMOMAX.W
            5'b11000: atomic_op = ATOMIC_MINU;   // AMOMINU.W
            5'b11100: atomic_op = ATOMIC_MAXU;   // AMOMAXU.W
            default: atomic_op = ATOMIC_SWAP;
        endcase
    end
    
    // ... rest of opcodes ...
endcase
```

---

## Memory Interface Changes

### External Memory Requirements

The external memory (testbench) must support atomic operations properly.

**For single-cycle implementation with simplified atomic operations:**
- Testbench must track LR/SC reservations
- AMO instructions read and write in same cycle
- All atomic operations must be word-aligned

---

## Testing Strategy

### Comprehensive Test Plan

#### CPU Integration Tests (`tests/src/cpu_test.rs`)

**New test functions (8-10 tests):**

1. `test_cpu_lr_sc_success()` - Successful LR/SC sequence
2. `test_cpu_lr_sc_failure()` - Failed LR/SC (reservation broken)
3. `test_cpu_lr_sc_intervening_write()` - Intervening write breaks reservation
4. `test_cpu_amo_operations()` - All AMO instructions
5. `test_cpu_amo_min_max()` - Min/max operations
6. `test_cpu_atomic_counter()` - Atomic counter using AMOADD
7. `test_cpu_atomic_lock()` - Spinlock using LR/SC
8. `test_cpu_atomic_alignment()` - Alignment checks

---

## Implementation Phases

### Phase 1: RTL Implementation - Decoder (2-3 days)
- Update `rtl/decoder.sv` with AMO opcode support
- Add atomic operation decoding
- Lint and test

### Phase 2: RTL Implementation - ALU (2-3 days)
- Add atomic ALU operations
- Implement min/max logic
- Test ALU operations

### Phase 3: RTL Implementation - Top Module (3-4 days)  
- Add reservation station for LR/SC
- Implement AMO read-modify-write logic
- Update write-back multiplexers
- Comprehensive integration

### Phase 4: Memory Testbench Support (2-3 days)
- Add reservation tracking
- Implement LR/SC semantics
- Add AMO support in testbench

### Phase 5: Integration Testing (3-4 days)
- Create CPU-level atomic tests
- Test LR/SC sequences
- Test all AMO operations
- Regression testing

### Phase 6: System-Level Testing (2-3 days)
- Create assembly test programs
- Create Rust test programs
- Test with CPU simulator

### Phase 7: Build Configuration Updates (1-2 days)
- Update to RV32IMA target
- Update CI/CD pipelines
- Update documentation

### Phase 8: Final Validation (1-2 days)
- Complete test suite
- Code quality checks
- CI pipeline verification
- Documentation finalization

**Total: 16-24 days**

---

## Risk Assessment

### High-Risk Areas

1. **Read-Modify-Write Atomicity in Single-Cycle Design**
   - Risk: Architectural challenge
   - Mitigation: Document as simplified model

2. **LR/SC Reservation Tracking Complexity**
   - Risk: Complex state management
   - Mitigation: Comprehensive testing, clear specification

3. **Memory Interface Changes Breaking Existing Tests**
   - Risk: Regression in existing functionality
   - Mitigation: Careful testing, separate atomic signals

---

## Validation Criteria

### Functional Validation
- [ ] All 11 atomic instructions decode correctly
- [ ] LR.W sets reservation correctly
- [ ] SC.W validates reservation correctly
- [ ] AMO operations compute correctly
- [ ] Reservation invalidation works properly

### Quality Validation
- [ ] All code quality checks pass
- [ ] Test count increases to 95+ tests
- [ ] All existing tests pass (no regressions)
- [ ] Documentation complete

### CI/CD Validation
- [ ] GitHub Actions CI passes
- [ ] All 95+ tests pass
- [ ] Build, format, clippy checks pass

---

## Appendices

### Appendix A: RISC-V A Extension Reference

**Instruction Encoding Summary:**

| Instruction | funct5 | Opcode | Description |
|-------------|--------|--------|-------------|
| LR.W        | 00010  | 0101111 | Load-Reserved Word |
| SC.W        | 00011  | 0101111 | Store-Conditional Word |
| AMOSWAP.W   | 00001  | 0101111 | Atomic Swap |
| AMOADD.W    | 00000  | 0101111 | Atomic Add |
| AMOXOR.W    | 00100  | 0101111 | Atomic XOR |
| AMOAND.W    | 01100  | 0101111 | Atomic AND |
| AMOOR.W     | 01000  | 0101111 | Atomic OR |
| AMOMIN.W    | 10000  | 0101111 | Atomic Min (signed) |
| AMOMAX.W    | 10100  | 0101111 | Atomic Max (signed) |
| AMOMINU.W   | 11000  | 0101111 | Atomic Min (unsigned) |
| AMOMAXU.W   | 11100  | 0101111 | Atomic Max (unsigned) |

### Appendix B: Resources

- [RISC-V Unprivileged ISA Specification](https://riscv.org/technical/specifications/) - Chapter 8: A Extension
- [RISC-V Compliance Test Suite](https://github.com/riscv-non-isa/riscv-arch-test)
- [Spike RISC-V ISA Simulator](https://github.com/riscv-software-src/riscv-isa-sim)

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2025-12-31 | GitHub Copilot | Initial draft |

---

**Document Status:** ✅ **Ready for Implementation**

This plan provides a comprehensive roadmap for adding RV32A atomic instruction support. The implementation will transform the CPU from RV32IM to RV32IMA, enabling lock-free synchronization primitives and atomic memory operations essential for concurrent programming.

**Key Takeaways:**
- 11 new atomic instructions (LR/SC + 9 AMO operations)
- Significant architectural changes required (reservation station, read-modify-write)
- Single-cycle implementation is simplified model
- Comprehensive testing critical for correctness
- 16-24 day estimated implementation timeline
