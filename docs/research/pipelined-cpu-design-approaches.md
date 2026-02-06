# Pipelined CPU Design Approaches for RISC-V RV32IMACF

**Research Document**  
**Context:** Upgrading the multi-cycle non-pipelined RISC-V CPU to a pipelined architecture  
**Date:** 2026-01-26

## Executive Summary

This document presents research findings and architectural approaches for upgrading the current multi-cycle non-pipelined RISC-V RV32IMACF CPU to a pipelined architecture. The research covers classic 5-stage pipeline design, hazard mitigation strategies, and special considerations for the instruction set extensions (M, A, C, F) implemented in the current design.

## Context: Current Multi-Cycle Architecture

The existing CPU implementation features:

- **12-State FSM Design:** S_BOOT, S_FETCH, S_DECODE, S_EXECUTE, S_MEM_ADDR, S_MEM_READ, S_MEM_WRITE, S_WRITEBACK, S_BRANCH, S_CSR, S_HALT, S_ATOMIC_RMW
- **Variable-latency memory:** Ready/valid handshaking for realistic memory operations
- **Multi-cycle ALU operations:** Integer division (DIV, DIVU, REM, REMU)
- **Multi-cycle FPU operations:** Floating-point division and square root
- **Atomic operations:** Load-Reserved/Store-Conditional and AMO instructions with dedicated S_ATOMIC_RMW state
- **Compressed instructions:** 16-bit RV32C instructions seamlessly mixed with 32-bit instructions
- **118 total instructions:** RV32I (40) + M (8) + A (11) + C (27) + F (26) + Zicsr (6)

### Current Instruction Cycle Counts

Based on the FSM implementation:

- **Base instructions:** 3-4 cycles (Fetch → Decode → Execute → Writeback)
- **Memory operations:** 4-5+ cycles (Fetch → Decode → MemAddr → MemRead/Write → Writeback) + memory latency
- **Branches:** 3 cycles (Fetch → Decode → Branch)
- **Multi-cycle ALU (division):** Variable cycles depending on division unit latency
- **Multi-cycle FPU:** Variable cycles depending on operation (addition: fast, division: slow)
- **Atomic operations:** 6+ cycles (Fetch → Decode → MemAddr → MemRead → AtomicRMW → MemWrite → Writeback)

## Research Findings

### 1. Classic 5-Stage Pipeline Architecture

The industry-standard RISC-V pipeline consists of five stages:

#### **Stage 1: Instruction Fetch (IF)**
- **Primary Role:** Fetch instruction from instruction memory using the Program Counter (PC)
- **Key Operations:**
  - Issue instruction memory read request
  - Update PC (PC+4 or PC+2 for compressed instructions)
  - Handle branch prediction (basic: assume not-taken; advanced: BTB/BHT)
- **Design Considerations:**
  - Instruction cache integration for performance
  - Branch target buffer (BTB) for branch prediction
  - RV32C decompression can happen here or in ID stage
  - PC alignment checking (must handle 2-byte alignment for compressed)

#### **Stage 2: Instruction Decode (ID)**
- **Primary Role:** Decode instruction and read source registers
- **Key Operations:**
  - Decode opcode, funct3, funct7 fields
  - Generate control signals
  - Read source registers (rs1, rs2) from register file
  - Generate immediate values
  - Detect hazards
- **Design Considerations:**
  - Hazard detection unit (identify data dependencies)
  - Register file has dual read ports (rs1, rs2)
  - Branch offset calculation may occur here
  - RV32C decompression if not done in IF

#### **Stage 3: Execute (EX)**
- **Primary Role:** Perform ALU operations, address calculation, branch decision
- **Key Operations:**
  - ALU arithmetic/logic operations
  - Memory address calculation (base + offset)
  - Branch condition evaluation
  - Data forwarding/bypassing
- **Design Considerations:**
  - ALU must support all RV32I operations
  - Forwarding paths from MEM and WB stages
  - Branch resolution (compare, calculate target)
  - Multi-cycle operations require pipeline stalling or separate functional units

#### **Stage 4: Memory Access (MEM)**
- **Primary Role:** Access data memory for load/store instructions
- **Key Operations:**
  - Issue memory read/write requests
  - Wait for memory ready signal (variable latency)
  - Data alignment and sign extension for loads
