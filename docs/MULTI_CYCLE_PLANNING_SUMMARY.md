# Multi-Cycle CPU Upgrade: Planning Deliverable Summary

## Deliverable Overview

This document summarizes the planning deliverable for upgrading the CPU design to a multi-cycle, latency-insensitive architecture.

**Status:** ✅ **Planning Phase Complete**

**Created:** 2026-01-03

---

## What Was Delivered

### 1. AI-Optimized Implementation Plan
**File:** `docs/multi-cycle-minimal-plan.md` (515 lines)

**Purpose:** Primary technical reference for AI coding agents implementing the multi-cycle CPU upgrade.

**Contents:**
- Executive summary with clear goals and strategy
- Quick reference table showing what changes and where
- Core design principles (FSM states, cycle counts, new signals)
- 8-phase implementation sequence with time estimates
- Critical implementation details with code templates:
  - FSM next-state logic template
  - Control signal output logic template
  - PC update logic template
  - Simulator update pattern
- Risk mitigation strategies
- Success criteria and timeline (10-12 days)
- Quick command reference

**Key Features:**
- Focuses on **minimal risk and complexity** (no pipelining, no advanced features)
- Provides **ready-to-use code templates** for critical components
- Maintains **external interface stability** (only adds `instr_complete` output)
- Includes **clear validation criteria** for each phase

---

### 2. Implementation Tracking Checklist
**File:** `docs/IMPLEMENTATION_CHECKLIST.md` (273 lines)

**Purpose:** Phase-by-phase tracking document for monitoring implementation progress.

**Contents:**
- Pre-implementation setup checklist
- 8 detailed phase checklists with granular tasks:
  - Phase 1: FSM Infrastructure (1-2 days)
  - Phase 2: Latching Registers (1 day)
  - Phase 3: Complete FSM Logic (2-3 days)
  - Phase 4: Update Helper Modules (1 day)
  - Phase 5: Simulator Updates (1 day)
  - Phase 6: Test Framework Updates (2 days)
  - Phase 7: Full Verification (2 days)
  - Phase 8: Documentation (1 day)
- Success criteria checklist (must ALL be true)
- Quick command reference
- Implementation notes and tips

**Key Features:**
- **Actionable checkboxes** for tracking progress
- **Granular tasks** within each phase
- **Time estimates** for planning
- **Verification steps** after each phase
- **Safety reminders** (commit frequently, test incrementally)

---

### 3. Multi-Cycle Documentation Navigation Guide
**File:** `docs/multi-cycle-implementation/README.md` (74 lines)

**Purpose:** Index and navigation guide for all multi-cycle documentation.

**Contents:**
- Documentation overview with clear "start here" pointer for AI agents
- Complete technical documentation links (7 existing detailed docs)
- Comparison table: AI-optimized vs. comprehensive documentation
- Implementation strategy summary
- Quick reference cycle count table
- Status indicators
- Related documentation links

**Key Features:**
- **Clear navigation** to appropriate documentation based on need
- **Quick reference** to key information (cycle counts, paths)
- **Status tracking** for overall project progress

---

## Design Approach

### Multi-Cycle Architecture Overview

**Current (Single-Cycle):**
- All instructions complete in exactly 1 clock cycle
- Long critical path limits maximum clock frequency
- Simple control logic (combinational)

**Target (Multi-Cycle):**
- Instructions take 3-5 clock cycles (variable based on type)
- Shorter critical path enables higher clock frequency
- FSM-based control with 11 states
- Resource sharing (ALU used in different cycles)

### Key Design Decisions

1. **Minimal Complexity:** No pipelining, no speculative execution, no advanced features
2. **Interface Stability:** External memory interfaces unchanged (only adds `instr_complete`)
3. **Latency Insensitive:** Each instruction takes as many cycles as needed (no fixed latency)
4. **State Machine Control:** Simple FSM with 11 well-defined states
5. **Resource Sharing:** ALU reused for address calculation, PC increment, arithmetic

### Cycle Count Specification

| Instruction Type | Cycles | Execution Path |
|------------------|--------|----------------|
| R-type (ADD, SUB, MUL, DIV, etc.) | 4 | FETCH → DECODE → EXECUTE → WRITEBACK |
| I-type Arithmetic | 4 | FETCH → DECODE → EXECUTE → WRITEBACK |
| Load Instructions | 5 | FETCH → DECODE → MEM_ADDR → MEM_READ → WRITEBACK |
| Store Instructions | 4 | FETCH → DECODE → MEM_ADDR → MEM_WRITE |
| Branch Instructions | 3 | FETCH → DECODE → BRANCH |
| Jump Instructions (JAL/JALR) | 4 | FETCH → DECODE → EXECUTE → WRITEBACK |
| LUI/AUIPC | 4 | FETCH → DECODE → EXECUTE → WRITEBACK |
| CSR Operations | 4 | FETCH → DECODE → CSR → WRITEBACK |
| FENCE | 2 | FETCH → DECODE |
| ECALL/EBREAK | 2+ | FETCH → DECODE → HALT |

---

## Implementation Roadmap

### Phase Summary

```
Pre-Implementation (Setup) → Already Complete
    ↓
Phase 1: FSM Infrastructure (1-2 days)
    ↓
Phase 2: Latching Registers (1 day)
    ↓
Phase 3: Complete FSM Logic (2-3 days)
    ↓
Phase 4: Update Helper Modules (1 day)
    ↓
Phase 5: Simulator Updates (1 day)
    ↓
Phase 6: Test Framework Updates (2 days)
    ↓
Phase 7: Full Verification (2 days)
    ↓
Phase 8: Documentation (1 day)
    ↓
Complete! ✅
```

