# Multi-Cycle CPU Implementation Plan

## Executive Summary

**Goal:** Convert the single-cycle RV32IM CPU to a **multi-cycle, latency-insensitive** design with minimal risk and complexity.

**Strategy:** 
- Avoid pipelining and complex performance features
- Focus on functional correctness with a straightforward state machine
- Support variable-latency memory operations with proper handshaking
- Use staging registers (flip-flops), not latches, for FPGA compatibility

**Impact:** Instructions will take variable cycles (minimum 3-5 cycles base, plus variable memory latency) instead of fixed 1 cycle. External memory interfaces enhanced with handshaking signals.

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture Changes](#architecture-changes)
3. [State Machine Design](#state-machine-design)
4. [Memory Interface with Variable Latency](#memory-interface-with-variable-latency)
5. [RTL Modifications](#rtl-modifications)
6. [Simulator Updates](#simulator-updates)
7. [Test Framework Updates](#test-framework-updates)
8. [Implementation Phases](#implementation-phases)
9. [Success Criteria](#success-criteria)
10. [Risk Mitigation](#risk-mitigation)

---

## Overview

### Current Architecture (Single-Cycle)

- All instructions complete in exactly 1 clock cycle
- Long critical path limits maximum clock frequency
- Simple combinational control logic
- Memory operations assumed to complete immediately
- No support for variable-latency operations

### Target Architecture (Multi-Cycle)

- Instructions take variable cycles (3-5+ cycles base, plus memory latency)
- Shorter critical path enables higher clock frequency
- FSM-based control with 11 states
- Resource sharing (ALU reused for address calculation, PC increment)
- **Variable-latency memory support with ready/valid handshaking**
- **Staging registers (flip-flops) for multi-cycle operation** (FPGA-safe, no latches)

---

## Architecture Changes

### What Changes

| Component | Change Level | Key Modifications |
|-----------|--------------|-------------------|
| `rtl/top.sv` | **MAJOR** | Add FSM, staging registers, multi-cycle control, memory handshaking |
| `rtl/pc_control.sv` | **REMOVE** | Logic moved into top.sv FSM |
| `rtl/mem_interface.sv` | **MAJOR** | Add ready/valid handshaking for variable-latency memory |
| `rtl/writeback_mux.sv` | Minor | Selection driven by registered signals |
| `rtl/decoder.sv` | None | Unchanged, outputs captured in registers |
| `rtl/alu.sv` | None | Unchanged, used in different cycles |
| `rtl/regfile.sv` | None | Unchanged, write enable gated by FSM |
| `rtl/branch_unit.sv` | None | Unchanged |
| `rtl/csr_file.sv` | None | Unchanged |
| `cpu-sim/src/sim.rs` | Minor | Loop until `instr_complete` signal, handle memory handshaking |
| `tests/src/cpu_test.rs` | Minor | Add helper macro for multi-cycle execution |

### New Output Signals

```systemverilog
// Instruction completion signaling
output logic instr_complete    // High for 1 cycle when instruction done

// Instruction memory handshaking (NEW for variable latency)
output logic imem_req          // Request instruction fetch
input  logic imem_ready        // Memory has valid data

// Data memory handshaking (NEW for variable latency)
output logic dmem_req          // Request data memory operation
input  logic dmem_ready        // Memory operation complete
```

### Staging Registers (Flip-Flops, NOT Latches)

**Important:** All intermediate storage uses regular synchronous flip-flops clocked on the rising edge. No combinational latches are used (FPGA-safe design).

```systemverilog
// Instruction staging register (flip-flop based)
logic [31:0] ir_reg;
logic ir_write;

always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) 
        ir_reg <= 32'h0;
    else if (ir_write)
        ir_reg <= imem_data;
end

// Operand staging registers (flip-flops)
logic [31:0] a_reg;  // rs1 data
logic [31:0] b_reg;  // rs2 data
logic a_reg_write, b_reg_write;

always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        a_reg <= 32'h0;
        b_reg <= 32'h0;
    end else begin
        if (a_reg_write) a_reg <= rs1_data;
        if (b_reg_write) b_reg <= rs2_data;
    end
end

// Result staging registers (flip-flops)
logic [31:0] alu_out_reg;  // ALU output
logic [31:0] mdr;          // Memory data register
logic alu_out_write, mdr_write;

always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        alu_out_reg <= 32'h0;
        mdr <= 32'h0;
    end else begin
        if (alu_out_write) alu_out_reg <= alu_result;
        if (mdr_write) mdr <= formatted_load_data;
    end
end

// Control signal staging registers (flip-flops)
// All decoder outputs captured synchronously
logic [6:0]  opcode_reg;
logic [4:0]  rd_reg, rs1_reg, rs2_reg;
logic [2:0]  funct3_reg;
logic [31:0] imm_i_reg, imm_s_reg, imm_b_reg, imm_u_reg, imm_j_reg;
// ... (all decoder outputs stored in flip-flops)
```

---

## State Machine Design

### FSM States (11 Total)

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
    S_HALT       = 4'b1010   // ECALL/EBREAK
} state_t;

state_t current_state, next_state;
```

### State Register (Flip-Flop Based)

```systemverilog
always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n)
        current_state <= S_IDLE;
    else
        current_state <= next_state;
end
```

### Instruction Execution Paths

**Note:** Cycle counts are **minimum** base cycles. Memory operations add variable latency.

| Instruction Type | Min Cycles | Path |
|------------------|------------|------|
| R-type (ADD, SUB, MUL, DIV) | 4 + mem | FETCH (wait) → DECODE → EXECUTE → WRITEBACK |
| I-type Arithmetic | 4 + mem | FETCH (wait) → DECODE → EXECUTE → WRITEBACK |
| Load | 5 + 2×mem | FETCH (wait) → DECODE → MEM_ADDR → MEM_READ (wait) → WRITEBACK |
| Store | 4 + 2×mem | FETCH (wait) → DECODE → MEM_ADDR → MEM_WRITE (wait) |
| Branch | 3 + mem | FETCH (wait) → DECODE → BRANCH |
| Jump (JAL/JALR) | 4 + mem | FETCH (wait) → DECODE → EXECUTE → WRITEBACK |
| CSR | 4 + mem | FETCH (wait) → DECODE → CSR → WRITEBACK |
| FENCE | 2 + mem | FETCH (wait) → DECODE |
| ECALL/EBREAK | 2 + mem | FETCH (wait) → DECODE → HALT |

**"+ mem"** indicates waiting for memory ready signal (variable latency)

---

## Memory Interface with Variable Latency

### Key Principle

**Memory operations do NOT assume fixed latency.** The CPU uses ready/valid handshaking to wait for memory transactions to complete. This allows:
- Variable DRAM refresh delays
- Cache misses taking arbitrary time
- Slow peripheral accesses
- Future addition of memory controllers

### Instruction Memory Handshaking

**Signals:**
```systemverilog
output logic [31:0] imem_addr      // Address to fetch
output logic        imem_req       // Request valid (NEW)
input  logic [31:0] imem_data      // Instruction data
input  logic        imem_ready     // Data valid (NEW)
```

**Protocol:**
1. CPU sets `imem_addr` and asserts `imem_req` in S_FETCH
2. CPU waits in S_FETCH until `imem_ready` is asserted
3. When `imem_ready` is high, CPU captures `imem_data` into `ir_reg`
4. CPU proceeds to S_DECODE

**Timing Diagram:**
```
Clock:    ___/‾‾‾\___/‾‾‾\___/‾‾‾\___/‾‾‾\___
State:      S_FETCH  S_FETCH  S_FETCH  S_DECODE
imem_req: ___/‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾\____
imem_ready: _____________/‾‾‾\______________
           (CPU waits 2 extra cycles for memory)
```

**FSM Logic for Variable-Latency Fetch:**
```systemverilog
S_FETCH: begin
    imem_req = 1'b1;
    if (imem_ready) begin
        ir_write = 1'b1;
        next_state = S_DECODE;
    end else begin
        next_state = S_FETCH;  // Stay in FETCH until ready
    end
end
```

### Data Memory Handshaking

**Signals:**
```systemverilog
output logic [31:0] dmem_addr      // Address for load/store
output logic [31:0] dmem_wdata     // Write data
input  logic [31:0] dmem_rdata     // Read data
output logic        dmem_we        // Write enable
output logic        dmem_re        // Read enable
output logic        dmem_req       // Request valid (NEW)
input  logic        dmem_ready     // Operation complete (NEW)
output logic [1:0]  dmem_size      // Size: 00=byte, 01=half, 10=word
```

**Read Protocol:**
1. CPU sets `dmem_addr`, asserts `dmem_re` and `dmem_req` in S_MEM_READ
2. CPU waits in S_MEM_READ until `dmem_ready` is asserted
3. When `dmem_ready` is high, CPU captures `dmem_rdata` into `mdr`
4. CPU proceeds to S_WRITEBACK

**Write Protocol:**
1. CPU sets `dmem_addr`, `dmem_wdata`, asserts `dmem_we` and `dmem_req` in S_MEM_WRITE
2. CPU waits in S_MEM_WRITE until `dmem_ready` is asserted
3. When `dmem_ready` is high, write is complete
4. CPU proceeds to S_FETCH

**FSM Logic for Variable-Latency Memory:**
```systemverilog
S_MEM_READ: begin
    dmem_re = 1'b1;
    dmem_req = 1'b1;
    if (dmem_ready) begin
        mdr_write = 1'b1;
        next_state = S_WRITEBACK;
    end else begin
        next_state = S_MEM_READ;  // Stay until ready
    end
end

S_MEM_WRITE: begin
    dmem_we = 1'b1;
    dmem_req = 1'b1;
    if (dmem_ready) begin
        pc_write = 1'b1;
        instr_complete = 1'b1;
        next_state = S_FETCH;
    end else begin
        next_state = S_MEM_WRITE;  // Stay until ready
    end
end
```

### Memory Model in Simulator

The simulator must implement variable-latency memory:

```rust
// In cpu-sim/src/sim.rs
pub struct MemoryController {
    imem: HashMap<u32, u32>,
    dmem: HashMap<u32, u32>,
    imem_latency_counter: u32,  // Simulate variable latency
    dmem_latency_counter: u32,
}

impl MemoryController {
    pub fn handle_imem_request(&mut self, addr: u32) -> bool {
        // Simulate 1-3 cycle random latency
        if self.imem_latency_counter > 0 {
            self.imem_latency_counter -= 1;
            return false;  // Not ready yet
        }
        // Ready now
        true
    }
    
    pub fn handle_dmem_request(&mut self, addr: u32, is_write: bool) -> bool {
        // Simulate 1-5 cycle random latency
        if self.dmem_latency_counter > 0 {
            self.dmem_latency_counter -= 1;
            return false;  // Not ready yet
        }
        // Ready now
        true
    }
}
```

---

## RTL Modifications

### Phase 1: FSM Infrastructure (1-2 days)

**File:** `rtl/top.sv`

**Add state machine skeleton:**

```systemverilog
// State type definition
typedef enum logic [3:0] {
    S_IDLE       = 4'b0000,
    S_FETCH      = 4'b0001,
    S_DECODE     = 4'b0010,
    S_EXECUTE    = 4'b0011,
    S_MEM_ADDR   = 4'b0100,
    S_MEM_READ   = 4'b0101,
    S_MEM_WRITE  = 4'b0110,
    S_WRITEBACK  = 4'b0111,
    S_BRANCH     = 4'b1000,
    S_CSR        = 4'b1001,
    S_HALT       = 4'b1010
} state_t;

// State registers
state_t current_state, next_state;

// State register (flip-flop)
always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n)
        current_state <= S_IDLE;
    else
        current_state <= next_state;
end

// Skeleton next-state logic
always_comb begin
    next_state = current_state;
    case (current_state)
        S_IDLE:      next_state = S_FETCH;
        S_FETCH:     next_state = imem_ready ? S_DECODE : S_FETCH;
        S_DECODE:    next_state = S_EXECUTE;  // Simplified
        S_EXECUTE:   next_state = S_WRITEBACK;
        S_WRITEBACK: next_state = S_FETCH;
        default:     next_state = S_IDLE;
    endcase
end
```

**Add new output signals to module interface:**
```systemverilog
module top (
    // ... existing ports ...
    
    // NEW: Instruction completion
    output logic instr_complete,
    
    // NEW: Instruction memory handshaking
    output logic imem_req,
    input  logic imem_ready,
    
    // NEW: Data memory handshaking
    output logic dmem_req,
    input  logic dmem_ready
);
```

**Verification:**
```bash
verilator --lint-only rtl/top.sv
```

### Phase 2: Staging Registers (1 day)

**Add flip-flop based staging registers:**

```systemverilog
// Instruction register (flip-flop)
logic [31:0] ir_reg;
logic ir_write;

always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n)
        ir_reg <= 32'h0;
    else if (ir_write)
        ir_reg <= imem_data;
end

// Operand registers (flip-flops)
logic [31:0] a_reg, b_reg;
logic a_reg_write, b_reg_write;

always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        a_reg <= 32'h0;
        b_reg <= 32'h0;
    end else begin
        if (a_reg_write) a_reg <= rs1_data;
        if (b_reg_write) b_reg <= rs2_data;
    end
end

// Result registers (flip-flops)
logic [31:0] alu_out_reg, mdr;
logic alu_out_write, mdr_write;

always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        alu_out_reg <= 32'h0;
        mdr <= 32'h0;
    end else begin
        if (alu_out_write) alu_out_reg <= alu_result;
        if (mdr_write) mdr <= formatted_load_data;
    end
end

// Decoder output registers (flip-flops)
logic [6:0]  opcode_reg;
logic [4:0]  rd_reg, rs1_reg, rs2_reg;
logic [2:0]  funct3_reg;
logic [6:0]  funct7_reg;
logic [31:0] imm_i_reg, imm_s_reg, imm_b_reg, imm_u_reg, imm_j_reg;
logic [4:0]  alu_op_reg;
logic        alu_src_reg, reg_write_reg, mem_write_reg, mem_read_reg;
logic        mem_to_reg_reg, branch_reg, jump_reg;
logic        is_ecall_reg, is_ebreak_reg, is_fence_reg, is_csr_reg;
logic        decode_reg_write;

always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        opcode_reg <= 7'h0;
        rd_reg <= 5'h0;
        // ... reset all
    end else if (decode_reg_write) begin
        opcode_reg <= opcode;
        rd_reg <= rd;
        rs1_reg <= rs1;
        rs2_reg <= rs2;
        funct3_reg <= funct3;
        funct7_reg <= funct7;
        imm_i_reg <= imm_i;
        imm_s_reg <= imm_s;
        imm_b_reg <= imm_b;
        imm_u_reg <= imm_u;
        imm_j_reg <= imm_j;
        alu_op_reg <= alu_op;
        alu_src_reg <= alu_src;
        reg_write_reg <= reg_write;
        mem_write_reg <= mem_write;
        mem_read_reg <= mem_read;
        mem_to_reg_reg <= mem_to_reg;
        branch_reg <= branch;
        jump_reg <= jump;
        is_ecall_reg <= is_ecall;
        is_ebreak_reg <= is_ebreak;
        is_fence_reg <= is_fence;
        is_csr_reg <= is_csr;
    end
end
```

### Phase 3: Complete FSM Logic (2-3 days)

**Implement full next-state logic:**

```systemverilog
always_comb begin
    next_state = current_state;
    
    case (current_state)
        S_IDLE: begin
            next_state = S_FETCH;
        end
        
        S_FETCH: begin
            // Wait for instruction memory ready
            if (imem_ready)
                next_state = S_DECODE;
            else
                next_state = S_FETCH;
        end
        
        S_DECODE: begin
            case (opcode)
                7'b0110011,  // R-type
                7'b0010011,  // I-type arithmetic
                7'b0110111,  // LUI
                7'b0010111,  // AUIPC
                7'b1101111,  // JAL
                7'b1100111:  // JALR
                    next_state = S_EXECUTE;
                
                7'b0000011,  // Load
                7'b0100011:  // Store
                    next_state = S_MEM_ADDR;
                
                7'b1100011:  // Branch
                    next_state = S_BRANCH;
                
                7'b1110011: begin  // SYSTEM
                    if (is_ecall || is_ebreak)
                        next_state = S_HALT;
                    else if (is_csr)
                        next_state = S_CSR;
                    else  // FENCE
                        next_state = S_FETCH;
                end
                
                default: next_state = S_FETCH;
            endcase
        end
        
        S_EXECUTE: begin
            next_state = S_WRITEBACK;
        end
        
        S_MEM_ADDR: begin
            if (mem_read_reg)
                next_state = S_MEM_READ;
            else
                next_state = S_MEM_WRITE;
        end
        
        S_MEM_READ: begin
            // Wait for data memory ready
            if (dmem_ready)
                next_state = S_WRITEBACK;
            else
                next_state = S_MEM_READ;
        end
        
        S_MEM_WRITE: begin
            // Wait for data memory ready
            if (dmem_ready)
                next_state = S_FETCH;
            else
                next_state = S_MEM_WRITE;
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
            next_state = S_HALT;
        end
        
        default: begin
            next_state = S_IDLE;
        end
    endcase
end
```

**Implement control signal output logic:**

```systemverilog
always_comb begin
    // Default all control signals to inactive
    ir_write = 1'b0;
    a_reg_write = 1'b0;
    b_reg_write = 1'b0;
    alu_out_write = 1'b0;
    mdr_write = 1'b0;
    pc_write = 1'b0;
    reg_write_en = 1'b0;
    decode_reg_write = 1'b0;
    imem_req = 1'b0;
    dmem_req = 1'b0;
    dmem_we = 1'b0;
    dmem_re = 1'b0;
    instr_complete = 1'b0;
    
    case (current_state)
        S_FETCH: begin
            imem_req = 1'b1;
            if (imem_ready)
                ir_write = 1'b1;
        end
        
        S_DECODE: begin
            a_reg_write = 1'b1;
            b_reg_write = 1'b1;
            decode_reg_write = 1'b1;
            // FENCE completes here
            if (is_fence) begin
                pc_write = 1'b1;
                instr_complete = 1'b1;
            end
        end
        
        S_EXECUTE: begin
            alu_out_write = 1'b1;
        end
        
        S_MEM_ADDR: begin
            alu_out_write = 1'b1;
        end
        
        S_MEM_READ: begin
            dmem_re = 1'b1;
            dmem_req = 1'b1;
            if (dmem_ready)
                mdr_write = 1'b1;
        end
        
        S_MEM_WRITE: begin
            dmem_we = 1'b1;
            dmem_req = 1'b1;
            if (dmem_ready) begin
                pc_write = 1'b1;
                instr_complete = 1'b1;
            end
        end
        
        S_WRITEBACK: begin
            reg_write_en = 1'b1;
            pc_write = 1'b1;
            instr_complete = 1'b1;
        end
        
        S_BRANCH: begin
            pc_write = 1'b1;
            instr_complete = 1'b1;
        end
        
        S_CSR: begin
            // CSR operations
        end
        
        default: begin
            // All inactive
        end
    endcase
end
```

**PC Update Logic:**

```systemverilog
logic [31:0] next_pc_value;

always_comb begin
    next_pc_value = pc + 32'd4;  // Default sequential
    
    if (current_state == S_BRANCH) begin
        if (take_branch)
            next_pc_value = pc + imm_b_reg;
    end else if (current_state == S_WRITEBACK) begin
        if (opcode_reg == 7'b1101111)  // JAL
            next_pc_value = pc + imm_j_reg;
        else if (opcode_reg == 7'b1100111)  // JALR
            next_pc_value = (a_reg + imm_i_reg) & ~32'h1;
    end
end

always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n)
        pc <= boot_addr;
    else if (pc_write)
        pc <= next_pc_value;
end
```

### Phase 4: Update Memory Interface (1 day)

**File:** `rtl/mem_interface.sv`

Add handshaking support:

```systemverilog
module mem_interface (
    // ... existing ports ...
    
    // NEW: Handshaking
    input  logic        dmem_req,
    output logic        dmem_ready,
    
    // Registered signals from FSM
    input  logic [2:0]  funct3_reg,
    input  logic        mem_write_reg,
    input  logic        mem_read_reg,
    // ...
);
    // Implementation handles req/ready protocol
    // For simple memory, ready can just follow req
    assign dmem_ready = dmem_req;
    
endmodule
```

### Phase 5: Remove pc_control.sv (1 day)

1. Delete `rtl/pc_control.sv`
2. Remove instantiation from `rtl/top.sv`
3. PC logic is now integrated into FSM (Phase 3)

---

## Simulator Updates

### Phase 6: Simulator Variable-Latency Support (1 day)

**File:** `cpu-sim/src/sim.rs`

Update `step()` method:

```rust
pub fn step(&mut self) -> Result<(), String> {
    const MAX_CYCLES_PER_INSTR: u32 = 100;  // Increased for variable latency
    let mut cycles = 0;
    
    loop {
        // Evaluate combinational logic
        self.dut.eval();
        
        // Handle instruction memory with variable latency
        if self.dut.imem_req() == 1 {
            let addr = self.dut.imem_addr();
            let ready = self.mem_controller.handle_imem_request(addr);
            self.dut.set_imem_ready(if ready { 1 } else { 0 });
            if ready {
                let data = self.imem.get(&addr).unwrap_or(&0);
                self.dut.set_imem_data(*data);
            }
        } else {
            self.dut.set_imem_ready(0);
        }
        
        // Handle data memory with variable latency
        if self.dut.dmem_req() == 1 {
            let addr = self.dut.dmem_addr();
            let is_write = self.dut.dmem_we() == 1;
            let ready = self.mem_controller.handle_dmem_request(addr, is_write);
            self.dut.set_dmem_ready(if ready { 1 } else { 0 });
            
            if ready {
                if is_write {
                    // Handle write
                    let data = self.dut.dmem_wdata();
                    self.dmem.insert(addr, data);
                } else {
                    // Handle read
                    let data = self.dmem.get(&addr).unwrap_or(&0);
                    self.dut.set_dmem_rdata(*data);
                }
            }
        } else {
            self.dut.set_dmem_ready(0);
        }
        
        // Check if instruction complete
        if self.dut.instr_complete() == 1 {
            break;
        }
        
        // Clock edge
        cycles += 1;
        if cycles >= MAX_CYCLES_PER_INSTR {
            return Err("Instruction exceeded maximum cycles".to_string());
        }
        
        self.clock_tick();
    }
    
    Ok(())
}
```

---

## Test Framework Updates

### Phase 7: Test Updates (2 days)

**File:** `tests/src/cpu_test.rs`

Add helper macro:

```rust
macro_rules! execute_instruction {
    ($dut:expr, $imem:expr, $dmem:expr) => {
        const MAX_CYCLES: usize = 100;
        for cycle in 0..MAX_CYCLES {
            $dut.eval();
            
            // Handle imem handshaking
            if $dut.imem_req() == 1 {
                let addr = $dut.imem_addr();
                $dut.set_imem_ready(1);  // Instant for tests
                let data = $imem.get(&addr).copied().unwrap_or(0);
                $dut.set_imem_data(data);
            } else {
                $dut.set_imem_ready(0);
            }
            
            // Handle dmem handshaking
            if $dut.dmem_req() == 1 {
                $dut.set_dmem_ready(1);  // Instant for tests
                if $dut.dmem_we() == 1 {
                    // Write
                    let addr = $dut.dmem_addr();
                    let data = $dut.dmem_wdata();
                    $dmem.insert(addr, data);
                } else if $dut.dmem_re() == 1 {
                    // Read
                    let addr = $dut.dmem_addr();
                    let data = $dmem.get(&addr).copied().unwrap_or(0);
                    $dut.set_dmem_rdata(data);
                }
            } else {
                $dut.set_dmem_ready(0);
            }
            
            if $dut.instr_complete() == 1 {
                break;
            }
            
            clock_cycle!($dut);
        }
    };
}
```

**Create multi-cycle specific tests:**

```rust
#[test]
fn test_variable_latency_load() {
    // Test that CPU waits for memory ready signal
    // Simulate 5-cycle memory latency
}

#[test]
fn test_variable_latency_store() {
    // Test store with variable latency
}

#[test]
fn test_variable_latency_fetch() {
    // Test instruction fetch with delays
}
```

---

## Implementation Phases

### Timeline: 10-12 Days

| Phase | Days | Tasks | Verification |
|-------|------|-------|--------------|
| **1. FSM Infrastructure** | 1-2 | Add state machine, new signals | `verilator --lint-only rtl/top.sv` |
| **2. Staging Registers** | 1 | Add flip-flop based registers | Compile check |
| **3. Complete FSM Logic** | 2-3 | Next-state, control signals, PC | Compile, lint clean |
| **4. Memory Interface** | 1 | Add handshaking, update mem_interface | Compile check |
| **5. Remove pc_control** | 0.5 | Delete module, update top | Compile check |
| **6. Simulator Updates** | 1 | Variable latency support | `cargo build --package cpu-sim` |
| **7. Test Updates** | 2 | Helper macros, new tests | Incremental test runs |
| **8. Full Verification** | 2 | All tests pass, cycle counting | `cargo test --verbose` |
| **9. Documentation** | 1 | Update README, AGENTS.md | Review |

### Implementation Checklist

**Phase 1: FSM Infrastructure**
- [ ] Add state type definition (11 states)
- [ ] Add state registers
- [ ] Implement state register with reset
- [ ] Add skeleton next-state logic
- [ ] Add `instr_complete`, `imem_req`, `imem_ready`, `dmem_req`, `dmem_ready` to module interface
- [ ] Verify: `verilator --lint-only rtl/top.sv`

**Phase 2: Staging Registers**
- [ ] Add IR (instruction register) - flip-flop
- [ ] Add A, B registers - flip-flops
- [ ] Add ALU_OUT register - flip-flop
- [ ] Add MDR (memory data register) - flip-flop
- [ ] Add decoder output registers - all flip-flops
- [ ] Implement all register update logic with proper reset
- [ ] Verify: Compile check, no latches

**Phase 3: Complete FSM Logic**
- [ ] Complete next-state logic for all instruction types
- [ ] Implement control signal output logic
- [ ] Add PC update logic
- [ ] Update register file write enable
- [ ] Verify: Compile, lint clean

**Phase 4: Memory Interface**
- [ ] Add handshaking to mem_interface.sv
- [ ] Update signals to use registered values
- [ ] Verify: Compile check

**Phase 5: Remove pc_control**
- [ ] Delete rtl/pc_control.sv
- [ ] Remove instantiation from top.sv
- [ ] Verify: Compile check

**Phase 6: Simulator Updates**
- [ ] Add MemoryController with variable latency
- [ ] Update step() with handshaking logic
- [ ] Verify: `cargo build --package cpu-sim`

**Phase 7: Test Updates**
- [ ] Add execute_instruction! macro
- [ ] Update existing tests
- [ ] Add variable-latency tests
- [ ] Verify: Incremental tests

**Phase 8: Full Verification**
- [ ] Run all 112+ tests: `cargo test --verbose`
- [ ] Verify cycle counts (variable with memory latency)
- [ ] Code quality: `cargo fmt`, `cargo clippy`, `verilator --lint-only`

**Phase 9: Documentation**
- [ ] Update README.md
- [ ] Update AGENTS.md
- [ ] Add inline comments

---

## Success Criteria

Implementation is complete when:

✅ **Functional Correctness**
- [ ] All 112+ existing tests pass
- [ ] 10+ new multi-cycle tests pass
- [ ] Variable-latency memory operations work correctly

✅ **Architecture**
- [ ] FSM has 11 states and transitions correctly
- [ ] All intermediate storage uses flip-flops (no latches)
- [ ] Memory handshaking works (req/ready protocol)
- [ ] CPU stalls correctly waiting for memory

✅ **Code Quality**
- [ ] RTL compiles: `verilator --lint-only rtl/*.sv`
- [ ] No latch warnings from Verilator
- [ ] Rust format: `cargo fmt -- --check`
- [ ] Rust lint: `cargo clippy -- -D warnings`

✅ **Documentation**
- [ ] README.md updated with multi-cycle info
- [ ] AGENTS.md updated with architecture details
- [ ] This plan document reviewed and accurate

---

## Risk Mitigation

### Identified Risks

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| FSM deadlock | Medium | High | Add timeout in simulator, extensive state testing |
| Accidental latches | Medium | High | Use `always_ff` exclusively, verify with lint tools |
| Memory handshaking bugs | Medium | Medium | Thorough testing with variable latencies |
| Test suite breakage | High | Medium | Incremental updates, helper macros |
| Timing issues | Low | High | Use flip-flops only, follow FSM best practices |

### Mitigation Strategies

1. **No Latches Policy:** Use `always_ff @(posedge clk)` exclusively for storage
2. **Lint Verification:** Run `verilator --lint-only` after every RTL change
3. **Incremental Testing:** Test each phase before moving to next
4. **Memory Testing:** Add specific tests for variable-latency scenarios
5. **Code Review:** Review FSM logic for completeness
6. **Simulator Safety:** Add MAX_CYCLES limit to prevent infinite loops

---

## Quick Command Reference

```bash
# Verify RTL (check for latches!)
verilator --lint-only rtl/*.sv

# Build
cargo build --verbose

# Test (incremental)
cargo test --package cpu_verifier -- alu_test
cargo test --package cpu_verifier -- regfile_test
cargo test --package cpu_verifier -- cpu_test

# Test (full)
cargo test --verbose

# Code quality
cargo fmt -- --check
cargo clippy -- -D warnings

# Run simulator
cargo run --package cpu-sim -- test_programs/test.elf --verbose
```

---

## Appendix: Key Terminology

**Staging Register:** A synchronous D flip-flop used to hold data between pipeline stages or FSM states. Clocked on rising edge, never transparent.

**Latch:** A level-sensitive storage element (AVOID for FPGA designs). Not used in this implementation.

**Variable Latency:** Memory operations that take an unpredictable number of cycles to complete.

**Handshaking:** Protocol using req/ready signals where requester asserts req and waits for ready.

**FSM (Finite State Machine):** Sequential logic that transitions between discrete states based on inputs and current state.

---

**Document Version:** 1.0

**Last Updated:** 2026-01-03

**Status:** Ready for Implementation
