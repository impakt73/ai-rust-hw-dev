# State Machine Design and Control Unit

## Overview

This document details the finite state machine (FSM) design for the multi-cycle CPU. The control unit is the heart of the multi-cycle implementation, orchestrating instruction execution across multiple clock cycles.

## State Encoding

### State List

We define the following states for instruction execution:

```systemverilog
typedef enum logic [3:0] {
    S_IDLE       = 4'b0000,  // Initial/reset state
    S_FETCH      = 4'b0001,  // Instruction fetch
    S_DECODE     = 4'b0010,  // Instruction decode
    S_EXECUTE    = 4'b0011,  // ALU execution (R-type, I-type)
    S_MEM_ADDR   = 4'b0100,  // Memory address calculation
    S_MEM_READ   = 4'b0101,  // Memory read (load)
    S_MEM_WRITE  = 4'b0110,  // Memory write (store)
    S_WRITEBACK  = 4'b0111,  // Register write-back
    S_BRANCH     = 4'b1000,  // Branch resolution
    S_JUMP       = 4'b1001,  // Jump execution
    S_CSR        = 4'b1010,  // CSR operation
    S_HALT       = 4'b1011   // ECALL/EBREAK halt
} state_t;
```

### State Descriptions

| State | Purpose | Actions |
|-------|---------|---------|
| S_IDLE | Initial state after reset | Wait for start, transition to FETCH |
| S_FETCH | Fetch instruction from memory | PC → imem_addr, latch imem_data to IR |
| S_DECODE | Decode instruction, read registers | Decoder runs, latch rs1/rs2 to A/B registers |
| S_EXECUTE | Perform ALU operation | ALU computes result, latch to ALU_OUT |
| S_MEM_ADDR | Calculate memory address | ALU computes rs1 + offset for load/store |
| S_MEM_READ | Read from data memory | dmem_re = 1, latch dmem_rdata to MDR |
| S_MEM_WRITE | Write to data memory | dmem_we = 1, write rs2 to dmem |
| S_WRITEBACK | Write result to register file | reg_write = 1, rd_data → regfile |
| S_BRANCH | Evaluate branch condition | Compare rs1/rs2, update PC if taken |
| S_JUMP | Execute JAL/JALR | Compute target, save return address |
| S_CSR | CSR read/write operation | Read/modify CSR, prepare writeback |
| S_HALT | CPU halted | Stop execution, hold PC |

## State Transition Diagram

### Main Flow

```
              ┌────────────────────────────────────────────────────────┐
              │                                                        │
              ▼                                                        │
        ┌───────────┐                                                  │
        │  S_IDLE   │                                                  │
        └─────┬─────┘                                                  │
              │ rst_n = 1                                              │
              ▼                                                        │
        ┌───────────┐                                                  │
    ┌───│  S_FETCH  │◄─────────────────────────────────────────────────┤
    │   └─────┬─────┘                                                  │
    │         │                                                        │
    │         ▼                                                        │
    │   ┌───────────┐                                                  │
    │   │ S_DECODE  │                                                  │
    │   └─────┬─────┘                                                  │
    │         │                                                        │
    │         ├──────────────────┬──────────────────┬─────────────┐    │
    │         │                  │                  │             │    │
    │         ▼                  ▼                  ▼             ▼    │
    │   ┌──────────┐      ┌──────────┐      ┌──────────┐   ┌─────────┐│
    │   │S_EXECUTE │      │S_MEM_ADDR│      │ S_BRANCH │   │ S_HALT  ││
    │   │(R/I-type)│      │(Load/Str)│      │          │   │         ││
    │   └────┬─────┘      └────┬─────┘      └────┬─────┘   └─────────┘│
    │        │                 │                 │                     │
    │        ▼                 ├─────────┐       │                     │
    │   ┌──────────┐          ▼         ▼       │                     │
    │   │S_WRITEBK │    ┌─────────┐┌─────────┐  │                     │
    │   └────┬─────┘    │S_MEM_RD ││S_MEM_WR │  │                     │
    │        │          └────┬────┘└────┬────┘  │                     │
    │        │               │          │       │                     │
    │        │               ▼          │       │                     │
    │        │          ┌─────────┐     │       │                     │
    │        │          │S_WRITEBK│     │       │                     │
    │        │          └────┬────┘     │       │                     │
    │        │               │          │       │                     │
    └────────┴───────────────┴──────────┴───────┘                     │
                             │                                         │
                             └─────────────────────────────────────────┘
```

### Detailed Transitions

#### From S_IDLE
```
S_IDLE → S_FETCH : rst_n = 1
```

#### From S_FETCH
```
S_FETCH → S_DECODE : always (after 1 cycle)
```

