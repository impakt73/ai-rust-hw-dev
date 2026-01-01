# Implementation Phases and Checklist

## Overview

This document provides a step-by-step implementation guide for converting the single-cycle CPU to a multi-cycle implementation. Each phase includes specific tasks, validation criteria, and estimated time.

## Implementation Summary

| Phase | Description | Estimated Time | Dependencies |
|-------|-------------|----------------|--------------|
| 1 | State Machine Infrastructure | 2-3 days | None |
| 2 | Latching Registers | 1-2 days | Phase 1 |
| 3 | Control Signal Logic | 2-3 days | Phase 2 |
| 4 | PC and Memory Interface | 1-2 days | Phase 3 |
| 5 | Host Simulator Updates | 1 day | Phase 4 |
| 6 | Test Updates | 2-3 days | Phase 5 |
| 7 | Verification and Debug | 2-3 days | Phase 6 |
| 8 | Documentation and Review | 1 day | Phase 7 |

**Total Estimated Time: 12-18 days**

---

## Phase 1: State Machine Infrastructure

### Objective
Add the FSM state definitions and basic state register to `top.sv`.

### Tasks

- [ ] **1.1** Add state type definition to `top.sv`:
  ```systemverilog
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
  ```

- [ ] **1.2** Add state register signals:
  ```systemverilog
  state_t current_state, next_state;
  ```

- [ ] **1.3** Implement state register (sequential):
  ```systemverilog
  always_ff @(posedge clk or negedge rst_n) begin
      if (!rst_n) begin
          current_state <= S_IDLE;
      end else begin
          current_state <= next_state;
      end
  end
  ```

- [ ] **1.4** Implement basic next state logic (skeleton):
  ```systemverilog
  always_comb begin
      next_state = current_state;
      case (current_state)
          S_IDLE:    next_state = S_FETCH;
          S_FETCH:   next_state = S_DECODE;
          S_DECODE:  next_state = S_EXECUTE; // Simplified
          S_EXECUTE: next_state = S_WRITEBACK;
          S_WRITEBACK: next_state = S_FETCH;
          default:   next_state = S_IDLE;
      endcase
  end
  ```

- [ ] **1.5** Add `instr_complete` output signal to module interface

- [ ] **1.6** Verify RTL compiles:
  ```bash
  verilator --lint-only rtl/top.sv rtl/alu.sv rtl/regfile.sv rtl/decoder.sv
  ```

### Validation Criteria
- RTL compiles without errors
- State transitions cycle correctly (IDLE→FETCH→DECODE→...)
- No functional correctness yet (just infrastructure)

### Estimated Time: 2-3 days

---

## Phase 2: Latching Registers

### Objective
Add all intermediate latching registers for multi-cycle operation.

### Tasks

- [ ] **2.1** Add Instruction Register (IR):
  ```systemverilog
  logic [31:0] ir;
  logic ir_write;
  
  always_ff @(posedge clk or negedge rst_n) begin
      if (!rst_n) ir <= 32'h0;
      else if (ir_write) ir <= imem_data;
  end
  ```

- [ ] **2.2** Add A and B registers (rs1/rs2 latches):
  ```systemverilog
  logic [31:0] a_reg, b_reg;
  logic a_reg_write, b_reg_write;
  
  always_ff @(posedge clk or negedge rst_n) begin
      if (!rst_n) a_reg <= 32'h0;
      else if (a_reg_write) a_reg <= rs1_data;
  end
  
  always_ff @(posedge clk or negedge rst_n) begin
      if (!rst_n) b_reg <= 32'h0;
      else if (b_reg_write) b_reg <= rs2_data;
  end
  ```

- [ ] **2.3** Add ALU output register:
  ```systemverilog
  logic [31:0] alu_out_reg;
  logic alu_out_write;
  
  always_ff @(posedge clk or negedge rst_n) begin
      if (!rst_n) alu_out_reg <= 32'h0;
      else if (alu_out_write) alu_out_reg <= alu_result;
  end
  ```

