# Multi-Cycle Non-Pipelined CPU Implementation Plan

## Executive Summary

**STATUS: ✅ COMPLETED** - This document describes the plan that was successfully implemented to convert the CPU from single-cycle to multi-cycle architecture.

This document series provided the comprehensive technical plan for converting the existing **single-cycle RV32IM CPU** implementation to a **multi-cycle non-pipelined CPU**. The plan was designed for consumption by AI coding agents and included detailed RTL modifications, host-side simulator changes, and comprehensive testing strategies.

**Previous Implementation:** Single-cycle RV32IM CPU where every instruction completed in exactly one clock cycle.

**Current Implementation:** Multi-cycle non-pipelined CPU where instructions take multiple clock cycles to complete, with different instruction types requiring different numbers of cycles.

## Document Organization

This plan is organized across multiple markdown files for clarity:

1. **00-overview.md** (this file) - Executive summary and high-level design
2. **01-current-architecture.md** - Analysis of existing RTL
3. **02-state-machine-design.md** - State machine and control unit design
4. **03-rtl-modifications.md** - Detailed RTL changes required
5. **04-host-simulator-changes.md** - Host-side simulator modifications
6. **05-testing-strategy.md** - Comprehensive testing approach
7. **06-implementation-phases.md** - Step-by-step implementation checklist

## Why Multi-Cycle?

### Single-Cycle Limitations

The current single-cycle design has several characteristics:

| Characteristic | Single-Cycle | Impact |
|----------------|--------------|--------|
| Cycle Time | Long (slowest instruction) | Division and multiplication limit clock speed |
| Resource Sharing | None | Separate datapaths for each operation |
| Instruction Latency | 1 cycle | All instructions same latency |
| Control Logic | Simple combinational | Easy to understand |

### Multi-Cycle Benefits

| Characteristic | Multi-Cycle | Impact |
|----------------|-------------|--------|
| Cycle Time | Short (single operation) | Clock can run faster |
| Resource Sharing | Yes | ALU reused for PC increment |
| Instruction Latency | Variable (3-5+ cycles) | Different instructions take different time |
| Control Logic | FSM-based | More complex but more flexible |

## High-Level Design Approach

### Core Principles

1. **Minimal Interface Changes:** The external memory interface remains unchanged. Host-side simulator changes should be minimal.

2. **State Machine Control:** A finite state machine (FSM) controls instruction execution across multiple cycles.

3. **Resource Sharing:** The ALU is shared for:
   - Address calculation (load/store)
   - PC increment
   - Arithmetic operations
   - Branch comparison

4. **Register Isolation:** Intermediate results are stored in internal registers between cycles.

### Instruction Cycle Counts

| Instruction Class | Cycles | States |
|-------------------|--------|--------|
| R-type (ADD, SUB, etc.) | 4 | FETCH → DECODE → EXECUTE → WRITEBACK |
| I-type Arithmetic | 4 | FETCH → DECODE → EXECUTE → WRITEBACK |
| Load (LW, LH, LB) | 5 | FETCH → DECODE → MEM_ADDR → MEM_READ → WRITEBACK |
| Store (SW, SH, SB) | 4 | FETCH → DECODE → MEM_ADDR → MEM_WRITE |
| Branch | 3 | FETCH → DECODE → BRANCH |
| Jump (JAL/JALR) | 4 | FETCH → DECODE → EXECUTE → WRITEBACK |
| Upper Immediate | 4 | FETCH → DECODE → EXECUTE → WRITEBACK |
| M-Extension (MUL) | 4 | FETCH → DECODE → EXECUTE → WRITEBACK |
| M-Extension (DIV) | 4 | FETCH → DECODE → EXECUTE → WRITEBACK |
| System (FENCE) | 2 | FETCH → DECODE |
| System (ECALL/EBREAK) | 2 | FETCH → DECODE (halt) |
| CSR Operations | 4 | FETCH → DECODE → CSR → WRITEBACK |

**Note on Division:** In real hardware, division typically uses iterative algorithms taking 32+ cycles. This plan uses SystemVerilog's `/` and `%` operators which synthesize to combinational logic, keeping division at 4 cycles (same as other ALU operations). A future enhancement could implement iterative division.

### State Diagram

