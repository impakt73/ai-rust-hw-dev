# Multi-Cycle CPU Upgrade: AI-Optimized Minimal Implementation Plan

## Executive Summary

**Goal:** Convert the single-cycle RV32IM CPU to a **multi-cycle, latency-insensitive** design with minimal risk and complexity.

**Strategy:** Avoid pipelining, avoid complex performance features. Focus on functional correctness with a straightforward state machine.

**Impact:** Instructions will take variable cycles (3-5 cycles) instead of fixed 1 cycle. External interfaces remain unchanged.

---

## Quick Reference: What Changes

| Component | Change Level | Key Modifications |
|-----------|--------------|-------------------|
| `rtl/top.sv` | **MAJOR** | Add FSM, latching registers, multi-cycle control |
| `rtl/decoder.sv` | None | Unchanged, outputs will be latched |
| `rtl/alu.sv` | None | Unchanged, used in different cycles |
| `rtl/regfile.sv` | None | Unchanged, write enable gated by FSM |
| `rtl/branch_unit.sv` | None | Unchanged |
| `rtl/pc_control.sv` | **REMOVE** | Logic moved into top.sv FSM |
| `rtl/mem_interface.sv` | Minor | Signals driven by FSM instead of combinational |
| `rtl/writeback_mux.sv` | Minor | Selection driven by latched signals |
| `rtl/csr_file.sv` | None | Unchanged |
| `cpu-sim/src/sim.rs` | Minor | Loop until `instr_complete` signal |
| `tests/src/cpu_test.rs` | Minor | Add helper macro for multi-cycle execution |

---

## Core Design Principles

### 1. State Machine (FSM) States

Minimal state set for functional implementation:

```systemverilog
typedef enum logic [3:0] {
    S_IDLE       = 4'b0000,  // After reset
    S_FETCH      = 4'b0001,  // Fetch instruction
    S_DECODE     = 4'b0010,  // Decode and read registers
    S_EXECUTE    = 4'b0011,  // ALU operation
    S_MEM_ADDR   = 4'b0100,  // Calculate memory address
    S_MEM_READ   = 4'b0101,  // Load from memory
    S_MEM_WRITE  = 4'b0110,  // Store to memory
    S_WRITEBACK  = 4'b0111,  // Write result to register
    S_BRANCH     = 4'b1000,  // Branch decision
    S_CSR        = 4'b1001,  // CSR operation
    S_HALT       = 4'b1010   // ECALL/EBREAK
} state_t;
```

### 2. Instruction Execution Paths

| Instruction Type | Cycle Count | Path |
|------------------|-------------|------|
| R-type (ADD, SUB, AND, MUL, DIV, etc.) | 4 | FETCH → DECODE → EXECUTE → WRITEBACK |
| I-type Arithmetic (ADDI, ORI, etc.) | 4 | FETCH → DECODE → EXECUTE → WRITEBACK |
| Load (LW, LH, LB, etc.) | 5 | FETCH → DECODE → MEM_ADDR → MEM_READ → WRITEBACK |
| Store (SW, SH, SB) | 4 | FETCH → DECODE → MEM_ADDR → MEM_WRITE |
| Branch (BEQ, BNE, etc.) | 3 | FETCH → DECODE → BRANCH |
| Jump (JAL, JALR) | 4 | FETCH → DECODE → EXECUTE → WRITEBACK |
| LUI/AUIPC | 4 | FETCH → DECODE → EXECUTE → WRITEBACK |
| CSR | 4 | FETCH → DECODE → CSR → WRITEBACK |
| FENCE | 2 | FETCH → DECODE (then back to FETCH) |
| ECALL/EBREAK | 2+ | FETCH → DECODE → HALT |

### 3. New Signals and Registers

**New Output Signal:**
```systemverilog
output logic instr_complete  // High for 1 cycle when instruction done
```

**Internal Latching Registers:**
```systemverilog
// Instruction register
logic [31:0] ir;

// Operand registers
logic [31:0] a_reg;  // rs1 data
logic [31:0] b_reg;  // rs2 data

// Result registers
logic [31:0] alu_out_reg;  // ALU output
logic [31:0] mdr;          // Memory data register

// Latched control signals (from decoder)
logic [6:0]  opcode_lat;
logic [4:0]  rd_lat, rs1_lat, rs2_lat;
logic [2:0]  funct3_lat;
logic [31:0] imm_i_lat, imm_s_lat, imm_b_lat, imm_u_lat, imm_j_lat;
// ... (all decoder outputs latched)
```

### 4. Interface Stability

**UNCHANGED External Interfaces:**
- Clock and reset
- Instruction memory (imem_addr, imem_data)
- Data memory (dmem_addr, dmem_wdata, dmem_rdata, dmem_we, dmem_re, dmem_size)
- Boot address
- Debug outputs

