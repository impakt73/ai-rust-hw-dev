# CPU Simulator (`cpu-sim`)

A command-line RISC-V RV32I CPU simulator that runs ELF executables on the Verilated hardware model.

## Features

- Loads RISC-V ELF executables
- Simulates the single-cycle RV32I CPU with external memory
- Supports the "tohost" mechanism for program termination (write to 0xFFFFFFF0)
- **Instruction trace callback** for programmatic access to executed instructions
- Configurable maximum cycle limit
- Verbose logging for debugging

## Usage

```bash
# Basic usage
cargo run --package cpu-sim -- <path-to-elf-file>

# With verbose logging
cargo run --package cpu-sim -- <path-to-elf-file> --verbose

# Custom cycle limit
cargo run --package cpu-sim -- <path-to-elf-file> --max-cycles 50000

# Or build and run directly
cargo build --package cpu-sim
./target/debug/cpu-sim program.elf
```

## Options

- `<ELF>`: Path to the RISC-V ELF executable (required, positional)
- `--max-cycles <N>`: Maximum number of cycles to simulate (default: 10000)
- `--verbose`: Enable verbose debug logging
- `--print-inst-trace`: Print each instruction as it executes (cycle-by-cycle trace)
- `--help`: Display help information

## Instruction Trace Callback

The simulator provides a programmatic interface for receiving instruction trace information via a callback. This is useful for automated testing, analysis, and debugging tools.

### Using the Trace Callback

```rust
use cpu_sim::{run_elf_with_trace_callback, InstructionTrace};
use riscv_core::trace::InstructionType;
use std::path::Path;

// Define a callback to process each instruction
let mut instruction_count = 0;
let trace_callback = |trace: &InstructionTrace| {
    instruction_count += 1;
    
    // Access structured trace information
    match trace.inst_type {
        InstructionType::Add => {
            println!("Found ADD instruction at PC: 0x{:08x}", trace.pc);
            if let Some(rd) = trace.rd {
                println!("  Result: {:?}", rd);
            }
        },
        InstructionType::Addi => {
            println!("Found ADDI instruction at PC: 0x{:08x}", trace.pc);
            if let Some(imm) = trace.immediate {
                println!("  Immediate value: {}", imm);
            }
        },
        _ => {}
    }
};

// Run simulation with trace callback
let result = run_elf_with_trace_callback(
    Path::new("program.elf"),
    10000,
    false,  // print_inst_trace (false to use only callback)
    Some(trace_callback)
)?;

println!("Executed {} instructions", instruction_count);
```

### InstructionTrace Structure

The `InstructionTrace` struct provides detailed information about each executed instruction:

- `pc`: Program counter (address of the instruction)
- `instruction`: Raw 32-bit instruction word
- `inst_type`: Parsed instruction type (enum: `Add`, `Addi`, `Lw`, `Sw`, etc.)
- `rd`: Destination register and its value (if applicable)
- `rs1`: Source register 1 and its value (if applicable)
- `rs2`: Source register 2 and its value (if applicable)
- `immediate`: Immediate value (if applicable)

### Available API Functions

- `run_elf(path, max_cycles, print_trace)` - Basic simulation (backward compatible)
- `run_elf_with_trace_callback(path, max_cycles, print_trace, trace_callback)` - With instruction trace callback
- `run_elf_with_callback(path, max_cycles, print_trace, fifo_callback)` - With FIFO callback
- `run_elf_with_all_callbacks(...)` - With both FIFO and trace callbacks

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
RUST_LOG=debug ./target/debug/cpu-sim program.elf
```

## Limitations

- Only supports RV32I base instruction set (as implemented in the RTL)
- No system calls or I/O beyond the tohost mechanism
- Memory is initialized only from the ELF LOAD segments
- Single-cycle execution model (no pipelining)