- **Design Considerations:**
  - Data cache integration
  - Memory disambiguation for load/store hazards
  - Variable memory latency handling
  - Atomic operations require special handling (see Section 3)

#### **Stage 5: Write Back (WB)**
- **Primary Role:** Write results back to register file
- **Key Operations:**
  - Select result source (ALU, memory, PC+4)
  - Write to destination register (rd)
- **Design Considerations:**
  - Forwarding source for earlier stages
  - Register write conflicts (structural hazard - resolved by design)
  - Floating-point register writeback uses separate FP regfile

### 2. Pipeline Registers

Between each stage, pipeline registers (flip-flops) latch data and control signals:

#### **IF/ID Pipeline Register**
- PC (current instruction address)
- Instruction word (32-bit, already decompressed if using RV32C)
- Valid bit (for pipeline flushing)

#### **ID/EX Pipeline Register**
- PC (for branch target calculation)
- Source operands (rs1_data, rs2_data, or fs1_data, fs2_data for FP)
- Immediate value
- Decoded control signals (alu_op, mem_read, mem_write, reg_write, etc.)
- Destination register address (rd)
- Valid bit

#### **EX/MEM Pipeline Register**
- ALU result (or FPU result)
- Store data (rs2_data or fs2_data)
- Memory control signals (mem_read, mem_write, mem_size)
- Destination register address (rd)
- Writeback control signals (reg_write, mem_to_reg)
- Valid bit

#### **MEM/WB Pipeline Register**
- Memory read data
- ALU/FPU result (forwarded from EX/MEM)
- Destination register address (rd)
- Writeback control signal (reg_write)
- Valid bit

### 3. Pipeline Hazards and Mitigation Strategies

#### **3.1 Structural Hazards**

**Definition:** Hardware resource conflicts when multiple pipeline stages need the same resource.

**Sources in RISC-V:**
- Single-ported memory (IF and MEM both need memory access)
- Register file write conflicts (if multiple instructions try to write simultaneously)

**Solutions:**
- **Separate instruction and data memories/caches:** Standard solution in modern processors
- **Dual-ported register file:** One write port, two read ports (already standard in RISC-V)
- **Pipeline register duplication:** Each stage has its own storage

**For our design:**
- Already using separate instruction and data memory interfaces
- Register file is dual-read, single-write
- **No structural hazards expected** with proper design

#### **3.2 Data Hazards**

**Definition:** Instructions depend on results of previous instructions still in the pipeline.

**Types:**
1. **RAW (Read After Write) - True Dependency:**
   ```assembly
   add x1, x2, x3   # x1 written in cycle 5 (WB)
   sub x4, x1, x5   # x1 needed in cycle 3 (EX)
   ```
   This is the most common and problematic hazard.

2. **WAR (Write After Read) - Anti-Dependency:**
   Generally not a problem in in-order 5-stage pipelines due to fixed ordering.

3. **WAW (Write After Write) - Output Dependency:**
   Can occur with out-of-order execution or multi-cycle operations.

**Mitigation Strategies:**

##### **Strategy 1: Pipeline Stalling (Interlocks)**
- **Mechanism:** Hazard detection unit in ID stage identifies dependencies
- **Action:** Insert "bubbles" (NOP operations) into pipeline until data is ready
- **Pros:** Simple to implement, guaranteed correctness
- **Cons:** Reduces throughput, wastes cycles

**Implementation:**
```systemverilog
// Hazard detection in ID stage
logic stall_pipeline;

assign stall_pipeline = (id_uses_rs1 && ex_writes_rd && (id_rs1 == ex_rd)) ||
                        (id_uses_rs1 && mem_writes_rd && (id_rs1 == mem_rd)) ||
                        (id_uses_rs2 && ex_writes_rd && (id_rs2 == ex_rd)) ||
                        (id_uses_rs2 && mem_writes_rd && (id_rs2 == mem_rd));

// When stall=1: hold IF/ID, insert bubble in ID/EX, don't update PC
```

##### **Strategy 2: Data Forwarding (Bypassing)**
- **Mechanism:** Forward results from later stages directly to EX stage, bypassing WB
- **Action:** Multiplexers select forwarded data instead of register file output
- **Pros:** Eliminates most stalls, maintains throughput
- **Cons:** Increased hardware complexity, longer critical path