- [ ] **2.4** Add Memory Data Register (MDR):
  ```systemverilog
  logic [31:0] mdr;
  logic mdr_write;
  
  always_ff @(posedge clk or negedge rst_n) begin
      if (!rst_n) mdr <= 32'h0;
      else if (mdr_write) mdr <= formatted_load_data;
  end
  ```

- [ ] **2.5** Add decoder output latches (all control signals):
  ```systemverilog
  // Latched control signals
  logic [6:0]  opcode_latched;
  logic [4:0]  rd_latched, rs1_latched, rs2_latched;
  logic [2:0]  funct3_latched;
  logic [6:0]  funct7_latched;
  logic [31:0] imm_i_latched, imm_s_latched, imm_b_latched, imm_u_latched, imm_j_latched;
  logic [4:0]  alu_op_latched;
  logic        alu_src_latched, reg_write_latched, mem_write_latched, mem_read_latched;
  logic        mem_to_reg_latched, branch_latched, jump_latched;
  logic        is_ecall_latched, is_ebreak_latched, is_fence_latched, is_csr_latched;
  logic [31:0] csr_rdata_latched;
  logic        decode_latch;
  
  always_ff @(posedge clk or negedge rst_n) begin
      if (!rst_n) begin
          // Reset all latched signals to 0
      end else if (decode_latch) begin
          // Latch all decoder outputs
      end
  end
  ```

- [ ] **2.6** Verify RTL compiles:
  ```bash
  verilator --lint-only rtl/*.sv
  ```

### Validation Criteria
- RTL compiles without errors
- All registers have proper reset values
- Register write enables are declared

### Estimated Time: 1-2 days

---

## Phase 3: Control Signal Logic

### Objective
Implement the full next-state logic and control signal output logic.

### Tasks

- [ ] **3.1** Complete next-state logic for all instruction types:
  - [ ] R-type path: DECODE → EXECUTE → WRITEBACK
  - [ ] I-type path: DECODE → EXECUTE → WRITEBACK
  - [ ] Load path: DECODE → MEM_ADDR → MEM_READ → WRITEBACK
  - [ ] Store path: DECODE → MEM_ADDR → MEM_WRITE
  - [ ] Branch path: DECODE → BRANCH
  - [ ] Jump path: DECODE → EXECUTE → WRITEBACK
  - [ ] CSR path: DECODE → CSR → WRITEBACK
  - [ ] FENCE path: DECODE → FETCH
  - [ ] HALT path: DECODE → HALT

- [ ] **3.2** Implement control signal output logic:
  ```systemverilog
  always_comb begin
      // Default all signals to 0
      ir_write = 1'b0;
      a_reg_write = 1'b0;
      b_reg_write = 1'b0;
      alu_out_write = 1'b0;
      mdr_write = 1'b0;
      pc_write = 1'b0;
      reg_write_en = 1'b0;
      decode_latch = 1'b0;
      dmem_we = 1'b0;
      dmem_re = 1'b0;
      instr_complete = 1'b0;
      
      case (current_state)
          S_FETCH: ir_write = 1'b1;
          S_DECODE: begin
              a_reg_write = 1'b1;
              b_reg_write = 1'b1;
              decode_latch = 1'b1;
          end
          // ... other states
      endcase
  end
  ```

- [ ] **3.3** Implement ALU input MUX logic:
  - [ ] alu_a selection (A_REG, PC, ZERO)
  - [ ] alu_b selection (B_REG, IMM, FOUR)

- [ ] **3.4** Implement write-back data selection:
  - [ ] ALU result path
  - [ ] Memory data path
  - [ ] CSR data path
  - [ ] PC+4 path (for JAL/JALR)

- [ ] **3.5** Verify RTL compiles:
  ```bash
  verilator --lint-only rtl/*.sv
  ```

### Validation Criteria
- RTL compiles without errors
- All control signals are generated for all states
- No undriven signals or latches

### Estimated Time: 2-3 days

---

## Phase 4: PC and Memory Interface

### Objective
Update PC logic and memory interface to work with FSM.