#### From S_DECODE
```
S_DECODE → S_EXECUTE  : R-type, I-type arithmetic, LUI, AUIPC, JAL, JALR
S_DECODE → S_MEM_ADDR : Load or Store
S_DECODE → S_BRANCH   : Branch instruction
S_DECODE → S_CSR      : CSR instruction
S_DECODE → S_HALT     : ECALL or EBREAK
S_DECODE → S_FETCH    : FENCE (NOP behavior)
```

#### From S_EXECUTE
```
S_EXECUTE → S_WRITEBACK : always (write ALU result to rd)
```

#### From S_MEM_ADDR
```
S_MEM_ADDR → S_MEM_READ  : Load instruction
S_MEM_ADDR → S_MEM_WRITE : Store instruction
```

#### From S_MEM_READ
```
S_MEM_READ → S_WRITEBACK : always (write loaded data to rd)
```

#### From S_MEM_WRITE
```
S_MEM_WRITE → S_FETCH : always (store complete)
```

#### From S_WRITEBACK
```
S_WRITEBACK → S_FETCH : always (instruction complete)
```

#### From S_BRANCH
```
S_BRANCH → S_FETCH : always (PC updated if taken)
```

#### From S_CSR
```
S_CSR → S_WRITEBACK : always (CSR value to rd)
```

#### From S_HALT
```
S_HALT → S_HALT : always (stay halted)
```

## Instruction Execution Sequences

### R-Type Instructions (ADD, SUB, AND, OR, XOR, SLL, SRL, SRA, SLT, SLTU, MUL, DIV, etc.)

| Cycle | State | Actions |
|-------|-------|---------|
| 1 | S_FETCH | IR ← Mem[PC] |
| 2 | S_DECODE | A ← rs1, B ← rs2, decode instruction |
| 3 | S_EXECUTE | ALU_OUT ← A op B |
| 4 | S_WRITEBACK | Reg[rd] ← ALU_OUT, PC ← PC + 4 |

**Total: 4 cycles**

### I-Type Arithmetic (ADDI, ANDI, ORI, XORI, SLTI, SLTIU, SLLI, SRLI, SRAI)

| Cycle | State | Actions |
|-------|-------|---------|
| 1 | S_FETCH | IR ← Mem[PC] |
| 2 | S_DECODE | A ← rs1, B ← imm_i, decode instruction |
| 3 | S_EXECUTE | ALU_OUT ← A op B |
| 4 | S_WRITEBACK | Reg[rd] ← ALU_OUT, PC ← PC + 4 |

**Total: 4 cycles**

### Load Instructions (LW, LH, LB, LHU, LBU)

| Cycle | State | Actions |
|-------|-------|---------|
| 1 | S_FETCH | IR ← Mem[PC] |
| 2 | S_DECODE | A ← rs1, decode instruction |
| 3 | S_MEM_ADDR | ALU_OUT ← A + imm_i (address) |
| 4 | S_MEM_READ | MDR ← Mem[ALU_OUT] (sign/zero extend) |
| 5 | S_WRITEBACK | Reg[rd] ← MDR, PC ← PC + 4 |

**Total: 5 cycles**

### Store Instructions (SW, SH, SB)

| Cycle | State | Actions |
|-------|-------|---------|
| 1 | S_FETCH | IR ← Mem[PC] |
| 2 | S_DECODE | A ← rs1, B ← rs2, decode instruction |
| 3 | S_MEM_ADDR | ALU_OUT ← A + imm_s (address) |
| 4 | S_MEM_WRITE | Mem[ALU_OUT] ← B, PC ← PC + 4 |

**Total: 4 cycles**

### Branch Instructions (BEQ, BNE, BLT, BGE, BLTU, BGEU)

| Cycle | State | Actions |
|-------|-------|---------|
| 1 | S_FETCH | IR ← Mem[PC] |
| 2 | S_DECODE | A ← rs1, B ← rs2, decode instruction |
| 3 | S_BRANCH | Compare A and B, PC ← taken ? (PC + imm_b) : (PC + 4) |

**Total: 3 cycles**

### Jump Instructions (JAL)

| Cycle | State | Actions |
|-------|-------|---------|
| 1 | S_FETCH | IR ← Mem[PC] |
| 2 | S_DECODE | decode instruction |
| 3 | S_EXECUTE | ALU_OUT ← PC + 4 (return address) |
| 4 | S_WRITEBACK | Reg[rd] ← ALU_OUT, PC ← PC + imm_j |

**Total: 4 cycles**

### Jump Register (JALR)

| Cycle | State | Actions |
|-------|-------|---------|
| 1 | S_FETCH | IR ← Mem[PC] |
| 2 | S_DECODE | A ← rs1, decode instruction |
| 3 | S_EXECUTE | ALU_OUT ← PC + 4, target ← (A + imm_i) & ~1 |
| 4 | S_WRITEBACK | Reg[rd] ← ALU_OUT, PC ← target |

