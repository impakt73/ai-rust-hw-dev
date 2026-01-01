# Current Architecture Analysis

## Overview

This document provides a detailed analysis of the existing single-cycle RV32IM CPU implementation. Understanding the current architecture is essential for planning the multi-cycle conversion.

## RTL Module Hierarchy

```
top.sv (CPU Top-Level)
├── decoder.sv (Instruction Decoder)
├── alu.sv (Arithmetic Logic Unit)
└── regfile.sv (Register File)
```

## Module Analysis

### 1. Top Module (`rtl/top.sv`)

The top module is the CPU core that instantiates and connects all submodules.

#### Interface Summary

```systemverilog
module top (
    // Clock and Reset
    input  logic        clk,
    input  logic        rst_n,
    input  logic [31:0] boot_addr,
    
    // Instruction Memory Interface
    output logic [31:0] imem_addr,    // PC value (instruction address)
    input  logic [31:0] imem_data,    // Fetched instruction
    
    // Data Memory Interface
    output logic [31:0] dmem_addr,    // Computed address (rs1 + imm)
    output logic [31:0] dmem_wdata,   // Store data (rs2 value)
    input  logic [31:0] dmem_rdata,   // Load data
    output logic        dmem_we,      // Write enable
    output logic        dmem_re,      // Read enable
    output logic [1:0]  dmem_size,    // Access size (byte/half/word)
    
    // System Control
    output logic        halted,       // CPU halted (ECALL/EBREAK)
    
    // Debug Outputs
    output logic [31:0] debug_rs1_data,
    output logic [31:0] debug_rs2_data,
    output logic [31:0] debug_rd_data
);
```

#### Key Internal Signals

```systemverilog
// Program Counter
logic [31:0] pc;
logic [31:0] next_pc;
logic [31:0] instruction;

// Decoder Outputs
logic [6:0]  opcode;
logic [4:0]  rd, rs1, rs2;
logic [2:0]  funct3;
logic [6:0]  funct7;
logic [31:0] imm_i, imm_s, imm_b, imm_u, imm_j;
logic [4:0]  alu_op;
logic        alu_src, reg_write, mem_write, mem_read;
logic        mem_to_reg, branch, jump;
logic        is_ecall, is_ebreak, is_fence, is_csr;

// Register File Signals
logic [31:0] rs1_data, rs2_data, rd_data;

// ALU Signals
logic [31:0] alu_a, alu_b, alu_result;
logic        alu_zero;

// Branch Logic
logic        take_branch;

// CSR Registers
logic [31:0] csr_file [0:4095];
logic [11:0] csr_addr;
logic [31:0] csr_rdata;
```

#### Current Datapath Flow (Single-Cycle)

```
     ┌─────────────────────────────────────────────────────────────────────┐
     │                         SINGLE CYCLE                                │
     │                                                                     │
     │  ┌────┐    ┌────────┐    ┌─────────┐    ┌─────┐    ┌─────────┐    │
     │  │ PC │───>│ IMEM   │───>│ DECODER │───>│ ALU │───>│ DMEM    │    │
     │  └────┘    │(extern)│    └─────────┘    └─────┘    │(extern) │    │
     │    │       └────────┘         │            │       └─────────┘    │
     │    │                          │            │            │         │
     │    │                          ▼            │            │         │
     │    │                     ┌─────────┐       │            │         │
     │    │                     │ REGFILE │◄──────┴────────────┘         │
     │    │                     └─────────┘                              │
     │    │                          │                                   │
     │    ◄──────────────────────────┘ (next_pc calculation)             │
     │                                                                   │
     └───────────────────────────────────────────────────────────────────┘
```

**All of the above happens in ONE clock cycle.**

#### PC Logic

```systemverilog
// PC Update (synchronous)
always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        pc <= boot_addr;
    end else if (!halted && !is_ecall && !is_ebreak) begin
        pc <= next_pc;
    end
end

// Next PC Calculation (combinational)
always_comb begin
    if (jump) begin
        if (opcode == 7'b1100111) begin  // JALR
            next_pc = (rs1_data + imm_i) & ~32'h1;
        end else begin  // JAL
            next_pc = pc + imm_j;
        end
    end else if (branch && take_branch) begin
        next_pc = pc + imm_b;
    end else begin
        next_pc = pc + 32'd4;
    end
end
```

