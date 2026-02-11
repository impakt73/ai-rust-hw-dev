# CPU Simulator (`cpu-sim`)

A command-line RISC-V RV32IM CPU simulator that runs ELF executables on the Verilated hardware model.

## Features

- Loads RISC-V ELF executables
- Simulates the multi-cycle non-pipelined RV32IM CPU (RV32I + M extension + Zicsr) with external memory
- Supports the "tohost" mechanism for program termination (write to 0xFFFFFFF0)
- **VCD waveform dumping** for signal-level debugging and analysis
- **Instruction trace callback** for programmatic access to executed instructions
- **FIFO-based debug packet protocol** for communication with bare-metal programs
- **Configurable memory latency** to test multi-cycle memory operations
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
use cpu_sim::run_elf;
use std::path::Path;

let result = run_elf(
    Path::new("program.elf"),
    10000,          // max_cycles
    false,          // print_inst_trace
    false,          // print_fsm_state
    None,           // inst_complete_callback
    None,           // trace_callback
    Some("trace.vcd"), // vcd_path
    0,              // mem_latency_cycles
    None,           // prep_callback
    |_sim, _result| {} // post_callback
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
use cpu_sim::{run_elf, InstructionTrace};
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
let result = run_elf(
    Path::new("program.elf"),
    10000,
    false,  // print_inst_trace (false to use only callback)
    false,  // print_fsm_state
    None,   // inst_complete_callback
    Some(trace_callback),
    None,   // vcd_path
    0,      // mem_latency_cycles
    None,   // prep_callback
    |_sim, _result| {} // post_callback
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

The library provides two main functions for different use cases:

- `run_elf(elf_path, max_cycles, print_inst_trace, print_fsm_state, inst_complete_callback, trace_callback, vcd_path, mem_latency_cycles, prep_callback, post_callback)` - Run an ELF file with full configuration and optional pre-execution setup
- `run_program(max_cycles, print_inst_trace, print_fsm_state, inst_complete_callback, trace_callback, vcd_path, mem_latency_cycles, prep_callback, post_callback)` - Run a program with custom loading logic

### Unified Execution API

The `run_program` function provides a unified interface for all simulator execution with custom program loading:

```rust
pub fn run_program<F, T, P, C>(
    max_cycles: u64,
    print_inst_trace: bool,
    print_fsm_state: bool,
    inst_complete_callback: Option<F>,
    trace_callback: Option<T>,
    vcd_path: Option<&str>,
    mem_latency_cycles: u32,
    prep_callback: P,      // Load program, return entry point
    post_callback: C,      // Access simulator after execution
) -> Result<SimulationResult, String>

// For ELF files, use run_elf which loads the ELF and provides a prep_callback for additional setup
pub fn run_elf<F, T, P, C>(
    elf_path: &Path,
    max_cycles: u64,
    print_inst_trace: bool,
    print_fsm_state: bool,
    inst_complete_callback: Option<F>,
    trace_callback: Option<T>,
    vcd_path: Option<&str>,
    mem_latency_cycles: u32,
    prep_callback: Option<P>,  // Optional additional setup after ELF is loaded
    post_callback: C,
) -> Result<SimulationResult, String>
```

Here, `run_program` is the underlying execution engine: it directly supports programmatic instruction loading via `prep_callback`, and `run_elf` is a convenience wrapper that first loads an ELF file and then invokes `run_program`. All other API functions delegate to `run_program` for consistency.


## Programmatic Testing with cpu-sim

For testing RTL implementations with programmatically generated instruction sequences, the `test_rtl_verification` module provides helper functions that simplify trace collection and VCD generation.

### Basic Programmatic Test

```rust
use riscv_core::instruction::*;

// Generate test instructions
let instructions = [
    addi(1, 0, 10),   // x1 = 10
    addi(2, 0, 20),   // x2 = 20
    add(3, 1, 2),     // x3 = x1 + x2 = 30
];

// Run and verify
run_program_with_options(&instructions, 100, false, None, None::<fn(&riscv_core::trace::InstructionTrace)>, |sim, result| {
    assert_eq!(result.tohost_value, Some(1));
}).expect("Test should pass");
```

### Enabling Trace and VCD in Tests

Use `run_program_with_options` to enable instruction tracing and VCD dumping:

```rust
// Enable instruction trace printing
run_program_with_options(&instructions, 100, true, None, None::<fn(&riscv_core::trace::InstructionTrace)>, |sim, result| {
    // Verify results
}).expect("Test should pass");

// Enable VCD waveform dumping
run_program_with_options(&instructions, 100, false, Some("/tmp/test.vcd"), None::<fn(&riscv_core::trace::InstructionTrace)>, |sim, result| {
    // Verify results
}).expect("Test should pass");

// Enable both trace and VCD
run_program_with_options(&instructions, 100, true, Some("/tmp/test.vcd"), None::<fn(&riscv_core::trace::InstructionTrace)>, |sim, result| {
    // Verify results
}).expect("Test should pass");
```

### Programmatic Trace Validation

Collect and validate instruction traces programmatically:

```rust
let mut traces = Vec::new();

run_program_with_options(
    &instructions,
    100,
    false,
    None,
    Some(|trace: &riscv_core::trace::InstructionTrace| {
        traces.push(trace.clone());
    }),
    |sim, result| {
        // Verify we got expected number of traces
        assert_eq!(traces.len(), 12);
        
        // Validate first instruction
        assert_eq!(traces[0].inst_type, InstructionType::Addi);
        assert_eq!(traces[0].pc, 0x8000_0000);
        assert_eq!(traces[0].rd.unwrap().value, 10);
    }
).expect("Test should pass");
```

### Comprehensive Validation Example

See `test_comprehensive_trace_validation` in `test_rtl_verification.rs` for a complete example that validates:
- PC values match expected sequence
- Instruction types decode correctly
- Register values are computed correctly
- Immediate values are extracted properly
- Control flow (branches) executes correctly

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

### Variable Memory Latency

The simulator supports configurable memory latency to test the CPU's ability to handle multi-cycle memory operations. By default, memory operations complete in zero cycles (immediately), but you can configure a fixed latency to verify that the CPU correctly waits for the `imem_ready` and `dmem_ready` signals.

#### Using Variable Latency

```rust
use cpu_sim::Simulator;
use device_runtime::SystemBus;

let runtime = riscv_core::create_cpu_runtime()?;
let bus = SystemBus::new();

// Configure memory latency at initialization (3 cycle latency)
let mut sim = Simulator::new(
    &runtime,
    bus,
    false,  // print_inst_trace
    false,  // print_fsm_state
    None::<fn(u32)>,
    None::<fn(&riscv_core::trace::InstructionTrace)>,
    3,      // mem_latency_cycles
)?;

// Load and run your program
let entry_point = cpu_sim::load_elf(&mut sim, Path::new("program.elf"))?;
let result = sim.run(entry_point, 10000)?;
```

#### How It Works

- **Zero Latency (default)**: Pass `0` for `mem_latency_cycles` - memory operations complete immediately (backward compatible)
- **Fixed Latency (configurable)**: Pass N for `mem_latency_cycles` - each memory request (instruction fetch or data access) takes exactly N cycles to complete
- **Counter-based Implementation**: The simulator uses internal delay counters that increment each cycle until the configured latency is reached, then asserts the `ready` signal

This feature helps verify that:
- The CPU's FSM correctly waits for memory ready signals
- Multi-cycle memory operations don't break instruction execution
- The CPU can handle realistic memory latencies without timing issues

See `cpu-sim/src/test_memory_latency.rs` for comprehensive examples and test cases.

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

- Supports RV32IM instruction set (RV32I base + M extension + Zicsr) as implemented in the RTL
- No system calls or I/O beyond the tohost mechanism and FIFO debug protocol
- Memory is initialized only from the ELF LOAD segments
- Multi-cycle non-pipelined execution model (no pipelining)

## Testing Constants

### GLOBAL_MAX_CYCLES

The `GLOBAL_MAX_CYCLES` constant defines the maximum number of cycles any test should run before being considered a runaway or hung simulation. This constant serves as a safety backstop to prevent infinite loops in tests.

**Current value:** 40,000 cycles

**Rationale:**
- Based on empirical measurement of all cpu-sim tests
- Maximum observed cycles across all tests: 17,296 (test_println_macro)
- Provides 2.3× safety margin above the maximum observed value
- Acts as a global safety net while the per-instruction hung detector (10,000 cycles/instruction) remains the primary detection mechanism
- Should never be reached by any legitimate test in normal operation

**Usage in tests:**
```rust
use cpu_sim::*;

let result = run_program(
    GLOBAL_MAX_CYCLES,  // Use the global constant
    false,
    false,
    // ...
)?;
```

**Special cases:**
Tests that intentionally test hung detection or long instruction scenarios may use higher limits with documented justification. See `test_audio_pattern` and `test_video_pattern` for examples.
