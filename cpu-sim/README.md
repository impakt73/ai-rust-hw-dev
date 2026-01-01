# CPU Simulator (`cpu-sim`)

A command-line RISC-V RV32I CPU simulator that runs ELF executables on the Verilated hardware model.

## Features

- Loads RISC-V ELF executables
- Simulates the single-cycle RV32I CPU with external memory
- Supports the "tohost" mechanism for program termination (write to 0xFFFFFFF0)
- **VCD waveform dumping** for signal-level debugging and analysis
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
- `--vcd <PATH>`: Enable VCD waveform dumping to the specified file path
- `--help`: Display help information

## VCD Waveform Dumping

The simulator can generate VCD (Value Change Dump) files for detailed signal-level analysis and debugging. VCD files can be viewed in waveform viewers like GTKWave or similar tools.

### Usage

```bash
# Generate VCD waveform dump
cargo run --package cpu-sim -- program.elf --vcd trace.vcd

# With other options
cargo run --package cpu-sim -- program.elf --vcd trace.vcd --max-cycles 50000 --verbose
```

### Viewing VCD Files

After generating a VCD file, you can view it with GTKWave or other waveform viewers:

```bash
# Install GTKWave (Ubuntu/Debian)
sudo apt-get install gtkwave

# Open the waveform
gtkwave trace.vcd
```

### Programmatic API

You can also enable VCD dumping programmatically:

```rust
use cpu_sim::run_elf_with_vcd;
use std::path::Path;

let result = run_elf_with_vcd(
    Path::new("program.elf"),
    10000,          // max_cycles
    false,          // print_inst_trace
    "trace.vcd"     // vcd_path
)?;

println!("VCD waveform saved to trace.vcd");
```

The VCD file captures all CPU signals including:
- Clock (`clk`) and reset (`rst_n`)
- Program counter (`imem_addr`)
- Instruction data (`imem_data`)
- Data memory interface (`dmem_addr`, `dmem_wdata`, `dmem_rdata`, `dmem_we`, `dmem_re`)
- Debug signals (`debug_rs1_data`, `debug_rs2_data`, `debug_rd_data`)
- Internal CPU state (register file, ALU operations, etc.)

## Instruction Trace Callback

The simulator provides a programmatic interface for receiving instruction trace information via a callback. This is useful for automated testing, analysis, and debugging tools.

### Using the Trace Callback

```rust
use cpu_sim::{run_elf_with_trace_callback, InstructionTrace};
use riscv_core::trace::InstructionType;
use std::{cell::Cell, path::Path};

// Define a callback to process each instruction
let instruction_count = Cell::new(0);
let trace_callback = |trace: &InstructionTrace| {
    instruction_count.set(instruction_count.get() + 1);
    
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

println!("Executed {} instructions", instruction_count.get());
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
- `run_elf_with_vcd(path, max_cycles, print_trace, vcd_path)` - With VCD waveform dumping
- `run_elf_with_trace_callback(path, max_cycles, print_trace, trace_callback)` - With instruction trace callback
- `run_elf_with_callback(path, max_cycles, print_trace, fifo_callback)` - With FIFO callback
- `run_elf_with_all_callbacks(...)` - With FIFO, trace callbacks, and optional VCD

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
