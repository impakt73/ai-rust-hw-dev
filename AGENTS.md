# AGENTS.md - Guide for AI Coding Agents

This document provides essential information for AI coding agents working on this RISC-V hardware verification project.

## Project Overview

This is a **multi-cycle non-pipelined RISC-V RV32IM CPU** implementation in SystemVerilog with Rust-based verification using the `marlin` crate and Verilator.

**Key Components:**
- **RTL (SystemVerilog):** Hardware implementation in `rtl/` directory
- **Verification (Rust):** Test harness in `tests/` directory using marlin + Verilator
- **Architecture:** Multi-cycle non-pipelined design with 11-state FSM and variable-latency memory support
- **Memory Interface:** Ready/valid handshaking for instruction and data memory operations
- **Debug Infrastructure:** FIFO-based packet protocol with formatted print macros for bare-metal programs

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

The project has 146 comprehensive tests across all packages:
- **tests package (63 tests):**
  - ALU tests: Validate arithmetic/logic operations + M extension (MUL, MULH, MULHSU, MULHU, DIV, DIVU, REM, REMU)
  - Register file tests: Validate register behavior (including x0 immutability)
  - CPU integration tests: Validate complete instruction execution including:
    - Arithmetic, logic, and memory operations
    - Branches and jumps
    - Byte/halfword operations
    - System instructions (FENCE, ECALL, EBREAK)
    - CSR operations (read/write, set/clear, immediate variants)
    - M extension operations (multiplication, division, remainder)
- **Other packages (83 tests):**
  - cpu-sim: 22 integration tests including:
    - ELF loading and execution
    - FIFO communication and packet protocol
    - VCD waveform dumping validation
    - Instruction trace callbacks with comprehensive validation
    - Programmatic instruction sequence testing with trace verification
    - Combined trace + VCD testing
    - Variable memory latency testing
  - riscv_core: 33 utility and tracing tests
  - riscv_protocol: 6 packet serialization/deserialization tests
  - riscv_macros: 13 macro functionality tests
  - cpu-sim test modules: 9 additional integration tests

**New Validation Tests:**
- `test_comprehensive_trace_validation`: Validates instruction trace accuracy for 12+ instructions with full operand checking (PC, register values, immediates)
- `test_trace_with_branches`: Ensures branch instructions skip correct sequences in trace output
- `test_trace_and_vcd_together`: Demonstrates VCD + instruction trace working simultaneously

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