**Total Estimated Time:** 10-12 days

### Major RTL Changes Required

| File | Change Level | Description |
|------|--------------|-------------|
| `rtl/top.sv` | **MAJOR** | Add FSM, latching registers, multi-cycle control logic |
| `rtl/pc_control.sv` | **REMOVE** | PC logic integrated into top.sv FSM |
| `rtl/mem_interface.sv` | Minor | Use latched signals instead of direct decoder outputs |
| `rtl/writeback_mux.sv` | Minor | Use latched signals instead of direct decoder outputs |
| `rtl/decoder.sv` | None | Unchanged (outputs will be latched) |
| `rtl/alu.sv` | None | Unchanged (used in different cycles) |
| `rtl/regfile.sv` | None | Unchanged (write enable gated by FSM) |
| `rtl/branch_unit.sv` | None | Unchanged |
| `rtl/csr_file.sv` | None | Unchanged |

### Simulator and Test Changes

| File | Change Level | Description |
|------|--------------|-------------|
| `cpu-sim/src/sim.rs` | Minor | Loop until `instr_complete` signal high |
| `tests/src/cpu_test.rs` | Minor | Add `execute_instruction!` helper macro |
| `tests/src/multicycle_test.rs` | **NEW** | FSM-specific tests (state transitions, cycle counts) |
| `tests/src/lib.rs` | Minor | Add multicycle_test module declaration |

---

## Success Criteria

The planning phase is complete. Implementation will be successful when:

✅ **Functional Correctness:**
- [ ] All 112+ existing tests pass
- [ ] 10+ new multi-cycle specific tests pass
- [ ] All instruction types execute correctly

✅ **Cycle Count Verification:**
- [ ] R-type instructions: 4 cycles
- [ ] I-type instructions: 4 cycles
- [ ] Load instructions: 5 cycles
- [ ] Store instructions: 4 cycles
- [ ] Branch instructions: 3 cycles
- [ ] Jump instructions: 4 cycles
- [ ] CSR instructions: 4 cycles
- [ ] FENCE: 2 cycles

✅ **Code Quality:**
- [ ] RTL compiles cleanly: `verilator --lint-only rtl/*.sv`
- [ ] Rust format check passes: `cargo fmt -- --check`
- [ ] Clippy check passes: `cargo clippy -- -D warnings`
- [ ] CI pipeline passes (all GitHub Actions checks)

✅ **Documentation:**
- [ ] README.md updated
- [ ] AGENTS.md updated
- [ ] Inline code comments added
- [ ] PR description complete

---

## Risk Assessment and Mitigation

### Identified Risks

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| FSM deadlock | Medium | High | Add timeout in simulator, extensive testing |
| Test suite breakage | High | Medium | Incremental updates, helper macros |
| Memory timing issues | Low | High | Keep interface unchanged, dedicated states |
| Cycle count errors | Medium | Low | Add verification tests, compare to spec |
| Integration issues | Medium | Medium | Incremental commits, phase-by-phase testing |

### Mitigation Strategies

1. **Incremental Implementation:** 8 phases with clear checkpoints
2. **Frequent Testing:** Test after each phase, not just at the end
3. **Interface Stability:** External interfaces remain unchanged
4. **Rollback Plan:** Git branching strategy, commit after each phase
5. **Safety Checks:** Timeout limits in loops, extensive assertions

---

## Next Steps (For Implementation)

**Ready to Begin Implementation:**

1. **Start with Phase 1:** FSM Infrastructure
   - Reference: `docs/multi-cycle-minimal-plan.md` Section "Phase 1"
   - Checklist: `docs/IMPLEMENTATION_CHECKLIST.md` Phase 1

2. **Use Provided Templates:** Code templates in minimal plan document

3. **Track Progress:** Check off items in `IMPLEMENTATION_CHECKLIST.md`

4. **Verify Incrementally:** Run verification commands after each phase

5. **Commit Frequently:** Commit after completing each phase

---

## Documentation Cross-Reference

### For Quick Implementation:
- **`multi-cycle-minimal-plan.md`** - Main technical reference with code templates
- **`IMPLEMENTATION_CHECKLIST.md`** - Phase tracking and task list

### For Detailed Background:
- **`multi-cycle-implementation/00-overview.md`** - Executive summary
- **`multi-cycle-implementation/01-current-architecture.md`** - Single-cycle analysis
- **`multi-cycle-implementation/02-state-machine-design.md`** - FSM details
- **`multi-cycle-implementation/03-rtl-modifications.md`** - Complete RTL changes
- **`multi-cycle-implementation/04-host-simulator-changes.md`** - Simulator details
- **`multi-cycle-implementation/05-testing-strategy.md`** - Testing approach
- **`multi-cycle-implementation/06-implementation-phases.md`** - Detailed phases

### For Navigation:
- **`multi-cycle-implementation/README.md`** - Documentation index

---

## Conclusion

The planning phase for the multi-cycle CPU upgrade is **complete**. This deliverable provides:

✅ **Clear Implementation Path:** 8 well-defined phases with time estimates
✅ **Actionable Code Templates:** Ready-to-use FSM and control logic patterns
✅ **Comprehensive Tracking:** Detailed checklist for monitoring progress
✅ **Risk Mitigation:** Incremental approach with frequent verification
✅ **Documentation:** Well-organized reference materials

**The project is ready to proceed to implementation.**

---

**Deliverable Status:** ✅ **Complete**

**Planning Completed:** 2026-01-03

**Estimated Implementation Time:** 10-12 days

**Next Phase:** Implementation Phase 1 - FSM Infrastructure