#### Memory Interface Logic

```systemverilog
// Instruction Fetch
assign imem_addr = pc;
assign instruction = imem_data;

// Data Memory
assign dmem_addr = alu_result;    // Address from ALU
assign dmem_wdata = rs2_data;     // Store data from rs2
assign dmem_we = mem_write;
assign dmem_re = mem_read;
assign dmem_size = funct3[1:0];   // Size encoding from instruction
```

#### Write-back Logic

```systemverilog
always_comb begin
    if (opcode == 7'b0110111) begin      // LUI
        rd_data = lui_result;
    end else if (opcode == 7'b0010111) begin  // AUIPC
        rd_data = pc + imm_u;
    end else if (jump) begin              // JAL/JALR
        rd_data = pc + 32'd4;
    end else if (is_csr) begin            // CSR instructions
        rd_data = csr_rdata;
    end else if (mem_to_reg) begin        // Load instructions
        rd_data = formatted_load_data;
    end else begin                        // ALU result
        rd_data = alu_result;
    end
end
```

### 2. Decoder Module (`rtl/decoder.sv`)

The decoder is purely combinational - it extracts fields and generates control signals.

#### Interface

```systemverilog
module decoder (
    input  logic [31:0] instruction,
    
    // Instruction Fields
    output logic [6:0]  opcode,
    output logic [4:0]  rd, rs1, rs2,
    output logic [2:0]  funct3,
    output logic [6:0]  funct7,
    
    // Immediate Values
    output logic [31:0] imm_i, imm_s, imm_b, imm_u, imm_j,
    
    // Control Signals
    output logic [4:0]  alu_op,
    output logic        alu_src,      // 0: rs2, 1: immediate
    output logic        reg_write,
    output logic        mem_write,
    output logic        mem_read,
    output logic        mem_to_reg,
    output logic        branch,
    output logic        jump,
    output logic        is_ecall,
    output logic        is_ebreak,
    output logic        is_fence,
    output logic        is_csr
);
```

#### Key Features

- **Field Extraction:** Extracts opcode, rd, rs1, rs2, funct3, funct7
- **Immediate Generation:** Computes all immediate types (I, S, B, U, J)
- **ALU Operation Selection:** Maps instruction to ALU operation code
- **Control Signal Generation:** Generates control signals based on opcode

**No changes needed for multi-cycle:** The decoder will run in the DECODE state and its outputs will be latched.

### 3. ALU Module (`rtl/alu.sv`)

The ALU is purely combinational - it performs arithmetic/logic operations.

#### Interface

```systemverilog
module alu (
    input  logic [31:0] a,
    input  logic [31:0] b,
    input  logic [4:0]  alu_op,
    output logic [31:0] result,
    output logic        zero
);
```

#### Supported Operations

| ALU Op | Operation | Description |
|--------|-----------|-------------|
| 00000 | ADD | a + b |
| 00001 | SUB | a - b |
| 00010 | AND | a & b |
| 00011 | OR | a \| b |
| 00100 | XOR | a ^ b |
| 00101 | SLL | a << b[4:0] |
| 00110 | SRL | a >> b[4:0] |
| 00111 | SRA | a >>> b[4:0] (arithmetic) |
| 01000 | SLT | (signed(a) < signed(b)) ? 1 : 0 |
| 01001 | SLTU | (a < b) ? 1 : 0 (unsigned) |
| 01010 | MUL | (a * b)[31:0] |
| 01011 | MULH | (signed(a) * signed(b))[63:32] |
| 01100 | MULHSU | (signed(a) * unsigned(b))[63:32] |
| 01101 | MULHU | (unsigned(a) * unsigned(b))[63:32] |
| 01110 | DIV | signed(a) / signed(b) |
| 01111 | DIVU | unsigned(a) / unsigned(b) |
| 10000 | REM | signed(a) % signed(b) |
| 10001 | REMU | unsigned(a) % unsigned(b) |

