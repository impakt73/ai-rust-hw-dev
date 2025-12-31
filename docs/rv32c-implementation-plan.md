# RV32C Compressed Instruction Extension Implementation Plan

## Executive Summary

This document provides a comprehensive technical plan for adding **RV32C (Compressed Instruction Extension)** support to the existing single-cycle RISC-V RV32IM CPU implementation. The RV32C extension adds 16-bit compressed instructions that improve code density by 25-30% while maintaining full compatibility with the base 32-bit instruction set.

This plan is specifically optimized for implementation by AI coding agents and includes detailed RTL modifications, comprehensive testing strategies, and step-by-step implementation phases.

## Table of Contents

1. [Overview of RV32C Extension](#overview-of-rv32c-extension)
2. [Current Architecture Analysis](#current-architecture-analysis)
3. [High-Level Design Strategy](#high-level-design-strategy)
4. [RTL Modifications Required](#rtl-modifications-required)
5. [Instruction Decompression Logic](#instruction-decompression-logic)
6. [Testing Strategy](#testing-strategy)
7. [Implementation Phases](#implementation-phases)
8. [Validation Criteria](#validation-criteria)
9. [Risk Assessment](#risk-assessment)
10. [Appendices](#appendices)

---

## Overview of RV32C Extension

### What is RV32C?

**RV32C** is the RISC-V Compressed Instruction Extension that provides:
- **16-bit instruction encoding** (half the size of standard 32-bit instructions)
- **40+ compressed instructions** covering common operations
- **Code density improvement** of 25-30% for typical programs
- **100% backward compatibility** with RV32IM base ISA
- **Seamless mixing** of 16-bit and 32-bit instructions in the same program

### Key Characteristics

1. **Dual Instruction Width:**
   - Standard instructions: 32 bits (4 bytes aligned)
   - Compressed instructions: 16 bits (2 bytes aligned)
   - Instructions can start at any 2-byte boundary

2. **Encoding Recognition:**
   - Bits [1:0] determine instruction width
   - `00`, `01`, `10` → 16-bit compressed instruction
   - `11` → 32-bit standard instruction

3. **Instruction Decompression:**
   - All compressed instructions map to equivalent 32-bit instructions
   - Decompression happens transparently in hardware
   - Programmer sees only the base ISA execution model

### RV32C Instruction Formats

The C extension defines **9 instruction formats**:

```
CR (Register):     funct4 | rd/rs1 | rs2 | op
CI (Immediate):    funct3 | imm | rd/rs1 | imm | op
CSS (Stack Store): funct3 | imm | rs2 | op
CIW (Wide Imm):    funct3 | imm | rd' | op
CL (Load):         funct3 | imm | rs1' | imm | rd' | op
CS (Store):        funct3 | imm | rs1' | imm | rs2' | op
CA (Arithmetic):   funct6 | rd'/rs1' | funct2 | rs2' | op
CB (Branch):       funct3 | offset | rs1' | offset | op
CJ (Jump):         funct3 | jump target | op
```

Where:
- `rd'`, `rs1'`, `rs2'` are compressed register specifiers (x8-x15 only, 3 bits)
- Standard `rd`, `rs1`, `rs2` use full 5-bit register addresses

### Compressed Instructions Summary

**Total: 40+ instructions across multiple categories**

#### 1. Stack Operations (6 instructions)
- `C.LWSP` - Load word from stack (sp-relative)
- `C.SWSP` - Store word to stack (sp-relative)
- `C.ADDI16SP` - Adjust stack pointer by immediate
- `C.ADDI4SPN` - Add immediate to sp and write to rd'

#### 2. Register-Immediate Instructions (10 instructions)
- `C.LI` - Load immediate
- `C.LUI` - Load upper immediate
- `C.ADDI` - Add immediate
- `C.SLLI` - Shift left logical immediate
- `C.SRLI` - Shift right logical immediate
- `C.SRAI` - Shift right arithmetic immediate
- `C.ANDI` - AND immediate

#### 3. Register-Register Instructions (8 instructions)
- `C.MV` - Move (copy register)
- `C.ADD` - Add registers
- `C.SUB` - Subtract
- `C.XOR` - Exclusive OR
- `C.OR` - OR
- `C.AND` - AND

#### 4. Load/Store Instructions (6 instructions)
- `C.LW` - Load word (base+offset)
- `C.SW` - Store word (base+offset)

#### 5. Control Transfer Instructions (6 instructions)
- `C.J` - Jump
- `C.JAL` - Jump and link (RV32C only, not in RV64C)
- `C.JR` - Jump register
- `C.JALR` - Jump and link register
- `C.BEQZ` - Branch if equal to zero
- `C.BNEZ` - Branch if not equal to zero

#### 6. Special Instructions (4 instructions)
- `C.NOP` - No operation
- `C.EBREAK` - Environment break

---

## Current Architecture Analysis

### Existing CPU Architecture

The current RV32IM CPU is a **single-cycle design** with the following characteristics:

```
Current Architecture:
┌─────────────────────────────────────────────────────────────┐
│                         TOP MODULE                           │
│                                                              │
│  ┌──────┐      ┌─────────┐      ┌─────┐      ┌─────────┐  │
│  │  PC  │─────>│ IMEM    │─────>│DEC  │─────>│   ALU   │  │
│  └──────┘      │(extern) │      │ODER │      └─────────┘  │
│     │          └─────────┘      └─────┘            │       │
│     │                              │                │       │
│     │          ┌─────────┐      ┌─────────┐        │       │
│     └─────────>│ REGFILE │<─────│  DMEM   │<──────┘       │
│                └─────────┘      │(extern) │                │
│                                 └─────────┘                │
└─────────────────────────────────────────────────────────────┘

Features:
- Fixed 32-bit instruction width
- PC always increments by 4
- Instruction memory provides 32 bits per access
- All instructions execute in single cycle
```

### Key Limitations for RV32C

1. **Fixed Instruction Width Assumption:**
   - Current design assumes all instructions are 32 bits
   - PC increments by 4 unconditionally
   - No support for 16-bit instruction fetch

2. **Instruction Fetch Interface:**
   - `imem_addr` output: 32-bit word address
   - `imem_data` input: 32-bit instruction
   - Cannot fetch partial instructions (16 bits)

3. **PC Management:**
   - Simple sequential: `next_pc = pc + 4`
   - Branch/jump targets assume 4-byte alignment
   - No handling of 2-byte aligned addresses

### Integration Points

To add RV32C support, we need to modify:

1. **Instruction Fetch Unit** (new module)
   - Handle 16-bit and 32-bit instruction fetching
   - Manage PC alignment (2-byte boundaries)
   - Buffer partial instructions across word boundaries

2. **Instruction Decompressor** (new module)
   - Detect compressed vs. standard instructions
   - Expand 16-bit instructions to 32-bit equivalents
   - Pass through standard 32-bit instructions unchanged

3. **PC Update Logic** (modify `top.sv`)
   - Increment PC by 2 or 4 based on instruction width
   - Handle branch/jump targets at 2-byte alignment

4. **Decoder** (no changes required)
   - Receives standard 32-bit instructions after decompression
   - Existing logic handles all decompressed instructions

5. **ALU, RegFile** (no changes required)
   - Operate on decompressed 32-bit instructions
   - No awareness of compressed encoding

---

## High-Level Design Strategy

### Design Philosophy

The implementation follows the **"Decompression-First"** approach:

```
┌──────────────────────────────────────────────────────────────┐
│                    RV32C CPU Architecture                     │
│                                                               │
│  ┌──────┐    ┌──────────┐    ┌────────────┐   ┌─────────┐  │
│  │  PC  │───>│ I-Fetch  │───>│Decompressor│──>│ Decoder │  │
│  └──────┘    │  Unit    │    │ (16→32 bit)│   │(32-bit) │  │
│     ▲        └──────────┘    └────────────┘   └─────────┘  │
│     │              │                                  │      │
│     │              │                                  ▼      │
│     │              ▼                              ┌─────┐   │
│     │         ┌────────┐                         │ ALU │   │
│     │         │ IMEM   │                         └─────┘   │
│     │         │(extern)│                            │      │
│     │         └────────┘                            │      │
│     │                                               ▼      │
│     │                                          ┌─────────┐ │
│     └──────────────────────────────────────── │ RegFile │ │
│              PC update logic                   └─────────┘ │
│         (increment by 2 or 4)                              │
└──────────────────────────────────────────────────────────────┘
```

**Key Principles:**

1. **Transparent Decompression:**
   - Compressed instructions are expanded early in the pipeline
   - Rest of the CPU sees only standard 32-bit instructions
   - Minimal changes to existing RTL

2. **Modular Design:**
   - New modules are self-contained and testable
   - Clear interfaces between components
   - Easy to verify correctness in isolation

3. **Backward Compatibility:**
   - RV32IM-only programs continue to work without modification
   - No performance penalty for non-compressed code
   - Mixed compressed/standard code works seamlessly

### PC Management Strategy

The PC must handle 2-byte alignment:

```
Memory Layout Example:
Address    Content
0x0000:    [16-bit compressed instruction]
0x0002:    [16-bit compressed instruction]
0x0004:    [32-bit standard instruction   ]
0x0008:    [16-bit comp.][16-bit comp.]
0x000C:    [32-bit standard instruction   ]

PC Increment Rules:
- After 16-bit instruction: PC = PC + 2
- After 32-bit instruction: PC = PC + 4
- PC can be any 2-byte aligned address (even addresses only)
```

**Implementation Approach:**
- Add `is_compressed` signal from decompressor
- PC increment: `next_pc = pc + (is_compressed ? 2 : 4)`
- Branch/jump targets support 2-byte alignment

### Instruction Fetch Strategy

**Problem:** Memory interface provides 32 bits, but instructions can be 16 bits.

**Solution:** Instruction buffer with lookahead

```systemverilog
// Simplified fetch logic
always_ff @(posedge clk) begin
    if (pc[1] == 0) begin
        // PC is word-aligned: fetch new 32-bit word
        fetch_buffer <= imem_data;
        current_insn <= imem_data[15:0];  // Lower half
        buffered_insn <= imem_data[31:16]; // Upper half
    end else begin
        // PC is half-word aligned: use buffered data
        current_insn <= buffered_insn;
        // Fetch next word for lookahead
    end
end
```

---

## RTL Modifications Required

### 1. New Module: Instruction Fetch Unit (`rtl/ifetch.sv`)

**Purpose:** Manage instruction fetching with 16/32-bit width awareness

**Interface:**

```systemverilog
module ifetch (
    input  logic        clk,
    input  logic        rst_n,
    input  logic [31:0] pc,              // Current PC (2-byte aligned)
    input  logic [31:0] imem_data,       // 32-bit word from memory
    output logic [31:0] imem_addr,       // Word-aligned address for memory
    output logic [15:0] instruction_16,  // 16-bit instruction output
    output logic        valid            // Instruction is valid
);
```

**Key Features:**
- Word-align memory addresses (mask PC[1:0] to get word address)
- Buffer instructions across word boundaries
- Handle PC at both word and half-word alignment

**Implementation:**

```systemverilog
module ifetch (
    input  logic        clk,
    input  logic        rst_n,
    input  logic [31:0] pc,
    input  logic [31:0] imem_data,
    output logic [31:0] imem_addr,
    output logic [15:0] instruction_16,
    output logic        valid
);
    logic [15:0] buffered_half;  // Buffered upper 16 bits
    logic        buffer_valid;
    
    // Word-align memory address
    assign imem_addr = {pc[31:2], 2'b00};
    
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            buffered_half <= 16'h0;
            buffer_valid <= 1'b0;
        end else begin
            if (!pc[1]) begin
                // PC is word-aligned: fetch from lower half
                instruction_16 <= imem_data[15:0];
                buffered_half <= imem_data[31:16];
                buffer_valid <= 1'b1;
                valid <= 1'b1;
            end else begin
                // PC is half-word aligned: use buffered data
                instruction_16 <= buffered_half;
                valid <= buffer_valid;
                // Prepare next buffer
                buffered_half <= imem_data[15:0];
                buffer_valid <= 1'b1;
            end
        end
    end
endmodule
```

### 2. New Module: Instruction Decompressor (`rtl/decompress.sv`)

**Purpose:** Expand compressed 16-bit instructions to standard 32-bit format

**Interface:**

```systemverilog
module decompress (
    input  logic [15:0] insn_16,        // 16-bit instruction input
    output logic [31:0] insn_32,        // 32-bit expanded instruction
    output logic        is_compressed,  // 1 if input was compressed
    output logic        is_valid        // 1 if valid instruction
);
```

**Decompression Logic:**

The module checks bits [1:0] to determine compression:
- `insn_16[1:0] != 2'b11` → Compressed (16-bit)
- `insn_16[1:0] == 2'b11` → Standard (need more bits)

**Major Decompression Cases:**

```systemverilog
// Example: C.ADDI4SPN decompression
// C.ADDI4SPN: addi rd', x2, nzuimm
// Format: 000 nzuimm[5:4|9:6|2|3] rd' 00
// Expands to: addi rd', x2, zero_ext(nzuimm)

if (insn_16[1:0] == 2'b00 && insn_16[15:13] == 3'b000) begin
    // C.ADDI4SPN
    rd = {2'b01, insn_16[4:2]};  // Compressed register (x8-x15)
    imm = {22'b0, insn_16[10:7], insn_16[12:11], insn_16[5], insn_16[6], 2'b00};
    insn_32 = {imm[11:0], 5'd2, 3'b000, rd, 7'b0010011};  // ADDI rd', x2, imm
    is_compressed = 1'b1;
    is_valid = (imm != 0);  // nzuimm must be non-zero
end
```

**Full Decompression Table:** (See Appendix A for complete mappings)

### 3. Modified Module: Top (`rtl/top.sv`)

**Changes Required:**

1. **Add new module instantiations:**

```systemverilog
// Instruction fetch signals
logic [15:0] fetched_insn_16;
logic        fetch_valid;
logic [31:0] decompressed_insn;
logic        is_compressed;
logic        decompress_valid;

// Instantiate instruction fetch unit
ifetch ifetch_inst (
    .clk(clk),
    .rst_n(rst_n),
    .pc(pc),
    .imem_data(imem_data),
    .imem_addr(imem_addr),
    .instruction_16(fetched_insn_16),
    .valid(fetch_valid)
);

// Instantiate decompressor
decompress decompress_inst (
    .insn_16(fetched_insn_16),
    .insn_32(decompressed_insn),
    .is_compressed(is_compressed),
    .is_valid(decompress_valid)
);
```

2. **Update instruction input to decoder:**

```systemverilog
// OLD: assign instruction = imem_data;
// NEW:
assign instruction = decompressed_insn;
```

3. **Update PC increment logic:**

```systemverilog
// OLD: next_pc = pc + 4;
// NEW: 
logic [31:0] pc_increment;
assign pc_increment = is_compressed ? 32'd2 : 32'd4;

// Sequential PC update
always_comb begin
    if (jump) begin
        // Jump target calculation (unchanged)
        next_pc = jump_target;
    end else if (take_branch) begin
        // Branch target calculation (unchanged)
        next_pc = pc + imm_b;
    end else begin
        // Sequential increment by 2 or 4
        next_pc = pc + pc_increment;
    end
end
```

4. **Add 32-bit instruction buffering for standard instructions:**

When PC[1] == 1 and we encounter a 32-bit instruction (insn_16[1:0] == 2'b11), we need the upper 16 bits from the next memory word.

```systemverilog
// Additional logic for 32-bit instruction assembly
logic [31:0] full_instruction;
logic [15:0] upper_half;

always_ff @(posedge clk) begin
    if (is_compressed) begin
        // 16-bit instruction: already complete
        full_instruction <= {16'h0, fetched_insn_16};
    end else begin
        // 32-bit instruction: need both halves
        if (!pc[1]) begin
            // Word-aligned: get both halves from imem_data
            full_instruction <= imem_data;
        end else begin
            // Half-word aligned: combine buffered + new fetch
            full_instruction <= {imem_data[15:0], fetched_insn_16};
        end
    end
end
```

### 4. No Changes Required

The following modules remain **unchanged**:
- `decoder.sv` - Receives standard 32-bit instructions
- `alu.sv` - Operates on decompressed instructions
- `regfile.sv` - No interface changes

---

## Instruction Decompression Logic

### Decompression Mapping Table

This section provides the complete mapping from compressed to standard instructions.

#### Quadrant 0 (insn[1:0] == 2'b00)

| Compressed | funct3 | Expands To | Notes |
|------------|--------|------------|-------|
| C.ADDI4SPN | 000 | `addi rd', x2, nzuimm` | rd' = x8-x15, nzuimm != 0 |
| C.LW | 010 | `lw rd', offset(rs1')` | offset = zero_ext(imm) |
| C.SW | 110 | `sw rs2', offset(rs1')` | offset = zero_ext(imm) |

#### Quadrant 1 (insn[1:0] == 2'b01)

| Compressed | funct3 | Expands To | Notes |
|------------|--------|------------|-------|
| C.NOP | 000 | `addi x0, x0, 0` | Only when rd == 0 and imm == 0 |
| C.ADDI | 000 | `addi rd, rd, nzimm` | nzimm != 0 (sign-extended) |
| C.JAL | 001 | `jal x1, offset` | RV32C only |
| C.LI | 010 | `addi rd, x0, imm` | Load immediate (sign-extended) |
| C.ADDI16SP | 011 | `addi x2, x2, nzimm` | Only when rd == 2, nzimm != 0 |
| C.LUI | 011 | `lui rd, nzimm` | Only when rd != 0, 2, nzimm != 0 |
| C.SRLI | 100/00 | `srli rd', rd', shamt` | shamt != 0 |
| C.SRAI | 100/01 | `srai rd', rd', shamt` | shamt != 0 |
| C.ANDI | 100/10 | `andi rd', rd', imm` | Sign-extended immediate |
| C.SUB | 100/11/00 | `sub rd', rd', rs2'` | |
| C.XOR | 100/11/01 | `xor rd', rd', rs2'` | |
| C.OR | 100/11/10 | `or rd', rd', rs2'` | |
| C.AND | 100/11/11 | `and rd', rd', rs2'` | |
| C.J | 101 | `jal x0, offset` | Unconditional jump |
| C.BEQZ | 110 | `beq rs1', x0, offset` | Branch if zero |
| C.BNEZ | 111 | `bne rs1', x0, offset` | Branch if not zero |

#### Quadrant 2 (insn[1:0] == 2'b10)

| Compressed | funct3 | Expands To | Notes |
|------------|--------|------------|-------|
| C.SLLI | 000 | `slli rd, rd, shamt` | shamt != 0, rd != 0 |
| C.LWSP | 010 | `lw rd, offset(x2)` | rd != 0, offset from sp |
| C.JR | 100/0 | `jalr x0, 0(rs1)` | rs1 != 0, rs2 == 0 |
| C.MV | 100/0 | `add rd, x0, rs2` | rd != 0, rs2 != 0 |
| C.EBREAK | 100/1 | `ebreak` | rd == 0, rs2 == 0 |
| C.JALR | 100/1 | `jalr x1, 0(rs1)` | rs1 != 0, rs2 == 0 |
| C.ADD | 100/1 | `add rd, rd, rs2` | rd != 0, rs2 != 0 |
| C.SWSP | 110 | `sw rs2, offset(x2)` | offset from sp |

### Immediate Encoding Examples

Compressed instructions use non-standard immediate encodings to maximize code density:

**C.ADDI4SPN Immediate (10-bit unsigned):**
```
Compressed: [12:5] contains nzuimm[5:4|9:6|2|3]
Expanded:   nzuimm = {2'b0, insn[10:7], insn[12:11], insn[5], insn[6], 2'b00}
Range:      4 to 1020, multiple of 4
```

**C.LW/C.SW Offset (7-bit unsigned):**
```
Compressed: [12:10|6:5] contains offset[5:3|2|6]
Expanded:   offset = {5'b0, insn[5], insn[12:10], insn[6], 2'b00}
Range:      0 to 124, multiple of 4
```

**C.J / C.JAL Offset (12-bit signed):**
```
Compressed: [12:2] contains offset[11|4|9:8|10|6|7|3:1|5]
Expanded:   offset = {insn[12], insn[8], insn[10:9], insn[6], 
                      insn[7], insn[2], insn[11], insn[5:3], 1'b0}
Range:      -2048 to +2046, multiple of 2
```

### Illegal Instruction Handling

Certain bit patterns in compressed instructions are **reserved** or **illegal**:

1. **All zeros (0x0000):** Illegal instruction
2. **C.ADDI4SPN with nzuimm == 0:** Illegal
3. **C.ADDI with rd == 0 and imm == 0:** Legal only as C.NOP
4. **C.LUI with rd == 0, 2 or nzimm == 0:** Illegal
5. **C.LWSP with rd == 0:** Illegal (reserved)
6. **C.JR with rs1 == 0:** Illegal

The decompressor should set `is_valid = 1'b0` for illegal instructions.

---

## Testing Strategy

### Test Organization

Tests will be added to the `tests/` directory following existing conventions:

```
tests/src/
├── lib.rs (update module declarations)
├── alu_test.rs (no changes)
├── regfile_test.rs (no changes)
├── cpu_test.rs (update with C extension tests)
├── decompress_test.rs (NEW - decompressor unit tests)
└── ifetch_test.rs (NEW - instruction fetch unit tests)
```

### Level 1: Decompressor Unit Tests (`decompress_test.rs`)

**Purpose:** Verify all 40+ compressed instructions decompress correctly

**Test Structure:**

```rust
#[test]
fn test_decompress_c_addi4spn() {
    let runtime = create_decompress_runtime()
        .expect("Failed to create decompressor runtime");
    let mut dut = runtime.create_model_simple::<Decompress>().unwrap();
    
    // Test C.ADDI4SPN: 000 nzuimm[5:4|9:6|2|3] rd' 00
    // Example: addi x8, x2, 64
    // nzuimm = 64 = 0b0001000000
    let insn_16: u16 = 0b000_01_0000_000_00;  // Encode compressed instruction
    dut.insn_16 = insn_16;
    dut.eval();
    
    // Expected: addi x8, x2, 64
    let expected: u32 = encode_i_type(0b0010011, 8, 0b000, 2, 64);
    
    assert_eq!(dut.insn_32, expected);
    assert_eq!(dut.is_compressed, 1);
    assert_eq!(dut.is_valid, 1);
}
```

**Test Coverage (60+ test cases):**

1. **Quadrant 0 tests (10 tests):**
   - C.ADDI4SPN: various immediate values, edge cases
   - C.LW: different offsets, register combinations
   - C.SW: different offsets, register combinations
   - Illegal cases: nzuimm == 0

2. **Quadrant 1 tests (25 tests):**
   - C.NOP, C.ADDI: all register/immediate combinations
   - C.JAL: various jump targets
   - C.LI: positive/negative immediates
   - C.ADDI16SP: stack adjustments
   - C.LUI: upper immediate loading
   - C.SRLI, C.SRAI, C.ANDI: shift/logic operations
   - C.SUB, C.XOR, C.OR, C.AND: arithmetic operations
   - C.J: jump targets
   - C.BEQZ, C.BNEZ: branch conditions
   - Illegal cases: zero immediates where required

3. **Quadrant 2 tests (15 tests):**
   - C.SLLI: shift amounts
   - C.LWSP: stack loads
   - C.JR, C.JALR: register jumps
   - C.MV: register moves
   - C.EBREAK: environment break
   - C.ADD: register addition
   - C.SWSP: stack stores
   - Illegal cases: rd == 0 where prohibited

4. **Edge case tests (10 tests):**
   - Maximum/minimum immediate values
   - All-zero instruction (illegal)
   - Reserved encodings
   - Boundary conditions

### Level 2: Instruction Fetch Unit Tests (`ifetch_test.rs`)

**Purpose:** Verify instruction fetching at different PC alignments

**Test Cases (20+ tests):**

```rust
#[test]
fn test_ifetch_word_aligned() {
    // PC = 0x0000 (word-aligned)
    // Memory[0x0000] = 0x12345678
    // Expected: instruction_16 = 0x5678 (lower half)
}

#[test]
fn test_ifetch_halfword_aligned() {
    // PC = 0x0002 (half-word aligned)
    // Memory[0x0000] = 0x12345678
    // Expected: instruction_16 = 0x1234 (upper half, buffered)
}

#[test]
fn test_ifetch_sequential() {
    // Test sequential fetching across multiple addresses
    // PC = 0x0000 → 0x0002 → 0x0004
}

#[test]
fn test_ifetch_boundary_crossing() {
    // PC = 0x0002, next instruction at 0x0004
    // Verify correct buffering
}
```

### Level 3: CPU Integration Tests (`cpu_test.rs`)

**Purpose:** Verify compressed instructions execute correctly in full CPU

**Test Structure:**

```rust
#[test]
fn test_cpu_compressed_addi() {
    let mut runtime = create_runtime();
    let mut dut = runtime.create_model_simple::<Top>().unwrap();
    
    // Setup memory with compressed instructions
    let mut imem: HashMap<u32, u32> = HashMap::new();
    
    // Address 0x0000: C.ADDI x10, x10, 5 (16-bit)
    // Followed by C.NOP (16-bit)
    let c_addi = encode_c_addi(10, 5);  // Helper function
    let c_nop = 0x0001;
    imem.insert(0, (c_nop as u32) << 16 | c_addi as u32);
    
    // Reset CPU
    dut.rst_n = 0;
    dut.boot_addr = 0;
    clock_cycle!(dut);
    dut.rst_n = 1;
    
    // Execute C.ADDI at PC=0
    dut.imem_data = *imem.get(&0).unwrap();
    clock_cycle!(dut);
    
    // Verify x10 incremented by 5
    // Verify PC incremented by 2 (compressed instruction)
}
```

**Test Coverage (40+ tests):**

1. **Basic compressed instructions (10 tests):**
   - C.ADDI, C.LI, C.LUI
   - C.ADD, C.SUB, C.MV
   - C.ANDI, C.SRLI, C.SLLI

2. **Memory operations (8 tests):**
   - C.LW, C.SW (base + offset)
   - C.LWSP, C.SWSP (stack operations)
   - Various offsets and alignments

3. **Control flow (10 tests):**
   - C.J, C.JAL (unconditional jumps)
   - C.JR, C.JALR (register jumps)
   - C.BEQZ, C.BNEZ (conditional branches)
   - Mixed compressed/standard jumps

4. **Mixed instruction sequences (10 tests):**
   - Compressed followed by standard
   - Standard followed by compressed
   - PC alignment transitions

5. **Stack operations (5 tests):**
   - C.ADDI4SPN, C.ADDI16SP
   - Stack push/pop sequences

6. **Edge cases (5 tests):**
   - Illegal instruction handling
   - PC at odd alignments (error)
   - Compressed instruction at end of memory

### Level 4: Program-Level Tests

**Assembly Test Program (`test_programs/c_extension_test.s`):**

```assembly
.section .text
.global _start

_start:
    # Test C.LI - Load immediate
    c.li x10, 42         # x10 = 42
    
    # Test C.ADDI - Add immediate
    c.addi x10, 8        # x10 = 50
    
    # Test C.MV - Move register
    c.mv x11, x10        # x11 = 50
    
    # Test C.ADD - Add registers
    c.add x12, x10       # x12 = x12 + 50
    
    # Test C.LW/C.SW - Load/Store
    c.li x13, 0x100      # Base address
    c.sw x10, 4(x13)     # Store x10 to mem[0x104]
    c.lw x14, 4(x13)     # Load from mem[0x104] to x14
    
    # Test C.BEQZ - Branch if zero
    c.li x15, 0
    c.beqz x15, skip     # Should branch
    c.addi x15, 1        # Should not execute
skip:
    # Test C.J - Jump
    c.j end
    c.addi x15, 2        # Should not execute
end:
    # Test C.JALR - Jump and link register
    c.li x16, 0x200
    c.jalr x16           # x1 = return address
    
    # Halt
    c.ebreak
```

**Rust Test Program (`rust-test-program/src/c_extension_test.rs`):**

```rust
#![no_std]
#![no_main]

#[no_mangle]
fn _start() -> ! {
    // Rust compiler should generate compressed instructions
    // when compiling for riscv32imc target
    
    let mut a: i32 = 10;
    let mut b: i32 = 20;
    
    // These operations can compile to compressed instructions
    a = a + 5;        // Potentially C.ADDI
    b = a;            // Potentially C.MV
    let c = a + b;    // Potentially C.ADD
    
    // Verify results
    assert_eq!(a, 15);
    assert_eq!(b, 15);
    assert_eq!(c, 30);
    
    loop {}
}
```

### Test Execution Commands

```bash
# Run decompressor unit tests
cargo test --package cpu_verifier -- decompress_test

# Run instruction fetch tests
cargo test --package cpu_verifier -- ifetch_test

# Run CPU integration tests with compressed instructions
cargo test --package cpu_verifier -- cpu_test::test_cpu_compressed

# Run all tests
cargo test --verbose

# Build assembly test program with compressed instructions
cd test_programs
riscv64-unknown-elf-as -march=rv32imc -mabi=ilp32 -o c_test.o c_extension_test.s
riscv64-unknown-elf-ld -T linker.ld -m elf32lriscv -o c_test.elf c_test.o

# Build Rust test program with compressed instructions
cd rust-test-program
cargo build --release --target riscv32imc-unknown-none-elf
```

---

## Implementation Phases

### Phase 1: Instruction Decompressor (Days 1-4)

**Tasks:**

1. **Create decompressor module (`rtl/decompress.sv`):**
   - [ ] Define module interface (inputs/outputs)
   - [ ] Implement quadrant detection logic (bits [1:0])
   - [ ] Implement Quadrant 0 decompression (C.ADDI4SPN, C.LW, C.SW)
   - [ ] Implement Quadrant 1 decompression (16 instructions)
   - [ ] Implement Quadrant 2 decompression (8 instructions)
   - [ ] Add illegal instruction detection
   - [ ] Implement immediate encoding/decoding

2. **Create decompressor test harness:**
   - [ ] Add `decompress_test.rs` to tests/src/
   - [ ] Update `lib.rs` with module declaration
   - [ ] Create Verilator runtime binding for decompressor
   - [ ] Add helper functions for encoding compressed instructions

3. **Write comprehensive unit tests:**
   - [ ] Test all 40+ compressed instruction patterns
   - [ ] Test illegal instruction detection
   - [ ] Test edge cases (max/min immediates)
   - [ ] Verify immediate encoding correctness

4. **Lint and verify:**
   ```bash
   verilator --lint-only rtl/decompress.sv
   cargo test --package cpu_verifier -- decompress_test
   ```

**Validation:**
- All decompressor tests pass (60+ test cases)
- No Verilator lint warnings
- Correct decompression for all instruction types

**Estimated Time:** 3-4 days

### Phase 2: Instruction Fetch Unit (Days 5-7)

**Tasks:**

1. **Create instruction fetch module (`rtl/ifetch.sv`):**
   - [ ] Define module interface
   - [ ] Implement word-aligned address calculation
   - [ ] Implement instruction buffering logic
   - [ ] Handle PC at word and half-word boundaries
   - [ ] Add state machine for multi-cycle fetch if needed

2. **Create instruction fetch test harness:**
   - [ ] Add `ifetch_test.rs` to tests/src/
   - [ ] Update `lib.rs` with module declaration
   - [ ] Create Verilator runtime binding for ifetch

3. **Write instruction fetch tests:**
   - [ ] Test word-aligned PC fetching
   - [ ] Test half-word-aligned PC fetching
   - [ ] Test sequential instruction fetching
   - [ ] Test boundary crossing scenarios
   - [ ] Test buffering correctness

4. **Lint and verify:**
   ```bash
   verilator --lint-only rtl/ifetch.sv
   cargo test --package cpu_verifier -- ifetch_test
   ```

**Validation:**
- All instruction fetch tests pass (20+ test cases)
- No Verilator lint warnings
- Correct instruction delivery at all alignments

**Estimated Time:** 2-3 days

### Phase 3: CPU Integration (Days 8-12)

**Tasks:**

1. **Modify top module (`rtl/top.sv`):**
   - [ ] Add ifetch module instantiation
   - [ ] Add decompress module instantiation
   - [ ] Connect modules to existing datapath
   - [ ] Update PC increment logic (2 vs. 4 bytes)
   - [ ] Handle 32-bit instruction assembly for unaligned PC
   - [ ] Update branch/jump target calculation for 2-byte alignment

2. **Update decoder if needed (`rtl/decoder.sv`):**
   - [ ] Verify decoder handles all decompressed instructions
   - [ ] No changes should be needed (decompressed = standard instructions)

3. **Test integration:**
   - [ ] Run existing CPU tests (regression testing)
   - [ ] Verify RV32IM instructions still work
   - [ ] Verify PC management works correctly

4. **Lint complete system:**
   ```bash
   verilator --lint-only rtl/*.sv
   ```

**Validation:**
- All existing 84 tests still pass (no regression)
- New modules integrate cleanly
- System lints without errors

**Estimated Time:** 4-5 days

### Phase 4: CPU Compressed Instruction Tests (Days 13-17)

**Tasks:**

1. **Add CPU-level compressed instruction tests:**
   - [ ] Create helper functions for compressed instruction encoding
   - [ ] Test basic compressed instructions (C.ADDI, C.LI, etc.)
   - [ ] Test memory operations (C.LW, C.SW, C.LWSP, C.SWSP)
   - [ ] Test control flow (C.J, C.JAL, C.JR, C.JALR)
   - [ ] Test branches (C.BEQZ, C.BNEZ)
   - [ ] Test mixed compressed/standard instruction sequences
   - [ ] Test PC alignment handling
   - [ ] Test edge cases and illegal instructions

2. **Run CPU tests:**
   ```bash
   cargo test --package cpu_verifier -- cpu_test
   ```

3. **Debug and fix issues:**
   - [ ] Investigate test failures
   - [ ] Fix RTL bugs
   - [ ] Re-run tests

**Validation:**
- All new CPU tests pass (40+ new test cases)
- Total test count increases to 120+ tests
- Mixed compressed/standard code executes correctly

**Estimated Time:** 4-5 days

### Phase 5: Assembly and Rust Program Tests (Days 18-20)

**Tasks:**

1. **Create assembly test program:**
   - [ ] Write `test_programs/c_extension_test.s`
   - [ ] Include all major compressed instruction types
   - [ ] Test control flow and branches
   - [ ] Build with `-march=rv32imc`

2. **Create Rust test program:**
   - [ ] Update `rust-test-program` target to `riscv32imc`
   - [ ] Write test program using operations that generate compressed instructions
   - [ ] Build and verify binary contains compressed instructions

3. **Update build configurations:**
   - [ ] Update `rust-test-program/.cargo/config.toml`
   - [ ] Change target: `riscv32i` → `riscv32imc`
   - [ ] Update assembly build commands in `test_programs/README.md`
   - [ ] Update CI/CD to install `riscv32imc-unknown-none-elf` target

4. **Run program-level tests:**
   ```bash
   cargo run --package cpu-sim -- test_programs/c_test.elf --verbose
   ```

**Validation:**
- Assembly programs execute correctly
- Rust programs execute correctly
- Compressed instructions observed in execution trace

**Estimated Time:** 2-3 days

### Phase 6: Documentation and CI Updates (Days 21-22)

**Tasks:**

1. **Update documentation:**
   - [ ] Update `README.md` to advertise RV32IMC support
   - [ ] Update `AGENTS.md` with:
     - New instruction count (54 + 40 = 94 total)
     - New test count (120+ tests)
     - RV32C implementation notes
   - [ ] Update `test_programs/README.md` with C extension examples
   - [ ] Update `cpu-sim/README.md` with RV32IMC notes

2. **Update CI/CD workflows:**
   - [ ] Modify `.github/workflows/copilot-setup-steps.yml`
   - [ ] Change `rustup target add riscv32im-unknown-none-elf` to `riscv32imc-unknown-none-elf`
   - [ ] Update target verification to check for `riscv32imc-unknown-none-elf`

3. **Final validation:**
   ```bash
   cargo fmt -- --check
   cargo clippy -- -D warnings
   cargo build --verbose
   cargo test --verbose
   verilator --lint-only rtl/*.sv
   ```

4. **Create PR description:**
   - [ ] Summary of changes
   - [ ] List of new instructions supported
   - [ ] Test coverage summary
   - [ ] Architecture diagrams
   - [ ] Breaking changes (target architecture)

**Validation:**
- All 120+ tests pass
- All CI checks pass
- Documentation is complete and accurate

**Estimated Time:** 1-2 days

### Phase 7: Code Review and Refinement (Days 23-25)

**Tasks:**

1. **Request code review:**
   - [ ] Submit PR for review
   - [ ] Address review comments
   - [ ] Refine implementation

2. **Performance analysis:**
   - [ ] Measure test execution time
   - [ ] Verify no performance regression
   - [ ] Optimize critical paths if needed

3. **Final testing:**
   - [ ] Run complete test suite multiple times
   - [ ] Verify reproducibility
   - [ ] Test on CI environment

**Validation:**
- Code review approved
- All tests pass consistently
- No regressions in existing functionality

**Estimated Time:** 2-3 days

---

## Validation Criteria

### Functional Validation

**Decompressor Level:**
- [ ] All 40+ compressed instructions decompress correctly
- [ ] Illegal instructions detected and flagged
- [ ] Immediate encoding/decoding is correct
- [ ] Pass-through of 32-bit instructions works

**Instruction Fetch Level:**
- [ ] Correct instruction fetch at word alignment
- [ ] Correct instruction fetch at half-word alignment
- [ ] Buffering works across word boundaries
- [ ] Sequential fetching works correctly

**CPU Integration Level:**
- [ ] Compressed instructions execute correctly
- [ ] PC increments by 2 for compressed, 4 for standard
- [ ] Mixed compressed/standard sequences work
- [ ] Branch/jump targets handle 2-byte alignment
- [ ] All RV32IM instructions still work (no regression)

**System Level:**
- [ ] Assembly programs with compressed instructions execute
- [ ] Rust programs compiled for RV32IMC execute
- [ ] CPU simulator runs compressed ELF files
- [ ] Correct halt behavior

### Quality Validation

**Code Quality:**
- [ ] `cargo fmt -- --check` passes
- [ ] `cargo clippy -- -D warnings` passes
- [ ] `verilator --lint-only rtl/*.sv` passes
- [ ] No compiler warnings

**Testing:**
- [ ] Test count increases to 120+ (84 existing + 40+ new)
- [ ] All new tests pass
- [ ] All existing tests pass (no regressions)
- [ ] Code coverage includes all compressed instructions

**Documentation:**
- [ ] README.md updated with RV32IMC support
- [ ] AGENTS.md updated with new instructions and test count
- [ ] Implementation plan complete (this document)
- [ ] Test programs documented with examples
- [ ] Build configuration changes documented

### CI/CD Validation

**Automated Checks:**
- [ ] GitHub Actions CI passes all jobs
- [ ] Build job completes successfully
- [ ] Test job runs all 120+ tests successfully
- [ ] Format check passes
- [ ] Clippy check passes

**Manual Review:**
- [ ] Code review completed
- [ ] Architecture changes approved
- [ ] Test coverage deemed sufficient
- [ ] Documentation reviewed

---

## Risk Assessment

### High-Risk Areas

#### 1. PC Management Complexity

**Risk:** Incorrect PC increment logic for compressed vs. standard instructions, especially at boundaries.

**Mitigation:**
- Comprehensive testing of PC at all alignments
- Test sequential execution across boundaries
- Test mixed compressed/standard instruction sequences
- Verify branch/jump target calculations

**Impact:** High  
**Likelihood:** Medium

#### 2. Instruction Buffering Logic

**Risk:** Incorrect buffering when fetching 16-bit instructions across 32-bit word boundaries.

**Mitigation:**
- Dedicated instruction fetch unit tests
- Test all PC alignment scenarios
- Verify buffer state across multiple cycles
- Test boundary crossing explicitly

**Impact:** High  
**Likelihood:** Medium

#### 3. Decompression Correctness

**Risk:** Incorrect decompression of compressed instructions, especially immediate encoding.

**Mitigation:**
- Comprehensive unit tests for all 40+ instructions
- Cross-reference with RISC-V specification
- Test all immediate value ranges
- Verify against reference decompressor

**Impact:** High  
**Likelihood:** Low

### Medium-Risk Areas

#### 4. Backward Compatibility

**Risk:** Changes break existing RV32IM functionality.

**Mitigation:**
- Run all 84 existing tests after each change
- Keep existing datapath unchanged
- Decompression adds new functionality without modifying old

**Impact:** High  
**Likelihood:** Low

#### 5. Timing/Critical Path

**Risk:** New decompression logic increases critical path delay.

**Mitigation:**
- Keep decompressor combinational logic simple
- Consider pipeline stage if timing issues arise
- Monitor synthesis reports

**Impact:** Medium  
**Likelihood:** Low

#### 6. Build Configuration Drift

**Risk:** Different test programs use different targets (RV32IM vs. RV32IMC).

**Mitigation:**
- Update all build scripts consistently
- Document target changes clearly
- Verify all programs build with same target

**Impact:** Medium  
**Likelihood:** Low

### Low-Risk Areas

#### 7. Tool Support

**Risk:** Assembler/compiler doesn't support RV32C properly.

**Mitigation:**
- Use well-tested GNU RISC-V toolchain
- Verify compressed instruction generation
- Test with multiple toolchain versions

**Impact:** Low  
**Likelihood:** Very Low

---

## Appendices

### Appendix A: Complete Decompression Pseudocode

```c
// Decompression function pseudocode
void decompress(uint16_t insn_16, uint32_t *insn_32, bool *is_compressed, bool *is_valid) {
    uint8_t opcode = insn_16 & 0x3;  // Bits [1:0]
    uint8_t funct3 = (insn_16 >> 13) & 0x7;
    
    *is_compressed = (opcode != 0x3);
    *is_valid = true;
    
    if (!*is_compressed) {
        // 32-bit instruction: need more data
        *insn_32 = 0;  // Will be assembled elsewhere
        return;
    }
    
    // Quadrant 0 (opcode == 00)
    if (opcode == 0x0) {
        switch (funct3) {
            case 0x0:  // C.ADDI4SPN
                uint8_t rd_p = 8 + ((insn_16 >> 2) & 0x7);
                uint16_t nzuimm = /* extract and decode */;
                if (nzuimm == 0) *is_valid = false;
                *insn_32 = encode_addi(rd_p, 2, nzuimm);
                break;
            case 0x2:  // C.LW
                // ... similar for all instructions
                break;
            // ... more cases
        }
    }
    
    // Quadrant 1 (opcode == 01)
    else if (opcode == 0x1) {
        // ... handle quadrant 1 instructions
    }
    
    // Quadrant 2 (opcode == 10)
    else if (opcode == 0x2) {
        // ... handle quadrant 2 instructions
    }
}
```

### Appendix B: Compressed Register Mapping

Compressed instructions use 3-bit register specifiers (`rd'`, `rs1'`, `rs2'`) that map to registers `x8-x15`:

| Compressed | Standard |
|------------|----------|
| 000 (0) | x8 (s0) |
| 001 (1) | x9 (s1) |
| 010 (2) | x10 (a0) |
| 011 (3) | x11 (a1) |
| 100 (4) | x12 (a2) |
| 101 (5) | x13 (a3) |
| 110 (6) | x14 (a4) |
| 111 (7) | x15 (a5) |

**Expansion logic:**
```systemverilog
assign rd_full = {2'b01, rd_compressed};  // 01xxx = x8-x15
```

### Appendix C: RISC-V ISA Compatibility

**Final ISA Support:**

| Extension | Instructions | Status |
|-----------|--------------|--------|
| RV32I | 40 instructions | ✅ Implemented |
| M | 8 instructions | ✅ Implemented |
| Zicsr | 6 instructions | ✅ Implemented |
| C | 40+ instructions | 🔄 **To Be Implemented** |
| **Total** | **94+ instructions** | |

**ISA String:** `rv32imc_zicsr`

### Appendix D: Estimated Timeline

**Total Estimated Time:** 22-25 days

| Phase | Duration | Dependencies |
|-------|----------|--------------|
| Phase 1: Decompressor | 3-4 days | None |
| Phase 2: Instruction Fetch | 2-3 days | Phase 1 complete (parallel) |
| Phase 3: CPU Integration | 4-5 days | Phases 1 & 2 complete |
| Phase 4: CPU Tests | 4-5 days | Phase 3 complete |
| Phase 5: Program Tests | 2-3 days | Phase 4 complete |
| Phase 6: Documentation | 1-2 days | Phase 5 complete |
| Phase 7: Review & Refinement | 2-3 days | Phase 6 complete |

**Critical Path:** Phase 1 → Phase 3 → Phase 4 → Phase 5 → Phase 6 → Phase 7

**Parallel Activities:** Phase 2 can run concurrently with Phase 1

### Appendix E: Resources

**RISC-V Specifications:**
- [RISC-V Unprivileged ISA Specification](https://riscv.org/technical/specifications/)
  - Chapter 16: "C" Standard Extension for Compressed Instructions

**Reference Implementations:**
- [RISC-V Spike Simulator](https://github.com/riscv-software-src/riscv-isa-sim)
- [RISC-V QEMU](https://www.qemu.org/docs/master/system/target-riscv.html)

**Testing Resources:**
- [RISC-V Compliance Test Suite](https://github.com/riscv-non-isa/riscv-arch-test)
- [RISC-V Tests Repository](https://github.com/riscv-software-src/riscv-tests)

**Toolchain:**
- [RISC-V GNU Toolchain](https://github.com/riscv-collab/riscv-gnu-toolchain)
- [Rust RISC-V Target Documentation](https://doc.rust-lang.org/rustc/platform-support.html)

### Appendix F: Implementation Checklist

**AI Agent Quick Reference**

Use this checklist to track implementation progress:

**Phase 1: Decompressor**
- [ ] Create `rtl/decompress.sv`
- [ ] Implement all quadrant decoders
- [ ] Add illegal instruction detection
- [ ] Create `tests/src/decompress_test.rs`
- [ ] Write 60+ unit tests
- [ ] Verify all tests pass

**Phase 2: Instruction Fetch**
- [ ] Create `rtl/ifetch.sv`
- [ ] Implement buffering logic
- [ ] Create `tests/src/ifetch_test.rs`
- [ ] Write 20+ fetch tests
- [ ] Verify all tests pass

**Phase 3: CPU Integration**
- [ ] Modify `rtl/top.sv`
- [ ] Add module instantiations
- [ ] Update PC logic
- [ ] Run regression tests (84 existing tests)
- [ ] Verify no regressions

**Phase 4: CPU Tests**
- [ ] Add compressed instruction helpers to `cpu_test.rs`
- [ ] Write 40+ CPU-level tests
- [ ] Test mixed instruction sequences
- [ ] Verify all tests pass

**Phase 5: Program Tests**
- [ ] Create `test_programs/c_extension_test.s`
- [ ] Update Rust test program target
- [ ] Build and run programs
- [ ] Verify correct execution

**Phase 6: Documentation**
- [ ] Update `README.md`
- [ ] Update `AGENTS.md`
- [ ] Update build documentation
- [ ] Update CI workflows

**Phase 7: Final Validation**
- [ ] Run all 120+ tests
- [ ] Pass all CI checks
- [ ] Complete code review
- [ ] Merge to main

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2025-12-31 | GitHub Copilot | Initial comprehensive plan |

---

**Document Status:** ✅ **Ready for Implementation**

This plan provides a complete roadmap for adding RV32C compressed instruction support to the RV32IM CPU. All phases are clearly defined with specific tasks, validation criteria, detailed testing strategies, and estimated timelines. The plan is optimized for AI coding agent implementation with comprehensive technical details and step-by-step guidance.