**Forwarding Paths:**
1. **EX/MEM → EX:** Result from previous instruction's ALU operation
2. **MEM/WB → EX:** Result from two instructions ago (from memory or ALU)

**Implementation:**
```systemverilog
// Forwarding unit
always_comb begin
    // Default: use register file data
    alu_operand_a = id_ex_rs1_data;
    
    // Forward from MEM stage
    if (ex_mem_reg_write && (ex_mem_rd != 0) && (ex_mem_rd == id_ex_rs1))
        alu_operand_a = ex_mem_alu_result;
    
    // Forward from WB stage (lower priority)
    if (mem_wb_reg_write && (mem_wb_rd != 0) && (mem_wb_rd == id_ex_rs1))
        alu_operand_a = wb_data;
end
```

##### **Strategy 3: Load-Use Hazard (Special Case)**
**Problem:** Load instructions have data ready only in MEM stage, but following instruction needs it in EX.

```assembly
lw  x1, 0(x2)    # x1 ready in MEM stage (cycle 4)
add x3, x1, x4   # x1 needed in EX stage (cycle 3)
```

**Solution:** **Mandatory 1-cycle stall** even with forwarding, OR **compiler scheduling** to insert independent instruction.

**Compiler optimization:**
```assembly
lw  x1, 0(x2)
add x5, x6, x7   # Independent instruction (scheduled by compiler)
add x3, x1, x4   # Now x1 is ready
```

#### **3.3 Control Hazards**

**Definition:** Pipeline doesn't know next PC until branch resolves.

**Problem:**
```assembly
beq x1, x2, target   # Branch decision in EX stage (cycle 3)
add x3, x4, x5       # Fetched at cycle 2 (might be wrong path)
sub x6, x7, x8       # Fetched at cycle 3 (might be wrong path)
```

**Mitigation Strategies:**

##### **Strategy 1: Stall Until Branch Resolves**
- **Mechanism:** Freeze pipeline until branch decision is known
- **Penalty:** 2-3 cycle stall per branch
- **Pros:** Simple, always correct
- **Cons:** Severe performance impact (15-20% of instructions are branches)

##### **Strategy 2: Assume Branch Not Taken**
- **Mechanism:** Continue fetching sequential instructions
- **Action on taken branch:** Flush incorrect instructions, restart at branch target
- **Penalty:** 2-3 cycles on taken branches, 0 cycles on not-taken
- **Pros:** Simple predictor, works well for loops (most branches not taken)
- **Cons:** Still wastes cycles on taken branches

##### **Strategy 3: Branch Prediction (Static)**
- **Backward branches:** Predict taken (loops)
- **Forward branches:** Predict not-taken (if-then)
- **Pros:** Better than always not-taken
- **Cons:** Still limited accuracy

##### **Strategy 4: Dynamic Branch Prediction**
- **Branch History Table (BHT):** 1-bit or 2-bit saturating counters
- **Branch Target Buffer (BTB):** Cache of branch targets
- **Pros:** 85-95% accuracy for many workloads
- **Cons:** Significant hardware complexity, storage overhead

**Pipeline Flush Implementation:**
```systemverilog
// When branch is taken (determined in EX stage)
if (branch_taken) begin
    // Flush IF/ID and ID/EX pipeline registers
    if_id_valid <= 1'b0;
    id_ex_valid <= 1'b0;
    // Update PC to branch target
    pc <= branch_target;
end
```

##### **Strategy 5: Early Branch Resolution**
- **Mechanism:** Move branch decision to ID stage instead of EX
- **Benefit:** Reduces flush penalty from 2 cycles to 1 cycle
- **Implementation:**
  - Add comparator in ID stage
  - Calculate branch target in ID using immediate
  - Requires forwarding for branch operands

### 4. Special Considerations for RV32IMACF Extensions

#### **4.1 M Extension (Multiply/Divide)**

**Current Implementation:**
- Multi-cycle division unit (DIV, DIVU, REM, REMU)
- Multiplication in single cycle

**Pipelined Approaches:**

##### **Option 1: Iterative Execution in EX Stage**
- **Mechanism:** Multi-cycle occupancy of EX stage
- **Stalling:** Pipeline stalls until division completes
- **Pros:** Simple, reuses existing division unit
- **Cons:** Long stalls (32+ cycles for division)

