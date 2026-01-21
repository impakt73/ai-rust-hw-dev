# AGENTS.md - AI Coding Agent Guide

**Welcome!** This is your starting point for working on the RISC-V hardware verification project. This document provides an overview and directs you to specialized documentation based on your task.

## Project Summary

This is a **multi-cycle non-pipelined RISC-V RV32IMACF CPU** implementation in SystemVerilog with Rust-based verification using the `marlin` crate and Verilator. The project features:

- **118 RISC-V instructions:** RV32IMACF + Zicsr (base, multiply/divide, atomics, compressed, floating-point, CSR)
- **Multi-cycle FSM architecture:** 12-state design with variable-latency memory
- **Comprehensive test suite:** 264 tests across RTL, verification, and utilities
- **Debug infrastructure:** FIFO-based packet protocol with formatted print macros

## Quick Navigation

### 🚀 Just Getting Started?
**→ Read [Getting Started Guide](docs/agents/getting-started.md)**
- Prerequisites (Verilator installation - **REQUIRED!**)
- Quick start commands
- Project structure overview
- Common setup issues

### 🎯 Need to Choose the Right Agent?
**→ Read [Custom Agent Selection Guide](docs/agents/custom-agents.md)**

This project has specialized agents for different work types:
- **Hardware-Software Integration Architect:** Cross-layer tasks (RTL + Rust)
- **FPGA Architect:** Pure RTL/hardware design
- **Rust Verification Architect:** Pure testing/verification
- **AI Instruction Architect:** Documentation and agent configuration

The selection guide helps you pick the right agent for your task.

### 📋 Task-Specific Documentation

Choose based on what you're working on:

| Working On | Read This Document |
|------------|-------------------|
| **Writing or debugging tests** | [Testing Guide](docs/agents/testing.md) |
| **RTL/hardware design** | [RTL Development Guide](docs/agents/rtl-development.md) |
| **Rust code or verification** | [Rust Development Guide](docs/agents/rust-development.md) |
| **CI failures or PR prep** | [CI/CD Guide](docs/agents/ci-cd.md) |
| **Debugging issues** | [Debugging Guide](docs/agents/debugging.md) |
| **Agent configuration** | [Custom Agent Selection](docs/agents/custom-agents.md) |

## Choosing the Right Custom Agent

**Quick Decision Tree:**
```
What files do you need to modify?
├─ Only .sv (RTL)?          → FPGA Architect
├─ Only .rs (Rust)?         → Rust Verification Architect  
├─ Both .sv and .rs?        → HW-SW Integration Architect
└─ Documentation/agents?    → AI Instruction Architect
```

**Detailed selection guide with expertise areas and example tasks:**
**→ [Custom Agent Selection Guide](docs/agents/custom-agents.md)**

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
- See [Debugging Guide](docs/agents/debugging.md) for delegation patterns

### Code Quality (Mandatory for Rust)

**All Rust code changes MUST:**
- ✅ Run `cargo fmt` before committing
- ✅ Run `cargo clippy --fix` to auto-fix warnings (do this FIRST!)
- ✅ Run `cargo clippy -- -D warnings` before committing  
- ✅ Address all clippy warnings (zero tolerance)

**Key Principle:** Use `cargo clippy --fix` **BEFORE** manually addressing warnings to save time and avoid fixing issues that can be automatically resolved.

**Memory management:**
- ❌ Never use `Box::leak()` to circumvent lifetime issues
- ✅ Use callbacks or proper ownership patterns
- ✅ Best solution depends on the situation (see [Rust Development Guide](docs/agents/rust-development.md))

## Documentation Map

### Core Guides

**[Getting Started Guide](docs/agents/getting-started.md)**
- Prerequisites and Verilator installation (**CRITICAL!**)
- Quick start commands and project structure
- Dependencies and common setup issues
- Performance notes

**[Testing Guide](docs/agents/testing.md)**
- Test structure (integration tests vs unit tests)
- Test suite overview (264 tests across packages)
- Running and debugging tests
- Best practices and naming conventions
- Verilator build process

**[RTL Development Guide](docs/agents/rtl-development.md)**
- Multi-cycle architecture (12-state FSM)
- Module hierarchy and supported instructions (118 total)
- Instruction cycle counts and memory interface
- Signal naming conventions
- Hardware debugging methodology

**[Rust Development Guide](docs/agents/rust-development.md)**
- Coding conventions and standards
- Mandatory code quality checks (fmt, clippy)
- Memory management best practices
- Error handling and type safety
- Project-specific patterns

