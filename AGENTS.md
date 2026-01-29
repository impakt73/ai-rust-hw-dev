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
├── sim-tests/             # Helper crate to build test programs from rust-test-program/
└── rust-test-program/     # Bare-metal Rust test programs (auto-built when tests run)
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

**Complete Memory Map:**

```
Address Range          | Device           | Type | Description
-----------------------|------------------|------|----------------------------
0x10000000-0x100000FF | SimControl       | Rust | Simulation control
0x20000000-0x2000000F | Video            | Rust | Video frame buffer
0x30000000-0x3000000F | Audio            | Rust | Audio buffer
0x40000000-0x40000007 | FIFO             | Rust | Host communication FIFO
0x50000000-0x5000000F | LED Controller   | RTL  | 8-bit LED output register
0x52000000-0x520000FF | UART Controller  | RTL  | UART TX/RX with 8-byte FIFOs
0x51000000-0x51FFFFFF | Reserved (RTL)   | RTL  | Reserved for future RTL peripherals
0x52000100-0x5FFFFFFF | Reserved (RTL)   | RTL  | Reserved for future RTL peripherals
0x80000000-0xFFFFFFFF | DRAM             | Both | System memory (2 GiB)
```

**LED Controller Peripheral:**
- **Address:** 0x50000000
- **Register:** LED_OUT (read/write)
  - Bits [7:0]: LED output data
  - Bits [31:8]: Reserved (read as 0, writes ignored)
- **Access sizes:** Byte, halfword, word
- **Latency:** Single-cycle (ready = 1'b1)

**RTL vs Rust Peripherals:**
- **RTL peripherals** (0x50000000-0x5FFFFFFF): Handled by Verilator, synthesizable to FPGA
- **Rust peripherals** (0x10000000-0x4FFFFFFF): Handled by SystemBus, simulation only

**DRAM range:** 0x80000000 - 0xFFFFFFFF

Tests must use addresses in the DRAM range:
```rust
lui(reg, 0x80000000);  // Load upper immediate
sw(reg, val, offset);   // Store with offset
```

**LED Controller Usage Example:**
```rust
// Write pattern to LED
lui(15, 0x50000000);  // Load LED base address
addi(14, 0, 0xAA);    // Load pattern 0xAA
sw(15, 14, 0);        // Write to LED_OUT register

// Read back LED value
lw(13, 15, 0);        // Read LED_OUT into register x13
```

**UART Controller Peripheral:**
- **Address:** 0x52000000-0x520000FF (256 bytes)
- **Registers:**
  - 0x00: TXDATA (WO) - Transmit data (write byte to TX FIFO)
  - 0x04: RXDATA (RO) - Receive data (read byte from RX FIFO)
  - 0x08: STATUS (RO) - Status register (FIFO status flags)
  - 0x0C: CTRL (RW) - Control register (reserved)
- **STATUS Register Bits:**
  - Bit 0: TX_FULL - TX FIFO is full
  - Bit 1: TX_EMPTY - TX FIFO is empty (all data transmitted)
  - Bit 2: TX_BUSY - TX shift register is active
  - Bit 4: RX_FULL - RX FIFO is full
  - Bit 5: RX_EMPTY - RX FIFO is empty (no data available)
  - Bit 6: RX_BUSY - RX shift register is active
  - Bit 7: RX_ERROR - Framing error detected

**UART Controller Usage Example:**
```rust
// UART Usage Example - Write byte and read received byte
lui(15, 0x52000000);  // Load UART base address

// Write byte to TX FIFO
addi(14, 0, 0x48);    // ASCII 'H' = 0x48
sw(15, 14, 0);        // Write to TXDATA (offset 0x00)

// Poll for TX_EMPTY (bit 1)
lw(13, 15, 0x08);     // Read STATUS
andi(12, 13, 0x02);   // Check TX_EMPTY bit
beq(12, 0, -8);       // Loop until TX_EMPTY

// Poll for !RX_EMPTY (bit 5) - for loopback testing
lw(13, 15, 0x08);     // Read STATUS  
andi(12, 13, 0x20);   // Check RX_EMPTY bit
bne(12, 0, -8);       // Loop until data available

// Read received byte
lw(11, 15, 0x04);     // Read RXDATA
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