##### **Option 2: Separate Divider Functional Unit**
- **Mechanism:** Parallel divider alongside main ALU
- **Implementation:**
  - Issue division to dedicated unit
  - Continue pipeline with independent instructions
  - Stall only when result is needed
- **Pros:** Reduced stalls for independent instructions
- **Cons:** Requires out-of-order completion handling

##### **Option 3: Pipelined Multiplier/Divider**
- **Mechanism:** Deeply pipelined arithmetic units
- **Multiplier:** 2-4 stage pipeline for full 32×32 multiply
- **Divider:** 34-stage pipeline (1 bit per cycle)
- **Pros:** Maintains throughput, no stalls
- **Cons:** High area cost, complex dependency tracking

**Recommendation:** Start with Option 1 (iterative), migrate to Option 2 for performance.

#### **4.2 A Extension (Atomics)**

**Current Implementation:**
- Dedicated S_ATOMIC_RMW state for read-modify-write
- LR/SC reservation station tracking
- 6+ cycle operation

**Pipelined Challenges:**
1. **Atomicity guarantee:** Must prevent other memory operations between load and store
2. **Memory ordering:** Cache coherence in multi-core systems
3. **Pipeline complexity:** RMW sequence must not be interrupted

**Pipelined Approaches:**

##### **Option 1: Multi-Cycle Atomic Operation in MEM Stage**
- **Mechanism:** Lock memory interface during atomic operation
- **Implementation:**
  - AMO instructions occupy MEM stage for multiple cycles
  - Pipeline stalls on memory operations until atomic completes
  - Reservation tracking in load/store unit
- **Pros:** Simplest approach, maintains atomicity
- **Cons:** Pipeline stalls on all memory operations during atomic

##### **Option 2: Reservation Station and Replay**
- **Mechanism:** Track reservations, replay failed SC operations
- **Implementation:**
  - LR.W records reservation address and ID
  - SC.W checks reservation, fails if broken
  - Failed SC returns to ID stage for replay
- **Pros:** Allows concurrent memory operations
- **Cons:** Complex replay logic, indeterminate latency

##### **Option 3: Out-of-Order Memory Operations**
- **Mechanism:** Memory queue with dependency tracking
- **Implementation:**
  - Load/store queue holds pending operations
  - Atomic operations enforce ordering constraints
  - Hazard checking against queue entries
- **Pros:** Maximum parallelism
- **Cons:** Very complex, overkill for single-core

**Recommendation:** Option 1 for simplicity, Option 2 for performance-critical applications.

#### **4.3 C Extension (Compressed Instructions)**

**Current Implementation:**
- Fetch buffer assembles 32-bit words from instruction stream
- Decompressor expands 16-bit instructions to 32-bit equivalents
- PC increments by 2 or 4 bytes

**Pipelined Considerations:**

##### **Instruction Alignment**
- **Challenge:** Instructions can be at any 2-byte boundary
- **IF Stage Complexity:** Must handle partial instruction fetches
- **Solutions:**
  1. **Fetch buffer:** Pre-fetch and align instructions before IF/ID register
  2. **Dual-fetch:** Fetch two words per cycle, select appropriate instruction

##### **PC Update Logic**
- **Challenge:** PC increment varies (2 or 4 bytes)
- **Solution:** Decode instruction width in IF stage, update PC accordingly

##### **Branch Target Alignment**
- **Challenge:** Branch targets must be 2-byte aligned
- **Hardware Check:** Detect misaligned PC, raise exception

**Implementation:**
```systemverilog
// Instruction fetch with compression support
logic [31:0] fetch_buffer;
logic [31:0] current_instruction;
logic instruction_is_compressed;

// Detect compressed instruction (bits [1:0] != 2'b11)
assign instruction_is_compressed = (fetch_buffer[1:0] != 2'b11);

// PC increment
assign pc_increment = instruction_is_compressed ? 32'd2 : 32'd4;

// Decompression (combinational logic in IF or ID stage)
decompress u_decompress (
    .compressed_instr(fetch_buffer[15:0]),
    .decompressed_instr(current_instruction),
    .is_compressed(instruction_is_compressed)
);
```

**Recommendation:** Keep existing fetch buffer and decompressor, integrate into IF stage.

#### **4.4 F Extension (Floating-Point)**

