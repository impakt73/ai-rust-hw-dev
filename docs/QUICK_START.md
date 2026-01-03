# 🚀 Multi-Cycle CPU Implementation: Quick Start Guide

**For AI Coding Agents Starting Implementation**

---

## ⚡ TL;DR - Start Here

If you're an AI coding agent tasked with implementing the multi-cycle CPU upgrade:

1. **Read This First:** [`docs/multi-cycle-minimal-plan.md`](multi-cycle-minimal-plan.md)
   - Main technical reference with code templates
   - 8-phase implementation plan (10-12 days)
   - Everything you need to start coding

2. **Track Progress Here:** [`docs/IMPLEMENTATION_CHECKLIST.md`](IMPLEMENTATION_CHECKLIST.md)
   - Phase-by-phase checklist with tasks
   - Check off items as you complete them
   - Success criteria at the end

3. **Begin Implementation:**
   - Start with **Phase 1: FSM Infrastructure**
   - File to modify: `rtl/top.sv`
   - Time estimate: 1-2 days

---

## 📚 Documentation Map

```
Multi-Cycle CPU Upgrade Documentation
│
├─ 🎯 FOR AI AGENTS (Quick Start)
│  │
│  ├─ multi-cycle-minimal-plan.md        ← START HERE (main plan)
│  ├─ IMPLEMENTATION_CHECKLIST.md        ← Track progress
│  └─ QUICK_START.md (this file)         ← You are here
│
├─ 📊 EXECUTIVE SUMMARY
│  │
│  └─ MULTI_CYCLE_PLANNING_SUMMARY.md    ← What was delivered & why
│
└─ 📖 DETAILED TECHNICAL DOCS
   │
   └─ multi-cycle-implementation/
      ├─ README.md                        ← Navigation guide
      ├─ 00-overview.md                   ← Executive summary
      ├─ 01-current-architecture.md       ← Single-cycle analysis
      ├─ 02-state-machine-design.md       ← FSM details
      ├─ 03-rtl-modifications.md          ← Complete RTL changes
      ├─ 04-host-simulator-changes.md     ← Simulator updates
      ├─ 05-testing-strategy.md           ← Testing approach
      └─ 06-implementation-phases.md      ← Detailed phase breakdown
```

---

## 🎯 What You're Implementing

**Goal:** Convert single-cycle CPU → multi-cycle, latency-insensitive CPU

**Strategy:**
- ✅ Minimal complexity (no pipelining, no advanced features)
- ✅ Functional correctness first
- ✅ Maintain external interface stability
- ✅ 8 clear implementation phases

**Result:**
- Instructions take 3-5 cycles (variable, based on type)
- FSM with 11 states controls execution
- External interfaces unchanged (only adds `instr_complete` signal)

---

## 🔧 Phase 1: Get Started Now

**File to Modify:** `rtl/top.sv`

**What to Add:**

```systemverilog
// 1. Add state type definition
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

// 2. Add state registers
state_t current_state, next_state;

// 3. Implement state register (sequential)
always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        current_state <= S_IDLE;
    end else begin
        current_state <= next_state;
    end
end

// 4. Add basic next-state logic (skeleton)
always_comb begin
    next_state = current_state;
    case (current_state)
        S_IDLE:      next_state = S_FETCH;
        S_FETCH:     next_state = S_DECODE;
        S_DECODE:    next_state = S_EXECUTE;
        S_EXECUTE:   next_state = S_WRITEBACK;
        S_WRITEBACK: next_state = S_FETCH;
        default:     next_state = S_IDLE;
    endcase
end
```

**Also Add to Module Interface:**
```systemverilog
module top (
    // ... existing ports ...
    output logic instr_complete  // NEW: instruction completion signal
);
```

**Verify:**
```bash
verilator --lint-only rtl/*.sv
```

**Next:** See Phase 2 in the checklist

---

## 📋 8-Phase Implementation Overview

| Phase | Days | Focus |
|-------|------|-------|
| 1. FSM Infrastructure | 1-2 | State machine skeleton |
| 2. Latching Registers | 1 | Add intermediate storage |
| 3. Complete FSM Logic | 2-3 | Full state transitions & control |
| 4. Update Helper Modules | 1 | Remove pc_control, update mem/wb |
| 5. Simulator Updates | 1 | Multi-cycle execution loop |
| 6. Test Framework Updates | 2 | Helper macros, new tests |
| 7. Full Verification | 2 | All tests pass |
| 8. Documentation | 1 | Update docs |
| **Total** | **10-12** | **Complete!** |

