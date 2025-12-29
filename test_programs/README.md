# Test Programs

This directory contains simple RISC-V test programs for the CPU simulator.

## test.elf

A simple assembly test program that demonstrates basic RISC-V instructions:
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

## rust_test.elf

A bare metal Rust test program that uses regular Rust code (not inline assembly) to test the CPU. This demonstrates that normally compiled Rust code can run on the simulated CPU.

The Rust source code is in the `rust-test-program` directory (not part of the main workspace).

### Features

- Arithmetic, logical, and shift operations
- Comparisons and conditional logic
- Memory reads and writes using `volatile` operations
- Loops and control flow
- Arrays and function calls

### Running

```bash
cargo test --package cpu-sim test_rust_bare_metal_elf
```

### Rebuilding

The prebuilt ELF is included in this directory. To rebuild from source:

```bash
cd rust-test-program
cargo build --release --target riscv32i-unknown-none-elf --bin rust_test
cp ../target/riscv32i-unknown-none-elf/release/rust_test ../test_programs/rust_test.elf
```

See `rust-test-program/README.md` for more details.