**Current Implementation:**
- Separate FP register file (32 registers)
- FPU with multi-cycle operations (FDIV, FSQRT variable latency)
- FCSR for rounding modes and exception flags
- Fused multiply-add instructions (FMADD.S, FMSUB.S, etc.)

**Pipelined Challenges:**

##### **Variable Latency Operations**
- **FADD.S, FSUB.S:** 3-4 cycles (alignment, add, normalize, round)
- **FMUL.S:** 4-5 cycles (multiply, normalize, round)
- **FDIV.S:** 10-20 cycles (iterative division)
- **FSQRT.S:** 15-25 cycles (iterative square root)
- **FMADD.S:** 6-8 cycles (multiply, add, normalize, round)

**Pipelined Approaches:**

##### **Option 1: Multi-Cycle Occupancy with Stalling**
- **Mechanism:** FP operations occupy EX stage for multiple cycles
- **Implementation:**
  - FPU asserts "busy" signal
  - Pipeline stalls until FPU ready
  - Simple control logic
- **Pros:** Simple, reuses existing FPU
- **Cons:** Long stalls, especially for division/sqrt

##### **Option 2: Separate FP Pipeline**
- **Mechanism:** Parallel FP execution unit alongside integer pipeline
- **Implementation:**
  - FP instructions dispatched to FP pipeline
  - Integer pipeline continues independently
  - Stall only on FP-to-FP or FP-to-int dependencies
- **Stages:** Decode → FP-EX1 → FP-EX2 → FP-EX3 → FP-WB
- **Pros:** Reduces stalls for mixed int/FP code
- **Cons:** Complex dependency tracking, dual writeback arbitration

##### **Option 3: Fully Pipelined FPU**
- **Mechanism:** Break FP operations into deep pipelines
- **FADD:** 4-stage pipeline (align → add → normalize → round)
- **FMUL:** 5-stage pipeline (multiply stages → normalize → round)
- **FDIV:** Non-pipelined or iteration unit (too complex to pipeline)
- **Pros:** Maximum throughput for add/mul operations
- **Cons:** High area cost, structural hazards if multiple FP ops

##### **Option 4: Scoreboarding / Tomasulo's Algorithm**
- **Mechanism:** Dynamic scheduling of FP operations
- **Implementation:**
  - Instruction queue with dependency tracking
  - Functional units report completion out-of-order
  - Scoreboard resolves WAR, WAW, RAW hazards
- **Pros:** Maximum instruction-level parallelism (ILP)
- **Cons:** Very complex, significant area overhead

**Register File Considerations:**
- Separate FP register file (f0-f31) already exists
- FP load/store use integer base address calculation
- Must handle dependencies between FP and integer registers for FMV instructions

**Forwarding for FP Operations:**
```systemverilog
// FP forwarding unit
always_comb begin
    fp_operand_a = id_ex_fs1_data;
    
    // Forward from FP-MEM stage
    if (fp_ex_mem_fp_write && (fp_ex_mem_fd != 0) && (fp_ex_mem_fd == id_ex_fs1))
        fp_operand_a = fp_ex_mem_result;
    
    // Forward from FP-WB stage
    if (fp_mem_wb_fp_write && (fp_mem_wb_fd != 0) && (fp_mem_wb_fd == id_ex_fs1))
        fp_operand_a = fp_wb_data;
end
```

**Exception Handling:**
- FP operations can raise exceptions (inexact, overflow, underflow, divide-by-zero, invalid)
- FCSR flags must be updated atomically with result writeback
- Precise exceptions require pipeline draining (complex in out-of-order execution)

**Recommendation:**
- **Initial Implementation:** Option 1 (multi-cycle with stalling)
- **Performance Upgrade:** Option 2 (separate FP pipeline)
- **High-Performance:** Option 3 (pipelined add/mul) + Option 4 (scoreboarding for div/sqrt)

#### **4.5 Zicsr Extension (CSR Instructions)**

**Current Implementation:**
- Dedicated CSR file
- CSR operations in S_CSR state
- Read-modify-write semantics

**Pipelined Considerations:**

##### **CSR Access Timing**
- **Read:** Must occur in ID or EX stage
- **Write:** Must occur in WB stage (after commit)
- **Side effects:** Some CSRs have side effects (e.g., writing to cycle counter)