**NEW Output:**
- `instr_complete` - tells simulator when instruction finishes

---

## Implementation Sequence (Minimal Risk)

### Phase 1: FSM Infrastructure (1-2 days)
**Goal:** Add state machine skeleton to top.sv

**Tasks:**
1. Add state type definition
2. Add state registers (current_state, next_state)
3. Implement state register (always_ff with reset)
4. Add basic next-state logic (simplified path: FETCH→DECODE→EXECUTE→WRITEBACK)
5. Add `instr_complete` output to module interface
6. Verify RTL compiles: `verilator --lint-only rtl/*.sv`

**Validation:** RTL compiles, states transition correctly

### Phase 2: Latching Registers (1 day)
**Goal:** Add all intermediate storage registers

**Tasks:**
1. Add IR (instruction register)
2. Add A and B registers (operand storage)
3. Add ALU_OUT register
4. Add MDR (memory data register)
5. Add latched decoder output signals
6. Verify RTL compiles

**Validation:** RTL compiles, no undriven signals

### Phase 3: Complete FSM Logic (2-3 days)
**Goal:** Implement full next-state logic and control outputs

**Tasks:**
1. Complete next-state logic for all instruction types
2. Implement control signal output logic:
   - ir_write (S_FETCH)
   - a_reg_write, b_reg_write (S_DECODE)
   - alu_out_write (S_EXECUTE, S_MEM_ADDR)
   - mdr_write (S_MEM_READ)
   - reg_write_en (S_WRITEBACK)
   - pc_write (S_WRITEBACK, S_BRANCH, S_MEM_WRITE, S_DECODE for FENCE)
   - dmem_we, dmem_re (S_MEM_WRITE, S_MEM_READ)
   - instr_complete (completion states)
3. Update PC logic to work with FSM
4. Update register file write enable gating
5. Verify RTL compiles and lints clean

**Validation:** RTL compiles, no lint warnings, all control signals defined

### Phase 4: Remove/Update Helper Modules (1 day)
**Goal:** Integrate or remove single-cycle helper modules

**Tasks:**
1. Remove `pc_control.sv` (logic moved to FSM)
2. Update `mem_interface.sv` to use latched signals
3. Update `writeback_mux.sv` to use latched signals
4. Update top.sv instantiations
5. Verify RTL compiles

**Validation:** RTL compiles, module hierarchy simplified

### Phase 5: Simulator Updates (1 day)
**Goal:** Update Rust simulator for multi-cycle execution

**Tasks:**
1. Modify `cpu-sim/src/sim.rs`:
   - Update `step()` to loop until `instr_complete`
   - Add MAX_CYCLES_PER_INSTR safety limit (e.g., 10)
   - Handle memory operations every cycle
2. Test simulator compiles: `cargo build --package cpu-sim`
3. Run basic test

**Validation:** Simulator compiles and runs

### Phase 6: Test Framework Updates (2 days)
**Goal:** Update tests to work with multi-cycle execution

**Tasks:**
1. Create helper macro in `tests/src/cpu_test.rs`:
   ```rust
   macro_rules! execute_instruction {
       ($dut:expr, $imem:expr, $dmem:expr) => {
           loop {
               $dut.eval();
               // Handle memory operations
               if $dut.instr_complete() == 1 { break; }
               clock_cycle!($dut);
           }
       };
   }
   ```
2. Update existing CPU tests to use new macro
3. Add multi-cycle specific tests (cycle counting, state transitions)
4. Run tests incrementally:
   - ALU tests (should pass unchanged)
   - RegFile tests (should pass unchanged)
   - CPU tests (updated with new macro)

**Validation:** All tests compile, targeted tests pass

### Phase 7: Full Verification (2 days)
**Goal:** Verify all existing functionality works

**Tasks:**
1. Run full test suite: `cargo test --verbose`
2. Debug any failures using VCD waveforms
3. Verify cycle counts match specification
4. Run code quality checks:
   ```bash
   cargo fmt -- --check
   cargo clippy -- -D warnings
   verilator --lint-only rtl/*.sv
   ```

**Validation:** All 112+ tests pass, code quality checks pass

### Phase 8: Documentation (1 day)
**Goal:** Update documentation

**Tasks:**
1. Update README.md (note multi-cycle implementation)
2. Update AGENTS.md (architecture description, test count)
3. Add inline comments to new FSM logic
4. Create PR description

**Validation:** Documentation accurate and complete

---

## Critical Implementation Details

### A. State Machine Next-State Logic Template

