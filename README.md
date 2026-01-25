# ai-rust-hw-dev

A **multi-cycle non-pipelined RISC-V RV32IMACF CPU** implementation in SystemVerilog with Rust-based verification using Verilator.

## Features

- ✅ **Complete RV32IMACF Instruction Set (RV32I + M + A + C + F + Zicsr)**: All 118 instructions including:
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
  - **A Extension (11 instructions):**
    - Load-Reserved/Store-Conditional: LR.W, SC.W
    - Atomic memory operations: AMOSWAP.W, AMOADD.W, AMOXOR.W, AMOAND.W, AMOOR.W
    - Atomic MIN/MAX operations: AMOMIN.W, AMOMAX.W, AMOMINU.W, AMOMAXU.W
  - **C Extension (27 instructions):**
    - 16-bit compressed instructions for improved code density (25-30% size reduction)
    - Includes compressed arithmetic, loads/stores, branches, and jumps
    - Seamlessly mixed with standard 32-bit instructions
  - **F Extension (26 instructions):**
    - Single-precision (32-bit) IEEE 754 floating-point operations
    - FP arithmetic: FADD.S, FSUB.S, FMUL.S, FDIV.S, FSQRT.S
    - FP fused multiply-add: FMADD.S, FMSUB.S, FNMSUB.S, FNMADD.S
    - FP comparisons: FEQ.S, FLT.S, FLE.S
    - FP conversions: FCVT.W.S, FCVT.WU.S, FCVT.S.W, FCVT.S.WU
    - FP load/store: FLW, FSW
    - FP moves and classification: FMV.X.W, FMV.W.X, FCLASS.S
    - FP sign injection: FSGNJ.S, FSGNJN.S, FSGNJX.S
    - FP min/max: FMIN.S, FMAX.S
    - Separate 32-register FP register file (f0-f31)
    - FCSR control and status register for rounding modes and exception flags
  - **Zicsr Extension (6 instructions):**
    - CSR (Control and Status Register) access instructions
- ✅ **Multi-cycle Non-pipelined Architecture**: FSM-based design with 12 states for efficient resource sharing
- ✅ **Variable-latency Memory Support**: Ready/valid handshaking for realistic memory operations
- ✅ **Verilator-based Verification**: 260+ comprehensive tests using Rust + marlin framework
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
# Run all tests (test programs are automatically built from source)
cargo test

# Run CPU simulator with a test program (built automatically when tests run)
# You can find built ELF files in target/debug/build/sim-tests-*/out/
cargo test --package sim-tests  # This builds the test programs
# Then use the path from the build output
```

## Architecture

The CPU uses a **multi-cycle non-pipelined design** with a 12-state finite state machine (FSM):
- **Multi-cycle**: Instructions take 3-6+ base clock cycles (plus memory latency) instead of completing in a single cycle
- **Non-pipelined**: One instruction executes at a time through the state machine
- **Variable-latency memory**: Ready/valid handshaking supports realistic memory delays
- **Resource sharing**: ALU and other resources are reused across different instruction phases
- **Atomic operations**: Dedicated S_ATOMIC_RMW state for atomic read-modify-write sequences

This design enables higher clock frequencies and more realistic hardware implementation compared to single-cycle architectures. The shorter critical path (one operation per cycle instead of an entire instruction) improves timing closure for FPGA synthesis and reduces the maximum clock period.

## Project Structure

- **`rtl/`** - SystemVerilog RTL implementation (ALU, register file, decoder, CSR file, top module)
- **`testbench/`** - Rust-based verification tests (testbench package, integration tests)
- **`cpu-sim/`** - Command-line CPU simulator for running ELF executables with VCD waveform dumping
- **`riscv_core/`** - Shared Verilator bindings and utilities
- **`riscv_protocol/`** - Debug packet protocol definitions
- **`riscv_macros/`** - Formatted print macros for bare-metal RISC-V programs
- **`sim-tests/`** - Helper crate that builds test programs from `rust-test-program/` automatically
- **`rust-test-program/`** - Bare-metal Rust test programs (separate workspace, automatically built when tests run)

## Documentation

- **[AGENTS.md](AGENTS.md)** - Comprehensive guide for developers and AI agents (includes FSM details and instruction cycle counts)
- **[cpu-sim/README.md](cpu-sim/README.md)** - CPU simulator usage, VCD waveform dumping, and debugging features
- **[rust-test-program/README.md](rust-test-program/README.md)** - Information about test programs (automatically built when tests run)
- **[riscv_macros/README.md](riscv_macros/README.md)** - Formatted print macros for bare-metal programs
- **[docs/multi-cycle-implementation/](docs/multi-cycle-implementation/)** - Historical: Multi-cycle architecture implementation plan (completed)

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.