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
   - ❌ **NOT for implementation plans** — Implementation plans require deep domain knowledge. Use the domain-specific agent instead: FPGA Architect (RTL plans), Rust Verification Architect (Rust/test plans), or HW-SW Integration Architect (cross-layer plans). Using the wrong agent produces low-quality plans.

## Critical Rules for ALL Agents

### Session Start Behavior

**Do NOT run tests or lints at the start of a session when the branch has no existing changes compared to the target branch (i.e. a brand new PR).**

- CI checks guarantee that all tests and lints pass on the target branch before a new agent session begins.
- When starting fresh on a branch with no prior changes, the codebase is already in a known-good state — running tests/lints before making any changes is redundant and wastes time.
- **Only run tests and lints AFTER making code changes** to verify that your specific changes are correct.
- If a session resumes mid-PR where the branch already has existing changes, running tests/lints to understand the current state may be appropriate.

### Reset Style (RTL)

- **Use synchronous resets only** across project RTL modules.
- Keep reset ports active-low (`rst_n`) unless the module interface requires otherwise.
- For sequential logic, use `always_ff @(posedge clk)` (or domain clock) and handle reset inside the block with `if (!rst_n)`.

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

**After making ANY Rust code changes, agents MUST run:**
- ✅ `cargo fmt` before committing
- ✅ `cargo clippy --fix --allow-dirty` to auto-fix warnings (do this FIRST!)
- ✅ `cargo clippy -- -D warnings` to check remaining warnings and address all (zero tolerance)

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

- **RTL:** SystemVerilog (in `rtl/common/` directory)
- **Verification:** Rust with marlin + Verilator (in `testbench/` directory)
- **Build System:** Cargo workspace with 11 members: cpu-sim, riscv_core, testbench, vcd-mcp, sim-view, riscv_shared, sim-tests, fpga-host, host-bus-handler, device-runtime, bus-shared
- **Debug Infrastructure:** FIFO-based packet protocol with formatted print macros

### Project Structure

