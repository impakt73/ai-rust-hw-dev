# RV32A Atomic Extension Implementation Plan

## Executive Summary

This document outlines a comprehensive plan to add the **RV32A (Atomic Instructions)** extension to the current **RV32IMC** multi-cycle non-pipelined RISC-V CPU implementation. The A extension provides atomic read-modify-write operations and load-reserved/store-conditional primitives essential for lock-free synchronization and multi-processor systems. This upgrade will transform the CPU from **RV32IMC** to **RV32IMAC**, adding 11 new atomic instructions.

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

**For multi-cycle non-pipelined, single-hart implementation:**
- Memory ordering is naturally satisfied (all operations are sequential and non-overlapping)
- aq/rl bits can be ignored in RTL (but must be preserved in encoding)
- Important for future multi-hart or pipelined implementations

### Special Cases and Edge Conditions

#### Alignment Requirements
- All atomic operations must be **word-aligned** (address[1:0] = 00)
- **This multi-cycle core treats misaligned atomic accesses as undefined behavior**:
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
- **Recommended for multi-cycle**: Exact word matching for simplicity

---

## Current Architecture Analysis

### Existing RTL Modules

```
top.sv (CPU top-level with 11-state FSM)
├── fetch_buffer.sv (RV32C fetch buffer - manages compressed instruction alignment)
├── decompress.sv (RV32C instruction decompressor - combinational)
├── decoder.sv (Instruction decoder)
├── alu.sv (ALU - arithmetic/logic/shift/multiply/divide operations)
│   └── div_unit.sv (Hardware division unit with multi-cycle operation)
├── regfile.sv (32×32-bit register file)
├── csr_file.sv (Control and Status Registers - Zicsr extension)
├── branch_unit.sv (Branch comparison logic)
├── mem_interface.sv (Memory interface logic)
└── writeback_mux.sv (Result selection for register writeback)
```

### Current Memory Interface

**Multi-Cycle Architecture:**
The CPU uses an 11-state FSM (S_IDLE, S_FETCH, S_DECODE, S_EXECUTE, S_MEM_ADDR, S_MEM_READ, S_MEM_WRITE, S_WRITEBACK, S_BRANCH, S_CSR, S_HALT) with variable-latency memory support via ready/valid handshaking.

**Instruction Memory (Read-Only with Handshaking):**
```systemverilog
output logic [31:0] imem_addr,    // Instruction address
input  logic [31:0] imem_data,    // Instruction data
output logic        imem_req,     // Request instruction fetch
input  logic        imem_ready    // Memory has valid data
```

**Data Memory (Read/Write with Handshaking):**
```systemverilog
output logic [31:0] dmem_addr,    // Data address
output logic        dmem_we,      // Write enable
output logic        dmem_re,      // Read enable
output logic [1:0]  dmem_size,    // Operation size: 00=byte, 01=halfword, 10=word
output logic [31:0] dmem_wdata,   // Write data
input  logic [31:0] dmem_rdata,   // Read data
output logic        dmem_req,     // Request data memory operation
input  logic        dmem_ready    // Memory operation complete
```

### Current Decoder Logic

The decoder currently handles:
- RV32I base instructions (40 instructions: arithmetic, logic, loads, stores, branches, jumps)
- M extension (8 instructions: multiplication and division)
- C extension (27 compressed instructions via decompress.sv)
- Zicsr extension (6 CSR operations)

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
- `0001111`: FENCE
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

The top module requires **significant changes** to support atomic operations due to the read-modify-write nature of AMO instructions and the multi-cycle FSM architecture.

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
        if (is_lr_instr && current_state == S_MEM_READ && dmem_ready) begin
            // Set reservation on LR.W completion using staged rs1_data (from a_reg)
            reservation_valid <= 1'b1;
            reservation_addr <= a_reg;  // LR base address from staged operand
        end else if (is_sc_instr || (dmem_we && reservation_valid && dmem_addr == reservation_addr)) begin
            // Clear reservation on SC or write to reserved address
            reservation_valid <= 1'b0;
        end
    end