```systemverilog
always_comb begin
    next_state = current_state;  // Default: hold state
    
    case (current_state)
        S_IDLE:    next_state = S_FETCH;
        S_FETCH:   next_state = S_DECODE;
        
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
                
                default: next_state = S_FETCH;  // Invalid
            endcase
        end
        
        S_EXECUTE:   next_state = S_WRITEBACK;
        S_MEM_ADDR:  next_state = mem_read_lat ? S_MEM_READ : S_MEM_WRITE;
        S_MEM_READ:  next_state = S_WRITEBACK;
        S_MEM_WRITE: next_state = S_FETCH;
        S_WRITEBACK: next_state = S_FETCH;
        S_BRANCH:    next_state = S_FETCH;
        S_CSR:       next_state = S_WRITEBACK;
        S_HALT:      next_state = S_HALT;  // Stay halted
        
        default:     next_state = S_IDLE;
    endcase
end
```

### B. Control Signal Output Logic Template

```systemverilog
always_comb begin
    // Default: all inactive
    ir_write = 1'b0;
    a_reg_write = 1'b0;
    b_reg_write = 1'b0;
    alu_out_write = 1'b0;
    mdr_write = 1'b0;
    pc_write = 1'b0;
    reg_write_en = 1'b0;
    decode_latch = 1'b0;
    instr_complete = 1'b0;
    
    case (current_state)
        S_FETCH: begin
            ir_write = 1'b1;
        end
        
        S_DECODE: begin
            a_reg_write = 1'b1;
            b_reg_write = 1'b1;
            decode_latch = 1'b1;
            // FENCE completes in decode
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
            mdr_write = 1'b1;
        end
        
        S_MEM_WRITE: begin
            pc_write = 1'b1;
            instr_complete = 1'b1;
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
            // CSR update happens here
        end
        
        default: begin
            // All inactive
        end
    endcase
end
```

### C. PC Update Logic

```systemverilog
logic [31:0] next_pc_value;

always_comb begin
    next_pc_value = pc + 32'd4;  // Default: sequential
    
    if (current_state == S_BRANCH) begin
        if (take_branch)
            next_pc_value = pc + imm_b_lat;
    end else if (current_state == S_WRITEBACK) begin
        if (opcode_lat == 7'b1101111)  // JAL
            next_pc_value = pc + imm_j_lat;
        else if (opcode_lat == 7'b1100111)  // JALR
            next_pc_value = (a_reg + imm_i_lat) & ~32'h1;
    end
end

always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n)
        pc <= boot_addr;
    else if (pc_write)
        pc <= next_pc_value;
end
```

### D. Simulator Update Pattern

```rust
// In cpu-sim/src/sim.rs, update step() method:
pub fn step(&mut self) -> Result<(), String> {
    const MAX_CYCLES_PER_INSTR: u32 = 10;
    let mut cycles = 0;
    
    loop {
        // Evaluate combinational logic
        self.dut.eval();
        
        // Handle memory operations (every cycle)
        self.handle_imem();
        self.handle_dmem();
        
        // Check if instruction complete
        if self.dut.instr_complete() == 1 {
            break;
        }
        
        // Clock edge
        self.cycle_count += 1;
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

## Risk Mitigation

### Known Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| FSM deadlock/infinite loop | Add state timeout in simulator, extensive FSM testing |
| Memory interface timing issues | Keep interface unchanged, memory accessed in dedicated states |
| Register file timing | No changes to regfile, just gate write enable |
| Test suite breakage | Update tests incrementally, add helpers |
| Cycle count errors | Add cycle counting tests, verify against spec |

### Rollback Plan

- Work on feature branch (copilot/multi-cycle)
- Commit after each phase
- Keep main branch stable
- If blocked, can revert to last working phase

---

## Success Criteria

Implementation is complete when:

- [ ] All existing tests pass (112+ tests)
- [ ] New multi-cycle tests pass (10+ tests)
- [ ] Cycle counts match specification (3-5 cycles per instruction)
- [ ] RTL compiles cleanly (`verilator --lint-only`)
- [ ] Rust code passes fmt and clippy
- [ ] Simulator runs multi-cycle correctly
- [ ] Documentation updated
- [ ] No performance regressions in test execution time

---

## Estimated Timeline

**Minimal Risk Path: 10-12 days**

| Phase | Days |
|-------|------|
| 1. FSM Infrastructure | 1-2 |
| 2. Latching Registers | 1 |
| 3. Complete FSM Logic | 2-3 |
| 4. Update Helper Modules | 1 |
| 5. Simulator Updates | 1 |
| 6. Test Framework Updates | 2 |
| 7. Full Verification | 2 |
| 8. Documentation | 1 |

**Total: 10-12 days**

---

## Quick Command Reference

```bash
# Verify RTL
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

**Status:** ✅ Ready for Implementation

**Created:** 2026-01-03

**Target:** Multi-cycle latency-insensitive CPU with minimal complexity
