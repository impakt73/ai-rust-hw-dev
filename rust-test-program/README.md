# Rust Bare Metal Test Program

This is a bare metal RISC-V test program written in Rust for the `riscv32i-unknown-none-elf` target.

## Purpose

This program tests the CPU simulator with a Rust-based test program that is equivalent to `test_programs/test.s`. It demonstrates that the CPU can run bare metal Rust code compiled for RISC-V.

## Features

- **No Standard Library**: Uses `#![no_std]` for bare metal execution
- **Custom Entry Point**: Defines `_start()` as the entry point
- **Inline Assembly**: Uses Rust's inline assembly to execute RISC-V instructions
- **Comprehensive Tests**: Tests arithmetic, logical, shift, comparison, memory operations, loops, and upper immediate instructions

## Building

The program is automatically built as part of the workspace:

```bash
cd rust-test-program
cargo build --release --bin rust_test
```

The resulting ELF file will be at:
```
../target/riscv32i-unknown-none-elf/release/rust_test
```

## Testing

The cpu-sim crate includes a test that runs this ELF file:

```bash
cd ..
cargo test --package cpu-sim test_rust_bare_metal_elf
```

## Implementation Details

- **Target**: `riscv32i-unknown-none-elf` (32-bit RISC-V with integer instructions only)
- **Linker Script**: `linker.ld` - Places code at 0x80000000
- **Exit Mechanism**: Writes to tohost address (0xFFFFFFF0) with value 42 on success
- **Test Coverage**: Mirrors the test.s assembly program with Rust inline assembly

## Configuration

- `.cargo/config.toml`: Specifies the target and linker script
- `linker.ld`: Memory layout matching the assembly test program
- `Cargo.toml`: Configures the binary with `test = false` to prevent standard library test compilation
