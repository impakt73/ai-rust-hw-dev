# Rust Bare Metal Test Program

This is a bare metal RISC-V test program written in Rust for the `riscv32imafc-unknown-none-elf` target.

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
cargo build --release --target riscv32imafc-unknown-none-elf --bin rust_test
```

The resulting ELF file will be at:
```
target/riscv32imafc-unknown-none-elf/release/rust_test
```

After building, copy it to the test_programs directory:
```bash
cp target/riscv32imafc-unknown-none-elf/release/rust_test ../test_programs/rust_test.elf
```

## Testing

The cpu-sim crate includes a test that runs the prebuilt ELF file from `test_programs/rust_test.elf`:

```bash
cd ..
cargo test --package cpu-sim test_rust_bare_metal_elf
```

## Implementation Details

- **Target**: `riscv32imafc-unknown-none-elf` (32-bit RISC-V with integer, multiply/divide, atomics, compressed, and floating-point instruction extensions)
- **Runtime**: Uses `riscv_rt` crate which provides proper stack pointer initialization and startup code
- **Linker Scripts**: Uses `memory.x` (memory layout) and `link.x` (from riscv_rt) for linking
- **Exit Mechanism**: Writes to tohost address (0xFFFFFFF0) with value 42 on success, or 0xDEAD on panic
- **Test Coverage**: Arithmetic, logical, shifts, comparisons, memory I/O, loops, arrays, and function calls

## Configuration

- `.cargo/config.toml`: Specifies the target (`riscv32imafc-unknown-none-elf`)
- `memory.x`: Memory layout defining RAM location and regions for riscv_rt
- `build.rs`: Configures linker to use both `memory.x` and `link.x` (from riscv_rt)
- `Cargo.toml`: Configures the binaries with `test = false` and includes `riscv-rt = "0.17.0"` dependency

## Notes

### Panic Handler

The panic handler in `common.rs` writes a special value (0xDEAD) to tohost when a panic occurs. This allows the simulator to detect panics and report them properly instead of timing out in an infinite loop. Programs that complete successfully write 0x2A (42) to tohost.

### Target Architecture

This crate uses the `riscv32imafc-unknown-none-elf` target with the full RV32IMACF instruction set. The CPU implementation fully supports:
- **RV32I**: Base integer instruction set
- **M Extension**: Integer multiplication and division
- **A Extension**: Atomic instructions
- **C Extension**: Compressed 16-bit instructions for code density
- **F Extension**: Single-precision floating-point operations

All test programs are built with the complete instruction set to ensure thorough testing of all CPU features.

### Workspace Isolation

This crate is **not** part of the main workspace to avoid build complexity. The prebuilt ELF binaries are included in `test_programs/` and used by the cpu-sim tests.