end
```

#### B. Add New FSM States for Atomic Operations

**New states required:**
```systemverilog
typedef enum logic [3:0] {
    S_IDLE       = 4'b0000,  // After reset
    S_FETCH      = 4'b0001,  // Fetch instruction (wait for imem_ready)
    S_DECODE     = 4'b0010,  // Decode and read registers
    S_EXECUTE    = 4'b0011,  // ALU operation
    S_MEM_ADDR   = 4'b0100,  // Calculate memory address
    S_MEM_READ   = 4'b0101,  // Load from memory (wait for dmem_ready)
    S_MEM_WRITE  = 4'b0110,  // Store to memory (wait for dmem_ready)
    S_WRITEBACK  = 4'b0111,  // Write result to register
    S_BRANCH     = 4'b1000,  // Branch decision
    S_CSR        = 4'b1001,  // CSR operation
    S_HALT       = 4'b1010,  // ECALL/EBREAK
    S_ATOMIC_RMW = 4'b1011   // NEW: Atomic read-modify-write second phase
} state_t;
```

**Atomic instruction flow:**
- **LR.W**: S_FETCH → S_DECODE → S_MEM_ADDR → S_MEM_READ (set reservation) → S_WRITEBACK
- **SC.W**: S_FETCH → S_DECODE → S_MEM_ADDR → S_MEM_WRITE (check reservation) → S_WRITEBACK
- **AMO**: S_FETCH → S_DECODE → S_MEM_ADDR → S_MEM_READ → S_ATOMIC_RMW (compute & write) → S_WRITEBACK

#### C. Modify Data Memory Interface Signals

**Add new control signals:**
```systemverilog
// Atomic operation signals
logic        is_atomic_instr;   // Indicates AMO or LR/SC instruction
logic        is_lr_instr;       // Load-Reserved
logic        is_sc_instr;       // Store-Conditional
logic        is_amo_instr;      // Atomic Memory Operation
logic [31:0] amo_result;        // Result of AMO operation (to write back)
logic [31:0] atomic_wdata;      // Data to write for atomic operations
logic        sc_success;        // SC success/failure flag
```

#### D. Implement Read-Modify-Write Logic for AMO

AMO instructions require reading memory, performing an operation, and writing back across multiple cycles.

**Multi-Cycle AMO Sequence:**
1. **S_MEM_ADDR**: Calculate address (from rs1, staged in a_reg)
2. **S_MEM_READ**: Read original value from memory, wait for dmem_ready
3. **S_ATOMIC_RMW**: Compute new value using ALU (original_value OP rs2_data), initiate write
4. **S_WRITEBACK**: Write original value to rd

**Implementation Approach:**
```systemverilog
// In S_MEM_READ state for AMO:
// - Original value arrives via dmem_rdata (latch into mdr)
// In S_ATOMIC_RMW state for AMO:
// - Configure ALU with mdr (original) and b_reg (rs2)
// - Wait for alu_ready
// - Write alu_result to memory via dmem_wdata
// - Assert dmem_req and dmem_we
// In S_WRITEBACK state for AMO:
// - Write mdr (original value) to rd via register file
```

**Note:** Unlike single-cycle designs, the multi-cycle architecture naturally supports the read-modify-write sequence through the FSM state transitions.

#### E. Update Write Data Multiplexer

```systemverilog
// Data memory write data selection
always_comb begin
    if (is_amo_instr && current_state == S_ATOMIC_RMW) begin
        dmem_wdata = alu_out_reg;    // AMO computed result (after RMW)
    end else if (is_sc_instr) begin
        dmem_wdata = b_reg;          // SC.W stores rs2 (staged)
    end else begin
        dmem_wdata = b_reg;          // Normal store data (rs2 staged)
    end