---

## ✅ Verification Commands

**After each phase, run:**

```bash
# Verify RTL compiles
verilator --lint-only rtl/*.sv

# Build everything
cargo build --verbose

# Run tests (incrementally)
cargo test --package cpu_verifier -- alu_test      # Should pass unchanged
cargo test --package cpu_verifier -- regfile_test  # Should pass unchanged
cargo test --package cpu_verifier -- cpu_test      # Update as you go

# Full test suite (when ready)
cargo test --verbose

# Code quality
cargo fmt -- --check
cargo clippy -- -D warnings
```

---

## 🎓 Key Concepts to Understand

### FSM States (11 Total)

```
S_IDLE       → Initial state after reset
S_FETCH      → Fetch instruction from memory
S_DECODE     → Decode instruction, read registers
S_EXECUTE    → Perform ALU operation
S_MEM_ADDR   → Calculate memory address
S_MEM_READ   → Load from memory
S_MEM_WRITE  → Store to memory
S_WRITEBACK  → Write result to register file
S_BRANCH     → Branch decision
S_CSR        → CSR operation
S_HALT       → CPU halted (ECALL/EBREAK)
```

### Typical Instruction Paths

```
R-type:  FETCH → DECODE → EXECUTE → WRITEBACK (4 cycles)
Load:    FETCH → DECODE → MEM_ADDR → MEM_READ → WRITEBACK (5 cycles)
Store:   FETCH → DECODE → MEM_ADDR → MEM_WRITE (4 cycles)
Branch:  FETCH → DECODE → BRANCH (3 cycles)
```

### New Registers Needed

```systemverilog
logic [31:0] ir;           // Instruction Register
logic [31:0] a_reg;        // Operand A (rs1 data)
logic [31:0] b_reg;        // Operand B (rs2 data)
logic [31:0] alu_out_reg;  // ALU output
logic [31:0] mdr;          // Memory Data Register
// + latched control signals from decoder
```

---

## 🚨 Important Notes

**DO:**
- ✅ Follow the phases in order
- ✅ Test after each phase
- ✅ Commit frequently
- ✅ Use provided code templates
- ✅ Keep external interfaces unchanged

**DON'T:**
- ❌ Skip verification steps
- ❌ Change external memory interfaces
- ❌ Add pipelining or advanced features
- ❌ Wait until the end to test
- ❌ Modify decoder, ALU, or regfile modules

---

## 🆘 If You Get Stuck

1. **Check the detailed plan:** `docs/multi-cycle-minimal-plan.md`
2. **Review code templates:** Section "Critical Implementation Details"
3. **Check existing docs:** `docs/multi-cycle-implementation/` directory
4. **Verify your changes:** Run `verilator --lint-only rtl/*.sv`
5. **Test incrementally:** Don't wait to find errors

---

## 📞 Quick Reference

**Main Documents:**
- Plan: `docs/multi-cycle-minimal-plan.md`
- Checklist: `docs/IMPLEMENTATION_CHECKLIST.md`
- Summary: `docs/MULTI_CYCLE_PLANNING_SUMMARY.md`

**Commands:**
```bash
verilator --lint-only rtl/*.sv        # Verify RTL
cargo build --verbose                 # Build
cargo test --verbose                  # Test all
cargo fmt -- --check                  # Format check
cargo clippy -- -D warnings           # Lint
```

**Files to Modify:**
- `rtl/top.sv` (major changes)
- `rtl/mem_interface.sv` (minor)
- `rtl/writeback_mux.sv` (minor)
- `cpu-sim/src/sim.rs` (minor)
- `tests/src/cpu_test.rs` (minor)

**Files to Remove:**
- `rtl/pc_control.sv` (logic moved to FSM)

---

## 🎯 Success Criteria

You're done when:
- [ ] All 112+ existing tests pass
- [ ] 10+ new multi-cycle tests pass
- [ ] Cycle counts match specification
- [ ] RTL compiles cleanly
- [ ] Code quality checks pass
- [ ] Documentation updated

---

**Ready? Start with Phase 1! → `docs/multi-cycle-minimal-plan.md`**

**Good luck! 🚀**
