# GitHub Copilot Instructions

## Project Overview

This is a **single-cycle RISC-V RV32I CPU** implementation in SystemVerilog with Rust-based verification using the `marlin` crate and Verilator.

## Critical Prerequisites

⚠️ **IMPORTANT:** Verilator MUST be installed before running any tests. This is the most common cause of build failures.

```bash
# Install Verilator on Ubuntu/Debian
sudo apt-get update
sudo apt-get install -y verilator

# Verify installation
verilator --version
```

## Quick Start

```bash
# From repository root
cargo test                    # Run all tests
cargo build                   # Build only
cargo fmt                     # Format code
cargo clippy -- -D warnings   # Lint code
verilator --lint-only rtl/*.sv # Lint SystemVerilog
```

## Tech Stack

- **RTL:** SystemVerilog (in `rtl/` directory)
- **Verification:** Rust with marlin + Verilator (in `tests/` directory)
- **Build System:** Cargo workspace with 3 members: cpu-sim, riscv_core, tests

## Coding Conventions

### Rust Code
- Follow standard Rust formatting (run `cargo fmt` before committing)
- Address all clippy warnings (`cargo clippy -- -D warnings`)
- Write tests for new functionality
- Use meaningful test names prefixed with `test_`

### SystemVerilog Code
- Use `snake_case` for signal names
- Prefix signals by purpose: `imem_`, `dmem_`, `alu_`, etc.
- Lint with `verilator --lint-only` before committing
- Keep ports consistent with RISC-V naming: `rs1`, `rs2`, `rd`, `funct3`, etc.

## Testing Requirements

- All 28 tests must pass before marking PR ready for review
- Run `cargo test --verbose` to verify
- After RTL changes, run `cargo clean` to clear Verilator cache
- Use `cargo test -- --nocapture` to see test output during debugging

## PR Readiness Checklist

Before marking a PR as ready for review, verify:
1. ✅ All tests pass: `cargo test --verbose`
2. ✅ Code is formatted: `cargo fmt -- --check`
3. ✅ No clippy warnings: `cargo clippy -- -D warnings`
4. ✅ SystemVerilog is linted (if modified): `verilator --lint-only rtl/*.sv`
5. ✅ All CI checks pass on GitHub Actions

## Detailed Documentation

**For comprehensive information, always refer to [AGENTS.md](../AGENTS.md) which includes:**
- Project architecture and design decisions
- Complete build and test procedures
- RTL module hierarchy and supported instructions
- Common issues and solutions
- Testing best practices and conventions
- CI/CD pipeline details
- Debugging tips and performance notes

**Always consult AGENTS.md for project-specific conventions, dependencies, and workflows.**