```
                    ┌───────────┐
                    │   IDLE    │ (after reset or halt)
                    └─────┬─────┘
                          │ rst_n deasserted
                          ▼
                    ┌───────────┐
            ┌───────│   FETCH   │◄──────────────────────┐
            │       └─────┬─────┘                        │
            │             │                              │
            │             ▼                              │
            │       ┌───────────┐                        │
            │       │  DECODE   │                        │
            │       └─────┬─────┘                        │
            │             │                              │
            │    ┌────────┼────────┬────────┬────────┐  │
            │    │        │        │        │        │  │
            │    ▼        ▼        ▼        ▼        ▼  │
            │  ┌─────┐  ┌─────┐  ┌─────┐  ┌─────┐ ┌────┐│
            │  │EXEC │  │MADDR│  │BRANCH│ │HALT │ │NOP ││
            │  └──┬──┘  └──┬──┘  └──┬───┘ └─────┘ └──┬─┘│
            │     │        │       │                 │  │
            │     │        ▼       │                 │  │
            │     │   ┌────────┐   │                 │  │
            │     │   │MEM_R/W │   │                 │  │
            │     │   └───┬────┘   │                 │  │
            │     │       │        │                 │  │
            │     ▼       ▼        │                 │  │
            │  ┌─────────────┐     │                 │  │
            │  │  WRITEBACK  │     │                 │  │
            │  └──────┬──────┘     │                 │  │
            │         │            │                 │  │
            └─────────┴────────────┴─────────────────┘  │
                      │                                  │
                      └──────────────────────────────────┘
```

## Impact on Existing Components

### RTL Modules

| Module | Change Required | Complexity |
|--------|-----------------|------------|
| `top.sv` | **Major** - Add FSM, register isolation, multi-cycle control | High |
| `decoder.sv` | **Minor** - No changes needed, runs in DECODE state | Low |
| `alu.sv` | **None** - ALU remains combinational | None |
| `regfile.sv` | **Minor** - Write enable gated by FSM state | Low |

### Host-Side Code

| Component | Change Required | Complexity |
|-----------|-----------------|------------|
| `sim.rs` | **Minimal** - Wait for instruction completion | Low |
| `riscv_core/lib.rs` | **None** - Module interfaces unchanged | None |
| Test harness | **Minor** - Update cycle counting expectations | Low |

## Key Design Decisions

### 1. Memory Interface

**Decision:** Keep the existing memory interface unchanged.

**Rationale:** 
- External instruction and data memory ports remain the same
- Simulator memory handling doesn't need modification
- Tests can largely remain unchanged

```systemverilog
// These interfaces remain UNCHANGED
output logic [31:0] imem_addr,
input  logic [31:0] imem_data,
output logic [31:0] dmem_addr,
output logic [31:0] dmem_wdata,
input  logic [31:0] dmem_rdata,
output logic        dmem_we,
output logic        dmem_re,
output logic [1:0]  dmem_size
```

### 2. Instruction Completion Signaling

**Decision:** Add an `instr_complete` output signal.

**Rationale:**
- Host simulator needs to know when an instruction finishes
- Enables accurate cycle counting
- Minimal impact on existing test infrastructure

```systemverilog
// NEW output signal
output logic instr_complete  // High for one cycle when instruction completes
```

### 3. State Encoding

**Decision:** Use one-hot encoding for FSM states.

**Rationale:**
- Simpler decode logic
- Clearer for debugging
- Easier for AI agents to understand

### 4. Intermediate Registers

**Decision:** Add latching registers for:
- Instruction Register (IR)
- ALU operand registers (A, B)
- ALU output register (ALU_OUT)
- Memory data register (MDR)

**Rationale:**
- Isolate pipeline stages
- Enable resource sharing
- Store intermediate results across cycles

## Compatibility Notes

### Backward Compatibility

The multi-cycle CPU will:
- Execute the same RV32IM instruction set
- Produce identical results for all programs
- Support same boot address mechanism
- Use same memory interface protocol

### Breaking Changes

The multi-cycle CPU will:
- Take multiple cycles per instruction (different cycle counts)
- Add new output signal (`instr_complete`)
- Change internal timing (register writes in later cycles)

### Test Impact

- **Existing tests will need updating** to account for multi-cycle execution
- PC tracking will need to wait for `instr_complete`
- Cycle counting assertions will change

## Next Steps

Proceed to the following documents in order:

1. **[01-current-architecture.md](01-current-architecture.md)** - Detailed analysis of existing RTL
2. **[02-state-machine-design.md](02-state-machine-design.md)** - FSM design details
3. **[03-rtl-modifications.md](03-rtl-modifications.md)** - Specific code changes
4. **[04-host-simulator-changes.md](04-host-simulator-changes.md)** - Simulator updates
5. **[05-testing-strategy.md](05-testing-strategy.md)** - Test plan
6. **[06-implementation-phases.md](06-implementation-phases.md)** - Implementation checklist

---

**Document Status:** ✅ Complete

**Last Updated:** 2026-01-01