##### **Serialization Point**
- **CSRRW, CSRRS, CSRRC:** Read-modify-write requires pipeline serialization
- **Implementation:** Stall pipeline until CSR instruction completes
- **Rationale:** CSR operations are rare, simplicity preferred over performance

**Implementation:**
```systemverilog
// CSR hazard detection
assign csr_stall = (id_is_csr) && (ex_valid || mem_valid);

// CSR read in EX stage
csr_rdata = csr_file[csr_addr];

// CSR write in WB stage (after commit point)
if (wb_is_csr && wb_valid)
    csr_file[wb_csr_addr] <= csr_wdata;
```

**Recommendation:** Simple serialization approach; CSR performance is not critical.

### 5. Performance Analysis

#### **Theoretical Speedup**

Assuming ideal pipelining with perfect forwarding and no hazards:

- **Multi-cycle CPU:** CPI (Cycles Per Instruction) = 3-6 average
- **Pipelined CPU:** CPI = 1.0 ideal, 1.2-1.5 realistic (with hazards)
- **Speedup:** 2.0x - 4.0x for typical workloads

#### **Realistic Performance Estimates**

**Hazard Penalties:**
- **Data hazards:** ~20% of instructions have dependencies
  - With forwarding: 0 cycles (most cases), 1 cycle (load-use)
  - Without forwarding: 1-2 cycles per dependency
- **Control hazards:** ~15% of instructions are branches
  - Assume not-taken: 0 cycles (not taken), 2 cycles (taken)
  - With prediction: 0 cycles (correct), 2 cycles (mispredict)
- **Multi-cycle operations:** ~5% of instructions (div, fdiv, fsqrt)
  - With stalling: 10-30 cycles per operation
  - With separate units: 1-2 cycle issue penalty

**Example CPI Calculation (conservative):**
```
Base CPI (ideal pipeline):        1.00
Data hazard stalls:               +0.15 (20% instr * 0.75 avg stall)
Load-use stalls:                  +0.10 (10% loads * 1.0 stall)
Control hazard stalls:            +0.15 (15% branches * 50% taken * 2 cycles)
Multi-cycle operation stalls:     +0.25 (5% * 5 avg stall)
-----------------------------------------------------------------
Realistic CPI:                    1.65
```

**Speedup vs. multi-cycle (CPI=4):**
- Speedup = 4.0 / 1.65 = **2.42x**

#### **Clock Frequency Considerations**

**Multi-cycle CPU:**
- Critical path: Single ALU operation + muxing
- Estimated: 100 MHz @ 65nm, 500 MHz @ 16nm

**Pipelined CPU:**
- Critical path: One pipeline stage (shortest: register read; longest: ALU op)
- Must balance stages to avoid bottleneck
- Estimated: 150 MHz @ 65nm, 750 MHz @ 16nm
- **Frequency increase:** 1.5x

**Total Performance Gain:**
- Throughput improvement: 2.42x (CPI reduction)
- Frequency improvement: 1.5x
- **Total speedup: 3.63x**

### 6. Implementation Strategies

#### **6.1 Incremental Pipeline Development**

**Phase 1: Basic 5-Stage Pipeline (RV32I only)**
- Implement IF → ID → EX → MEM → WB stages
- Pipeline registers between stages
- Basic hazard detection (stalling only, no forwarding)
- Simple branch handling (assume not-taken, flush on taken)
- Target: Working pipeline for integer instructions

**Phase 2: Forwarding and Hazard Resolution**
- Implement forwarding unit (EX/MEM → EX, MEM/WB → EX)
- Optimize load-use detection
- Add early branch resolution in ID stage
- Target: Reduce CPI from ~2.0 to ~1.4

**Phase 3: RV32C Compressed Instructions**
- Integrate existing decompressor into IF stage
- Implement variable PC increment (2 or 4 bytes)
- Handle instruction alignment
- Target: Full RV32IC support

**Phase 4: RV32M Multiply/Divide**
- Add multi-cycle division handling in EX stage
- Implement pipeline stalling for long-latency operations
- Optional: Separate divider unit for better performance
- Target: Full RV32IMC support

**Phase 5: RV32A Atomic Operations**
- Implement atomic RMW in MEM stage
- Add reservation station for LR/SC
- Ensure atomicity with memory locking
- Target: Full RV32IMAC support

