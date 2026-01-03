# Multi-Cycle CPU Implementation: Quick Checklist

This is a condensed checklist for AI coding agents implementing the multi-cycle CPU upgrade.

**📋 Use this as a tracking document during implementation.**

---

## Pre-Implementation Setup

- [ ] Read `docs/multi-cycle-minimal-plan.md` (main implementation guide)
- [ ] Understand current single-cycle architecture (`rtl/top.sv`)
- [ ] Verify baseline: `cargo test --verbose` (all 112+ tests should pass)
- [ ] Create feature branch (if not already on one)

---

## Phase 1: FSM Infrastructure ⏱️ 1-2 days

### RTL Changes (rtl/top.sv)
- [ ] Add state type definition (11 states: IDLE, FETCH, DECODE, EXECUTE, MEM_ADDR, MEM_READ, MEM_WRITE, WRITEBACK, BRANCH, CSR, HALT)
- [ ] Add state registers: `state_t current_state, next_state`
- [ ] Implement state register with reset: `always_ff @(posedge clk or negedge rst_n)`
- [ ] Add skeleton next-state logic: `always_comb` block with basic transitions
- [ ] Add `instr_complete` output to module interface

### Verification
- [ ] Compile RTL: `verilator --lint-only rtl/*.sv`
- [ ] No errors or warnings
- [ ] States transition correctly (can verify with simple test later)

---

## Phase 2: Latching Registers ⏱️ 1 day

### RTL Changes (rtl/top.sv)
- [ ] Add Instruction Register (IR): `logic [31:0] ir` + write enable
- [ ] Add A register: `logic [31:0] a_reg` + write enable
- [ ] Add B register: `logic [31:0] b_reg` + write enable
- [ ] Add ALU output register: `logic [31:0] alu_out_reg` + write enable
- [ ] Add Memory Data Register: `logic [31:0] mdr` + write enable
- [ ] Add latched control signals from decoder (opcode, rd, rs1, rs2, funct3, funct7, all immediates, all control signals)
- [ ] Implement all register update logic: `always_ff` blocks with reset

### Verification
- [ ] Compile RTL: `verilator --lint-only rtl/*.sv`
- [ ] No undriven signals
- [ ] All registers have proper reset values

---

## Phase 3: Complete FSM Logic ⏱️ 2-3 days

### RTL Changes (rtl/top.sv)
- [ ] Complete next-state logic for all instruction types:
  - [ ] R-type path: DECODE → EXECUTE → WRITEBACK
  - [ ] I-type path: DECODE → EXECUTE → WRITEBACK
  - [ ] Load path: DECODE → MEM_ADDR → MEM_READ → WRITEBACK
  - [ ] Store path: DECODE → MEM_ADDR → MEM_WRITE
  - [ ] Branch path: DECODE → BRANCH
  - [ ] Jump path: DECODE → EXECUTE → WRITEBACK
  - [ ] CSR path: DECODE → CSR → WRITEBACK
  - [ ] FENCE path: DECODE → FETCH (2 cycles)
  - [ ] HALT path: DECODE → HALT (stay in HALT)

- [ ] Implement control signal output logic (`always_comb`):
  - [ ] `ir_write` in S_FETCH
  - [ ] `a_reg_write`, `b_reg_write`, `decode_latch` in S_DECODE
  - [ ] `alu_out_write` in S_EXECUTE and S_MEM_ADDR
  - [ ] `mdr_write` in S_MEM_READ
  - [ ] `reg_write_en` in S_WRITEBACK
  - [ ] `pc_write` in appropriate states
  - [ ] `dmem_we` in S_MEM_WRITE
  - [ ] `dmem_re` in S_MEM_READ
  - [ ] `instr_complete` in completion states

- [ ] Update PC logic to work with FSM control
- [ ] Update register file write enable gating

### Verification
- [ ] Compile RTL: `verilator --lint-only rtl/*.sv`
- [ ] No lint warnings
- [ ] All control signals defined for all states

---

## Phase 4: Update Helper Modules ⏱️ 1 day

### RTL Changes
- [ ] **Remove** `rtl/pc_control.sv` (logic integrated into top.sv FSM)
- [ ] Update `rtl/mem_interface.sv` to use latched signals from FSM
- [ ] Update `rtl/writeback_mux.sv` to use latched signals
- [ ] Update instantiations in `rtl/top.sv`
- [ ] Remove pc_control module instantiation

### Verification
- [ ] Compile RTL: `verilator --lint-only rtl/*.sv`
- [ ] Build succeeds: `cargo build`
- [ ] Module hierarchy simplified

---

## Phase 5: Simulator Updates ⏱️ 1 day

### Rust Changes (cpu-sim/src/sim.rs)
- [ ] Update `step()` method to loop until `instr_complete`:
  ```rust
  const MAX_CYCLES_PER_INSTR: u32 = 10;
  loop {
      self.dut.eval();
      self.handle_imem();
      self.handle_dmem();
      if self.dut.instr_complete() == 1 { break; }
      self.clock_tick();
      cycles += 1;
      if cycles >= MAX_CYCLES_PER_INSTR { return Err(...); }
  }
  ```