**No changes needed for multi-cycle:** The ALU remains purely combinational. It will be reused in different cycles with different operands.

### 4. Register File (`rtl/regfile.sv`)

The register file has asynchronous reads and synchronous writes.

#### Interface

```systemverilog
module regfile (
    input  logic        clk,
    input  logic        we,           // Write enable
    input  logic [4:0]  rs1_addr,     // Read port 1 address
    input  logic [4:0]  rs2_addr,     // Read port 2 address
    input  logic [4:0]  rd_addr,      // Write port address
    input  logic [31:0] rd_data,      // Write data
    output logic [31:0] rs1_data,     // Read port 1 data
    output logic [31:0] rs2_data      // Read port 2 data
);
```

#### Key Features

- **x0 Hardwired:** Register x0 always reads as 0
- **Dual Read Ports:** Simultaneous read of rs1 and rs2
- **Single Write Port:** Writes on clock edge when we=1 and rd≠0

**Minor changes for multi-cycle:** Write enable will be gated by FSM state (only in WRITEBACK).

## Current Timing Analysis

### Single-Cycle Critical Path

In the current design, the critical path is:

```
PC → IMEM → Decoder → RegFile(read) → ALU → DMEM → RegFile(write)
     ↑                                       ↓
     └───────────────────────────────────────┘
                  (next_pc calculation)
```

This entire path must complete in one clock cycle.

### Cycle Breakdown

| Operation | Approximate Delay |
|-----------|-------------------|
| PC to IMEM | Memory access time |
| Instruction Decode | Combinational logic |
| Register Read | Mux + read |
| ALU Operation | Varies (division is slowest) |
| Memory Access | Memory access time |
| Write-back Mux | Combinational logic |

**Division is the bottleneck:** Hardware division can take 30+ gate delays.

## Data Dependencies and Hazards

In the single-cycle design, there are no hazards because:

1. **Each instruction completes in one cycle**
2. **Register file writes happen at clock edge**
3. **Next instruction uses updated values**

In multi-cycle, we need to consider:

1. **Structural Hazards:** Sharing ALU between operations (resolved by FSM)
2. **Data Hazards:** Not applicable (no pipelining, sequential execution)
3. **Control Hazards:** Not applicable (no speculative execution)

## Memory Access Patterns

### Instruction Memory

| Access Type | When | Duration |
|-------------|------|----------|
| Fetch | Every cycle (current) | Immediate |
| Fetch | FETCH state (multi-cycle) | 1 cycle |

### Data Memory

| Access Type | When | Duration |
|-------------|------|----------|
| Load | Load instruction | Immediate (current) |
| Store | Store instruction | Immediate (current) |
| Load | MEM_READ state (multi-cycle) | 1 cycle |
| Store | MEM_WRITE state (multi-cycle) | 1 cycle |

## CSR Implementation

The current implementation includes a CSR file:

```systemverilog
logic [31:0] csr_file [0:4095];  // 4096 CSR registers
```

CSR operations:
- CSRRW: Read CSR, write rs1 to CSR
- CSRRS: Read CSR, set bits from rs1
- CSRRC: Read CSR, clear bits from rs1
- CSRRWI/CSRRSI/CSRRCI: Immediate variants

**For multi-cycle:** CSR operations will execute in DECODE→EXECUTE→WRITEBACK sequence.

## Summary of Components to Modify

| Component | Modification Level | Key Changes |
|-----------|-------------------|-------------|
| `top.sv` | **Major** | Add FSM, registers, multi-cycle control |
| `decoder.sv` | **None** | Runs in DECODE state, outputs latched |
| `alu.sv` | **None** | Runs in EXECUTE state, reused for PC |
| `regfile.sv` | **Minor** | Write gated by WRITEBACK state |

## Key Observations for Multi-Cycle Conversion

1. **ALU is reusable:** Can compute address, arithmetic, and PC increment
2. **Decoder outputs are stable:** Latch once in DECODE state
3. **Memory interface is separate:** Can be accessed in dedicated states
4. **Register file timing works:** Async read, sync write fits multi-cycle

---

**Next Document:** [02-state-machine-design.md](02-state-machine-design.md)