**Total: 4 cycles**

### LUI (Load Upper Immediate)

| Cycle | State | Actions |
|-------|-------|---------|
| 1 | S_FETCH | IR ← Mem[PC] |
| 2 | S_DECODE | decode instruction |
| 3 | S_EXECUTE | ALU_OUT ← imm_u |
| 4 | S_WRITEBACK | Reg[rd] ← ALU_OUT, PC ← PC + 4 |

**Total: 4 cycles**

### AUIPC (Add Upper Immediate to PC)

| Cycle | State | Actions |
|-------|-------|---------|
| 1 | S_FETCH | IR ← Mem[PC] |
| 2 | S_DECODE | decode instruction |
| 3 | S_EXECUTE | ALU_OUT ← PC + imm_u |
| 4 | S_WRITEBACK | Reg[rd] ← ALU_OUT, PC ← PC + 4 |

**Total: 4 cycles**

### CSR Instructions (CSRRW, CSRRS, CSRRC, etc.)

| Cycle | State | Actions |
|-------|-------|---------|
| 1 | S_FETCH | IR ← Mem[PC] |
| 2 | S_DECODE | A ← rs1 (or zimm), read CSR |
| 3 | S_CSR | Modify CSR, prepare old value |
| 4 | S_WRITEBACK | Reg[rd] ← old_csr, PC ← PC + 4 |

**Total: 4 cycles**

### FENCE

| Cycle | State | Actions |
|-------|-------|---------|
| 1 | S_FETCH | IR ← Mem[PC] |
| 2 | S_DECODE | No action needed, PC ← PC + 4 |

**Total: 2 cycles** (NOP-like behavior)

### ECALL/EBREAK

| Cycle | State | Actions |
|-------|-------|---------|
| 1 | S_FETCH | IR ← Mem[PC] |
| 2 | S_DECODE | Detect halt condition |
| 3+ | S_HALT | CPU halted |

**Total: 2 cycles + halt**

## Control Signal Generation

### Control Signals by State

| State | IR_Write | PC_Write | Reg_Write | Mem_Read | Mem_Write | ALU_SrcA | ALU_SrcB |
|-------|----------|----------|-----------|----------|-----------|----------|----------|
| S_IDLE | 0 | 0 | 0 | 0 | 0 | - | - |
| S_FETCH | 1 | 0 | 0 | 0 | 0 | - | - |
| S_DECODE | 0 | 0 | 0 | 0 | 0 | - | - |
| S_EXECUTE | 0 | 0 | 0 | 0 | 0 | A_reg | B_reg/imm |
| S_MEM_ADDR | 0 | 0 | 0 | 0 | 0 | A_reg | imm |
| S_MEM_READ | 0 | 0 | 0 | 1 | 0 | - | - |
| S_MEM_WRITE | 0 | 1 | 0 | 0 | 1 | - | - |
| S_WRITEBACK | 0 | 1 | 1 | 0 | 0 | - | - |
| S_BRANCH | 0 | 1 | 0 | 0 | 0 | A_reg | B_reg |
| S_CSR | 0 | 0 | 0 | 0 | 0 | - | - |
| S_HALT | 0 | 0 | 0 | 0 | 0 | - | - |

### ALU Source Selection

```systemverilog
typedef enum logic [1:0] {
    ALUSRC_A_REG     = 2'b00,  // A register (latched rs1)
    ALUSRC_A_PC      = 2'b01,  // Current PC
    ALUSRC_A_ZERO    = 2'b10   // Zero (for LUI)
} alu_src_a_t;

typedef enum logic [1:0] {
    ALUSRC_B_REG     = 2'b00,  // B register (latched rs2)
    ALUSRC_B_IMM     = 2'b01,  // Immediate (from decoder)
    ALUSRC_B_FOUR    = 2'b10   // Constant 4 (for PC+4)
} alu_src_b_t;
```

### Write-back Source Selection

```systemverilog
typedef enum logic [1:0] {
    WBSRC_ALU       = 2'b00,  // ALU output
    WBSRC_MEM       = 2'b01,  // Memory data (MDR)
    WBSRC_CSR       = 2'b10   // CSR read data
} wb_src_t;
```

## FSM Implementation

### State Register

```systemverilog
state_t current_state, next_state;

always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        current_state <= S_IDLE;
    end else begin
        current_state <= next_state;
    end
end
```

### Next State Logic