### Tasks

- [ ] **4.1** Update PC update logic:
  ```systemverilog
  logic pc_write;
  logic [31:0] next_pc_value;
  
  always_comb begin
      next_pc_value = pc + 32'd4;  // Default
      case (current_state)
          S_BRANCH: next_pc_value = take_branch ? (pc + imm_b_latched) : (pc + 32'd4);
          S_WRITEBACK: begin
              if (opcode_latched == 7'b1101111) next_pc_value = pc + imm_j_latched;
              else if (opcode_latched == 7'b1100111) next_pc_value = (a_reg + imm_i_latched) & ~32'h1;
              else next_pc_value = pc + 32'd4;
          end
          default: next_pc_value = pc + 32'd4;
      endcase
  end
  
  always_ff @(posedge clk or negedge rst_n) begin
      if (!rst_n) pc <= boot_addr;
      else if (pc_write && !halted) pc <= next_pc_value;
  end
  ```

- [ ] **4.2** Update instruction input to decoder:
  ```systemverilog
  assign instruction = (current_state == S_FETCH) ? imem_data : ir;
  ```

- [ ] **4.3** Update data memory interface:
  ```systemverilog
  assign dmem_addr = alu_out_reg;
  assign dmem_wdata = b_reg;
  assign dmem_size = funct3_latched[1:0];
  // dmem_we and dmem_re set in control logic
  ```

- [ ] **4.4** Update branch decision logic to use latched values

- [ ] **4.5** Update halt logic:
  ```systemverilog
  assign halted = (current_state == S_HALT);
  ```

- [ ] **4.6** Update register file write enable:
  ```systemverilog
  .we(reg_write_en && reg_write_latched)
  ```

- [ ] **4.7** Verify RTL compiles and lint passes:
  ```bash
  verilator --lint-only rtl/*.sv
  ```

### Validation Criteria
- RTL compiles without errors
- No lint warnings
- PC logic handles all cases

### Estimated Time: 1-2 days

---

## Phase 5: Host Simulator Updates

### Objective
Update the Rust simulator to work with multi-cycle execution.

### Tasks

- [ ] **5.1** Update `cpu-sim/src/sim.rs`:
  - [ ] Modify `step()` to loop until `instr_complete`
  - [ ] Add MAX_CYCLES safety limit
  - [ ] Handle memory operations every cycle

- [ ] **5.2** Test simulator compiles:
  ```bash
  cargo build --package cpu-sim
  ```

- [ ] **5.3** Run basic simulation test:
  ```bash
  cargo run --package cpu-sim -- --help
  ```

### Validation Criteria
- Simulator compiles without errors
- Can load and reset CPU
- Basic simulation loop works

### Estimated Time: 1 day

---

## Phase 6: Test Updates

### Objective
Update all tests to work with multi-cycle execution.

### Tasks

- [ ] **6.1** Create `tests/src/multicycle_test.rs`:
  - [ ] Add FSM state tests
  - [ ] Add cycle count verification tests
  - [ ] Add module declaration to `lib.rs`

- [ ] **6.2** Update `tests/src/cpu_test.rs`:
  - [ ] Add `execute_instruction!` macro
  - [ ] Add `handle_memory_write` helper
  - [ ] Update existing tests to use new macros

- [ ] **6.3** Run ALU and RegFile tests (should pass unchanged):
  ```bash
  cargo test --package cpu_verifier -- alu_test
  cargo test --package cpu_verifier -- regfile_test
  ```

- [ ] **6.4** Run multi-cycle specific tests:
  ```bash
  cargo test --package cpu_verifier -- multicycle_test
  ```

- [ ] **6.5** Run updated CPU tests:
  ```bash
  cargo test --package cpu_verifier -- cpu_test
  ```

- [ ] **6.6** Run full test suite:
  ```bash
  cargo test --verbose
  ```

### Validation Criteria
- All ALU tests pass (16+ tests)
- All RegFile tests pass (6+ tests)
- All multi-cycle tests pass (10+ new tests)
- All updated CPU tests pass (28+ tests)

