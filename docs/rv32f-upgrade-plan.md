# RV32F Single Precision Floating Point Extension - Implementation Plan

**⚠️ IMPORTANT: Architecture Update Notice**

This document was originally written for a **single-cycle RV32IM CPU**. The CPU has since evolved to a **multi-cycle non-pipelined RV32IMAC** implementation with:
- **12-state FSM** (IDLE, FETCH, DECODE, EXECUTE, MEM_ADDR, MEM_READ, MEM_WRITE, WRITEBACK, BRANCH, CSR, ATOMIC_RMW, HALT)
- **Variable-latency memory** with ready/valid handshaking
- **RV32A (Atomic)** and **RV32C (Compressed)** extensions already implemented
- **196 existing tests** (not 84 as originally assumed)

**Key Impact on F Extension Implementation:**
- FPU operations may require **multi-cycle execution** (especially DIV, SQRT)
- FSM must be extended to handle FP instruction states
- Integration complexity is higher due to existing compressed instruction handling
- Test baseline starts at 196 tests (target: ~231-241 with FP tests)

**Updated sections marked with 🔄**

---

## Executive Summary

This document provides a comprehensive technical plan to upgrade the current **RV32IMAC** multi-cycle RISC-V CPU implementation to **RV32IMACF**, adding the **F (Single-Precision Floating Point)** extension. This plan is specifically optimized for implementation by AI coding agents, with clear phase-by-phase instructions, detailed technical specifications, and comprehensive testing strategies.

## Table of Contents