```
.
├── Cargo.toml              # Workspace root
├── rtl/                    # All SystemVerilog RTL modules
│   ├── common/            # Shared RTL modules
│   │   ├── top.sv         # Top-level module (CPU with RTL peripherals)
│   │   ├── cpu/           # CPU core modules (cpu.sv, alu.sv, decoder.sv, etc.)
│   │   ├── fpu/           # Floating-point unit modules
│   │   ├── io/            # I/O modules (uart.sv, host_bus_interface.sv, etc.)
│   │   ├── memory/        # Memory modules (bus.sv, sram.sv, etc.)
│   │   ├── peripherals/   # RTL peripherals (LED, clock, SRAM, system controller)
│   │   ├── primitives/    # Primitive modules (ff_sync.sv, sync_fifo.sv, etc.)
│   │   └── wrappers/      # Test wrapper modules
│   └── fpga/              # FPGA synthesis files for iCE40-HX8K
├── testbench/              # Rust verification (integration tests)
│   └── tests/             # Integration test files
├── cpu-sim/               # CPU simulator
├── riscv_core/            # Shared Verilator bindings
├── riscv_shared/          # Shared constants and peripheral register definitions
├── bus-shared/            # Shared bus/SystemBus implementation
├── sim-tests/             # Helper crate to build test programs from rust-test-program/
├── sim-view/              # Real-time video/audio viewer for simulated programs
├── device-runtime/        # Unified runtime abstraction for simulator and FPGA backends
├── fpga-host/             # Host-side FPGA communication utilities
├── host-bus-handler/      # Host bus protocol handler
├── vcd-mcp/               # VCD waveform MCP server
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
find rtl/common -name '*.sv' -exec verilator --lint-only {} +  # Lint SystemVerilog
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
5. ✅ SystemVerilog linted (if modified): `find rtl/common -name '*.sv' -exec verilator --lint-only {} +`
6. ✅ FPGA synthesis verified (if SystemVerilog modified): `(cd rtl/fpga && make)`

**rust-test-program Workspace:**
7. ✅ Code formatted: `(cd rust-test-program && cargo fmt -- --check)`
8. ✅ Clippy auto-fix run: `(cd rust-test-program && cargo clippy --fix --allow-dirty)` (do this FIRST!)
9. ✅ No clippy warnings: `(cd rust-test-program && cargo clippy -- -D warnings)` (rerun after auto-fix)

**Additional Checks:**
10. ✅ All CI checks pass on GitHub Actions

### Security Scanning

**DO NOT run CodeQL or security scans for this project.**
- Not needed for hardware verification projects
- Often times out and wastes time
- Skip all automated security scanning tools

### Important Memory Addresses

**Complete Memory Map** (see `docs/memory-map.md` for the full reference):

```
Address Range          | Device           | Type | Description
-----------------------|------------------|------|----------------------------
0x40000000-0x40000003 | SimControl       | Rust | Simulation control
0x40001000-0x4000100F | Video            | Rust | Video frame buffer
0x40002000-0x4000200F | Audio            | Rust | Audio buffer
0x40003000-0x40003007 | FIFO             | Rust | Host communication FIFO
0x40004000-0x40004013 | DMA              | Rust | DMA controller
0x50000000-0x5000000F | LED Controller   | RTL  | 8-bit LED output register
0x51000000-0x5100000F | Clock Peripheral | RTL  | Elapsed time counters (us/ms/s)
0x52000000-0x52002FFF | SRAM Peripheral  | RTL  | 12KB on-chip SRAM
0x52003000-0x52FFFFFF | Reserved (RTL)   | RTL  | Reserved for future RTL peripherals
0x53000000-0x5300000F | System Controller| RTL  | CPU boot and reset control
0x80000000-0xFFFFFFFF | DRAM             | Both | System memory (2 GiB)
```

**LED Controller Peripheral:**
- **Address:** 0x50000000
- **Register:** LED_OUT (read/write)
  - Bits [7:0]: LED output data
  - Bits [31:8]: Reserved (read as 0, writes ignored)
- **Access sizes:** Byte, halfword, word
- **Latency:** Single-cycle (ready = 1'b1)

**Clock Peripheral:**
- **Address:** 0x51000000
- **Registers (all read-only):**
  - 0x00: ELAPSED_US - Elapsed microseconds since reset
  - 0x04: ELAPSED_MS - Elapsed milliseconds since reset
  - 0x08: ELAPSED_S - Elapsed seconds since reset
- **Access sizes:** Word (32-bit)
- **Latency:** Single-cycle (ready = 1'b1)
- **Note:** Clock frequency is configurable via CLK_FREQ_HZ parameter

**RTL vs Rust Peripherals:**
- **RTL peripherals** (0x50000000-0x5FFFFFFF): Handled by Verilator, synthesizable to FPGA
- **Rust peripherals** (0x40000000-0x4FFFFFFF): Handled by SystemBus, simulation only

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

**Clock Peripheral Usage Example:**
```rust
// Read elapsed time from clock peripheral
lui(15, 0x51000000);  // Load Clock peripheral base address

// Read elapsed microseconds
lw(10, 15, 0x00);     // Read ELAPSED_US into register x10

// Read elapsed milliseconds
lw(11, 15, 0x04);     // Read ELAPSED_MS into register x11

// Read elapsed seconds
lw(12, 15, 0x08);     // Read ELAPSED_S into register x12

// Delay loop using clock peripheral (wait 100ms)
lw(10, 15, 0x04);     // Read start time (ms)
addi(11, 10, 100);    // target = start + 100ms
delay_loop:
lw(12, 15, 0x04);     // Read current time (ms)
blt(12, 11, delay_loop); // Loop until current >= target
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

**Last Updated:** 2026-02-22  
**Maintained by:** GitHub Copilot Custom Agents
