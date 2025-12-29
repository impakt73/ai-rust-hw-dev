# Test Programs

This directory contains simple RISC-V test programs for the CPU simulator.

## test.elf

A simple test program that demonstrates basic RISC-V instructions:
- Immediate loads (addi)
- Arithmetic operations (add, sub)
- Logic operations (and, or, xor)
- Load upper immediate (lui)
- Store word (sw) to halt address

The program writes to the tohost address (0xFFFFFFF0) to signal completion.

### Building

To rebuild the test program:

```bash
cd test_programs
riscv64-unknown-elf-as -march=rv32i -mabi=ilp32 -o test.o test.s
riscv64-unknown-elf-ld -T linker.ld -m elf32lriscv -o test.elf test.o
```

### Running

```bash
cargo run --bin cpu-sim -- test_programs/test.elf --debug
```

The `--debug` flag will show cycle-by-cycle execution with disassembled instructions and register values.

## Rust Bare Metal Test Program

The repository also includes a bare metal Rust test program in the `rust-test-program` directory. This program implements similar test logic to test.s but is written in Rust using inline assembly. See `rust-test-program/README.md` for more details.

The Rust test program is automatically tested by the cpu-sim test suite:

```bash
cargo test --package cpu-sim test_rust_bare_metal_elf
```
