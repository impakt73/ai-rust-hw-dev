# ai-rust-hw-dev

A **multi-cycle non-pipelined RISC-V RV32IM CPU** implementation in SystemVerilog with Rust-based verification using Verilator.

## Features

- ✅ **Complete RV32IM Instruction Set (RV32I + M Extension + Zicsr)**: All 54 instructions including:
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
  - **Zicsr Extension (6 instructions):**
    - CSR (Control and Status Register) access instructions
- ✅ **Multi-cycle Non-pipelined Architecture**: FSM-based design with 11 states for efficient resource sharing
- ✅ **Variable-latency Memory Support**: Ready/valid handshaking for realistic memory operations
- ✅ **Verilator-based Verification**: 146 comprehensive tests using Rust + marlin framework
- ✅ **CPU Simulator**: Run bare-metal RISC-V ELF executables with VCD waveform dumping and configurable memory latency
- ✅ **Exposed Memory Ports**: Instruction and data memory managed externally for flexibility
- ✅ **Debug Infrastructure**: FIFO-based packet protocol with formatted print macros for bare-metal programs

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

- **`rtl/`** - SystemVerilog RTL implementation (ALU, register file, decoder, CSR file, top module)
- **`tests/`** - Rust-based verification tests (cpu_verifier package)
- **`cpu-sim/`** - Command-line CPU simulator for running ELF executables with VCD waveform dumping
- **`riscv_core/`** - Shared Verilator bindings and utilities
- **`riscv_protocol/`** - Debug packet protocol definitions
- **`riscv_macros/`** - Formatted print macros for bare-metal RISC-V programs
- **`test_programs/`** - Example RISC-V assembly and Rust test programs
- **`rust-test-program/`** - Bare-metal Rust test programs (separate workspace)

## Documentation

- **[AGENTS.md](AGENTS.md)** - Comprehensive guide for developers and AI agents
- **[cpu-sim/README.md](cpu-sim/README.md)** - CPU simulator usage, VCD waveform dumping, and debugging features
- **[test_programs/README.md](test_programs/README.md)** - Information about test programs
- **[riscv_macros/README.md](riscv_macros/README.md)** - Formatted print macros for bare-metal programs

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.