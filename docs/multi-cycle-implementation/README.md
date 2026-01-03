# Multi-Cycle CPU Implementation Documentation

This directory contains comprehensive documentation for converting the single-cycle RISC-V RV32IM CPU to a multi-cycle, latency-insensitive design.

## Documentation Overview

### For AI Coding Agents: Quick Start

**👉 START HERE:** For a streamlined, minimal-risk implementation plan optimized for AI coding agents, read:
- **[`../multi-cycle-minimal-plan.md`](../multi-cycle-minimal-plan.md)** - Condensed, actionable plan with code templates

### Complete Technical Documentation

For detailed technical background and comprehensive implementation details:

1. **[00-overview.md](00-overview.md)** - Executive summary, motivation, and high-level design
2. **[01-current-architecture.md](01-current-architecture.md)** - Detailed analysis of existing single-cycle RTL
3. **[02-state-machine-design.md](02-state-machine-design.md)** - FSM states, transitions, and control signals
4. **[03-rtl-modifications.md](03-rtl-modifications.md)** - Detailed RTL code changes
5. **[04-host-simulator-changes.md](04-host-simulator-changes.md)** - Rust simulator updates
6. **[05-testing-strategy.md](05-testing-strategy.md)** - Comprehensive testing approach
7. **[06-implementation-phases.md](06-implementation-phases.md)** - Step-by-step implementation checklist

## Key Differences Between Documents

| Document | Target Audience | Focus | Length |
|----------|----------------|-------|--------|
| `multi-cycle-minimal-plan.md` | **AI Coding Agents** | Minimal implementation, code templates, quick reference | Concise (~500 lines) |
| `multi-cycle-implementation/*.md` | Human Engineers & Detailed Review | Complete technical details, rationale, comprehensive coverage | Detailed (~2000+ lines total) |

## Implementation Strategy

**Goal:** Multi-cycle CPU with latency-insensitive design (no pipelined execution, no complex performance features)

**Key Changes:**
- Add FSM with 11 states (IDLE, FETCH, DECODE, EXECUTE, MEM_ADDR, MEM_READ, MEM_WRITE, WRITEBACK, BRANCH, CSR, HALT)
- Instructions take 3-5 cycles (variable based on type)
- Add `instr_complete` output signal for simulator synchronization
- External memory interface remains unchanged
- Add latching registers for multi-cycle operation (IR, A/B registers, ALU_OUT, MDR, latched control signals)

**Estimated Timeline:** 10-12 days for minimal-risk implementation

## Quick Reference: Cycle Counts

| Instruction Type | Cycles | Path |
|------------------|--------|------|
| R-type (ADD, MUL, DIV) | 4 | FETCH → DECODE → EXECUTE → WRITEBACK |
| I-type Arithmetic | 4 | FETCH → DECODE → EXECUTE → WRITEBACK |
| Load | 5 | FETCH → DECODE → MEM_ADDR → MEM_READ → WRITEBACK |
| Store | 4 | FETCH → DECODE → MEM_ADDR → MEM_WRITE |
| Branch | 3 | FETCH → DECODE → BRANCH |
| Jump | 4 | FETCH → DECODE → EXECUTE → WRITEBACK |
| CSR | 4 | FETCH → DECODE → CSR → WRITEBACK |
| FENCE | 2 | FETCH → DECODE |
| ECALL/EBREAK | 2+ | FETCH → DECODE → HALT |

## Related Documentation

- **[../../AGENTS.md](../../AGENTS.md)** - Main project documentation for AI agents
- **[../../README.md](../../README.md)** - Project overview

## Status

- ✅ **Planning Complete** - Documentation finalized
- ⏳ **Implementation Pending** - Ready to begin
- ⬜ **Testing Pending** - Awaits implementation
- ⬜ **Verification Pending** - Awaits testing

---

**Last Updated:** 2026-01-03

**Created By:** AI Coding Agent (GitHub Copilot)
