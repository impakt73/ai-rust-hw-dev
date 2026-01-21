# Getting Started Guide

## Critical Prerequisites

### 1. Verilator Installation (REQUIRED)

**⚠️ IMPORTANT:** Verilator MUST be installed before running any tests. This is the most common cause of build failures.

```bash
# Install Verilator on Ubuntu/Debian
sudo apt-get update
sudo apt-get install -y verilator

# Verify installation
verilator --version
```

Without Verilator, all tests will fail with errors like:
```
Error: Invocation of Verilator failed
Error: No such file or directory
```

### 2. Rust Toolchain

Standard Rust toolchain (tested with 1.92.0+):
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Quick Start Commands

```bash
# From repository root
cargo test                    # Run all tests
cargo build                   # Build only
cargo fmt                     # Format code
cargo clippy -- -D warnings   # Lint code
verilator --lint-only rtl/*.sv # Lint SystemVerilog
```

## Tech Stack Overview

- **RTL:** SystemVerilog (in `rtl/` directory)
- **Verification:** Rust with marlin + Verilator (in `testbench/` directory)
- **Build System:** Cargo workspace with 6 members: cpu-sim, riscv_core, testbench, riscv_protocol, riscv_macros, vcd-mcp
- **Debug Infrastructure:** FIFO-based packet protocol with formatted print macros

## Project Structure

```
.
├── Cargo.toml              # Workspace root
├── rtl/                    # SystemVerilog RTL
│   ├── alu.sv             # Arithmetic Logic Unit (RV32I + M extension)
│   ├── regfile.sv         # 32x32-bit register file
│   ├── decoder.sv         # Instruction decoder
│   ├── decompress.sv      # RV32C instruction decompressor
│   ├── fetch_buffer.sv    # RV32C fetch buffer (handles compressed instruction alignment)
│   ├── div_unit.sv        # Hardware division unit (used by ALU)
│   ├── csr_file.sv        # Control and Status Registers (Zicsr)
│   ├── branch_unit.sv     # Branch comparison logic
│   ├── mem_interface.sv   # Memory interface logic
│   ├── writeback_mux.sv   # Writeback multiplexer
│   └── top.sv             # Top-level CPU module (multi-cycle FSM control)
├── testbench/              # Rust verification (integration tests)
│   ├── Cargo.toml         # Test package dependencies
│   └── tests/             # Integration test files
│       ├── alu_test.rs    # ALU verification tests
│       ├── regfile_test.rs # Register file tests
│       ├── decompress_test.rs # RV32C decompressor tests (41 tests)
│       ├── fp_regfile_test.rs # FP register file tests
│       └── fpu_test.rs    # FPU tests
├── cpu-sim/               # CPU simulator
├── riscv_core/            # Shared Verilator bindings
├── riscv_protocol/        # Debug packet protocol
├── riscv_macros/          # Print macros for bare-metal programs
└── test_programs/         # Example test programs (ELF binaries)
```

## Dependencies

### Rust Crates

- **marlin 0.10:** Hardware simulation framework with Verilator backend
  - Feature flag: `verilog` (enabled)
- **rand 0.8:** Random number generation for test inputs

### System Dependencies

- **Verilator 5.020+:** SystemVerilog simulator/compiler
- **C++ compiler:** Required by Verilator (usually pre-installed)

## Common Issues and Solutions

### Issue: Tests fail with "Verilator not found"
**Solution:** Install Verilator (see Prerequisites section above)

### Issue: Tests fail after RTL changes
**Solution:** 
```bash
cargo clean  # Clear cached Verilator builds
cargo test   # Rebuild from scratch
```

### Issue: Formatting errors in CI
**Solution:**
```bash
cargo fmt    # Auto-format all Rust code
```

### Issue: Clippy warnings
**Solution:**
```bash
cargo clippy --fix  # Auto-fix when possible
```

## Performance Notes

- First test run is slow (~15-30 seconds) due to Verilator compilation
- Subsequent runs are fast (~1-2 seconds) due to caching
- Parallel test execution is safe (tests are independent)
- Use `cargo test -- --test-threads=1` if debugging race conditions

## Additional Resources

- **RISC-V Spec:** https://riscv.org/technical/specifications/
- **Marlin Documentation:** https://docs.rs/marlin/
- **Verilator Manual:** https://verilator.org/guide/latest/