**Phase 6: RV32F Floating-Point**
- Add FP pipeline or multi-cycle FPU handling
- Implement FP register file writeback
- Handle variable-latency FP operations
- Add FCSR exception flag updates
- Target: Full RV32IMACF support

#### **6.2 Verification Strategy**

**Unit Testing:**
- Test each pipeline stage independently
- Verify pipeline register functionality
- Test hazard detection logic in isolation

**Integration Testing:**
- Run existing RISC-V test suite
- Verify correct instruction execution across pipeline
- Test hazard scenarios (RAW, load-use, branches)
- Validate multi-cycle operation handling

**Performance Testing:**
- Measure CPI for benchmark programs
- Compare against multi-cycle implementation
- Profile hazard occurrences and stalls

**Waveform Analysis:**
- Use VCD dumps to visualize pipeline behavior
- Verify correct forwarding paths
- Debug stall and flush conditions

#### **6.3 Simulation and Debugging Tools**

**Pipeline Visualization:**
- Add debug outputs for each pipeline stage
- Display current instruction in each stage
- Show forwarding and stalling decisions

**Performance Counters:**
- Count total cycles
- Count instructions completed
- Count stalls by type (data hazard, load-use, branch)
- Calculate CPI dynamically

**Assertion-Based Verification:**
- Assert no register writes to x0
- Assert valid control signal combinations
- Assert pipeline register integrity
- Assert forwarding correctness

### 7. Design Trade-offs and Recommendations

#### **7.1 Complexity vs. Performance**

| Approach | Complexity | Performance | Recommendation |
|----------|-----------|-------------|----------------|
| Simple 5-stage + stalling | Low | 2.0x speedup | ✅ Start here |
| Forwarding + branch prediction | Medium | 3.0x speedup | ✅ Essential |
| Separate FP pipeline | Medium-High | 3.5x speedup | ⚠️ If FP-heavy |
| Pipelined FPU | High | 4.0x speedup | ❌ Overkill |
| Out-of-order execution | Very High | 5.0x+ speedup | ❌ Not worth it |

#### **7.2 Area and Power Trade-offs**

**Area Increase:**
- Pipeline registers: +10-15%
- Forwarding logic: +5-10%
- Hazard detection: +3-5%
- Separate FP pipeline: +20-30%
- **Total:** 20-60% depending on features

**Power Increase:**
- Increased activity: +15-25%
- More flip-flops: +10-15%
- Forwarding muxes: +5%
- **Total:** 30-45% power increase

**Justification:** 2-3x performance for 1.5x power = improved energy efficiency

#### **7.3 Recommended Implementation Plan**

**For educational/reference design:**
1. Implement basic 5-stage pipeline with stalling (Phase 1)
2. Add forwarding logic (Phase 2)
3. Integrate RV32C support (Phase 3)
4. Stop here; excellent balance of simplicity and performance

**For high-performance design:**
1. Complete Phases 1-3 (basic pipeline + forwarding + RV32C)
2. Add multi-cycle M extension support (Phase 4)
3. Add atomic operations (Phase 5)
4. Implement separate FP pipeline (Phase 6, advanced)
5. Add dynamic branch prediction (BTB/BHT)

**For research/advanced design:**
1. Complete all phases (1-6)
2. Implement fully pipelined FPU (FADD, FMUL)
3. Add scoreboarding for out-of-order FP completion
4. Explore superscalar execution (dual-issue)

### 8. Appendix: Example Pipeline Register Definitions

```systemverilog
// IF/ID Pipeline Register
typedef struct packed {
    logic [31:0] pc;
    logic [31:0] instruction;
    logic        valid;
} if_id_reg_t;

// ID/EX Pipeline Register
typedef struct packed {
    logic [31:0] pc;
    logic [31:0] rs1_data;
    logic [31:0] rs2_data;
    logic [31:0] imm;
    logic [4:0]  rd;
    logic [4:0]  rs1;
    logic [4:0]  rs2;
    logic [4:0]  alu_op;
    logic        alu_src;      // 0=rs2, 1=imm
    logic        mem_read;
    logic        mem_write;
    logic        reg_write;
    logic        mem_to_reg;
    logic        branch;
    logic        jump;
    logic        valid;
} id_ex_reg_t;

// EX/MEM Pipeline Register
typedef struct packed {
    logic [31:0] alu_result;
    logic [31:0] store_data;
    logic [4:0]  rd;
    logic [1:0]  mem_size;
    logic        mem_read;
    logic        mem_write;
    logic        reg_write;
    logic        mem_to_reg;
    logic        valid;
} ex_mem_reg_t;

// MEM/WB Pipeline Register
typedef struct packed {
    logic [31:0] mem_data;
    logic [31:0] alu_result;
    logic [4:0]  rd;
    logic        reg_write;
    logic        mem_to_reg;
    logic        valid;
} mem_wb_reg_t;
```

