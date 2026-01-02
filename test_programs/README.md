# Test Programs

This directory contains RISC-V test programs for the CPU simulator and verification infrastructure.

## Core Test Programs

### test.elf

A simple assembly test program that demonstrates basic RISC-V instructions:
- Immediate loads (addi)
- Arithmetic operations (add, sub)
- Logic operations (and, or, xor)
- Load upper immediate (lui)
- Store word (sw) to halt address

The program writes to the tohost address (0xFFFFFFF0) to signal completion.

#### Building

To rebuild the test program:

```bash
cd test_programs
riscv64-unknown-elf-as -march=rv32i -mabi=ilp32 -o test.o test.s
riscv64-unknown-elf-ld -T linker.ld -m elf32lriscv -o test.elf test.o
```

#### Running

```bash
cargo run --package cpu-sim -- test_programs/test.elf --print-inst-trace
```

The `--print-inst-trace` flag will show cycle-by-cycle execution with disassembled instructions and register values.

### rust_test.elf

A bare metal Rust test program that uses regular Rust code (not inline assembly) to test the CPU. This demonstrates that normally compiled Rust code can run on the simulated CPU.

The Rust source code is in the `rust-test-program` directory (not part of the main workspace).

**Features:**
- Arithmetic, logical, and shift operations
- Comparisons and conditional logic
- Memory reads and writes using `volatile` operations
- Loops and control flow
- Arrays and function calls

**Running:**

```bash
cargo test --package cpu-sim test_rust_bare_metal_elf
```

See `rust-test-program/README.md` for rebuild instructions.

## Debug Infrastructure Test Programs

### hello_world.elf

A bare metal Rust program that demonstrates FIFO operations and writes "Hello World" to a simulated I/O device using the debug packet protocol.

The Rust source code is in `rust-test-program/src/hello_world.rs`.

**Running:**

```bash
cargo test --package cpu-sim test_fifo_hello_world
```

### println_test.elf

Demonstrates the `rvprintln!` macro for formatted printing from bare-metal RISC-V programs.

**Running:**

```bash
cargo test --package cpu-sim test_println_macro
```

### packet_test.elf

Tests the DebugPacket protocol serialization and deserialization via FIFO.

### simple_fifo_test.elf

Basic FIFO write/read test program.

## Memory and Allocator Tests

### test_allocator.elf

Tests heap allocation using the embedded allocator.

### test_heap_directly.elf

Tests heap memory operations with byte-level stores and loads.

### test_stack_memory.elf

Tests stack memory operations with word-level stores.

### test_alloc_only.elf

Tests basic allocation without deallocation.

### test_byte_store_simple.elf

Simple test for byte store (SB) instruction.

### test_memory_pattern.elf

Tests various memory access patterns.

### test_static_heap.elf

Tests static heap allocation.

## Trace and Debug Tests

### trace_test.elf

Tests instruction trace functionality.

### register_trace_audit.elf

Validates register state tracking through instruction trace.

### minimal_debug_test.elf

Minimal debug packet test.

### minimal_postcard_test.elf, minimal_postcard_test2.elf

Tests postcard serialization library.

## Other Tests

### simple_test.elf

Basic CPU functionality test.

### test_minimal_halt.elf

Minimal program that immediately halts.

### test_image_data.elf

Tests handling of image data structures.

### test_one_println.elf

Single println test.

## Program Termination

Programs signal completion by writing to the special "tohost" address `0xFFFFFFF0`. The simulator detects this write and terminates successfully.

Example assembly:
```asm
# Store result to tohost
addi x1, x0, -16    # x1 = 0xFFFFFFF0
sw x2, 0(x1)        # Write x2 to tohost (triggers halt)
```

## Rebuilding Rust Programs

Most Rust test programs are built from source in the `rust-test-program` directory:

```bash
cd rust-test-program
cargo build --release --target riscv32i-unknown-none-elf --bin <program_name>
cp target/riscv32i-unknown-none-elf/release/<program_name> ../test_programs/<program_name>.elf
```

See `rust-test-program/README.md` for details on the build environment and configuration.
