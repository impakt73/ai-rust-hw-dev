# Rust Bare Metal Test Program

This is a bare metal RISC-V test program written in Rust for the `riscv32i-unknown-none-elf` target.

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
cargo build --release --target riscv32i-unknown-none-elf --bin rust_test
```

The resulting ELF file will be at:
```
target/riscv32i-unknown-none-elf/release/rust_test
```

After building, copy it to the test_programs directory:
```bash
cp target/riscv32i-unknown-none-elf/release/rust_test ../test_programs/rust_test.elf
```

## Testing

The cpu-sim crate includes a test that runs the prebuilt ELF file from `test_programs/rust_test.elf`:

```bash
cd ..
cargo test --package cpu-sim test_rust_bare_metal_elf
```

## Implementation Details

- **Target**: `riscv32i-unknown-none-elf` (32-bit RISC-V with integer instructions only)
- **Runtime**: Uses `riscv_rt` crate which provides proper stack pointer initialization and startup code
- **Linker Scripts**: Uses `memory.x` (memory layout) and `link.x` (from riscv_rt) for linking
- **Exit Mechanism**: Writes to tohost address (0xFFFFFFF0) with value 42 on success
- **Test Coverage**: Arithmetic, logical, shifts, comparisons, memory I/O, loops, arrays, and function calls

## Configuration

- `.cargo/config.toml`: Specifies the target (`riscv32i-unknown-none-elf`)
- `memory.x`: Memory layout defining RAM location and regions for riscv_rt
- `build.rs`: Configures linker to use both `memory.x` and `link.x` (from riscv_rt)
- `Cargo.toml`: Configures the binaries with `test = false` and includes `riscv-rt = "0.17.0"` dependency

## Note

This crate is **not** part of the main workspace to avoid build complexity. The prebuilt ELF binary is included in `test_programs/rust_test.elf` and used by the cpu-sim tests.