```systemverilog
always_comb begin
    next_state = current_state;  // Default: stay in current state
    
    case (current_state)
        S_IDLE: begin
            next_state = S_FETCH;
        end
        
        S_FETCH: begin
            next_state = S_DECODE;
        end
        
        S_DECODE: begin
            case (opcode)
                OP_REG, OP_IMM, OP_LUI, OP_AUIPC, OP_JAL, OP_JALR:
                    next_state = S_EXECUTE;
                OP_LOAD, OP_STORE:
                    next_state = S_MEM_ADDR;
                OP_BRANCH:
                    next_state = S_BRANCH;
                OP_SYSTEM: begin
                    if (is_ecall || is_ebreak)
                        next_state = S_HALT;
                    else if (is_csr)
                        next_state = S_CSR;
                    else  // FENCE
                        next_state = S_FETCH;
                end
                default:
                    next_state = S_FETCH;  // Invalid: skip
            endcase
        end
        
        S_EXECUTE: begin
            next_state = S_WRITEBACK;
        end
        
        S_MEM_ADDR: begin
            if (mem_read)
                next_state = S_MEM_READ;
            else
                next_state = S_MEM_WRITE;
        end
        
        S_MEM_READ: begin
            next_state = S_WRITEBACK;
        end
        
        S_MEM_WRITE: begin
            next_state = S_FETCH;
        end
        
        S_WRITEBACK: begin
            next_state = S_FETCH;
        end
        
        S_BRANCH: begin
            next_state = S_FETCH;
        end
        
        S_CSR: begin
            next_state = S_WRITEBACK;
        end
        
        S_HALT: begin
            next_state = S_HALT;  // Stay halted
        end
        
        default: begin
            next_state = S_IDLE;
        end
    endcase
end
```

### Control Signal Output Logic

```systemverilog
always_comb begin
    // Default all control signals to inactive
    ir_write = 1'b0;
    pc_write = 1'b0;
    reg_write_en = 1'b0;
    dmem_re = 1'b0;
    dmem_we = 1'b0;
    a_reg_write = 1'b0;
    b_reg_write = 1'b0;
    alu_out_write = 1'b0;
    mdr_write = 1'b0;
    instr_complete = 1'b0;
    
    case (current_state)
        S_FETCH: begin
            ir_write = 1'b1;  // Latch instruction
        end
        
        S_DECODE: begin
            a_reg_write = 1'b1;  // Latch rs1 data
            b_reg_write = 1'b1;  // Latch rs2 data
            // Handle FENCE as complete
            if (is_fence) begin
                pc_write = 1'b1;
                instr_complete = 1'b1;
            end
        end
        
        S_EXECUTE: begin
            alu_out_write = 1'b1;  // Latch ALU result
        end
        
        S_MEM_ADDR: begin
            alu_out_write = 1'b1;  // Latch address
        end
        
        S_MEM_READ: begin
            dmem_re = 1'b1;
            mdr_write = 1'b1;  // Latch memory data
        end
        
        S_MEM_WRITE: begin
            dmem_we = 1'b1;
            pc_write = 1'b1;
            instr_complete = 1'b1;
        end
        
        S_WRITEBACK: begin
            reg_write_en = 1'b1;
            pc_write = 1'b1;
            instr_complete = 1'b1;
        end
        
        S_BRANCH: begin
            pc_write = 1'b1;  // Update PC (taken or not)
            instr_complete = 1'b1;
        end
        
        S_CSR: begin
            // CSR operations happen here
            // Writeback in next state
        end
        
        S_HALT: begin
            // Do nothing, stay halted
        end
        
        default: begin
            // Default: all signals inactive
        end
    endcase
end
```

## Instruction Complete Signal

The `instr_complete` signal is asserted for exactly one cycle when an instruction finishes:

```systemverilog
// States where instruction completes:
// - S_WRITEBACK (R-type, I-type, Load, LUI, AUIPC, JAL, JALR, CSR)
// - S_MEM_WRITE (Store)
// - S_BRANCH (Branch)
// - S_DECODE (FENCE only)

assign instr_complete = (current_state == S_WRITEBACK) ||
                        (current_state == S_MEM_WRITE) ||
                        (current_state == S_BRANCH) ||
                        (current_state == S_DECODE && is_fence);
```

This signal is crucial for the host simulator to know when an instruction has finished executing.

## PC Update Logic

```systemverilog
logic [31:0] next_pc_value;

always_comb begin
    next_pc_value = pc + 32'd4;  // Default: sequential
    
    case (current_state)
        S_BRANCH: begin
            if (take_branch)
                next_pc_value = pc + imm_b_latched;
            else
                next_pc_value = pc + 32'd4;
        end
        
        S_WRITEBACK: begin
            if (is_jal_latched)
                next_pc_value = pc + imm_j_latched;
            else if (is_jalr_latched)
                next_pc_value = (a_reg + imm_i_latched) & ~32'h1;
            else
                next_pc_value = pc + 32'd4;
        end
        
        default: begin
            next_pc_value = pc + 32'd4;
        end
    endcase
end

always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        pc <= boot_addr;
    end else if (pc_write) begin
        pc <= next_pc_value;
    end
end
```

---

**Next Document:** [03-rtl-modifications.md](03-rtl-modifications.md)