### 9. Conclusion

Converting the current multi-cycle RISC-V RV32IMACF CPU to a pipelined architecture offers significant performance benefits (2-4x speedup) at the cost of increased design complexity and area. The recommended approach is to:

1. **Start simple:** Implement a basic 5-stage pipeline with stalling for RV32I
2. **Add forwarding:** Reduce data hazard penalties
3. **Integrate RV32C:** Reuse existing decompressor in IF stage
4. **Handle extensions incrementally:** M, A, and F extensions each add complexity
5. **Optimize selectively:** Add advanced features (branch prediction, separate FP pipeline) only if needed

The classic 5-stage pipeline architecture is well-understood, extensively documented, and strikes an excellent balance between performance and complexity. For most applications, a pipeline with forwarding and basic branch prediction will provide the best return on investment.

## References

1. **Classic Pipeline Architecture:**
   - [Lecture 09: Pipeline – Basis](https://www.cse.cuhk.edu.hk/~byu/CENG3420/2024Spring/slides/Lec09-pipeline.pdf)
   - [Part- 2: Understanding the RISC-V Core: The 5-Stage Pipeline](https://vlsi-design-hub.blogspot.com/2024/10/part-2-understanding-risc-v-core-5.html)
   - [Performance-Optimised Design of the RISC-V Five-Stage Pipelined](https://thesai.org/Downloads/Volume15No2/Paper_29-Performance_Optimised_Design.pdf)

2. **Pipeline Hazards:**
   - [Designing RISC-V CPU from scratch – Part 3: Dealing with Pipeline Hazards](https://chipmunklogic.com/digital-logic-design/designing-pequeno-risc-v-cpu-from-scratch-part-3-dealing-with-pipeline-hazards/)
   - [Design and hazard solving of five-stage pipeline RISC-V processor](https://www.researchgate.net/publication/378435482_Design_and_hazard_solving_of_five-stage_pipeline_RISC-V_processor_structure/fulltext/65d93d28adc608480ae7d43d/Design-and-hazard-solving-of-five-stage-pipeline-RISC-V-processor-structure.pdf)
   - [Pipeline Hazards - GeeksforGeeks](https://www.geeksforgeeks.org/computer-organization-architecture/pipeline-hazards/)

3. **SystemVerilog Implementation:**
   - [EngineerWaqasAhmad/Risc-V-Pipelined-Architecture-System-Verilog](https://github.com/EngineerWaqasAhmad/Risc-V-Pipelined-Architecture-System-Verilog-Implementation)
   - [estufa-cin-ufpe/RISC-V-Pipeline](https://github.com/estufa-cin-ufpe/RISC-V-Pipeline)
   - [Design a Three-Stage Pipelined RISC-V Processor Using SystemVerilog](https://kth.diva-portal.org/smash/get/diva2:1713647/FULLTEXT01.pdf)

4. **Multi-cycle to Pipelined Conversion:**
   - [Pipelined Processor Design](https://faculty.kfupm.edu.sa/COE/mudawar/ics233/lectures/09-PipelinedProcessor.pdf)
   - [Computer Architecture - Chapter 3 CPU Pipelining](https://riscv.cs.hm.edu/slides/ca_2020_03_pipelining.pdf)

5. **Floating-Point Pipelining:**
   - [RV32-IF: Implementation of RISC-V 32](https://github.com/NikhilDave10000/RV32-IF)
   - [Computer Architecture - Chapter 4 Complex Pipelining](https://riscv.cs.hm.edu/slides/ca_2020_04_complexpipelining.pdf)
   - [Implementation of high precision/low latency FP divider](https://link.springer.com/article/10.1007/s10617-019-09225-2)