**⚠️ BEFORE PR REVIEW:** Verify all CI checks pass. See [PR Readiness and CI Verification](#pr-readiness-and-ci-verification) for complete requirements.

## Project Structure

```
.
├── Cargo.toml              # Workspace root
├── rtl/                    # SystemVerilog RTL
│   ├── alu.sv             # Arithmetic Logic Unit (RV32I + M extension)
│   ├── regfile.sv         # 32x32-bit register file
│   ├── decoder.sv         # Instruction decoder
│   ├── csr_file.sv        # Control and Status Registers (Zicsr)
│   ├── branch_unit.sv     # Branch comparison logic
│   ├── pc_control.sv      # Program counter control
│   ├── mem_interface.sv   # Memory interface logic
│   ├── writeback_mux.sv   # Writeback multiplexer
│   └── top.sv             # Top-level CPU module
├── tests/                  # Rust verification
│   ├── Cargo.toml         # Test package dependencies
│   ├── build.rs           # Build script (watches RTL changes)
│   └── src/
│       ├── lib.rs         # Test module declarations
│       ├── alu_test.rs    # ALU verification tests
│       ├── regfile_test.rs # Register file tests
│       └── cpu_test.rs    # CPU integration tests
├── cpu-sim/               # CPU simulator
│   └── src/
│       ├── main.rs        # CLI entry point
│       ├── sim.rs         # Simulator implementation
│       └── tests.rs       # Integration tests
├── riscv_core/            # Shared Verilator bindings
├── riscv_protocol/        # Debug packet protocol
├── riscv_macros/          # Print macros for bare-metal programs
├── test_programs/         # Example test programs (ELF binaries)
└── .github/
    └── workflows/
        └── ci.yml         # GitHub Actions CI/CD
```

## RTL Architecture Details

### Module Hierarchy

```
top (CPU)
├── decoder (Instruction decoder)
├── alu (ALU operations - RV32I + M extension)
├── regfile (Register file)
├── csr_file (Control and Status Registers)
├── branch_unit (Branch comparison)
├── pc_control (Program counter logic)
├── mem_interface (Memory interface logic)
└── writeback_mux (Result selection)
```

### Key Design Decisions

1. **Multi-cycle execution:** Instructions take 3-5+ base cycles plus variable memory latency
2. **FSM-based control:** 11-state finite state machine (IDLE, FETCH, DECODE, EXECUTE, MEM_ADDR, MEM_READ, MEM_WRITE, WRITEBACK, BRANCH, CSR, HALT)
3. **Variable-latency memory:** Ready/valid handshaking on instruction and data memory interfaces
4. **Exposed memory ports:** Instruction and data memory are external (managed by testbench)
5. **Register x0 hardwired to zero:** Hardware enforcement (not just software convention)
6. **Separate branch unit:** Dedicated branch comparison logic (not ALU-based)
7. **CSR support:** Full Control and Status Register implementation (Zicsr extension)
8. **FIFO-based debug:** MMIO FIFO at 0x40000000 for host communication with packet protocol
9. **Staging registers:** Flip-flop based intermediate storage for multi-cycle operation (FPGA-safe, no latches)

### Supported Instructions

**Complete RV32IM Instruction Set (54 instructions):**

**RV32I Base:**
- **Arithmetic:** ADD, ADDI, SUB
- **Logic:** AND, ANDI, OR, ORI, XOR, XORI
- **Shifts:** SLL, SLLI, SRL, SRLI, SRA, SRAI
- **Comparison:** SLT, SLTI, SLTU, SLTIU
- **Branches:** BEQ, BNE, BLT, BGE, BLTU, BGEU
- **Memory:** LW, LH, LB, LHU, LBU, SW, SH, SB
- **Upper Immediate:** LUI, AUIPC
- **Jumps:** JAL, JALR
- **Memory Ordering:** FENCE
- **System:** ECALL, EBREAK

**M Extension (Integer Multiplication and Division):**
- **Multiplication:** MUL, MULH, MULHSU, MULHU
- **Division:** DIV, DIVU
- **Remainder:** REM, REMU

**Zicsr Extension:**
- **CSR Access:** CSRRW, CSRRS, CSRRC, CSRRWI, CSRRSI, CSRRCI

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

## PR Readiness and CI Verification

### Before Marking a PR Ready for Review

**⚠️ CRITICAL:** A pull request should ONLY be marked as ready for review after verifying that all CI checks pass successfully.

#### Required Pre-Review Checklist

1. **Run all tests locally:**
   ```bash
   cargo test --verbose
   ```
   All 146 tests must pass.

2. **Verify code formatting:**
   ```bash
   cargo fmt -- --check
   ```
   No formatting issues should be reported.

3. **Check for clippy warnings:**
   ```bash
   cargo clippy -- -D warnings
   ```
   No warnings or errors should appear.

4. **Lint SystemVerilog files (if RTL was modified):**
   ```bash
   verilator --lint-only rtl/*.sv
   ```
   No lint errors should be reported.

5. **Verify CI pipeline status:**
   - Push your changes to the branch
   - Wait for GitHub Actions CI workflow to complete
   - Check that all CI jobs pass successfully (green checkmark)
   - If any CI check fails, investigate and fix before requesting review

#### How to Check CI Status

Using GitHub CLI:
```bash
# Check status of latest workflow run for your branch
gh run list --branch your-branch-name --limit 1

# View details of a specific run
gh run view <run-id>

# View logs if there are failures
gh run view <run-id> --log-failed
```

Using GitHub Web Interface:
- Navigate to the "Actions" tab in the repository
- Find the workflow run for your latest commit
- Verify all jobs show a green checkmark (✓)
- Click on any failed jobs to view logs and diagnose issues

#### Common CI Failure Scenarios

**Build failures:**
- Check that all Rust code compiles without errors
- Verify SystemVerilog files are syntactically correct
- Run `cargo build --verbose` locally to reproduce

**Test failures:**
- Run `cargo test --verbose` locally to identify failing tests
- Review test output and fix the underlying issues
- Ensure RTL changes haven't broken existing functionality

**Formatting check failures:**
- Run `cargo fmt` to auto-format code
- Commit the formatting changes
- Push and wait for CI to re-run

**Clippy warnings:**
- Run `cargo clippy --fix` to auto-fix when possible
- Manually address remaining warnings
- Commit fixes and verify CI passes

### CI Pipeline Details

The CI workflow runs automatically on:
- Every push to branches matching `copilot/**` pattern
- Every pull request targeting `main` branch

The workflow executes the following checks:
1. ✅ **Build:** `cargo build --verbose`
2. ✅ **Tests:** `cargo test --verbose` (all 146 tests must pass)
3. ✅ **Formatting:** `cargo fmt -- --check` (must pass - blocking)
4. ✅ **Clippy:** `cargo clippy -- -D warnings` (must pass - blocking)

**Note:** All checks including formatting and clippy are now blocking in CI. Your code must pass all checks before it can be merged.

## Modifying RTL

### After Changing SystemVerilog Files

1. **Lint the RTL:** `verilator --lint-only rtl/modified_file.sv`
2. **Clean build:** `cargo clean` (Verilator cache may be stale)
3. **Run tests:** `cargo test`
4. **Verify all tests pass:** Look for `test result: ok` with 146 total tests passed

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
5. Checks formatting with `cargo fmt --check` (blocking - must pass)
6. Runs `cargo clippy -- -D warnings` (blocking - must pass)

**Note:** CI runs on every push to `copilot/**` branches and PRs to `main`. All checks are mandatory and must pass for CI to succeed.

**⚠️ IMPORTANT:** Before marking a PR as ready for review, verify that all CI checks pass. See the [PR Readiness and CI Verification](#pr-readiness-and-ci-verification) section for detailed requirements.

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

## Multi-cycle Architecture Details

### FSM States

The CPU uses an 11-state finite state machine:

1. **S_IDLE (0x0):** After reset, before first fetch
2. **S_FETCH (0x1):** Request instruction from memory, wait for `imem_ready`
3. **S_DECODE (0x2):** Decode instruction, read registers
4. **S_EXECUTE (0x3):** Execute ALU operation
5. **S_MEM_ADDR (0x4):** Calculate memory address for load/store
6. **S_MEM_READ (0x5):** Request data from memory, wait for `dmem_ready`
7. **S_MEM_WRITE (0x6):** Write data to memory, wait for `dmem_ready`
8. **S_WRITEBACK (0x7):** Write result to destination register
9. **S_BRANCH (0x8):** Evaluate branch condition and update PC
10. **S_CSR (0x9):** Execute CSR operation
11. **S_HALT (0xA):** ECALL/EBREAK halt state

### Instruction Cycle Counts

Different instruction types require different numbers of cycles:

| Instruction Class | Base Cycles | States |
|-------------------|-------------|--------|
| R-type (ADD, SUB, etc.) | 4 | FETCH → DECODE → EXECUTE → WRITEBACK |
| I-type Arithmetic | 4 | FETCH → DECODE → EXECUTE → WRITEBACK |
| Load (LW, LH, LB) | 5 | FETCH → DECODE → MEM_ADDR → MEM_READ → WRITEBACK |
| Store (SW, SH, SB) | 4 | FETCH → DECODE → MEM_ADDR → MEM_WRITE |
| Branch | 3 | FETCH → DECODE → BRANCH |
| Jump (JAL/JALR) | 4 | FETCH → DECODE → EXECUTE → WRITEBACK |
| Upper Immediate | 4 | FETCH → DECODE → EXECUTE → WRITEBACK |
| M-Extension (MUL/DIV) | 4 | FETCH → DECODE → EXECUTE → WRITEBACK |
| System (FENCE) | 2 | FETCH → DECODE |
| System (ECALL/EBREAK) | 2 | FETCH → DECODE → HALT |
| CSR Operations | 4 | FETCH → DECODE → CSR → WRITEBACK |

**Note:** Memory latency adds additional cycles. For example, with 3-cycle memory latency, a load instruction takes 5 base cycles + 3 cycles in FETCH + 3 cycles in MEM_READ = 11 total cycles.

### Memory Interface Signals

The multi-cycle design adds handshaking signals:

**Instruction Memory:**
- `imem_req` (output): CPU requests instruction fetch
- `imem_ready` (input): Memory has valid instruction data
- `imem_addr` (output): Instruction address
- `imem_data` (input): Instruction data

**Data Memory:**
- `dmem_req` (output): CPU requests memory operation
- `dmem_ready` (input): Memory operation complete
- `dmem_addr` (output): Data address
- `dmem_wdata` (output): Write data
- `dmem_rdata` (input): Read data
- `dmem_we` (output): Write enable
- `dmem_re` (output): Read enable
- `dmem_size` (output): Operation size (byte/halfword/word)

### Instruction Completion Signal

- `instr_complete` (output): High for 1 cycle when instruction finishes execution

---

**Last Updated:** 2026-01-09
**Maintainer:** Automated by GitHub Copilot