### Estimated Time: 2-3 days

---

## Phase 7: Verification and Debug

### Objective
Verify correct operation and debug any issues.

### Tasks

- [ ] **7.1** Verify cycle counts for each instruction type:
  | Instruction | Expected | Verified |
  |-------------|----------|----------|
  | R-type | 4 | [ ] |
  | I-type | 4 | [ ] |
  | Load | 5 | [ ] |
  | Store | 4 | [ ] |
  | Branch | 3 | [ ] |
  | JAL/JALR | 4 | [ ] |
  | LUI/AUIPC | 4 | [ ] |
  | CSR | 4 | [ ] |
  | FENCE | 2 | [ ] |
  | ECALL/EBREAK | 2 | [ ] |

- [ ] **7.2** Verify all existing tests pass:
  ```bash
  cargo test --verbose 2>&1 | tee test_results.txt
  grep -E "(PASSED|FAILED)" test_results.txt
  ```

- [ ] **7.3** Debug any failing tests using VCD:
  - [ ] Generate VCD dumps for failing tests
  - [ ] Analyze state transitions
  - [ ] Fix RTL bugs

- [ ] **7.4** Run code quality checks:
  ```bash
  cargo fmt -- --check
  cargo clippy -- -D warnings
  verilator --lint-only rtl/*.sv
  ```

### Validation Criteria
- All tests pass
- Cycle counts match expected values
- No lint warnings
- Code is properly formatted

### Estimated Time: 2-3 days

---

## Phase 8: Documentation and Review

### Objective
Update documentation and prepare for code review.

### Tasks

- [ ] **8.1** Update `README.md`:
  - [ ] Note multi-cycle implementation
  - [ ] Update cycle count information
  - [ ] Update test instructions

- [ ] **8.2** Update `AGENTS.md`:
  - [ ] Document multi-cycle architecture
  - [ ] Update test count
  - [ ] Add debugging tips

- [ ] **8.3** Update inline code comments:
  - [ ] Document FSM states
  - [ ] Document control signals
  - [ ] Document timing

- [ ] **8.4** Prepare PR description:
  - [ ] Summary of changes
  - [ ] Test coverage
  - [ ] Breaking changes

- [ ] **8.5** Request code review

### Validation Criteria
- Documentation is complete and accurate
- PR description is clear
- Ready for review

### Estimated Time: 1 day

---

## Quick Reference: Critical Files

| File | Changes | Priority |
|------|---------|----------|
| `rtl/top.sv` | Major - FSM, registers, control | High |
| `rtl/decoder.sv` | None | - |
| `rtl/alu.sv` | None | - |
| `rtl/regfile.sv` | None | - |
| `cpu-sim/src/sim.rs` | Minor - step() loop | Medium |
| `tests/src/cpu_test.rs` | Minor - macros | Medium |
| `tests/src/multicycle_test.rs` | New file | Medium |
| `tests/src/lib.rs` | Add module | Low |

## Quick Reference: Commands

```bash
# Lint RTL
verilator --lint-only rtl/*.sv

# Build all
cargo build --verbose

# Test all
cargo test --verbose

# Format check
cargo fmt -- --check

# Clippy
cargo clippy -- -D warnings

# Run specific test
cargo test --package cpu_verifier -- test_name

# Test with output
cargo test -- --nocapture
```

---

## Rollback Plan

If the implementation encounters blocking issues:

1. **Git branch strategy**: Work on a feature branch, keep main stable
2. **Incremental commits**: Commit after each phase for easy rollback
3. **Backward compatibility**: Single-cycle tests should still be runnable

---

## Success Criteria

The implementation is complete when:

- [ ] All 28+ existing tests pass
- [ ] 10+ new multi-cycle tests pass
- [ ] Cycle counts match specification
- [ ] RTL compiles and lints clean
- [ ] Rust code passes fmt and clippy
- [ ] Documentation is updated
- [ ] Code review approved

---

**Document Status:** ✅ Complete

**Last Updated:** 2026-01-01