**[CI/CD Guide](docs/agents/ci-cd.md)**
- CI pipeline overview and workflow steps
- PR readiness checklist (**read before marking PR ready!**)
- How to check CI status (CLI and web)
- Common failure scenarios and solutions
- Local development workflow

**[Debugging Guide](docs/agents/debugging.md)**
- Concrete data over abstract reasoning (critical principle!)
- Debugging hardware (RTL) with `$display()` statements
- Debugging Rust tests with verbose output
- When and how to delegate complex debugging
- Common issues and advanced techniques

**[Custom Agent Selection Guide](docs/agents/custom-agents.md)**
- Detailed guide to choosing the right agent
- Expertise areas and example tasks for each agent
- Decision tree and delegation patterns
- Agent-specific rules and best practices

## Essential Information for Quick Reference

### Prerequisites (MUST READ!)

**⚠️ Verilator MUST be installed before running tests!**

```bash
sudo apt-get update && sudo apt-get install -y verilator
verilator --version  # Verify installation
```

Without Verilator, all tests fail. This is the #1 cause of build failures.

### Quick Start Commands

```bash
cargo test                    # Run all tests (264 total)
cargo build                   # Build only
cargo fmt                     # Format Rust code (mandatory before commit)
cargo clippy --fix            # Auto-fix clippy warnings (run FIRST!)
cargo clippy -- -D warnings   # Lint Rust code (mandatory before commit)
verilator --lint-only rtl/*.sv # Lint SystemVerilog
cargo clean                   # Clear Verilator cache (after RTL changes)
```

### PR Readiness Checklist

Before marking PR ready for review:

1. ✅ All tests pass: `cargo test --verbose`
2. ✅ Code formatted: `cargo fmt -- --check`
3. ✅ Clippy auto-fix run: `cargo clippy --fix` (do this FIRST!)
4. ✅ No clippy warnings: `cargo clippy -- -D warnings`
5. ✅ SystemVerilog linted (if modified): `verilator --lint-only rtl/*.sv`
6. ✅ All CI checks pass on GitHub Actions

**See [CI/CD Guide](docs/agents/ci-cd.md) for detailed verification steps.**

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

## Common Workflows

### Adding a New Instruction

1. Check [Custom Agent Selection](docs/agents/custom-agents.md) → Use **HW-SW Integration Architect**
2. Read [RTL Development Guide](docs/agents/rtl-development.md) for architecture
3. Read [Testing Guide](docs/agents/testing.md) for test patterns
4. Implement RTL changes and tests
5. Follow [CI/CD Guide](docs/agents/ci-cd.md) for PR readiness

### Debugging a Test Failure

1. Check [Debugging Guide](docs/agents/debugging.md) for methodology
2. Consider delegating to specialized agent (see guide)
3. Use concrete data approach (no abstract reasoning!)
4. Add debug output to observe actual behavior

### Improving Test Coverage

1. Check [Custom Agent Selection](docs/agents/custom-agents.md) → Use **Rust Verification Architect**
2. Read [Testing Guide](docs/agents/testing.md) for structure and best practices
3. Read [Rust Development Guide](docs/agents/rust-development.md) for conventions
4. Follow [CI/CD Guide](docs/agents/ci-cd.md) for validation

### Optimizing RTL

1. Check [Custom Agent Selection](docs/agents/custom-agents.md) → Use **FPGA Architect**
2. Read [RTL Development Guide](docs/agents/rtl-development.md) for architecture
3. Use [Debugging Guide](docs/agents/debugging.md) for verification
4. Follow [CI/CD Guide](docs/agents/ci-cd.md) for PR readiness

## Getting Help

### When You Need More Detail

Each sub-document provides comprehensive information on its topic:
- Architecture details
- Code examples  
- Best practices
- Common pitfalls
- Troubleshooting

**Browse the `docs/agents/` directory** or use the navigation table above.

### When You're Stuck

1. Check [Debugging Guide](docs/agents/debugging.md) for troubleshooting
2. Review [Getting Started Guide](docs/agents/getting-started.md) for common issues
3. Consider delegating to a specialized agent (see Custom Agent Selection)

## Document Organization

This documentation follows **progressive disclosure** principles:

- **AGENTS.md (this file):** Overview and navigation
- **docs/agents/*.md:** Detailed topic-specific guides
- **Each guide is self-contained** for the topic it covers
- **Links between guides** help you navigate related topics

Load only the documentation relevant to your current task to keep context focused and efficient.

---

**Last Updated:** 2026-01-18  
**Maintained by:** GitHub Copilot Custom Agents
