# AGENTS.md - Guide for AI Coding Agents

This document provides essential information for AI coding agents working on this RISC-V hardware verification project.

## Project Overview

This is a **single-cycle RISC-V RV32I CPU** implementation in SystemVerilog with Rust-based verification using the `marlin` crate and Verilator.

**Key Components:**
- **RTL (SystemVerilog):** Hardware implementation in `rtl/` directory
- **Verification (Rust):** Test harness in `tests/` directory using marlin + Verilator
- **Architecture:** Single-cycle design with exposed memory ports (no internal memory)

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

## Building and Testing

### Quick Start

```bash
# From repository root
cargo test

# Run specific test suite
cargo test --package cpu_verifier -- alu_test
cargo test --package cpu_verifier -- regfile_test
cargo test --package cpu_verifier -- cpu_test

# Build only (without running tests)
cargo build

# Clean build artifacts
cargo clean
```

### Test Structure

The project has 22 comprehensive tests:
- **7 ALU tests:** Validate arithmetic/logic operations
- **6 Register file tests:** Validate register behavior (including x0 immutability)
- **9 CPU integration tests:** Validate complete instruction execution

### Verilator Build Process

The marlin crate automatically:
1. Compiles SystemVerilog files with Verilator
2. Creates shared libraries in `target/verilator/`
3. Links them to Rust test code

Build artifacts are cached between runs for performance.

## Code Quality Standards

### Linting and Formatting

```bash
# Format Rust code
cargo fmt

# Check formatting without modifying
cargo fmt -- --check

# Run clippy for warnings
cargo clippy -- -D warnings

# Lint SystemVerilog files
verilator --lint-only rtl/*.sv
```

All code should pass these checks before committing.

## Project Structure

```
.
├── Cargo.toml              # Workspace root
├── rtl/                    # SystemVerilog RTL
│   ├── alu.sv             # Arithmetic Logic Unit
│   ├── regfile.sv         # 32x32-bit register file
│   ├── decoder.sv         # Instruction decoder
│   └── top.sv             # Top-level CPU module
├── tests/                  # Rust verification
│   ├── Cargo.toml         # Test package dependencies
│   ├── build.rs           # Build script (watches RTL changes)
│   └── src/
│       ├── lib.rs         # Test module declarations
│       ├── alu_test.rs    # ALU verification tests
│       ├── regfile_test.rs # Register file tests
│       └── cpu_test.rs    # CPU integration tests
└── .github/
    └── workflows/
        └── ci.yml         # GitHub Actions CI/CD
```

## RTL Architecture Details

### Module Hierarchy

```
top (CPU)
├── decoder (Instruction decoder)
├── alu (ALU operations)
└── regfile (Register file)
```

### Key Design Decisions

1. **Single-cycle execution:** All instructions complete in one clock cycle
2. **Exposed memory ports:** Instruction and data memory are external (managed by testbench)
3. **Register x0 hardwired to zero:** Hardware enforcement (not just software convention)
4. **Branch comparison in top module:** Direct comparison logic (not ALU-based)
5. **Immediate selection:** Store instructions use `imm_s`, others use `imm_i`

### Supported Instructions

- **Arithmetic:** ADD, ADDI, SUB
- **Logic:** AND, ANDI, OR, ORI, XOR, XORI
- **Shifts:** SLL, SLLI, SRL, SRLI, SRA, SRAI
- **Comparison:** SLT, SLTI, SLTU, SLTIU
- **Branches:** BEQ, BNE, BLT, BGE, BLTU, BGEU
- **Memory:** LW, SW
- **Upper Immediate:** LUI, AUIPC
- **Jumps:** JAL, JALR

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

## Testing Best Practices

### When Adding New Tests

1. **Location:** Add to appropriate test file (`alu_test.rs`, `regfile_test.rs`, or `cpu_test.rs`)
2. **Register in lib.rs:** Add module declaration if creating new test file
3. **Use helper macros:** 
   - `clock_cycle!(dut)` for clock edge transitions
   - `create_runtime()` for consistent test setup
4. **Memory management:** 
   - Use `HashMap<u32, u32>` for instruction/data memory
   - Read `dmem_addr` AFTER `eval()` for stores
   - Set `dmem_rdata` BEFORE `eval()` for loads

### Test Naming Convention

- Prefix with `test_`
- Use descriptive names: `test_cpu_branch_beq_bne`, `test_alu_shift_ops`
- Group related tests logically

## Modifying RTL

### After Changing SystemVerilog Files

1. **Lint the RTL:** `verilator --lint-only rtl/modified_file.sv`
2. **Clean build:** `cargo clean` (Verilator cache may be stale)
3. **Run tests:** `cargo test`
4. **Verify all tests pass:** Look for `test result: ok` with 22 passed

### Signal Naming Conventions

- Use `snake_case` for signal names
- Prefix with purpose: `imem_`, `dmem_`, `alu_`, etc.
- Keep ports consistent with RISC-V naming: `rs1`, `rs2`, `rd`, `funct3`, etc.

## CI/CD Pipeline

GitHub Actions automatically:
1. Installs Verilator and Rust
2. Caches dependencies for faster builds
3. Runs `cargo build --verbose`
4. Runs `cargo test --verbose`
5. Checks formatting with `cargo fmt --check` (non-blocking)
6. Runs `cargo clippy` (non-blocking)

**Note:** CI runs on every push to `copilot/**` branches and PRs to `main`.

## Dependencies

### Rust Crates

- **marlin 0.10:** Hardware simulation framework with Verilator backend
  - Feature flag: `verilog` (enabled)
- **rand 0.8:** Random number generation for test inputs

### System Dependencies

- **Verilator 5.020+:** SystemVerilog simulator/compiler
- **C++ compiler:** Required by Verilator (usually pre-installed)

## Performance Notes

- First test run is slow (~15-30 seconds) due to Verilator compilation
- Subsequent runs are fast (~1-2 seconds) due to caching
- Parallel test execution is safe (tests are independent)
- Use `cargo test -- --test-threads=1` if debugging race conditions

## Debugging Tips

### Enable Verbose Output

```bash
cargo test -- --nocapture  # See println! output from tests
cargo test -- --show-output # Show output even for passing tests
```

### Run Single Test

```bash
cargo test --package cpu_verifier -- test_cpu_branch_beq_bne --nocapture
```

### Check Verilator Compilation

Verilator creates intermediate C++ files in `target/verilator/`. Check these if you suspect compilation issues.

## Additional Resources

- **RISC-V Spec:** https://riscv.org/technical/specifications/
- **Marlin Documentation:** https://docs.rs/marlin/
- **Verilator Manual:** https://verilator.org/guide/latest/

## Contact and Support

For issues specific to this implementation, refer to:
- PR discussions in the repository
- Code comments in RTL and test files
- This AGENTS.md file

---

**Last Updated:** 2025-12-28
**Maintainer:** Automated by GitHub Copilot
