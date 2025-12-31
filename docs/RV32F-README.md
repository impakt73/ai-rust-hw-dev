# RV32F Implementation Documentation

This directory contains comprehensive documentation for implementing the RISC-V RV32F (Single-Precision Floating Point) extension.

## Documents

### 1. [rv32f-upgrade-plan.md](rv32f-upgrade-plan.md) - Main Implementation Plan

**Purpose:** Complete technical specification and implementation roadmap

**Contents:**
- RV32F extension overview (26 instructions)
- IEEE 754-2008 compliance requirements
- RTL architecture and module designs
- Complete code examples for all new modules
- 10-phase implementation plan (16-25 days)
- Build configuration updates
- Risk assessment and mitigation
- Validation criteria

**Length:** 1,879 lines / 58 KB

**Audience:** AI coding agents, hardware engineers, project planners

### 2. [rv32f-testing-guide.md](rv32f-testing-guide.md) - Testing Implementation Guide

**Purpose:** Practical guide for implementing comprehensive tests

**Contents:**
- Test file organization
- Test templates with working code examples
- Instruction encoding helper functions
- FP register file tests (5 tests)
- FPU unit tests (20-25 tests)
- CPU integration tests (10-15 tests)
- Test execution and debugging procedures

**Length:** 433 lines / 12 KB

**Audience:** Test developers, AI coding agents implementing tests

## Quick Start

### For Implementation

1. **Read** `rv32f-upgrade-plan.md` sections 1-3 to understand requirements
2. **Follow** Phase 1-10 in section 7 for step-by-step implementation
3. **Reference** appendices for IEEE 754 details and instruction encodings
4. **Use** testing guide to create tests in parallel with RTL development

### For Testing

1. **Read** `rv32f-testing-guide.md` for test organization
2. **Copy** test templates as starting point
3. **Use** instruction encoding helpers for CPU integration tests
4. **Follow** test execution section for running and debugging

## Implementation Timeline

| Phase | Estimated Time | Key Deliverables |
|-------|----------------|------------------|
| Phase 1 | 1-2 days | FP register file + tests |
| Phase 2 | 3-5 days | Basic FPU + tests |
| Phase 3 | 2-3 days | Complete FPU operations |
| Phase 4 | 1-2 days | Updated decoder |
| Phase 5 | 2-3 days | Integrated top module |
| Phase 6 | 2-3 days | CPU integration tests |
| Phase 7 | 1-2 days | Assembly test programs |
| Phase 8 | 1 day | Build configuration |
| Phase 9 | 1 day | Documentation updates |
| Phase 10 | 2-3 days | Final validation |

**Total:** 16-25 days

## Key Features

### F Extension Adds:
- 32 floating point registers (f0-f31)
- 26 floating point instructions
- IEEE 754-2008 single-precision compliance
- 5 rounding modes
- 5 exception flags
- FP load/store (FLW, FSW)
- FP arithmetic (FADD, FSUB, FMUL, FDIV, FSQRT)
- FP comparisons (FEQ, FLT, FLE)
- FP conversions (int ↔ float)
- Fused multiply-add operations

### New RTL Modules:
- `rtl/fp_regfile.sv` - 32×32-bit FP register file
- `rtl/fpu.sv` - Floating point unit

### Updated RTL Modules:
- `rtl/decoder.sv` - FP instruction decoding
- `rtl/top.sv` - FPU/FP regfile integration, FCSR

### New Test Files:
- `tests/src/fp_regfile_test.rs` - FP register file tests
- `tests/src/fpu_test.rs` - FPU unit tests
- `tests/src/cpu_fp_test.rs` - CPU FP integration tests

## Success Criteria

**Implementation Complete When:**
- [ ] All 26 F extension instructions implemented
- [ ] All RTL modules pass Verilator lint
- [ ] 35+ new FP tests pass (total 119-129 tests)
- [ ] IEEE 754 compliance verified
- [ ] All existing RV32IM tests still pass (no regressions)
- [ ] Documentation updated
- [ ] CI/CD pipeline passes

## Resources

**RISC-V Specifications:**
- [RISC-V Unprivileged ISA](https://riscv.org/technical/specifications/)
- Chapter 11: "F" Standard Extension for Single-Precision Floating-Point

**IEEE Standards:**
- IEEE 754-2008: Standard for Floating-Point Arithmetic

**Testing:**
- [RISC-V Architectural Test Suite](https://github.com/riscv-non-isa/riscv-arch-test)

## Notes for AI Coding Agents

These documents are specifically optimized for AI-driven implementation:
- Clear phase-by-phase instructions
- Complete code examples (copy-paste ready)
- Specific file names and paths
- Validation checklists
- Expected outputs and test counts
- Common pitfalls and solutions

Follow the phases sequentially, validate after each phase, and use `report_progress` to commit completed work.

---

**Created:** 2025-12-31
**Status:** Ready for implementation
**Total Documentation:** 2,312 lines / 70 KB