1. [Overview of RV32F Extension](#overview-of-rv32f-extension)
2. [Current Architecture Analysis](#current-architecture-analysis)
3. [RV32F Architecture Overview](#rv32f-architecture-overview)
4. [RTL Modifications Required](#rtl-modifications-required)
5. [Testing Strategy](#testing-strategy)
6. [Build Configuration Updates](#build-configuration-updates)
7. [Implementation Phases](#implementation-phases)
8. [Risk Assessment](#risk-assessment)
9. [Validation Criteria](#validation-criteria)
10. [Appendices](#appendices)

---

## Overview of RV32F Extension

### What is RV32F?

RV32F = RV32I (base integer instruction set) + F (single-precision floating point extension)

The F extension adds support for 32-bit IEEE 754-2008 floating point operations, including:
- 32 dedicated floating point registers (f0-f31)
- Floating point arithmetic operations
- Floating point comparison operations
- Floating point load/store operations
- Floating point conversion operations (int ↔ float)
- Floating point classification and sign injection

### RV32F Key Characteristics

- **Register File:** 32 × 32-bit floating point registers (separate from integer registers)
- **IEEE 754-2008:** Compliant single-precision (32-bit) floating point
- **Rounding Modes:** 5 modes (RNE, RTZ, RDN, RUP, RMM) controlled via fcsr.frm
- **Exception Flags:** 5 flags (NV, DZ, OF, UF, NX) in fcsr.fflags
- **NaN Boxing:** Upper bits must be all 1's for valid single-precision values (not required for RV32F-only implementation)


### F Extension Instructions (26 instructions)

The F extension adds **26 new instructions** across multiple categories:

#### Floating Point Load/Store (2 instructions)
| Instruction | Opcode | Funct3 | Description |
|------------|--------|--------|-------------|
| **FLW** | 0000111 | 010 | Load floating point word from memory to FP register |
| **FSW** | 0100111 | 010 | Store floating point word from FP register to memory |

#### Floating Point Computational (14 instructions)
| Instruction | Opcode | Funct7 | Funct3 | Description |
|------------|--------|--------|--------|-------------|
| **FADD.S** | 1010011 | 0000000 | rm | Add single-precision (fd = fs1 + fs2) |
| **FSUB.S** | 1010011 | 0000100 | rm | Subtract single-precision (fd = fs1 - fs2) |
| **FMUL.S** | 1010011 | 0001000 | rm | Multiply single-precision (fd = fs1 × fs2) |
| **FDIV.S** | 1010011 | 0001100 | rm | Divide single-precision (fd = fs1 ÷ fs2) |
| **FSQRT.S** | 1010011 | 0101100 | rm | Square root (fd = √fs1) |
| **FMIN.S** | 1010011 | 0010100 | 000 | Minimum (fd = min(fs1, fs2)) |
| **FMAX.S** | 1010011 | 0010100 | 001 | Maximum (fd = max(fs1, fs2)) |
| **FMADD.S** | 1000011 | rs3[4:0] | rm | Fused multiply-add (fd = fs1 × fs2 + fs3) |
| **FMSUB.S** | 1000111 | rs3[4:0] | rm | Fused multiply-sub (fd = fs1 × fs2 - fs3) |
| **FNMSUB.S** | 1001011 | rs3[4:0] | rm | Fused negate-mul-sub (fd = -(fs1 × fs2 - fs3)) |
| **FNMADD.S** | 1001111 | rs3[4:0] | rm | Fused negate-mul-add (fd = -(fs1 × fs2 + fs3)) |
| **FSGNJ.S** | 1010011 | 0010000 | 000 | Sign injection (fd = {fs2[31], fs1[30:0]}) |
| **FSGNJN.S** | 1010011 | 0010000 | 001 | Sign injection negated |
| **FSGNJX.S** | 1010011 | 0010000 | 010 | Sign injection XOR |

#### Floating Point Comparison (3 instructions)
| Instruction | Opcode | Funct7 | Funct3 | Description |
|------------|--------|--------|--------|-------------|
| **FLE.S** | 1010011 | 1010000 | 000 | Less than or equal (rd = fs1 ≤ fs2) |
| **FLT.S** | 1010011 | 1010000 | 001 | Less than (rd = fs1 < fs2) |
| **FEQ.S** | 1010011 | 1010000 | 010 | Equal (rd = fs1 == fs2) |

#### Floating Point Conversion (6 instructions)
| Instruction | Opcode | Funct7 | Funct3 | rs2   | Description |
|------------|--------|--------|--------|-------|-------------|
| **FCVT.W.S**   | 1010011 | 1100000 | rm     | 00000 | Convert float to signed int |
| **FCVT.WU.S**  | 1010011 | 1100000 | rm     | 00001 | Convert float to unsigned int |
| **FCVT.S.W**   | 1010011 | 1101000 | rm     | 00000 | Convert signed int to float |
| **FCVT.S.WU**  | 1010011 | 1101000 | rm     | 00001 | Convert unsigned int to float |
| **FMV.X.W**    | 1010011 | 1110000 | 000    | 00000 | Move FP reg to int reg (bitwise) |
| **FMV.W.X**    | 1010011 | 1111000 | 000    | 00000 | Move int reg to FP reg (bitwise) |

#### Floating Point Classification (1 instruction)
| Instruction | Opcode | Funct7 | Funct3 | rs2   | Description |
|------------|--------|--------|--------|-------|-------------|
| **FCLASS.S**   | 1010011 | 1110000 | 001    | 00000 | Classify FP number |

### IEEE 754-2008 Special Values

- **Positive Zero:** `0x00000000`
- **Negative Zero:** `0x80000000`
- **Positive Infinity:** `0x7F800000`
- **Negative Infinity:** `0xFF800000`
- **Quiet NaN (canonical):** `0x7FC00000`
- **Signaling NaN:** Any NaN with MSB of mantissa = 0

### Rounding Modes (frm field)

| Mode | Value | Description |
|------|-------|-------------|
| **RNE** | 000 | Round to Nearest, ties to Even (default) |
| **RTZ** | 001 | Round Towards Zero |
| **RDN** | 010 | Round Down (towards -∞) |
| **RUP** | 011 | Round Up (towards +∞) |
| **RMM** | 100 | Round to Nearest, ties to Max Magnitude |
| **Reserved** | 101-110 | Reserved for future use |
| **DYN** | 111 | Dynamic (use fcsr.frm) |

### Exception Flags (fflags field)

| Bit | Flag | Description |
|-----|------|-------------|
| 0 | **NX** | Inexact |
| 1 | **UF** | Underflow |
| 2 | **OF** | Overflow |
| 3 | **DZ** | Divide by Zero |
| 4 | **NV** | Invalid Operation |

### FCSR (Floating Point Control and Status Register)

```
31                   8 7   5 4   0
┌─────────────────────┬─────┬─────┐
│     Reserved (0)     │ frm │fflags│
└─────────────────────┴─────┴─────┘
```

- **CSR Address:** 0x003
- **frm (bits 7-5):** Rounding mode
- **fflags (bits 4-0):** Exception flags

---

## Current Architecture Analysis

🔄 **Updated for Multi-Cycle RV32IMAC Architecture**

### Existing RTL Modules

```
top.sv (CPU top-level - multi-cycle FSM)
├── fetch_buffer.sv (RV32C fetch buffer - handles compressed instruction alignment)
├── decompress.sv (RV32C decompressor - 27 compressed instructions)
├── decoder.sv (Instruction decoder for RV32IMAC + Zicsr)
├── alu.sv (Integer ALU - RV32I + M extension)
│   └── div_unit.sv (Hardware division unit)
├── regfile.sv (32×32-bit integer register file)
├── csr_file.sv (Control and Status Registers)
├── branch_unit.sv (Branch comparison logic)
├── mem_interface.sv (Memory interface with ready/valid handshaking)
└── writeback_mux.sv (Result selection multiplexer)
```

### Current Capabilities

The CPU currently implements:
- **RV32I Base:** 40 instructions (arithmetic, logic, shifts, branches, jumps, loads, stores)
- **M Extension:** 8 instructions (MUL, MULH, MULHSU, MULHU, DIV, DIVU, REM, REMU)
- **A Extension:** 11 instructions (LR.W, SC.W, AMOSWAP.W, AMOADD.W, AMOXOR.W, AMOAND.W, AMOOR.W, AMOMIN.W, AMOMAX.W, AMOMINU.W, AMOMAXU.W)
- **C Extension:** 27 compressed instructions (C.ADDI, C.LW, C.SW, C.JALR, etc.)
- **Zicsr Extension:** 6 instructions (CSR read/write/set/clear)
- **Multi-cycle execution:** Instructions take 3-5+ cycles plus variable memory latency
- **12-state FSM:** Including dedicated ATOMIC_RMW state for atomic operations
- **Exposed memory ports:** Instruction and data memory are external
- **196 comprehensive tests:** Across ALU, regfile, decompressor, and CPU integration

### Gaps for RV32F Support

1. **No floating point register file** - Need 32 × 32-bit FP registers (f0-f31)
2. **No floating point unit (FPU)** - Need FP arithmetic, comparison, conversion logic
3. **No FP control/status register (fcsr)** - Need frm and fflags in CSR space
4. **Decoder doesn't recognize FP opcodes** - Need to decode 6 new opcodes (integrate with RV32IMAC decoder)
5. **Top module doesn't route FP data paths** - Need FP register file and FPU integration
6. **No FP load/store support** - Need separate FP data path for FLW/FSW
7. **FSM may need FP execution states** - Multi-cycle FP operations (DIV, SQRT) may require dedicated states
8. **Compressed FP instructions** - RV32FC (compressed FP) NOT included in this plan (future extension)

---

## RV32F Architecture Overview

🔄 **Updated for Multi-Cycle Architecture**

### Proposed Module Hierarchy

```
top.sv (CPU top-level - multi-cycle FSM)
├── fetch_buffer.sv (RV32C fetch buffer - unchanged)
├── decompress.sv (RV32C decompressor - unchanged)
├── decoder.sv (Instruction decoder - updated for FP instructions)
├── alu.sv (Integer ALU - unchanged)
│   └── div_unit.sv (Hardware division - unchanged)
├── regfile.sv (Integer register file - unchanged)
├── fp_regfile.sv (NEW: 32×32-bit FP register file)
├── fpu.sv (NEW: Floating Point Unit - may use multi-cycle state machine)
├── csr_file.sv (Updated for FCSR support)
├── branch_unit.sv (Branch comparison - unchanged)
├── mem_interface.sv (Memory interface - unchanged)
└── writeback_mux.sv (Result selection - updated for FP path)
```

### Data Path Overview

```
┌────────────────────────────────────────────────────────────┐
│                      Top Module                             │
│                   (12-state FSM Control)                    │
│                                                              │
│  ┌──────────┐    ┌──────────┐    ┌────────────────────┐   │
│  │  Integer │    │ Floating  │    │   Decoder          │   │
│  │ Regfile  │    │  Point    │    │  (RV32IMACF +      │   │
│  │  (x0-x31)│    │ Regfile   │    │   Zicsr)           │   │
│  └──────────┘    │  (f0-f31) │    └────────────────────┘   │
│       │          └──────────┘                               │
│       │                │                                     │
│  ┌────▼────┐     ┌────▼────┐                               │
│  │   ALU   │     │   FPU   │                               │
│  │  (INT)  │     │  (FP)   │ ← May use multi-cycle states  │
│  └────┬────┘     └────┬────┘                               │
│       │               │                                     │
│       └───────┬───────┘                                     │
│        (Writeback Mux)                                      │
│                 │                                           │
│          (FSM-controlled                                    │
│           Writeback Logic)                                  │
└────────────────────────────────────────────────────────────┘
```

### Key Design Decisions

1. **Separate FP register file** - Independent from integer registers for clarity
2. **Dedicated FPU module** - Encapsulates all FP operations
3. **IEEE 754-2008 compliant** - Use SystemVerilog real/shortreal types or custom implementation
4. **Multi-cycle FP operations** - Complex operations (DIV, SQRT) may take multiple cycles
   - **Option A:** Single-cycle FPU using SystemVerilog synthesis (simpler, may not meet timing)
   - **Option B:** Multi-cycle FPU with dedicated FSM states (more complex, better timing)
5. **FSM state additions** - May add FP_EXECUTE or FP_WAIT states if Option B chosen
6. **Shared memory interface** - FLW/FSW use existing dmem ports with FP register file
7. **CSR integration** - fcsr at address 0x003 (standard F extension address)
8. **Exception flag updates** - FPU sets fflags on each operation
9. **NaN propagation** - Canonical NaN (0x7FC00000) for all NaN results
10. **Compressed instruction interaction** - FP instructions can follow/precede compressed instructions

---

## RTL Modifications Required

### 1. Create FP Register File (`rtl/fp_regfile.sv`)

**New module needed:** 32 × 32-bit floating point register file

```systemverilog
module fp_regfile (
    input  logic        clk,
    input  logic        rst,
    input  logic [4:0]  rs1,        // FP source register 1
    input  logic [4:0]  rs2,        // FP source register 2  
    input  logic [4:0]  rs3,        // FP source register 3 (for FMADD, etc.)
    input  logic [4:0]  rd,         // FP destination register
    input  logic [31:0] rd_data,    // FP write data
    input  logic        wr_en,      // Write enable
    output logic [31:0] rs1_data,   // FP read data 1
    output logic [31:0] rs2_data,   // FP read data 2
    output logic [31:0] rs3_data    // FP read data 3
);

    // 32 floating point registers
    logic [31:0] fp_regs [31:0];
    
    // Asynchronous reads
    assign rs1_data = fp_regs[rs1];
    assign rs2_data = fp_regs[rs2];
    assign rs3_data = fp_regs[rs3];
    
    // Synchronous write
    always_ff @(posedge clk) begin
        if (rst) begin
            // Reset all FP registers to +0.0 (0x00000000)
            for (int i = 0; i < 32; i++) begin
                fp_regs[i] <= 32'h00000000;
            end
        end else if (wr_en) begin
            fp_regs[rd] <= rd_data;
        end
    end

endmodule
```

**Key Features:**
- 3 read ports (rs1, rs2, rs3 for fused multiply-add)
- 1 write port
- All registers can be written (unlike x0 in integer regfile)
- Reset to +0.0 (not strictly required by spec, but good practice)

### 2. Create Floating Point Unit (`rtl/fpu.sv`)

**New module needed:** Complete floating point unit with all F extension operations

```systemverilog
module fpu (
    input  logic [31:0] fs1,         // FP source 1
    input  logic [31:0] fs2,         // FP source 2
    input  logic [31:0] fs3,         // FP source 3 (for fused ops)
    input  logic [31:0] int_src,     // Integer source (for conversions)
    input  logic [4:0]  fpu_op,      // FPU operation selector
    input  logic [2:0]  rm,          // Rounding mode
    output logic [31:0] fp_result,   // FP result
    output logic [31:0] int_result,  // Integer result (for conversions/compares)
    output logic [4:0]  fflags       // Exception flags (NV, DZ, OF, UF, NX)
);

    // FPU Operation Encodings
    localparam logic [4:0] FPU_ADD    = 5'b00000;  // FADD.S
    localparam logic [4:0] FPU_SUB    = 5'b00001;  // FSUB.S
    localparam logic [4:0] FPU_MUL    = 5'b00010;  // FMUL.S
    localparam logic [4:0] FPU_DIV    = 5'b00011;  // FDIV.S
    localparam logic [4:0] FPU_SQRT   = 5'b00100;  // FSQRT.S
    localparam logic [4:0] FPU_MIN    = 5'b00101;  // FMIN.S
    localparam logic [4:0] FPU_MAX    = 5'b00110;  // FMAX.S
    localparam logic [4:0] FPU_MADD   = 5'b00111;  // FMADD.S
    localparam logic [4:0] FPU_MSUB   = 5'b01000;  // FMSUB.S
    localparam logic [4:0] FPU_NMSUB  = 5'b01001;  // FNMSUB.S
    localparam logic [4:0] FPU_NMADD  = 5'b01010;  // FNMADD.S
    localparam logic [4:0] FPU_SGNJ   = 5'b01011;  // FSGNJ.S
    localparam logic [4:0] FPU_SGNJN  = 5'b01100;  // FSGNJN.S
    localparam logic [4:0] FPU_SGNJX  = 5'b01101;  // FSGNJX.S
    localparam logic [4:0] FPU_CVTWS  = 5'b01110;  // FCVT.W.S
    localparam logic [4:0] FPU_CVTWUS = 5'b01111;  // FCVT.WU.S
    localparam logic [4:0] FPU_CVTSW  = 5'b10000;  // FCVT.S.W
    localparam logic [4:0] FPU_CVTSWU = 5'b10001;  // FCVT.S.WU
    localparam logic [4:0] FPU_FEQ    = 5'b10010;  // FEQ.S
    localparam logic [4:0] FPU_FLT    = 5'b10011;  // FLT.S
    localparam logic [4:0] FPU_FLE    = 5'b10100;  // FLE.S
    localparam logic [4:0] FPU_FCLASS = 5'b10101;  // FCLASS.S
    localparam logic [4:0] FPU_MVXW   = 5'b10110;  // FMV.X.W
    localparam logic [4:0] FPU_MVWX   = 5'b10111;  // FMV.W.X

    // Convert inputs to shortreal (IEEE 754 single precision)
    shortreal fs1_real, fs2_real, fs3_real;
    shortreal result_real;
    integer int_temp;
    
    assign fs1_real = $bitstoshortreal(fs1);
    assign fs2_real = $bitstoshortreal(fs2);
    assign fs3_real = $bitstoshortreal(fs3);
    
    always_comb begin
        // Default values
        fp_result = 32'h00000000;
        int_result = 32'h00000000;
        fflags = 5'b00000;
        result_real = 0.0;
        int_temp = 0;
        
        case (fpu_op)
            FPU_ADD: begin
                result_real = fs1_real + fs2_real;
                fp_result = $shortrealtobits(result_real);
                // Set fflags based on result (simplified - full implementation needs IEEE checks)
            end
            
            FPU_SUB: begin
                result_real = fs1_real - fs2_real;
                fp_result = $shortrealtobits(result_real);
            end
            
            FPU_MUL: begin
                result_real = fs1_real * fs2_real;
                fp_result = $shortrealtobits(result_real);
            end
            
            FPU_DIV: begin
                if (fs2_real == 0.0) begin
                    // Division by zero
                    if (fs1_real == 0.0) begin
                        fp_result = 32'h7FC00000;  // NaN
                        fflags[4] = 1'b1;  // Invalid
                    end else begin
                        // Return infinity with appropriate sign
                        fp_result = fs1[31] ? 32'hFF800000 : 32'h7F800000;
                        fflags[3] = 1'b1;  // Divide by zero
                    end
                end else begin
                    result_real = fs1_real / fs2_real;
                    fp_result = $shortrealtobits(result_real);
                end
            end
            
            FPU_SQRT: begin
                if (fs1_real < 0.0 && fs1 != 32'h80000000) begin
                    // Negative number (not -0.0)
                    fp_result = 32'h7FC00000;  // NaN
                    fflags[4] = 1'b1;  // Invalid
                end else begin
                    result_real = $sqrt(fs1_real);
                    fp_result = $shortrealtobits(result_real);
                end
            end
            
            FPU_MIN: begin
                // Handle NaN and signed zero cases
                logic is_fs1_nan, is_fs2_nan;
                is_fs1_nan = (fs1[30:23] == 8'hFF) && (fs1[22:0] != 23'h0);
                is_fs2_nan = (fs2[30:23] == 8'hFF) && (fs2[22:0] != 23'h0);
                
                if (is_fs1_nan || is_fs2_nan) begin
                    fp_result = 32'h7FC00000;  // Propagate canonical NaN
                end else begin
                    result_real = (fs1_real < fs2_real) ? fs1_real : fs2_real;
                    fp_result = $shortrealtobits(result_real);
                end
            end
            
            FPU_MAX: begin
                logic is_fs1_nan, is_fs2_nan;
                is_fs1_nan = (fs1[30:23] == 8'hFF) && (fs1[22:0] != 23'h0);
                is_fs2_nan = (fs2[30:23] == 8'hFF) && (fs2[22:0] != 23'h0);
                
                if (is_fs1_nan || is_fs2_nan) begin
                    fp_result = 32'h7FC00000;  // Propagate canonical NaN
                end else begin
                    result_real = (fs1_real > fs2_real) ? fs1_real : fs2_real;
                    fp_result = $shortrealtobits(result_real);
                end
            end
            
            FPU_MADD: begin
                // fd = fs1 * fs2 + fs3
                result_real = (fs1_real * fs2_real) + fs3_real;
                fp_result = $shortrealtobits(result_real);
            end
            
            FPU_MSUB: begin
                // fd = fs1 * fs2 - fs3
                result_real = (fs1_real * fs2_real) - fs3_real;
                fp_result = $shortrealtobits(result_real);
            end
            
            FPU_NMSUB: begin
                // fd = -(fs1 * fs2 - fs3) = fs3 - fs1 * fs2
                result_real = fs3_real - (fs1_real * fs2_real);
                fp_result = $shortrealtobits(result_real);
            end
            
            FPU_NMADD: begin
                // fd = -(fs1 * fs2 + fs3)
                result_real = -((fs1_real * fs2_real) + fs3_real);
                fp_result = $shortrealtobits(result_real);
            end
            
            FPU_SGNJ: begin
                // Copy sign of fs2 to fs1
                fp_result = {fs2[31], fs1[30:0]};
            end
            
            FPU_SGNJN: begin
                // Copy inverted sign of fs2 to fs1
                fp_result = {~fs2[31], fs1[30:0]};
            end
            
            FPU_SGNJX: begin
                // XOR signs
                fp_result = {fs1[31] ^ fs2[31], fs1[30:0]};
            end
            
            FPU_CVTWS: begin
                // Float to signed int
                int_temp = $rtoi(fs1_real);
                int_result = int_temp[31:0];
            end
            
            FPU_CVTWUS: begin
                // Float to unsigned int
                if (fs1_real < 0.0) begin
                    int_result = 32'h00000000;
                    fflags[4] = 1'b1;  // Invalid
                end else begin
                    int_temp = $rtoi(fs1_real);
                    int_result = int_temp[31:0];
                end
            end
            
            FPU_CVTSW: begin
                // Signed int to float
                int_temp = $signed(int_src);
                result_real = $itor(int_temp);
                fp_result = $shortrealtobits(result_real);
            end
            
            FPU_CVTSWU: begin
                // Unsigned int to float
                result_real = $itor(int_src);
                fp_result = $shortrealtobits(result_real);
            end
            
            FPU_FEQ: begin
                // Floating point equal
                int_result = (fs1_real == fs2_real) ? 32'h00000001 : 32'h00000000;
            end
            
            FPU_FLT: begin
                // Floating point less than
                int_result = (fs1_real < fs2_real) ? 32'h00000001 : 32'h00000000;
            end
            
            FPU_FLE: begin
                // Floating point less than or equal
                int_result = (fs1_real <= fs2_real) ? 32'h00000001 : 32'h00000000;
            end
            
            FPU_FCLASS: begin
                // Classify floating point number
                // Bit 0: negative infinity
                // Bit 1: negative normal
                // Bit 2: negative subnormal
                // Bit 3: negative zero
                // Bit 4: positive zero
                // Bit 5: positive subnormal
                // Bit 6: positive normal
                // Bit 7: positive infinity
                // Bit 8: signaling NaN
                // Bit 9: quiet NaN
                
                logic is_zero, is_inf, is_nan, is_neg;
                is_zero = (fs1[30:0] == 31'h00000000);
                is_inf = (fs1[30:23] == 8'hFF) && (fs1[22:0] == 23'h000000);
                is_nan = (fs1[30:23] == 8'hFF) && (fs1[22:0] != 23'h000000);
                is_neg = fs1[31];
                
                if (is_nan) begin
                    int_result = (fs1[22]) ? 32'h00000200 : 32'h00000100;  // Quiet or signaling NaN
                end else if (is_inf) begin
                    int_result = is_neg ? 32'h00000001 : 32'h00000080;  // -inf or +inf
                end else if (is_zero) begin
                    int_result = is_neg ? 32'h00000008 : 32'h00000010;  // -0 or +0
                end else begin
                    // Normal or subnormal (simplified - doesn't check for subnormal)
                    int_result = is_neg ? 32'h00000002 : 32'h00000040;  // Negative or positive normal
                end
            end
            
            FPU_MVXW: begin
                // Move FP register to integer register (bitwise)
                int_result = fs1;
            end
            
            FPU_MVWX: begin
                // Move integer register to FP register (bitwise)
                fp_result = int_src;
            end
            
            default: begin
                fp_result = 32'h00000000;
                int_result = 32'h00000000;
            end
        endcase
    end

endmodule
```

**Implementation Notes:**
- Uses SystemVerilog `shortreal` type for IEEE 754 compliance
- Simplified exception flag handling (full implementation needs detailed IEEE checks)
- NaN canonicalization to 0x7FC00000
- Rounding mode handling simplified (SystemVerilog handles rounding internally)
- Division by zero and invalid operation handling


### 3. Update Decoder Module (`rtl/decoder.sv`)

**Changes needed:** Add FP instruction decoding logic

**New outputs to add:**
```systemverilog
output logic [4:0]  fpu_op,        // FPU operation selector
output logic        fp_reg_write,  // FP register write enable
output logic        fp_to_int,     // FP result goes to integer register
output logic        int_to_fp,     // Integer source goes to FP unit
output logic        is_fp_load,    // FLW instruction
output logic        is_fp_store    // FSW instruction
```

**New local parameters:**
```systemverilog
// FP opcodes
localparam logic [6:0] OP_FP_LOAD  = 7'b0000111;
localparam logic [6:0] OP_FP_STORE = 7'b0100111;
localparam logic [6:0] OP_FP       = 7'b1010011;  // FP computational
localparam logic [6:0] OP_FMADD    = 7'b1000011;
localparam logic [6:0] OP_FMSUB    = 7'b1000111;
localparam logic [6:0] OP_FNMSUB   = 7'b1001011;
localparam logic [6:0] OP_FNMADD   = 7'b1001111;
```

**Decoding logic to add:**
```systemverilog
always_comb begin
    // ... existing decoder logic ...
    
    // FP instruction decoding
    case (opcode)
        OP_FP_LOAD: begin  // FLW
            if (funct3 == 3'b010) begin
                alu_src = 1'b1;  // Use immediate
                mem_read = 1'b1;
                is_fp_load = 1'b1;
                fp_reg_write = 1'b1;
            end
        end
        
        OP_FP_STORE: begin  // FSW
            if (funct3 == 3'b010) begin
                alu_src = 1'b1;  // Use immediate
                mem_write = 1'b1;
                is_fp_store = 1'b1;
            end
        end
        
        OP_FP: begin  // FP computational instructions
            fp_reg_write = 1'b1;
            case (funct7)
                7'b0000000: fpu_op = FPU_ADD;    // FADD.S
                7'b0000100: fpu_op = FPU_SUB;    // FSUB.S
                7'b0001000: fpu_op = FPU_MUL;    // FMUL.S
                7'b0001100: fpu_op = FPU_DIV;    // FDIV.S
                7'b0101100: fpu_op = FPU_SQRT;   // FSQRT.S
                7'b0010000: begin  // Sign injection
                    case (funct3)
                        3'b000: fpu_op = FPU_SGNJ;
                        3'b001: fpu_op = FPU_SGNJN;
                        3'b010: fpu_op = FPU_SGNJX;
                    endcase
                end
                7'b0010100: begin  // MIN/MAX
                    fpu_op = (funct3 == 3'b000) ? FPU_MIN : FPU_MAX;
                end
                7'b1010000: begin  // Comparisons
                    fp_reg_write = 1'b0;
                    reg_write = 1'b1;  // Write to integer register
                    fp_to_int = 1'b1;
                    case (funct3)
                        3'b000: fpu_op = FPU_FLE;
                        3'b001: fpu_op = FPU_FLT;
                        3'b010: fpu_op = FPU_FEQ;
                    endcase
                end
                7'b1100000: begin  // FCVT.W.S, FCVT.WU.S
                    fp_reg_write = 1'b0;
                    reg_write = 1'b1;
                    fp_to_int = 1'b1;
                    fpu_op = (rs2 == 5'b00000) ? FPU_CVTWS : FPU_CVTWUS;
                end
                7'b1101000: begin  // FCVT.S.W, FCVT.S.WU
                    int_to_fp = 1'b1;
                    fpu_op = (rs2 == 5'b00000) ? FPU_CVTSW : FPU_CVTSWU;
                end
                7'b1110000: begin
                    if (funct3 == 3'b000) begin
                        // FMV.X.W
                        fp_reg_write = 1'b0;
                        reg_write = 1'b1;
                        fp_to_int = 1'b1;
                        fpu_op = FPU_MVXW;
                    end else begin
                        // FCLASS.S
                        fp_reg_write = 1'b0;
                        reg_write = 1'b1;
                        fp_to_int = 1'b1;
                        fpu_op = FPU_FCLASS;
                    end
                end
                7'b1111000: begin  // FMV.W.X
                    int_to_fp = 1'b1;
                    fpu_op = FPU_MVWX;
                end
            endcase
        end
        
        OP_FMADD:  begin  // FMADD.S
            fp_reg_write = 1'b1;
            fpu_op = FPU_MADD;
        end
        
        OP_FMSUB:  begin  // FMSUB.S
            fp_reg_write = 1'b1;
            fpu_op = FPU_MSUB;
        end
        
        OP_FNMSUB: begin  // FNMSUB.S
            fp_reg_write = 1'b1;
            fpu_op = FPU_NMSUB;
        end
        
        OP_FNMADD: begin  // FNMADD.S
            fp_reg_write = 1'b1;
            fpu_op = FPU_NMADD;
        end
    endcase
end
```

### 4. Update Top Module (`rtl/top.sv`)

**Changes needed:** Integrate FP register file and FPU

**New internal signals:**
```systemverilog
// FP register file signals
logic [31:0] fp_rs1_data, fp_rs2_data, fp_rs3_data;
logic [31:0] fp_rd_data;
logic        fp_reg_write;
logic [4:0]  fpu_op;
logic        fp_to_int, int_to_fp;
logic        is_fp_load, is_fp_store;

// FPU signals
logic [31:0] fpu_fp_result, fpu_int_result;
logic [4:0]  fpu_fflags;

// CSR for FP
logic [31:0] fcsr;
logic [2:0]  frm;     // Rounding mode (fcsr[7:5])
logic [4:0]  fflags;  // Exception flags (fcsr[4:0])
```

**Module instantiations to add:**
```systemverilog
// Floating point register file
fp_regfile fp_regfile_inst (
    .clk       (clk),
    .rst       (rst),
    .rs1       (instruction[19:15]),  // fs1
    .rs2       (instruction[24:20]),  // fs2
    .rs3       (instruction[31:27]),  // fs3 (for fused ops)
    .rd        (instruction[11:7]),   // fd
    .rd_data   (fp_rd_data),
    .wr_en     (fp_reg_write),
    .rs1_data  (fp_rs1_data),
    .rs2_data  (fp_rs2_data),
    .rs3_data  (fp_rs3_data)
);

// Floating point unit
fpu fpu_inst (
    .fs1        (fp_rs1_data),
    .fs2        (fp_rs2_data),
    .fs3        (fp_rs3_data),
    .int_src    (rs1_data),  // From integer register file
    .fpu_op     (fpu_op),
    .rm         (frm),
    .fp_result  (fpu_fp_result),
    .int_result (fpu_int_result),
    .fflags     (fpu_fflags)
);
```

**Data path updates:**
```systemverilog
// FP load/store handling
always_comb begin
    if (is_fp_load) begin
        // FLW: load from memory to FP register
        fp_rd_data = dmem_rdata;
    end else if (is_fp_store) begin
        // FSW: store from FP register to memory
        dmem_wdata = fp_rs2_data;
    end else if (int_to_fp) begin
        // Integer to FP conversion/move
        fp_rd_data = fpu_fp_result;
    end else begin
        // Normal FP operation
        fp_rd_data = fpu_fp_result;
    end
end

// Writeback to integer registers (for FP comparisons, conversions)
always_comb begin
    if (fp_to_int) begin
        wr_data = fpu_int_result;
    end else begin
        // Existing integer writeback logic
        wr_data = ...;  // (unchanged)
    end
end

// FCSR updates
always_ff @(posedge clk) begin
    if (rst) begin
        fcsr <= 32'h00000000;  // Default: RNE rounding, no flags
    end else begin
        // Update fflags on FP operations
        if (fp_reg_write || fp_to_int) begin
            fcsr[4:0] <= fcsr[4:0] | fpu_fflags;  // Accumulate exception flags
        end
        
        // CSR writes to fcsr (from CSRRW, CSRRS, etc.)
        if (is_csr && csr_addr == 12'h003) begin
            fcsr <= csr_wdata;
        end
    end
end

assign frm = fcsr[7:5];
assign fflags = fcsr[4:0];
```

### 5. Update CSR Handling

**Expand CSR decoder** to recognize fcsr (0x003), frm (0x002), fflags (0x001):

```systemverilog
case (csr_addr)
    12'h001: csr_rdata = {27'b0, fflags};        // fflags
    12'h002: csr_rdata = {29'b0, frm};           // frm
    12'h003: csr_rdata = fcsr;                   // fcsr
    // ... existing CSRs ...
endcase
```

---

## Testing Strategy

### Test Categories

#### 1. FPU Unit Tests (`cpu-sim/tests/fpu_test.rs`)

Create comprehensive FPU unit tests for isolated testing of floating point operations.

**Test structure:**
```rust
#[test]
fn test_fpu_add_basic() {
    // Test FADD.S with simple values
    // 1.0 + 2.0 = 3.0
    // -1.5 + 2.5 = 1.0
}

#[test]
fn test_fpu_add_special_values() {
    // +inf + 1.0 = +inf
    // -inf + -inf = -inf
    // +inf + -inf = NaN
    // NaN + anything = NaN
}

#[test]
fn test_fpu_mul_div() {
    // Test FMUL.S and FDIV.S
    // 3.5 * 2.0 = 7.0
    // 10.0 / 2.5 = 4.0
    // Division by zero handling
}

#[test]
fn test_fpu_sqrt() {
    // Test FSQRT.S
    // sqrt(4.0) = 2.0
    // sqrt(0.0) = 0.0
    // sqrt(-1.0) = NaN (invalid)
}

#[test]
fn test_fpu_comparisons() {
    // Test FEQ.S, FLT.S, FLE.S
    // 1.5 < 2.0 => 1
    // 2.0 == 2.0 => 1
    // NaN == NaN => 0 (per IEEE 754)
}

#[test]
fn test_fpu_conversions() {
    // Test FCVT.W.S, FCVT.WU.S, FCVT.S.W, FCVT.S.WU
    // float(5) = 5.0
    // int(3.7) = 3 (with RTZ rounding)
}

#[test]
fn test_fpu_fused_ops() {
    // Test FMADD.S, FMSUB.S, FNMSUB.S, FNMADD.S
    // (2.0 * 3.0) + 1.0 = 7.0
}

#[test]
fn test_fpu_sign_injection() {
    // Test FSGNJ.S, FSGNJN.S, FSGNJX.S
    // FSGNJ(1.5, -0.0) = -1.5
}

#[test]
fn test_fpu_classify() {
    // Test FCLASS.S
    // fclass(0.0) = positive zero
    // fclass(+inf) = positive infinity
    // fclass(NaN) = quiet NaN
}
```

**Estimated:** 15-20 test functions, ~60+ individual test cases

#### 2. FP Register File Tests (`cpu-sim/tests/fp_regfile_test.rs`)

```rust
#[test]
fn test_fp_regfile_basic_read_write() {
    // Test writing and reading FP registers
    // Verify all 32 registers work independently
}

#[test]
fn test_fp_regfile_simultaneous_reads() {
    // Test reading rs1, rs2, rs3 simultaneously
    // Important for fused multiply-add operations
}

#[test]
fn test_fp_regfile_write_priority() {
    // Test write timing and priority
}

#[test]
fn test_fp_regfile_reset() {
    // Verify reset behavior (all registers to +0.0)
}
```

**Estimated:** 5 test functions

#### 3. CPU Integration Tests (`cpu-sim/tests/cpu_test.rs`)

```rust
#[test]
fn test_cpu_flw_fsw() {
    // Test FP load/store
    // Load floating point value from memory
    // Perform operation
    // Store back to memory
    // Verify correct value
}

#[test]
fn test_cpu_fp_arithmetic_sequence() {
    // Test sequence of FP operations
    // a = 1.5
    // b = 2.0
    // c = a + b  // 3.5
    // d = c * a  // 5.25
    // e = d / b  // 2.625
}

#[test]
fn test_cpu_fp_int_interaction() {
    // Test mixed FP and integer operations
    // Load integer, convert to float
    // Perform FP arithmetic
    // Convert back to integer, store
}

#[test]
fn test_cpu_fcsr_operations() {
    // Test CSR access to fcsr, frm, fflags
    // Set rounding mode
    // Perform operation
    // Check exception flags
}

#[test]
fn test_cpu_fp_branching() {
    // Test FP comparisons with branches
    // if (fa < fb) goto label
}

#[test]
fn test_cpu_fused_multiply_add() {
    // Test FMADD and variants in CPU context
    // Verify single-rounding (vs separate mul+add)
}
```

**Estimated:** 10-15 test functions

#### 4. Compliance Tests

Use RISC-V Architectural Test Suite for F extension:
```bash
# Clone riscv-arch-test
git clone https://github.com/riscv-non-isa/riscv-arch-test

# Run F extension tests (after implementing test harness)
```

Key test cases from compliance suite:
- `fadd` - Addition tests
- `fsub` - Subtraction tests
- `fmul` - Multiplication tests
- `fdiv` - Division tests
- `fsqrt` - Square root tests
- `fmadd` - Fused multiply-add tests
- `fcmp` - Comparison tests
- `fcvt` - Conversion tests
- `fclass` - Classification tests

#### 5. Edge Case Testing

**Critical edge cases to test:**

1. **NaN handling:**
   - Signaling vs quiet NaN
   - NaN propagation
   - NaN canonicalization

2. **Infinity handling:**
   - inf + inf, inf - inf, inf * 0, etc.
   - Comparisons with infinity

3. **Zero handling:**
   - Signed zero (+0.0 vs -0.0)
   - Operations with zero

4. **Rounding modes:**
   - Test all 5 rounding modes (RNE, RTZ, RDN, RUP, RMM)
   - Verify correct rounding in edge cases

5. **Subnormal numbers:**
   - Operations with denormalized numbers
   - Underflow conditions

6. **Overflow/Underflow:**
   - Very large number operations
   - Very small number operations

---

## Build Configuration Updates

### 1. Rust Test Program Target

🔄 **Updated for RV32IMAC → RV32IMACF**

Update to include F extension support:

**File:** `rust-test-program/.cargo/config.toml`

```toml
[build]
target = "riscv32imacf-unknown-none-elf"  # CHANGED: riscv32imac → riscv32imacf
```

**Note:** The `riscv32imacf-unknown-none-elf` target may not be available in standard Rust. You may need to:
- Use custom target specification (preferred for exact control)
- Or compile with `riscv32gc-unknown-none-elf` (includes F and D extensions, then disable D)
- Or use `-C target-feature=+f` flag with riscv32imac target

**Recommended approach for RV32IMACF:**
```toml
[build]
target = "riscv32gc-unknown-none-elf"

[target.riscv32gc-unknown-none-elf]
rustflags = ["-C", "target-feature=+f,-d"]  # Enable F, disable D (double-precision)
```

**Alternative - Custom target JSON:**
Create `riscv32imacf-unknown-none-elf.json` based on existing riscv32imac target with F extension enabled.

### 2. Assembly Test Programs

🔄 **Updated for RV32IMAC → RV32IMACF**

Update assembler flags:

**Old:**
```bash
riscv64-unknown-elf-as -march=rv32im -mabi=ilp32 -o test.o test.s
```

**New:**
```bash
riscv64-unknown-elf-as -march=rv32imacf -mabi=ilp32 -o test.o test.s
```

**Note:** 
- Architecture changed from `rv32im` to `rv32imacf` (includes A and C extensions already implemented)
- ABI remains `ilp32` (not `ilp32f`) since we're not using hardware FP calling conventions
- The `_Zicsr` suffix is implicit in modern toolchains

### 3. Create F Extension Test Programs

**New assembly test:** `test_programs/f_extension_test.s`

```assembly
.section .text
.global _start

_start:
    # Test FLW/FSW
    la x1, float_data
    flw f0, 0(x1)      # Load 3.14
    flw f1, 4(x1)      # Load 2.71
    
    # Test FADD.S
    fadd.s f2, f0, f1  # f2 = 3.14 + 2.71 = 5.85
    
    # Test FSUB.S
    fsub.s f3, f0, f1  # f3 = 3.14 - 2.71 = 0.43
    
    # Test FMUL.S
    fmul.s f4, f0, f1  # f4 = 3.14 * 2.71 = 8.5094
    
    # Test FDIV.S
    fdiv.s f5, f0, f1  # f5 = 3.14 / 2.71 = 1.1587
    
    # Test FLT.S (comparison)
    flt.s x2, f1, f0   # x2 = (2.71 < 3.14) = 1
    
    # Test FCVT.W.S (float to int)
    fcvt.w.s x3, f0    # x3 = int(3.14) = 3
    
    # Test FCVT.S.W (int to float)
    addi x4, x0, 42
    fcvt.s.w f6, x4    # f6 = float(42) = 42.0
    
    # Test FSQRT.S
    fsqrt.s f7, f0     # f7 = sqrt(3.14) ≈ 1.772
    
    # Test FMADD.S
    fmadd.s f8, f0, f1, f2  # f8 = (3.14 * 2.71) + 5.85
    
    # Store results
    fsw f2, 8(x1)
    fsw f3, 12(x1)
    fsw f4, 16(x1)
    
    # Halt
    j .

.section .data
float_data:
    .float 3.14        # Offset 0
    .float 2.71        # Offset 4
    .float 0.0         # Offset 8 (result slot)
    .float 0.0         # Offset 12 (result slot)
    .float 0.0         # Offset 16 (result slot)
```

### 4. CI/CD Pipeline Updates

**File:** `.github/workflows/copilot-setup-steps.yml`

```yaml
# Update target installation
- name: Install RISC-V Rust Target
  run: rustup target add riscv32gc-unknown-none-elf  # Includes F extension

# Update verification
- name: Verify RISC-V Rust target
  run: rustup target list --installed | grep riscv32gc-unknown-none-elf
```

**File:** `.github/workflows/ci.yml`

Add optional F extension test builds:
```yaml
- name: Build F extension test programs
  run: |
    cd test_programs
    riscv64-unknown-elf-as -march=rv32imf -mabi=ilp32 -o f_test.o f_extension_test.s
    riscv64-unknown-elf-ld -T linker.ld -o f_test.elf f_test.o
```

### 5. Documentation Updates

**Files to update:**
- `README.md` - Add RV32F to feature list
- `AGENTS.md` - Add F extension instructions, update test count
- `cpu-sim/README.md` - Add FP operation examples

---

## Implementation Phases

### Phase 1: Create FP Register File (Estimated: 1-2 days)

**Objective:** Implement and test the floating point register file module

**Tasks:**
1. [ ] Create `rtl/fp_regfile.sv`
   - 32 × 32-bit register array
   - 3 read ports (rs1, rs2, rs3)
   - 1 write port
   - Synchronous write, asynchronous read
   - Reset all registers to +0.0 (0x00000000)

2. [ ] Create unit tests in `tests/src/fp_regfile_test.rs`
   - Test basic read/write
   - Test simultaneous 3-port reads
   - Test write-through behavior
   - Test reset functionality
   - Test all 32 registers independently

3. [ ] Lint and verify RTL
   ```bash
   verilator --lint-only rtl/fp_regfile.sv
   ```

4. [ ] Run unit tests
   ```bash
   cargo test --package cpu_verifier -- fp_regfile_test
   ```

**Validation:**
- [ ] FP register file module compiles without errors
- [ ] All lint checks pass
- [ ] All unit tests pass (5+ tests)

**Deliverables:**
- `rtl/fp_regfile.sv` (new file)
- `tests/src/fp_regfile_test.rs` (new file)
- Update `tests/src/lib.rs` to include `mod fp_regfile_test;`

---

### Phase 2: Create Basic FPU (Estimated: 4-6 days)

🔄 **Updated for Multi-Cycle Considerations**

**Objective:** Implement floating point unit with core arithmetic operations

**Important Decision Point:** Choose FPU implementation strategy:
- **Option A:** Single-cycle FPU using SystemVerilog synthesis (simpler, may not meet timing)
- **Option B:** Multi-cycle FPU with dedicated states (recommended, better timing, more complex)

**Tasks:**
1. [ ] Create `rtl/fpu.sv` with basic operations
   - FPU_ADD, FPU_SUB, FPU_MUL, FPU_DIV
   - Use SystemVerilog `shortreal` for IEEE 754 compliance OR custom implementation
   - Implement special value handling (NaN, infinity, zero)
   - Basic exception flag generation
   - **If multi-cycle:** Add state machine for DIV operation (iterative algorithm)

2. [ ] Create unit tests in `tests/src/fpu_test.rs`
   - Test FADD.S with various inputs
   - Test FSUB.S edge cases
   - Test FMUL.S including overflow
   - Test FDIV.S including division by zero
   - Test NaN propagation
   - Test infinity handling
   - **If multi-cycle:** Test that operations complete correctly across multiple cycles

3. [ ] Lint and verify RTL
   ```bash
   verilator --lint-only rtl/fpu.sv
   ```

4. [ ] Run unit tests
   ```bash
   cargo test --package cpu_verifier -- fpu_test
   ```

**Validation:**
- [ ] FPU module compiles without errors
- [ ] Basic arithmetic operations work correctly
- [ ] Special values handled per IEEE 754
- [ ] 10+ FPU tests pass
- [ ] **If multi-cycle:** Verify cycle counts match expected latency

**Deliverables:**
- `rtl/fpu.sv` (new file, basic operations only)
- `tests/src/fpu_test.rs` (new file)
- Update `tests/src/lib.rs` to include `mod fpu_test;`

**Multi-Cycle Implementation Notes:**
- Division and square root are prime candidates for multi-cycle execution
- Consider 8-16 cycle iterative divider (non-restoring division algorithm)
- Add `fpu_busy` and `fpu_done` signals if multi-cycle
- FSM must wait for FPU completion before proceeding to WRITEBACK state

---

### Phase 3: Expand FPU Operations (Estimated: 3-4 days)

🔄 **Updated Duration for Multi-Cycle Complexity**

**Objective:** Add remaining FP operations to FPU

**Tasks:**
1. [ ] Add to `rtl/fpu.sv`:
   - FSQRT.S (square root) - **Multi-cycle candidate** (consider 16-32 cycle iterative)
   - FMIN.S, FMAX.S
   - FSGNJ.S, FSGNJN.S, FSGNJX.S (sign injection)
   - FEQ.S, FLT.S, FLE.S (comparisons)
   - FCVT.W.S, FCVT.WU.S, FCVT.S.W, FCVT.S.WU (conversions)
   - FMV.X.W, FMV.W.X (moves)
   - FCLASS.S (classification)
   - FMADD.S, FMSUB.S, FNMSUB.S, FNMADD.S (fused operations)

2. [ ] Expand tests in `tests/src/fpu_test.rs`
   - Test each new operation
   - Test edge cases for each
   - Test rounding mode effects
   - Test exception flag generation
   - **If multi-cycle:** Test cycle counts for SQRT

3. [ ] Run expanded tests
   ```bash
   cargo test --package cpu_verifier -- fpu_test
   ```

**Validation:**
- [ ] All 26 FP operations implemented
- [ ] All FPU tests pass (20+ tests, 60+ test cases)
- [ ] **Multi-cycle operations:** Verify busy/done signaling

**Deliverables:**
- Updated `rtl/fpu.sv` (complete)
- Updated `tests/src/fpu_test.rs` (complete)

---

### Phase 4: Update Decoder (Estimated: 2-3 days)

🔄 **Updated for RV32IMAC Integration**

**Objective:** Add FP instruction decoding to decoder module

**Tasks:**
1. [ ] Update `rtl/decoder.sv`
   - Add new output ports (fpu_op, fp_reg_write, fp_to_int, int_to_fp, is_fp_load, is_fp_store)
   - Add FP opcode parameters (OP_FP_LOAD, OP_FP_STORE, OP_FP, OP_FMADD, etc.)
   - Add FPU operation parameters (matching fpu.sv)
   - Implement FP instruction decoding logic **alongside existing RV32IMAC decoding**
   - Handle all 6 FP opcode types
   - **Important:** Ensure FP instructions can follow compressed instructions (fetch_buffer integration)

2. [ ] Lint updated decoder
   ```bash
   verilator --lint-only rtl/decoder.sv
   ```

3. [ ] Verify existing tests still pass
   ```bash
   cargo test --package cpu_verifier -- decoder_test
   # Should still have all 196 tests passing
   ```

**Validation:**
- [ ] Decoder compiles without errors
- [ ] FP instructions decoded correctly
- [ ] Existing RV32IMAC decoding unaffected (regression test)
- [ ] All 196 existing tests still pass

**Deliverables:**
- Updated `rtl/decoder.sv`

**Integration Notes:**
- Decoder must handle FP instructions that may be preceded/followed by compressed instructions
- Ensure decode signals are properly set for FP operations
- Consider FSM state requirements (may need FP-specific execute states)

---

### Phase 5: Integrate into Top Module (Estimated: 3-4 days)

🔄 **Updated for Multi-Cycle FSM Integration**

**Objective:** Connect FP register file and FPU to top module

**Tasks:**
1. [ ] Update `rtl/top.sv`
   - Add FP register file instantiation
   - Add FPU instantiation
   - Add internal signals for FP data paths
   - Implement FP load/store logic (integrate with mem_interface.sv)
   - Implement FP-to-int and int-to-FP data routing
   - Add FCSR register (fcsr, frm, fflags) to csr_file.sv OR top.sv
   - Update CSR read/write logic
   - Connect exception flags to FCSR
   - **Critical:** Update 12-state FSM to handle FP operations
     - May add FP_EXECUTE or FP_WAIT states if multi-cycle FPU
     - Or extend existing EXECUTE state to handle FPU completion
   - **Update writeback_mux.sv** to include FP result path
   - Ensure FP instructions work with compressed instruction fetch (fetch_buffer integration)

2. [ ] Lint updated top module
   ```bash
   verilator --lint-only rtl/top.sv
   ```

3. [ ] Run regression tests
   ```bash
   cargo test --verbose
   # All 196 tests must still pass
   ```

**Validation:**
- [ ] Top module compiles without errors
- [ ] All existing tests still pass (regression - 196 tests)
- [ ] FP modules properly integrated
- [ ] FSM state transitions correct for FP instructions
- [ ] No timing violations introduced (check synthesis reports if available)

**Deliverables:**
- Updated `rtl/top.sv`
- Updated `rtl/writeback_mux.sv`
- Updated `rtl/csr_file.sv` (for FCSR support)

**FSM Integration Details:**
```
Possible FSM flow for FP instruction:
S_FETCH → S_DECODE → S_EXECUTE (FPU operation)
  ↓ (if multi-cycle FPU)
S_FP_WAIT (optional new state, wait for fpu_done)
  ↓
S_WRITEBACK → S_FETCH
```

---

### Phase 6: CPU Integration Tests (Estimated: 3-4 days)

🔄 **Updated for Multi-Cycle Testing**

**Objective:** Test FP instructions in full CPU context

**Tasks:**
1. [ ] Create CPU-level FP tests in `tests/src/cpu_test.rs` or new `tests/src/cpu_fp_test.rs`
   - Test FLW/FSW (FP load/store)
   - Test FP arithmetic in CPU
   - Test FP comparisons with branches
   - Test FP/integer conversions
   - Test FP/integer interaction
   - Test CSR access (fcsr, frm, fflags)
   - Test exception flag accumulation
   - **Multi-cycle specific:** Test that FP instructions complete correctly with memory latency
   - **Multi-cycle specific:** Test FP operations following compressed instructions
   - Test FP instruction after atomic operation (state transition)

2. [ ] Run CPU integration tests
   ```bash
   cargo test --package cpu_verifier -- cpu_fp_test
   # Or: cargo test --package cpu_verifier -- cpu_test::fp
   ```

3. [ ] Debug any failures
   - Use `--nocapture` for debugging output
   - Check signal values and data paths
   - Verify instruction encoding
   - **Multi-cycle:** Verify cycle counts and FSM state transitions

**Validation:**
- [ ] All CPU FP tests pass (10-15 new tests)
- [ ] All existing CPU tests pass (regression - 196 tests)
- [ ] Total test count: 196 (existing) + 35+ (FP) = 231+ tests
- [ ] FSM correctly sequences through FP instruction execution

**Deliverables:**
- Updated `tests/src/cpu_test.rs` with FP tests OR
- New `tests/src/cpu_fp_test.rs` file
- Update `tests/src/lib.rs` if new file created

**Testing Notes:**
- Follow existing test patterns from `cpu_test.rs`
- Use `clock_cycle!` macro for cycle advancement
- Handle variable memory latency in tests
- Verify FP operations work correctly in multi-cycle context

---

### Phase 7: Assembly Test Programs (Estimated: 1-2 days)

**Objective:** Create and test F extension assembly programs

**Tasks:**
1. [ ] Create `test_programs/f_extension_test.s`
   - Test all major FP operations
   - Test FP arithmetic sequence
   - Test FP load/store
   - Test FP/int conversions
   - Include floating point data section
   - **Note:** Assembly must use `rv32imacf` architecture

2. [ ] Build test program
   ```bash
   cd test_programs
   riscv64-unknown-elf-as -march=rv32imacf -mabi=ilp32 -o f_test.o f_extension_test.s
   riscv64-unknown-elf-ld -T linker.ld -o f_test.elf f_test.o
   ```

3. [ ] Run in CPU simulator
   ```bash
   cargo run --package cpu-sim -- test_programs/f_test.elf --verbose
   ```

4. [ ] Verify results
   - Check FP register final values
   - Check memory contents
   - Check FCSR flags
   - **Multi-cycle:** Verify instruction trace shows correct FP execution

**Validation:**
- [ ] Test program assembles without errors
- [ ] Program executes correctly in simulator
- [ ] All FP operations produce expected results
- [ ] Multi-cycle execution timing is correct

**Deliverables:**
- `test_programs/f_extension_test.s` (new file)
- `test_programs/f_test.elf` (generated)

---

### Phase 8: Build Configuration Updates (Estimated: 1 day)

**Objective:** Update build configurations and toolchain support

**Tasks:**
1. [ ] Update Rust target configuration
   - Modify `rust-test-program/.cargo/config.toml`
   - Change to `riscv32gc-unknown-none-elf` or custom target

2. [ ] Update CI/CD workflows
   - Modify `.github/workflows/copilot-setup-steps.yml`
   - Update target installation
   - Update verification checks

3. [ ] Verify builds work
   ```bash
   cd rust-test-program
   cargo build --release
   ```

**Validation:**
- [ ] Rust programs build with F extension support
- [ ] CI workflows install correct target

**Deliverables:**
- Updated `rust-test-program/.cargo/config.toml`
- Updated `.github/workflows/copilot-setup-steps.yml`

---

### Phase 9: Documentation Updates (Estimated: 1 day)

**Objective:** Update all documentation to reflect F extension support

**Tasks:**
1. [ ] Update `README.md`
   - Change "RV32IMAC" to "RV32IMACF"
   - Add F extension to feature list
   - Update instruction count (92 → 118 instructions)

2. [ ] Update `AGENTS.md`
   - Add F extension instructions to supported list
   - Update test count (196 → 231+)
   - Add FP operation notes
   - Document FCSR and exception flags

3. [ ] Update `test_programs/README.md`
   - Add F extension examples
   - Update assembler flags

4. [ ] Update `cpu-sim/README.md`
   - Add FP operation documentation
   - Add example FP programs

**Validation:**
- [ ] All documentation accurate and complete
- [ ] No broken links or references

**Deliverables:**
- Updated `README.md`
- Updated `AGENTS.md`
- Updated `test_programs/README.md`
- Updated `cpu-sim/README.md`

---

### Phase 10: Final Validation and Compliance (Estimated: 2-3 days)

🔄 **Updated for Current Test Baseline**

**Objective:** Comprehensive testing and compliance verification

**Tasks:**
1. [ ] Run complete test suite
   ```bash
   cargo test --verbose
   cargo test --package cpu-sim
   # Expect 231-241 total tests passing
   ```

2. [ ] Run code quality checks
   ```bash
   cargo fmt -- --check
   cargo clippy -- -D warnings
   verilator --lint-only rtl/*.sv
   ```

3. [ ] Test with RISC-V compliance suite (optional)
   ```bash
   # Run riscv-arch-test F extension tests
   ```

4. [ ] Performance testing
   - Measure simulation speed
   - Check for timing issues
   - Profile critical paths
   - **Multi-cycle:** Verify FP instruction cycle counts are as expected

5. [ ] Edge case testing
   - Test all rounding modes
   - Test all exception conditions
   - Test NaN, infinity, subnormal handling
   - Test FP after compressed instructions
   - Test FP after atomic operations

**Validation:**
- [ ] All 231+ tests pass
- [ ] Code quality checks pass
- [ ] Compliance tests pass (if run)
- [ ] No performance regressions
- [ ] Multi-cycle timing correct

**Deliverables:**
- Test report document
- Performance analysis (optional)

---

## Summary of Implementation Phases

🔄 **Updated Timeline and Dependencies**

| Phase | Duration | Dependencies | Key Deliverables |
|-------|----------|--------------|------------------|
| Phase 1 | 1-2 days | None | FP register file RTL + tests |
| Phase 2 | 4-6 days | Phase 1 | Basic FPU RTL + tests (multi-cycle decision) |
| Phase 3 | 3-4 days | Phase 2 | Complete FPU with all operations |
| Phase 4 | 2-3 days | Phase 3 | Updated decoder (RV32IMAC integration) |
| Phase 5 | 3-4 days | Phases 1-4 | Integrated top module (12-state FSM) |
| Phase 6 | 3-4 days | Phase 5 | CPU integration tests (multi-cycle) |
| Phase 7 | 1-2 days | Phase 6 | Assembly test programs (rv32imacf) |
| Phase 8 | 1 day | None (parallel) | Build configuration |
| Phase 9 | 1 day | All phases | Documentation |
| Phase 10 | 2-3 days | All phases | Final validation (231+ tests) |

**Total Estimated Time: 20-32 days** (increased from original 16-25 due to multi-cycle complexity)

**Critical Path:** Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6 → Phase 10

**Parallel Activities:** Phase 8 can start anytime after Phase 1

**Key Complexity Factors:**
- Multi-cycle architecture requires more careful FSM integration (+4-7 days)
- Integration with existing RV32AC extensions adds testing overhead
- Higher test baseline (196 vs 84) means more regression testing

---

## Risk Assessment

🔄 **Updated for Multi-Cycle Architecture**

### High-Risk Areas

#### 1. Floating Point Hardware Complexity

**Risk:** FP arithmetic (especially division and square root) creates very long combinational paths, potentially violating timing in single-cycle design.

**Impact:** HIGH (blocking issue for implementation)

**Mitigation (UPDATED):**
- ✅ **Multi-cycle architecture makes this more manageable** - can spread operations across cycles
- Initial implementation uses SystemVerilog `shortreal` with synthesis tool inference
- Monitor synthesis reports for critical path violations
- **Recommended:** Use multi-cycle FP operations (iterative DIV/SQRT with 8-32 cycles)
- Add dedicated FP_EXECUTE or FP_WAIT states to FSM if needed
- May need to adjust FSM to wait for FPU completion (busy/done signals)
- Simpler operations (ADD, MUL, CMP) can likely be single-cycle within EXECUTE state

**Likelihood:** MEDIUM (reduced from HIGH due to multi-cycle architecture)

#### 2. FSM Integration Complexity (NEW RISK)

**Risk:** Integrating FP execution into existing 12-state FSM is complex, especially with atomic operations and compressed instructions.

**Impact:** HIGH (could break existing functionality)

**Mitigation:**
- Thorough regression testing (all 196 existing tests must pass)
- Test FP instructions following compressed instructions
- Test FP instructions following atomic operations
- Use concrete simulation data ($display) rather than abstract reasoning
- Add FP-specific FSM states if single EXECUTE state becomes too complex
- Document state machine transitions clearly

**Likelihood:** MEDIUM

#### 3. IEEE 754 Compliance

**Risk:** Incorrect handling of special values (NaN, infinity, subnormal) and rounding modes leads to non-compliant behavior.

**Mitigation:**
- Use SystemVerilog built-in FP types (`shortreal`)
- Extensive testing with RISC-V compliance suite
- Test all edge cases explicitly
- Reference implementation comparison (QEMU, Spike)
- Document any known deviations

**Impact:** HIGH  
**Likelihood:** MEDIUM

### Medium-Risk Areas

#### 4. Backwards Compatibility (ELEVATED PRIORITY)

**Risk:** F extension changes break existing RV32IMAC functionality.

**Impact:** HIGH (would require extensive rework)  
**Likelihood:** MEDIUM (higher due to complex existing architecture)

**Mitigation:**
- Run all 196 existing tests after each change (not just 84)
- Keep integer and FP data paths separate
- No changes to integer ALU or register file
- Regression testing at every phase
- Test interactions: FP after compressed, FP after atomic operations
- Ensure fetch_buffer, decompress, mem_interface remain unchanged

#### 5. Multi-Cycle Timing Complexity (NEW RISK)

**Risk:** FP operations introduce unexpected stalls or timing issues in multi-cycle execution.

**Impact:** MEDIUM  
**Likelihood:** MEDIUM

**Mitigation:**
- Carefully design FPU busy/done signaling
- Test with variable memory latency
- Document expected cycle counts for each FP operation
- Add cycle count validation tests
- Profile performance before and after FP addition

#### 5. Toolchain Limitations

**Risk:** Rust/assembly toolchain may not fully support RV32F or may have bugs.

**Mitigation:**
- Test with multiple toolchain versions
- Use well-established targets (riscv32gc)
- Verify assembly output manually
- Document toolchain requirements

**Impact:** Medium
**Likelihood:** Medium

#### 6. Simulation Performance

**Risk:** FP operations significantly slow down simulation.

**Mitigation:**
- Profile simulation performance
- Optimize FPU implementation if needed
- Use caching strategies
- Consider parallel testing

**Impact:** Low
**Likelihood:** Medium

### Low-Risk Areas

#### 7. Documentation Drift

**Risk:** Documentation doesn't reflect actual implementation.

**Mitigation:**
- Update documentation in parallel with implementation
- Review documentation in final phase
- Include examples that are actually tested

**Impact:** Low
**Likelihood:** Low

---

## Validation Criteria

### Functional Validation

🔄 **Updated for Multi-Cycle Architecture**

**RTL Level:**
- [ ] FP register file stores and retrieves 32-bit FP values correctly
- [ ] FPU produces IEEE 754-compliant results for all operations
- [ ] Special values (NaN, infinity, zero) handled correctly
- [ ] All 26 F extension instructions decode correctly
- [ ] Exception flags (fflags) set correctly
- [ ] Rounding modes honored (or documented as unsupported)
- [ ] **Multi-cycle:** FPU busy/done signals work correctly
- [ ] **Multi-cycle:** FSM properly sequences through FP instruction execution

**CPU Level:**
- [ ] FLW/FSW load and store FP values correctly
- [ ] FP instructions execute in CPU context
- [ ] FP and integer register files operate independently
- [ ] FP/integer conversions work bidirectionally
- [ ] FCSR CSR accessible and functional
- [ ] Exception flags accumulate correctly
- [ ] **Multi-cycle:** FP operations complete in expected cycle counts
- [ ] **Multi-cycle:** FP instructions work with variable memory latency
- [ ] **Integration:** FP instructions work after compressed instructions
- [ ] **Integration:** FP instructions work after atomic operations

**System Level:**
- [ ] Assembly FP programs execute correctly
- [ ] Rust FP programs (using `f32`) work correctly
- [ ] CPU simulator runs F extension ELF files
- [ ] Multi-instruction FP sequences produce correct results
- [ ] **Multi-cycle:** Instruction trace shows correct FP execution timing

### Quality Validation

**Code Quality:**
- [ ] `cargo fmt -- --check` passes
- [ ] `cargo clippy -- -D warnings` passes
- [ ] `verilator --lint-only rtl/*.sv` passes for all files
- [ ] No compiler warnings in Rust or SystemVerilog

**Testing:**
- [ ] Test count increases from 196 to 231+ (35+ new tests)
- [ ] All new FP tests pass
- [ ] All existing tests pass (no regressions on 196 tests)
- [ ] Code coverage includes all FP instructions
- [ ] Edge cases comprehensively tested
- [ ] Multi-cycle timing verified in tests

**Documentation:**
- [ ] README.md updated with RV32F support (RV32IMAC → RV32IMACF)
- [ ] AGENTS.md updated with F extension instructions
- [ ] Test programs documented with FP examples
- [ ] Build configuration changes documented
- [ ] Known limitations documented
- [ ] Multi-cycle FP execution timing documented

### CI/CD Validation

**Automated Checks:**
- [ ] GitHub Actions CI passes all jobs
- [ ] Build job completes successfully
- [ ] Test job runs all 231+ tests successfully
- [ ] Format check passes
- [ ] Clippy check passes

**Manual Review:**
- [ ] Code review completed
- [ ] Architecture changes approved
- [ ] Test coverage deemed sufficient
- [ ] Performance acceptable

---

## Appendices

### Appendix A: RISC-V F Extension Quick Reference

**Floating Point Register Naming:**
- **f0-f31:** 32 floating point registers (separate from x0-x31)

**Common Instructions:**
```assembly
flw f0, 0(x1)          # Load FP word from memory
fsw f1, 4(x2)          # Store FP word to memory
fadd.s f2, f0, f1      # f2 = f0 + f1
fmul.s f3, f0, f1      # f3 = f0 * f1
fmadd.s f4, f0, f1, f2 # f4 = (f0 * f1) + f2
flt.s x3, f0, f1       # x3 = (f0 < f1)
fcvt.w.s x4, f0        # x4 = int(f0)
fcvt.s.w f5, x5        # f5 = float(x5)
```

### Appendix B: IEEE 754 Single-Precision Format

```
31    30      23 22                    0
┌─────┬─────────┬──────────────────────┐
│ S   │ Exponent│      Mantissa        │
└─────┴─────────┴──────────────────────┘
  1 bit  8 bits        23 bits

Sign: 0 = positive, 1 = negative
Exponent: Biased by 127 (actual exponent = exponent - 127)
Mantissa: Implicit leading 1 (1.mantissa)

Value = (-1)^S × 1.mantissa × 2^(exponent-127)
```

**Special Values:**
- **Zero:** Exponent = 0, Mantissa = 0
- **Subnormal:** Exponent = 0, Mantissa ≠ 0
- **Infinity:** Exponent = 255, Mantissa = 0
- **NaN:** Exponent = 255, Mantissa ≠ 0

### Appendix C: Rounding Mode Details

**RNE (Round to Nearest, ties to Even):**
- Round to nearest representable value
- If exactly halfway, round to even (LSB = 0)
- Default mode, most commonly used

**RTZ (Round Toward Zero):**
- Truncate toward zero (like C-style int cast)
- Always rounds down for positive, up for negative

**RDN (Round Down):**
- Always round toward negative infinity
- Useful for interval arithmetic

**RUP (Round Up):**
- Always round toward positive infinity
- Useful for interval arithmetic

**RMM (Round to Nearest, ties to Max Magnitude):**
- Round to nearest representable value
- If exactly halfway, round away from zero

### Appendix D: Exception Flag Meanings

**NV (Invalid Operation):**
- sqrt(negative)
- infinity - infinity
- 0 / 0
- infinity / infinity
- Invalid conversions

**DZ (Divide by Zero):**
- Finite non-zero number divided by zero
- Result is infinity with appropriate sign

**OF (Overflow):**
- Result exceeds maximum representable value
- Result rounded to infinity (or max value depending on rounding mode)

**UF (Underflow):**
- Non-zero result smaller than minimum normal number
- Result may be denormalized or rounded to zero

**NX (Inexact):**
- Result cannot be represented exactly
- Most common exception (almost all FP ops set this)

### Appendix E: FPU Operation Encoding Summary

| Operation | Code | Operands | Result Type |
|-----------|------|----------|-------------|
| FPU_ADD | 0x00 | fs1, fs2 | FP |
| FPU_SUB | 0x01 | fs1, fs2 | FP |
| FPU_MUL | 0x02 | fs1, fs2 | FP |
| FPU_DIV | 0x03 | fs1, fs2 | FP |
| FPU_SQRT | 0x04 | fs1 | FP |
| FPU_MIN | 0x05 | fs1, fs2 | FP |
| FPU_MAX | 0x06 | fs1, fs2 | FP |
| FPU_MADD | 0x07 | fs1, fs2, fs3 | FP |
| FPU_MSUB | 0x08 | fs1, fs2, fs3 | FP |
| FPU_NMSUB | 0x09 | fs1, fs2, fs3 | FP |
| FPU_NMADD | 0x0A | fs1, fs2, fs3 | FP |
| FPU_SGNJ | 0x0B | fs1, fs2 | FP |
| FPU_SGNJN | 0x0C | fs1, fs2 | FP |
| FPU_SGNJX | 0x0D | fs1, fs2 | FP |
| FPU_CVTWS | 0x0E | fs1 | INT |
| FPU_CVTWUS | 0x0F | fs1 | INT |
| FPU_CVTSW | 0x10 | int_src | FP |
| FPU_CVTSWU | 0x11 | int_src | FP |
| FPU_FEQ | 0x12 | fs1, fs2 | INT |
| FPU_FLT | 0x13 | fs1, fs2 | INT |
| FPU_FLE | 0x14 | fs1, fs2 | INT |
| FPU_FCLASS | 0x15 | fs1 | INT |
| FPU_MVXW | 0x16 | fs1 | INT |
| FPU_MVWX | 0x17 | int_src | FP |

### Appendix F: Resources and References

**RISC-V Specifications:**
- [RISC-V Unprivileged ISA v20191213](https://riscv.org/wp-content/uploads/2019/12/riscv-spec-20191213.pdf)
  - Chapter 11: "F" Standard Extension for Single-Precision Floating-Point
- [RISC-V Reader (Patterson & Waterman)](http://riscvbook.com/)

**IEEE 754 Standard:**
- [IEEE 754-2008 Standard](https://ieeexplore.ieee.org/document/4610935)
- [What Every Computer Scientist Should Know About Floating-Point Arithmetic](https://docs.oracle.com/cd/E19957-01/806-3568/ncg_goldberg.html)

**Testing Resources:**
- [RISC-V Architectural Test Suite](https://github.com/riscv-non-isa/riscv-arch-test)
- [RISC-V Tests (riscv-tests)](https://github.com/riscv-software-src/riscv-tests)
- [Berkeley TestFloat](http://www.jhauser.us/arithmetic/TestFloat.html)

**Tools and Simulators:**
- [Spike RISC-V Simulator](https://github.com/riscv-software-src/riscv-isa-sim)
- [QEMU RISC-V](https://www.qemu.org/)
- [Verilator](https://verilator.org/)

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2025-12-31 | GitHub Copilot | Initial comprehensive plan for RV32F extension |
| 1.1 | 2026-01-11 | GitHub Copilot | Updated for multi-cycle RV32IMAC architecture |

**Version 1.1 Changes:**
- Updated architecture from single-cycle RV32IM to multi-cycle RV32IMAC
- Increased test baseline from 84 to 196 tests
- Added multi-cycle FPU implementation considerations
- Updated FSM integration notes (12-state FSM)
- Adjusted timeline estimates (20-32 days vs 16-25 days)
- Updated build targets (riscv32imacf vs riscv32imf)
- Added integration considerations for Atomic and Compressed extensions
- Updated risk assessment with multi-cycle specific risks

---

## Document Status

✅ **Ready for Implementation** (Updated for Current Architecture)

This plan provides a complete roadmap for adding single-precision floating point support to the **multi-cycle RV32IMAC** RISC-V CPU. All phases are detailed with specific tasks, validation criteria, RTL code examples, and estimated timelines. The plan is optimized for AI coding agent implementation with:

- Clear, sequential phases with dependencies
- Specific file names and code snippets
- Comprehensive testing strategy
- Build configuration details
- Risk mitigation strategies
- Validation checklists
- **NEW:** Multi-cycle architecture considerations
- **NEW:** Integration with existing RV32AC extensions

**Important Notes for Implementation:**

⚠️ **Architecture has changed since original plan:**
- CPU is now **multi-cycle** (not single-cycle)
- **12-state FSM** (not 11-state)
- **RV32IMAC** baseline (not RV32IM)
- **196 tests** baseline (not 84)
- Target is `riscv32imacf` (not `riscv32imf`)

⚠️ **Key Implementation Decisions Required:**
1. **FPU Architecture:** Single-cycle vs multi-cycle (recommend multi-cycle for DIV/SQRT)
2. **FSM Extension:** Dedicated FP states vs extending EXECUTE state
3. **Code Examples:** Verify marlin API patterns match current usage

⚠️ **Before Starting:**
1. Review current test patterns in `tests/src/` directory
2. Understand multi-cycle FSM flow in `rtl/top.sv`
3. Study existing decoder logic for RV32IMAC integration patterns
4. Verify Verilator and toolchain versions

**Next Steps:**
1. Review and approve this updated plan
2. Make FPU implementation decision (single vs multi-cycle)
3. Begin Phase 1 implementation
4. Track progress using the phase checklists
5. Report progress after each phase completion

---

**END OF DOCUMENT**