- [ ] Add MAX_CYCLES_PER_INSTR safety limit
- [ ] Handle memory operations every cycle (not just once)

### Verification
- [ ] Build simulator: `cargo build --package cpu-sim`
- [ ] No compilation errors
- [ ] Simulator runs (can test with simple ELF later)

---

## Phase 6: Test Framework Updates ⏱️ 2 days

### Rust Changes (tests/src/cpu_test.rs)
- [ ] Create `execute_instruction!` macro:
  ```rust
  macro_rules! execute_instruction {
      ($dut:expr, $imem:expr, $dmem:expr) => {
          const MAX_CYCLES: usize = 10;
          for _ in 0..MAX_CYCLES {
              $dut.eval();
              // handle memory...
              if $dut.instr_complete() == 1 { break; }
              clock_cycle!($dut);
          }
      };
  }
  ```
- [ ] Update existing CPU tests to use new macro
- [ ] Create `tests/src/multicycle_test.rs` for FSM-specific tests
- [ ] Add module declaration to `tests/src/lib.rs`

### Verification (Incremental)
- [ ] ALU tests pass (should work unchanged): `cargo test --package cpu_verifier -- alu_test`
- [ ] RegFile tests pass (should work unchanged): `cargo test --package cpu_verifier -- regfile_test`
- [ ] Multi-cycle tests pass: `cargo test --package cpu_verifier -- multicycle_test`
- [ ] CPU tests compile (may not all pass yet)

---

## Phase 7: Full Verification ⏱️ 2 days

### Testing
- [ ] Run full test suite: `cargo test --verbose 2>&1 | tee test_results.txt`
- [ ] All 112+ existing tests pass
- [ ] New multi-cycle tests pass (10+ tests)
- [ ] Debug any failures:
  - [ ] Use VCD dumps for debugging
  - [ ] Analyze state transitions
  - [ ] Fix RTL bugs
- [ ] Verify cycle counts match specification for each instruction type:
  - [ ] R-type: 4 cycles
  - [ ] I-type: 4 cycles
  - [ ] Load: 5 cycles
  - [ ] Store: 4 cycles
  - [ ] Branch: 3 cycles
  - [ ] Jump: 4 cycles
  - [ ] CSR: 4 cycles
  - [ ] FENCE: 2 cycles

### Code Quality
- [ ] Format check: `cargo fmt -- --check`
- [ ] Clippy check: `cargo clippy -- -D warnings`
- [ ] RTL lint: `verilator --lint-only rtl/*.sv`
- [ ] All checks pass

---

## Phase 8: Documentation ⏱️ 1 day

### Documentation Updates
- [ ] Update `README.md`:
  - [ ] Note multi-cycle implementation
  - [ ] Update cycle count information
  - [ ] Update performance characteristics
- [ ] Update `AGENTS.md`:
  - [ ] Document multi-cycle architecture
  - [ ] Update test count if changed
  - [ ] Add FSM debugging tips
- [ ] Add inline comments to new FSM code in `rtl/top.sv`:
  - [ ] Document each state's purpose
  - [ ] Document control signal meanings
  - [ ] Document timing requirements

### PR Preparation
- [ ] Write comprehensive PR description:
  - [ ] Summary of changes
  - [ ] Cycle count table
  - [ ] Test coverage
  - [ ] Breaking changes (if any)
  - [ ] Migration guide (if needed)
- [ ] Verify all commits are clean and well-described
- [ ] Request code review

---

## Success Criteria ✅

Implementation is complete when ALL of these are true:

- [ ] All 112+ existing tests pass
- [ ] 10+ new multi-cycle tests pass
- [ ] Cycle counts match specification (3-5 cycles per instruction)
- [ ] RTL compiles cleanly (`verilator --lint-only rtl/*.sv`)
- [ ] Rust code passes format check (`cargo fmt -- --check`)
- [ ] Rust code passes clippy (`cargo clippy -- -D warnings`)
- [ ] Simulator executes multi-cycle instructions correctly
- [ ] Documentation is updated and accurate
- [ ] CI pipeline passes (all GitHub Actions checks)
- [ ] Code review approved

---

## Quick Commands

```bash
# RTL Verification
verilator --lint-only rtl/*.sv

# Build Everything
cargo build --verbose

# Test Incrementally
cargo test --package cpu_verifier -- alu_test
cargo test --package cpu_verifier -- regfile_test
cargo test --package cpu_verifier -- multicycle_test
cargo test --package cpu_verifier -- cpu_test

# Full Test Suite
cargo test --verbose

# Code Quality
cargo fmt -- --check
cargo clippy -- -D warnings

# Run Simulator
cargo run --package cpu-sim -- test_programs/test.elf --verbose
```

---

## Notes

- **Commit frequently**: After each phase, commit your changes
- **Test incrementally**: Don't wait until the end to test
- **Use VCD dumps**: When debugging, generate waveforms to visualize state transitions
- **Keep external interfaces stable**: Only add `instr_complete`, don't modify existing signals
- **Safety first**: Add timeout checks in loops to prevent infinite cycles

---

**Status:** ⏳ Ready for Implementation

**Estimated Total Time:** 10-12 days

**Current Phase:** Pre-Implementation Setup
