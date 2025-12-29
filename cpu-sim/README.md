# CPU Simulator (`cpu-sim`)

A command-line RISC-V RV32I CPU simulator that runs ELF executables on the Verilated hardware model.

## Features

- Loads RISC-V ELF executables
- Simulates the single-cycle RV32I CPU with external memory
- Supports the "tohost" mechanism for program termination (write to 0xFFFFFFF0)
- Configurable maximum cycle limit
- Verbose logging for debugging

## Usage

```bash
# Basic usage
cargo run --package cpu-sim -- --elf <path-to-elf-file>

# With verbose logging
cargo run --package cpu-sim -- --elf <path-to-elf-file> --verbose

# Custom cycle limit
cargo run --package cpu-sim -- --elf <path-to-elf-file> --max-cycles 50000

# Or build and run directly
cargo build --package cpu-sim
./target/debug/cpu-sim --elf program.elf
```

## Options

- `--elf <PATH>`: Path to the RISC-V ELF executable (required)
- `--max-cycles <N>`: Maximum number of cycles to simulate (default: 10000)
- `--verbose`: Enable verbose debug logging
- `--help`: Display help information

## Program Termination

Programs can signal completion by writing to the special "tohost" address `0xFFFFFFF0`. The simulator will detect this write and terminate successfully.

Example assembly:
```asm
# Store result to tohost
addi x1, x0, -16    # x1 = 0xFFFFFFF0
sw x2, 0(x1)        # Write x2 to tohost (triggers halt)
```

## Memory Model

The simulator uses a sparse byte-addressable memory model (HashMap-based). 

**Important:** The CPU's PC resets to address `0x00000000`. The ELF entry point is currently **ignored** - execution always starts at address 0. Programs should be linked to start at address 0, or include a trampoline at address 0 that jumps to the actual entry point.

## Architecture

The simulator connects to the Verilated RTL model from `riscv_core`:
- **Instruction Memory**: Provided by the simulator's memory model via `imem_data` port
- **Data Memory**: Bidirectional access via `dmem_addr`, `dmem_we`, `dmem_wdata`, `dmem_rdata` ports
- **Control**: Reset via `rst_n`, clock via `clk`

## Logging

The simulator uses the `env_logger` crate. Log levels:
- `INFO`: Basic simulation progress
- `DEBUG`: Cycle-by-cycle execution trace and memory accesses

Set the `RUST_LOG` environment variable for fine-grained control:
```bash
RUST_LOG=debug ./target/debug/cpu-sim --elf program.elf
```

## Limitations

- Only supports RV32I base instruction set (as implemented in the RTL)
- No system calls or I/O beyond the tohost mechanism
- Memory is initialized only from the ELF LOAD segments
- Single-cycle execution model (no pipelining)
