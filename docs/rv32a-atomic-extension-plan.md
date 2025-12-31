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
- **rs2**: Source data register (for AMO) or **must be 0 for LR.W**
- **rd**: Destination register

**Note:** For LR.W, the rs2 field is reserved and must be set to `00000`.

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
mem_addr = rs1_data                    // Address from rs1 register
if (reservation_addr == mem_addr && reservation_valid) {
    mem[mem_addr] = rs2_data           // Store word if reservation is still valid
    rd = 0                             // Success
    clear_reservation()
} else {
    rd = 1                             // Failure (non-zero)
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
- **This single-cycle core treats misaligned atomic accesses as undefined behavior**:
  - Software must not generate misaligned atomic accesses
  - The RTL implementation is **not required** to detect or trap misaligned atomic accesses; they may read/write incorrect data without raising an exception
  - Implementers may optionally add simulation-time assertions to flag misaligned atomic accesses during verification

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
output logic        dmem_we,      // Write enable
output logic        dmem_re,      // Read enable
output logic [3:0]  dmem_be,      // Byte enable (one bit per byte)
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

**Available encoding space:** 5 bits support up to 32 operations, so we have room for 14 more. This is sufficient for the RV32A atomic operations, so no ALU encoding width expansion is required.

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
            // Set reservation on LR.W using rs1_data (LR base address)
            reservation_valid <= 1'b1;
            reservation_addr <= rs1_data;
        end else if (is_sc_instr || (dmem_we && reservation_valid && alu_result == reservation_addr)) begin
            // Clear reservation on SC or write to reserved address
            // Note: alu_result is used here as the address being accessed
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

AMO instructions require reading memory, performing an operation, and writing back in a single instruction. 

**Architectural Challenge:** In the current design, `dmem_addr` is derived from `alu_result` (see top.sv line 199), but AMO operations require using rs1 directly as the address while the ALU performs the atomic operation. This creates a conflict: the ALU needs to compute the AMO result (e.g., ADD for AMOADD.W), but the address calculation also uses the ALU result.

**Recommended Solution: Bypass ALU for AMO Address Calculation**
```systemverilog
// For AMO instructions, use rs1_data directly as address (bypass ALU)
// The ALU is used only to compute the new value for AMO operations
always_comb begin
    if (is_amo_instr) begin
        // AMO uses rs1 directly for address (no ALU computation for address)
        dmem_addr_override = rs1_data;
        // ALU computes: original_value OP rs2_data
        // dmem_rdata provides original value, ALU computes new value
        atomic_wdata = alu_result;  // New value from ALU
        dmem_we = 1'b1;
    end
end

// Address selection: use rs1 directly for AMO, otherwise use alu_result
assign dmem_addr = is_amo_instr ? rs1_data : alu_result;
```

**Note:** This is a simplified single-cycle model. Real hardware would use multi-cycle or pipelined implementation with cache coherency protocols.

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

**Note:** The current top.sv uses conditional logic based on opcode types, not a case statement with enumerated WB_* selectors. The write-back logic should be extended using the existing architectural pattern:

```systemverilog
// Write-back data selection (extending existing pattern in top.sv)
always_comb begin
    if (is_amo_instr) begin
        wr_data = dmem_rdata;          // AMO returns original value from memory
    end else if (is_sc_instr) begin
        wr_data = sc_result;           // SC returns 0 (success) or 1 (failure)
    end else if (mem_to_reg) begin
        wr_data = formatted_load_data; // Normal load
    end else if (is_csr_instr) begin
        wr_data = csr_rdata;           // CSR read
    end else begin
        wr_data = alu_result;          // ALU result (default)
    end
end
```

Alternatively, the write-back logic could be refactored to use a case statement with explicit selectors.

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
                mem_write = 1'b0;     // LR only reads
            end
            5'b00011: begin  // SC.W
                is_sc = 1'b1;
                atomic_op = ATOMIC_SC;
                mem_write = 1'b1;     // SC writes (conditionally)
            end
            5'b00001: begin          // AMOSWAP.W
                atomic_op = ATOMIC_SWAP;
                mem_write = 1'b1;     // AMO reads and writes
            end
            5'b00000: begin          // AMOADD.W
                atomic_op = ATOMIC_ADD;
                mem_write = 1'b1;
            end
            5'b00100: begin          // AMOXOR.W
                atomic_op = ATOMIC_XOR;
                mem_write = 1'b1;
            end
            5'b01100: begin          // AMOAND.W
                atomic_op = ATOMIC_AND;
                mem_write = 1'b1;
            end
            5'b01000: begin          // AMOOR.W
                atomic_op = ATOMIC_OR;
                mem_write = 1'b1;
            end
            5'b10000: begin          // AMOMIN.W
                atomic_op = ATOMIC_MIN;
                mem_write = 1'b1;
            end
            5'b10100: begin          // AMOMAX.W
                atomic_op = ATOMIC_MAX;
                mem_write = 1'b1;
            end
            5'b11000: begin          // AMOMINU.W
                atomic_op = ATOMIC_MINU;
                mem_write = 1'b1;
            end
            5'b11100: begin          // AMOMAXU.W
                atomic_op = ATOMIC_MAXU;
                mem_write = 1'b1;
            end
            default: begin
                atomic_op = ATOMIC_SWAP;
                mem_write = 1'b1;
            end
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
2. `test_cpu_lr_sc_failure()` - Failed LR/SC (reservation broken by second LR)
3. `test_cpu_lr_sc_intervening_write()` - **(Future multi-hart / external-agent)** Intervening write breaks reservation; not implementable on current single-hart core
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
- Add new ALU opcodes for min/max AMOs: `AMOMIN`, `AMOMAX`, `AMOMINU`, `AMOMAXU`
- Reuse existing ALU operations (ADD, XOR, AND, OR) for other AMOs; LR/SC require no new ALU operations
- Test ALU min/max operations and AMO integration

### Phase 3: RTL Implementation - Top Module (3-4 days)  
- Add reservation station for LR/SC
- Implement AMO read-modify-write logic
- Update write-back multiplexers
- Comprehensive integration

### Phase 4: Memory Testbench Support (2-3 days)
- Add reservation tracking  
  - Extend the Rust/marlin data-memory model to maintain a per-core reservation address and valid bit
  - On `LR.W`, record the reserved address without modifying memory contents
  - Invalidate the reservation on any write (store or AMO) to the reserved word address, regardless of source
- Implement LR/SC semantics  
  - On `SC.W`, check the reservation: if the reservation is valid and the address matches, perform the store and return success; otherwise, do **not** modify memory and return failure
  - Ensure that failed `SC.W` operations are side-effect free with respect to the data memory model
  - Model LR/SC as atomic with respect to all other testbench memory accesses observing the same word
- Add AMO support in testbench  
  - Update the data memory model used by tests to perform an atomic read-modify-write when an AMO access is detected
  - For each AMO: (1) read the current word, (2) compute the new value according to the AMO operation (swap/add/min/max/AND/OR/XOR), (3) write the new value back, and (4) return the original word to the core
  - Implement this as a single logical transaction so that, from the DUT's point of view, the read of the old value and the write of the new value happen in the same cycle with no tearing
  - Ensure the memory model correctly handles a read and write to the same address in one simulated cycle (read-before-write ordering inside the atomic operation)
  - Keep the existing marlin/Verilator memory interface, but extend the Rust-side adapter to recognize LR/SC/AMO accesses and apply the appropriate atomic semantics

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
- Update build targets to RV32IMA:
  - Adjust `Cargo.toml` target/feature specifications for all workspace members (e.g., `cpu-sim`, `riscv_core`, `tests`) to reflect RV32IMA
  - Update test program build configuration (e.g., assembly test Makefiles or build scripts) to use an RV32IMA-capable toolchain
  - Update any Verilator/simulation build flags or scripts that currently assume RV32IM
- Update CI/CD pipelines to run RV32IMA tests and builds
- Update documentation to reflect the RV32IM → RV32IMA upgrade:
  - Edit `AGENTS.md` (including the architecture description on line 7) to state RV32IMA support
  - Update `README.md` and any other top-level docs that describe the CPU as RV32IM

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
- [ ] Test suite expanded with 8-10 new atomic-focused test functions/modules (as described in the Testing Strategy), increasing the total test count above the current 84 tests
- [ ] All existing tests pass (no regressions)
- [ ] Documentation complete

### CI/CD Validation
- [ ] GitHub Actions CI passes
- [ ] All tests (existing and new) pass
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

- [RISC-V Unprivileged ISA Specification, Version 20191213, Chapter 8 "A Standard Extension for Atomic Instructions"](https://github.com/riscv/riscv-isa-manual/releases/download/Ratified-IMAFDQC/riscv-spec-20191213.pdf)
- [RISC-V Compliance Test Suite](https://github.com/riscv-non-isa/riscv-arch-test)
- [Spike RISC-V ISA Simulator v1.1.0](https://github.com/riscv-software-src/riscv-isa-sim/tree/v1.1.0)

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
