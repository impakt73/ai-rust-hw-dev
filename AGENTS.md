# AGENTS.md - AI Coding Agent Guide

**Welcome!** This is your starting point for working on the RISC-V hardware verification project.

## Project Summary

This is a **multi-cycle non-pipelined RISC-V RV32IMACF CPU** implementation in SystemVerilog with Rust-based verification using the `marlin` crate and Verilator. The project features:

- **118 RISC-V instructions:** RV32IMACF + Zicsr (base, multiply/divide, atomics, compressed, floating-point, CSR)
- **Multi-cycle FSM architecture:** 12-state design with variable-latency memory
- **Comprehensive test suite:** 264 tests across RTL, verification, and utilities
- **Debug infrastructure:** FIFO-based packet protocol with formatted print macros

## Choosing the Right Custom Agent

This project has **four specialized custom agents** for different work types:

**Quick Decision Tree:**
```
What files do you need to modify?
├─ Only .sv (RTL)?          → FPGA Architect
├─ Only .rs (Rust)?         → Rust Verification Architect  
├─ Both .sv and .rs?        → HW-SW Integration Architect
└─ Documentation/agents?    → AI Instruction Architect
```

### Custom Agent Overview

1. **Hardware-Software Integration Architect** - Cross-layer tasks (RTL + Rust)
   - Adding/modifying RISC-V instructions
   - Integration issues between RTL and Rust testbench
   - Tasks involving both `.sv` and `.rs` files

2. **FPGA Architect** - Pure RTL/hardware design
   - RTL refactoring and optimization
   - State machine design
   - Pure `.sv` file changes

3. **Rust Verification Architect** - Pure testing/verification
   - Adding test cases
   - Test infrastructure improvements
   - Pure `.rs` file changes

4. **AI Instruction Architect** - Documentation and agent configuration
   - Creating/modifying agent definitions
   - Documentation updates
   - Prompt engineering

## Critical Rules for ALL Agents

### Debugging Methodology

**For hardware-related work (FPGA Architect, HW-SW Integration):**
- ❌ **Never rely on abstract reasoning** during hardware debugging
- ✅ **Always use concrete data** from simulation (`$display()` statements)
- ✅ **Observe actual signal values** before forming hypotheses
- ✅ **Treat debugging like experimental science** - gather data first, then reason

**For all agents:**
- **Delegate non-trivial debugging** to specialized agents unless debugging IS the main task
- Preserves context and leverages specialized expertise

### Code Quality (Mandatory for Rust)

**All Rust code changes MUST:**
- ✅ Run `cargo fmt` before committing
- ✅ Run `cargo clippy --fix --allow-dirty` to auto-fix warnings (do this FIRST!)
- ✅ Rerun `cargo clippy -- -D warnings` to check remaining warnings
- ✅ Address all clippy warnings (zero tolerance)

**Key Principle:** Use `cargo clippy --fix --allow-dirty` **BEFORE** manually addressing warnings to save time and avoid fixing issues that can be automatically resolved. The `--allow-dirty` flag is required to fix warnings when you have uncommitted changes. Always rerun clippy after auto-fix to detect any new warnings introduced by the fixes.

**Memory management:**
- ❌ Never use `Box::leak()` to circumvent lifetime issues
- ✅ Use callbacks or proper ownership patterns
- ✅ Best solution depends on the situation

## Essential Quick Reference

### Prerequisites (MUST READ!)

**⚠️ Verilator MUST be installed before running tests!**

```bash
sudo apt-get update && sudo apt-get install -y verilator
verilator --version  # Verify installation
```

Without Verilator, all tests fail. This is the #1 cause of build failures.

**Rust Toolchain** (tested with 1.92.0+):
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Tech Stack

- **RTL:** SystemVerilog (in `rtl/` directory)
- **Verification:** Rust with marlin + Verilator (in `testbench/` directory)
- **Build System:** Cargo workspace with 6 members: cpu-sim, riscv_core, testbench, riscv_protocol, riscv_macros, vcd-mcp
- **Debug Infrastructure:** FIFO-based packet protocol with formatted print macros

### Project Structure

```
.
├── Cargo.toml              # Workspace root
├── rtl/                    # SystemVerilog RTL modules
│   ├── top.sv             # Top-level CPU (multi-cycle FSM control)
│   ├── alu.sv, decoder.sv, regfile.sv, etc.
├── testbench/              # Rust verification (integration tests)
│   └── tests/             # Integration test files
├── cpu-sim/               # CPU simulator
├── riscv_core/            # Shared Verilator bindings
└── test_programs/         # Example test programs (ELF binaries)
```

### Quick Start Commands

**Main Workspace (Root):**
```bash
cargo test                             # Run all tests (264 total)
cargo build                            # Build only
cargo fmt                              # Format Rust code (mandatory before commit)
cargo clippy --fix --allow-dirty       # Auto-fix clippy warnings (run FIRST!)
cargo clippy -- -D warnings            # Lint Rust code (mandatory before commit)
verilator --lint-only rtl/*.sv         # Lint SystemVerilog
cargo clean                            # Clear Verilator cache (after RTL changes)
```

**Separate rust-test-program Workspace:**
```bash
cd rust-test-program
cargo build                            # Build only
cargo fmt                              # Format Rust code
cargo clippy --fix --allow-dirty       # Auto-fix clippy warnings
cargo clippy -- -D warnings            # Lint Rust code
cd ..
```

### PR Readiness Checklist

Before marking PR ready for review:

**Main Workspace (Root):**
1. ✅ All tests pass: `cargo test --verbose`
2. ✅ Code formatted: `cargo fmt -- --check`
3. ✅ Clippy auto-fix run: `cargo clippy --fix --allow-dirty` (do this FIRST!)
4. ✅ No clippy warnings: `cargo clippy -- -D warnings` (rerun after auto-fix)
5. ✅ SystemVerilog linted (if modified): `verilator --lint-only rtl/*.sv`

**rust-test-program Workspace:**
6. ✅ Code formatted: `(cd rust-test-program && cargo fmt -- --check)`
7. ✅ Clippy auto-fix run: `(cd rust-test-program && cargo clippy --fix --allow-dirty)` (do this FIRST!)
8. ✅ No clippy warnings: `(cd rust-test-program && cargo clippy -- -D warnings)` (rerun after auto-fix)

**Additional Checks:**
9. ✅ All CI checks pass on GitHub Actions

### Security Scanning

**DO NOT run CodeQL or security scans for this project.**
- Not needed for hardware verification projects
- Often times out and wastes time
- Skip all automated security scanning tools

### Important Memory Addresses

From repository memory: DRAM range is **0x80000000 - 0xFFFFFFFF**

Tests must use addresses in this range:
```rust
lui(reg, 0x80000000);  // Load upper immediate
sw(reg, val, offset);   // Store with offset
```

### Common Setup Issues

**Tests fail with "Verilator not found":**
```bash
sudo apt-get install -y verilator
```

**Tests fail after RTL changes:**
```bash
cargo clean  # Clear cached Verilator builds
cargo test
```

**Performance Notes:**
- First test run is slow (~15-30 seconds) due to Verilator compilation
- Subsequent runs are fast (~1-2 seconds) due to caching

---

**Last Updated:** 2026-01-23  
**Maintained by:** GitHub Copilot Custom Agents