end
```

#### F. Update Result Selection for Register Write

The writeback_mux.sv module should be extended to handle atomic instruction results:

```systemverilog
// Write-back data selection (in writeback_mux.sv or top.sv)
always_comb begin
    if (is_amo_instr) begin
        wr_data = mdr;                 // AMO returns original value from memory
    end else if (is_sc_instr) begin
        wr_data = {31'b0, ~sc_success}; // SC returns 0 (success) or 1 (failure)
    end else if (mem_to_reg) begin
        wr_data = mdr;                 // Normal load (formatted)
    end else if (is_csr_instr) begin
        wr_data = csr_rdata_reg;       // CSR read
    end else begin
        wr_data = alu_out_reg;         // ALU result (default)
    end
end
```

#### G. Update FSM Transitions for Atomic Instructions

**Decoder output additions:**
```systemverilog
// New decoder outputs needed:
output logic is_lr,      // LR.W instruction
output logic is_sc,      // SC.W instruction  
output logic is_amo,     // AMO instruction
output logic [4:0] funct5 // For atomic operation type
```

**FSM transition logic:**
```systemverilog
S_DECODE: begin
    if (is_amo_instr) begin
        next_state = S_MEM_ADDR;  // AMO: calculate address
    end else if (is_lr_instr || is_sc_instr) begin
        next_state = S_MEM_ADDR;  // LR/SC: calculate address
    end
    // ... other instruction types
end

S_MEM_READ: begin
    if (dmem_ready) begin
        if (is_amo_instr) begin
            next_state = S_ATOMIC_RMW;  // AMO: proceed to RMW
        end else if (is_lr_instr) begin
            next_state = S_WRITEBACK;   // LR: complete after read
        end else begin
            next_state = S_WRITEBACK;   // Normal load
        end
    end
end

S_ATOMIC_RMW: begin
    if (alu_ready && dmem_ready) begin  // Wait for ALU and memory
        next_state = S_WRITEBACK;  // Write original value to rd
    end
end

S_MEM_WRITE: begin
    if (dmem_ready) begin
        if (is_sc_instr) begin
            next_state = S_WRITEBACK;  // SC: write success/failure to rd
        end else begin
            next_state = S_FETCH;      // Normal store: done
        end
    end
end
```

### 2. Decoder Module (`rtl/decoder.sv`)

#### A. Add AMO Opcode Recognition

```systemverilog
// Opcode definitions (add to existing list)
localparam logic [6:0] OP_AMO = 7'b0101111;  // Atomic operations

// New outputs needed
output logic        is_lr,       // Load-Reserved instruction
output logic        is_sc,       // Store-Conditional instruction  
output logic        is_amo,      // Atomic Memory Operation
output logic [4:0]  funct5       // For atomic operation type (bits [31:27])

// Extract funct5 from instruction
assign funct5 = instruction[31:27];

// Inside decoder main logic
case (opcode)
    // ... existing opcodes ...
    
    OP_AMO: begin
        // Atomic operations (all require memory access)
        alu_src = 1'b1;           // Use immediate (zero offset from rs1)
        reg_write = 1'b1;         // Write result to rd
        mem_read = 1'b1;          // All atomics read from memory
        
        // Decode specific atomic operation
        case (funct5)
            5'b00010: begin  // LR.W
                is_lr = 1'b1;
                mem_write = 1'b0;     // LR only reads
            end
            5'b00011: begin  // SC.W
                is_sc = 1'b1;
                mem_write = 1'b1;     // SC conditionally writes
            end
            5'b00001: begin          // AMOSWAP.W
                is_amo = 1'b1;
                mem_write = 1'b1;     // AMO reads and writes
            end
            5'b00000: begin          // AMOADD.W
                is_amo = 1'b1;
                mem_write = 1'b1;
            end
            5'b00100: begin          // AMOXOR.W
                is_amo = 1'b1;
                mem_write = 1'b1;
            end
            5'b01100: begin          // AMOAND.W
                is_amo = 1'b1;
                mem_write = 1'b1;
            end
            5'b01000: begin          // AMOOR.W
                is_amo = 1'b1;
                mem_write = 1'b1;
            end
            5'b10000: begin          // AMOMIN.W
                is_amo = 1'b1;
                mem_write = 1'b1;
            end
            5'b10100: begin          // AMOMAX.W
                is_amo = 1'b1;
                mem_write = 1'b1;
            end
            5'b11000: begin          // AMOMINU.W
                is_amo = 1'b1;
                mem_write = 1'b1;
            end
            5'b11100: begin          // AMOMAXU.W
                is_amo = 1'b1;
                mem_write = 1'b1;
            end
            default: begin
                is_amo = 1'b1;
                mem_write = 1'b1;
            end
        endcase
    end
    
    // ... rest of opcodes ...
endcase
```

#### B. ALU Operation Mapping for AMO

The ALU operation for AMO instructions depends on the funct5 field:
- Use existing ALU operations where possible (ADD, XOR, AND, OR)
- Add new ALU operations for MIN/MAX variants

**Mapping:**
- AMOSWAP: No ALU operation needed (direct data path)
- AMOADD: ALU_ADD
- AMOXOR: ALU_XOR
- AMOAND: ALU_AND
- AMOOR: ALU_OR
- AMOMIN: ALU_MIN (new)
- AMOMAX: ALU_MAX (new)
- AMOMINU: ALU_MINU (new)
- AMOMAXU: ALU_MAXU (new)

---

## Memory Interface Changes

### External Memory Requirements

The external memory (testbench/simulator) must support atomic operations properly within the multi-cycle architecture.

**For multi-cycle implementation with atomic operations:**
- Memory model must track LR/SC reservations per hart
- AMO instructions read and write across multiple cycles (S_MEM_READ → S_ATOMIC_RMW states)
- Memory must maintain atomicity guarantees even with multi-cycle access
- All atomic operations must be word-aligned
- Ready/valid handshaking must be respected for both read and write phases

**Memory Model Extensions Required:**

1. **LR/SC Reservation Tracking:**
   - Maintain reservation_valid flag and reservation_addr per hart
   - On LR.W (when dmem_req && dmem_re && is_lr): Record address, set reservation_valid
   - On SC.W (when dmem_req && dmem_we && is_sc): Check reservation, write conditionally
   - Clear reservation on: Any SC.W, any write to reserved address

2. **AMO Atomicity Guarantee:**
   - Treat multi-cycle AMO sequence as atomic transaction
   - Read phase (S_MEM_READ): Return original value via dmem_rdata
   - Write phase (S_ATOMIC_RMW): Accept new value via dmem_wdata
   - Prevent interleaving with other accesses to same word

3. **Handshaking Protocol:**
   - Assert dmem_ready when operation completes
   - Support variable latency (configurable delay)
   - Respect dmem_req as transaction start signal

---

## Testing Strategy

### Comprehensive Test Plan

The project currently has **210 tests** across all packages (86 in cpu-sim, 63 in tests/cpu_verifier, 33 in riscv_core, 6 in riscv_protocol, 13 in riscv_macros, 9 in RV32C integration).

#### CPU Integration Tests (`cpu-sim/src/test_rtl_verification.rs`)

**Note:** CPU integration tests have been migrated from `tests/src/cpu_test.rs` to `cpu-sim/src/test_rtl_verification.rs` for better infrastructure (SystemBus, VCD dumps, instruction tracing).

**New test functions to add (8-10 tests):**

1. `test_cpu_lr_sc_success()` - Successful LR/SC sequence
2. `test_cpu_lr_sc_failure_double_lr()` - Failed LR/SC (reservation broken by second LR)
3. `test_cpu_lr_sc_failure_store()` - Failed LR/SC (reservation broken by intervening store)
4. `test_cpu_amo_swap()` - AMOSWAP.W operation
5. `test_cpu_amo_add()` - AMOADD.W operation
6. `test_cpu_amo_logical()` - AMOXOR, AMOAND, AMOOR operations
7. `test_cpu_amo_min_max()` - AMOMIN, AMOMAX operations (signed)
8. `test_cpu_amo_minu_maxu()` - AMOMINU, AMOMAXU operations (unsigned)
9. `test_cpu_atomic_counter()` - Atomic counter using AMOADD
10. `test_cpu_atomic_lock()` - Spinlock using LR/SC

**Expected test count after implementation:** ~220 tests (current 210 + 10 new atomic tests)

---

## Implementation Phases

### Phase 1: RTL Implementation - Decoder (2-3 days)
- Update `rtl/decoder.sv` with AMO opcode support
- Add funct5 field extraction
- Add is_lr, is_sc, is_amo decoder outputs
- Add atomic operation decoding
- Lint with `verilator --lint-only rtl/decoder.sv`
- Test decoder in isolation if possible

### Phase 2: RTL Implementation - ALU (2-3 days)
- Add new ALU opcodes for min/max AMOs: `ALU_MIN`, `ALU_MAX`, `ALU_MINU`, `ALU_MAXU`
- Update `rtl/alu.sv` with min/max comparison logic
- Reuse existing ALU operations (ADD, XOR, AND, OR) for other AMOs
- LR/SC require no new ALU operations
- Test ALU min/max operations with unit tests in `tests/src/alu_test.rs`

### Phase 3: RTL Implementation - Top Module (4-5 days)  
- Add reservation station for LR/SC (flip-flops for valid flag and address)
- Add new FSM state S_ATOMIC_RMW for AMO write-back phase
- Update FSM transition logic for atomic instructions
- Implement AMO read-modify-write sequence across states
- Update write-back multiplexer logic (in writeback_mux.sv or top.sv)
- Add control signals for atomic operations
- Comprehensive integration testing
- Lint with `verilator --lint-only rtl/top.sv`

### Phase 4: Memory Testbench Support (3-4 days)
- **Extend Rust/marlin memory model** in cpu-sim:
  - Add reservation tracking in SystemBus or DRAM model
  - Implement LR.W: Record reserved address without modifying memory
  - Implement SC.W: Check reservation, write conditionally, return success/failure
  - Invalidate reservation on any write to reserved word
  - Model LR/SC as atomic with respect to all testbench memory accesses
- **Add AMO support** in memory model:
  - Recognize AMO access pattern (read in S_MEM_READ, write in S_ATOMIC_RMW)
  - Perform atomic read-modify-write transaction
  - Return original word value on read, accept new value on write
  - Handle read-before-write ordering correctly for same-cycle access
  - Maintain atomicity guarantees across multi-cycle sequence
- **Extend marlin/Verilator interface:**
  - Keep existing interface, extend Rust adapter logic
  - Add state tracking for multi-phase atomic operations
  - Ensure proper handshaking with dmem_req/dmem_ready

### Phase 5: Integration Testing (4-5 days)
- Create CPU-level atomic tests in `cpu-sim/src/test_rtl_verification.rs`
- Test LR/SC sequences (success and failure cases)
- Test all 9 AMO operations individually
- Test atomic counter and spinlock patterns
- Verify multi-cycle FSM transitions with VCD dumps
- Regression testing: ensure all 210 existing tests still pass
- Run with variable memory latency to stress-test FSM

### Phase 6: System-Level Testing (2-3 days)
- Create assembly test programs for atomic operations
- Create Rust test programs using atomic primitives
- Test with CPU simulator (cpu-sim)
- Verify instruction trace output for atomic operations
- Test with VCD waveform dumps for debugging

### Phase 7: Build Configuration Updates (1-2 days)
- Update build targets to RV32IMAC:
  - Adjust `Cargo.toml` feature flags if needed
  - Update test program build configuration (assembly toolchain flags)
  - Verify Verilator build configuration
- Update CI/CD pipelines:
  - Ensure atomic tests run in GitHub Actions
  - Verify all 220+ tests pass in CI
- Update documentation:
  - Edit `AGENTS.md` to reflect RV32IMAC support (currently states RV32IMC)
  - Update `README.md` if it mentions instruction set architecture
  - Update any architecture diagrams or tables

### Phase 8: Final Validation (1-2 days)
- Complete test suite execution (all 220+ tests)
- Code quality checks:
  - `cargo fmt -- --check`
  - `cargo clippy -- -D warnings`
  - `verilator --lint-only rtl/*.sv`
- CI pipeline verification (all checks must pass)
- Documentation finalization and review
- Performance regression check (ensure no significant slowdown)

**Total: 19-27 days**

---

## Risk Assessment

### High-Risk Areas

1. **Multi-Cycle FSM Complexity for AMO Operations**
   - Risk: Managing read-modify-write across 3 states (S_MEM_READ → S_ATOMIC_RMW → S_WRITEBACK)
   - Mitigation: Careful state machine design, comprehensive FSM testing, VCD waveform analysis

2. **LR/SC Reservation Tracking Across Cycles**
   - Risk: Reservation invalidation timing, ensuring atomicity in multi-cycle architecture
   - Mitigation: Clear specification, comprehensive testing, edge case analysis

3. **Memory Interface Changes Breaking Existing Tests**
   - Risk: Regression in existing 210 tests due to memory model changes
   - Mitigation: Careful memory model extension, maintain backward compatibility, run full regression suite

4. **ALU Datapath Conflicts for AMO**
   - Risk: ALU must compute both address and operation value in multi-cycle sequence
   - Mitigation: Use staging registers (a_reg, b_reg, mdr) to hold intermediate values across states

5. **Ready/Valid Handshaking Timing**
   - Risk: Incorrect FSM transitions if ready signals not properly handled
   - Mitigation: Review existing memory handshaking code, follow established patterns

---

## Validation Criteria

### Functional Validation
- [ ] All 11 atomic instructions decode correctly
- [ ] LR.W sets reservation correctly (verified via internal signal inspection)
- [ ] SC.W validates reservation correctly (success and failure cases)
- [ ] SC.W returns correct value to rd (0 for success, 1 for failure)
- [ ] AMO operations compute correctly (all 9 operations)
- [ ] AMO operations return original value to rd
- [ ] Reservation invalidation works properly (on SC, on write to reserved address)
- [ ] FSM transitions correctly for all atomic instruction types
- [ ] Multi-cycle sequence completes correctly (verified with VCD dumps)
- [ ] Ready/valid handshaking works with variable memory latency

### Quality Validation
- [ ] All code quality checks pass:
  - [ ] `cargo fmt -- --check`
  - [ ] `cargo clippy -- -D warnings`
  - [ ] `verilator --lint-only rtl/*.sv`
- [ ] Test suite expanded to ~220 tests (current 210 + 10 new atomic tests)
- [ ] All existing tests pass (no regressions)
- [ ] New atomic tests cover all 11 instructions
- [ ] Documentation complete and accurate

### CI/CD Validation
- [ ] GitHub Actions CI passes all checks
- [ ] All tests (existing and new) pass in CI
- [ ] Build succeeds with no warnings
- [ ] Format and clippy checks pass (blocking)

### Performance Validation
- [ ] No significant regression in test execution time
- [ ] Multi-cycle atomic operations complete within expected cycle counts
- [ ] VCD dumps confirm correct timing and state transitions

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
| 1.1 | 2026-01-11 | GitHub Copilot | Updated for multi-cycle architecture, RV32IMC base, corrected test counts, added FSM details, updated memory interface with ready/valid handshaking |

---

**Document Status:** ✅ **Updated for Multi-Cycle Architecture - Ready for Implementation**

This plan provides a comprehensive roadmap for adding RV32A atomic instruction support to the multi-cycle non-pipelined RV32IMC CPU. The implementation will transform the CPU from RV32IMC to RV32IMAC, enabling lock-free synchronization primitives and atomic memory operations essential for concurrent programming.

**Key Takeaways:**
- 11 new atomic instructions (LR/SC + 9 AMO operations)
- Significant architectural changes required (reservation station, multi-cycle read-modify-write via FSM)
- New FSM state (S_ATOMIC_RMW) for AMO write-back phase
- Multi-cycle implementation naturally supports atomic operation sequencing
- Memory model extensions needed for atomicity guarantees across cycles
- Comprehensive testing critical for correctness (~220 tests after implementation)
- 19-27 day estimated implementation timeline

**Architecture Compatibility:**
- Current: RV32IMC (multi-cycle, 11-state FSM, 210 tests)
- Target: RV32IMAC (multi-cycle, 12-state FSM, ~220 tests)
- Maintains backward compatibility with existing instruction set
