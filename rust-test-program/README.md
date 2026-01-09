# Rust Bare Metal Test Program

This is a bare metal RISC-V test program written in Rust for the `riscv32imc-unknown-none-elf` target.

## Purpose

This program tests the CPU simulator with normally compiled Rust code (not inline assembly). It demonstrates that the CPU can run regular bare metal Rust code compiled for RISC-V.

## Features

- **No Standard Library**: Uses `#![no_std]` for bare metal execution
- **riscv_rt Runtime**: Uses the `riscv_rt` crate for proper runtime initialization with the `#[entry]` macro
- **Regular Rust Code**: Uses standard Rust operators and control flow
- **Comprehensive Tests**: Tests arithmetic, logical, shift, comparison, memory operations, loops, arrays, and function calls

## Building

To rebuild the program:

```bash
cd rust-test-program
cargo build --release --target riscv32imc-unknown-none-elf --bin rust_test
```

The resulting ELF file will be at:
```
target/riscv32imc-unknown-none-elf/release/rust_test
```

After building, copy it to the test_programs directory:
```bash
cp target/riscv32imc-unknown-none-elf/release/rust_test ../test_programs/rust_test.elf
```

## Testing

The cpu-sim crate includes a test that runs the prebuilt ELF file from `test_programs/rust_test.elf`:

```bash
cd ..
cargo test --package cpu-sim test_rust_bare_metal_elf
```

## Implementation Details

- **Target**: `riscv32imc-unknown-none-elf` (32-bit RISC-V with integer, multiply/divide, and compressed instructions)
- **Runtime**: Uses `riscv_rt` crate which provides proper stack pointer initialization and startup code
- **Linker Scripts**: Uses `memory.x` (memory layout) and `link.x` (from riscv_rt) for linking
- **Exit Mechanism**: Writes to tohost address (0xFFFFFFF0) with value 42 on success
- **Test Coverage**: Arithmetic, logical, shifts, comparisons, memory I/O, loops, arrays, and function calls

## Configuration

- `.cargo/config.toml`: Specifies the target (`riscv32imc-unknown-none-elf`)
- `memory.x`: Memory layout defining RAM location and regions for riscv_rt
- `build.rs`: Configures linker to use both `memory.x` and `link.x` (from riscv_rt)
- `Cargo.toml`: Configures the binaries with `test = false` and includes `riscv-rt = "0.17.0"` dependency

## Note on RV32IMC Target

The target is configured as `riscv32imc-unknown-none-elf` which includes support for compressed instructions (C extension). However, the current prebuilt ELF binaries in `test_programs/` were built with `riscv32im-unknown-none-elf` to maintain compatibility with existing tests.

When rebuilding programs with the RV32IMC target, the Rust compiler may generate compressed instructions in the standard library and runtime code. The CPU implementation fully supports compressed instructions (all 27 RV32C instructions are implemented and tested), but some test programs may encounter issues with specific runtime library interactions.

To rebuild without compressed instructions while keeping the RV32IMC target:
```bash
CARGO_BUILD_RUSTFLAGS="-C target-feature=-c" cargo build --release
```

## Note

This crate is **not** part of the main workspace to avoid build complexity. The prebuilt ELF binaries are included in `test_programs/` and used by the cpu-sim tests.
