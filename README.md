# ai-rust-hw-dev

A **single-cycle RISC-V RV32IMC CPU** implementation in SystemVerilog with Rust-based verification using Verilator.

## Features

- ✅ **Complete RV32IMC Instruction Set (RV32I + M Extension + C Extension + Zicsr)**: All 81 instructions including:
  - **RV32I Base (40 instructions):**
    - Arithmetic, logic, and shift operations
    - Load/store with byte, halfword, and word access (LB, LH, LW, LBU, LHU, SB, SH, SW)
    - Branch and jump instructions
    - Upper immediate instructions (LUI, AUIPC)
    - Memory ordering (FENCE)
    - System instructions (ECALL, EBREAK)
  - **M Extension (8 instructions):**
    - Integer multiplication: MUL, MULH, MULHSU, MULHU
    - Integer division and remainder: DIV, DIVU, REM, REMU
  - **C Extension (27 instructions):**
    - 16-bit compressed instruction support for improved code density (25-30% size reduction)
    - Automatic decompression to 32-bit equivalents
    - Dynamic PC increment (2 bytes for compressed, 4 for standard)
    - All quadrants supported: arithmetic, memory access, control flow
  - **Zicsr Extension (6 instructions):**
    - CSR (Control and Status Register) access instructions
- ✅ **Single-cycle Execution**: All instructions complete in one clock cycle
- ✅ **Verilator-based Verification**: 92 comprehensive tests using Rust + marlin framework
- ✅ **CPU Simulator**: Run bare-metal RISC-V ELF executables
- ✅ **Exposed Memory Ports**: Instruction and data memory managed externally for flexibility

## Quick Start

### Prerequisites

```bash
# Install Verilator (Ubuntu/Debian)
sudo apt-get update && sudo apt-get install -y verilator

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Build and Test

```bash
# Run all tests
cargo test

# Run CPU simulator with example program
cargo run --package cpu-sim -- test_programs/test.elf --verbose
```

## Project Structure

- **`rtl/`** - SystemVerilog RTL implementation (ALU, register file, decoder, top module)
- **`tests/`** - Rust-based verification tests (cpu_verifier package)
- **`cpu-sim/`** - Command-line CPU simulator for running ELF executables
- **`riscv_core/`** - Shared Verilator bindings and utilities
- **`test_programs/`** - Example RISC-V assembly and Rust test programs

## Documentation

- **[AGENTS.md](AGENTS.md)** - Comprehensive guide for developers and AI agents
- **[cpu-sim/README.md](cpu-sim/README.md)** - CPU simulator usage and details
- **[test_programs/README.md](test_programs/README.md)** - Information about test programs

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.