# Programmatic Memory Loading Examples

This document demonstrates how to use the CPU simulator with programmatically generated instructions instead of ELF files.

## Basic Example: Writing Instructions Directly to Memory

```rust
use cpu_sim::*;

// Create empty DRAM and system bus
let dram = dram::Dram::new();
let bus = bus::SystemBus::new(dram);

// Initialize CPU Simulator without an ELF file
let runtime = riscv_core::create_cpu_runtime()?;
let mut sim = sim::Simulator::new(
    &runtime,
    bus,
    false,                          // print_inst_trace
    None::<fn(u32)>,               // fifo_callback
    None::<fn(&riscv_core::trace::InstructionTrace)>, // trace_callback
)?;

// Define a simple program (in little-endian byte format):
// addi x10, x0, 42  -> 0x02a00513
// sw x10, -16(x0)   -> 0xfea02823 (write to tohost to halt)
let program: Vec<u8> = vec![
    0x13, 0x05, 0xa0, 0x02,  // addi x10, x0, 42
    0x23, 0x28, 0xa0, 0xfe,  // sw x10, -16(x0)
];

// Write program to memory at RISC-V start address
const START_ADDR: u32 = 0x8000_0000;
sim.write_memory_region(START_ADDR, &program);

// Run the simulation with the start address as boot PC
let result = sim.run(START_ADDR, 100)?;

println!("Simulation completed in {} cycles", result.cycles);
println!("Tohost value: {:?}", result.tohost_value);
// Output: Simulation completed in 2 cycles
//         Tohost value: Some(42)
```

## Advanced Example: Using the Instruction Encoder

For more complex programs, you can use the `riscv_core` instruction encoder:

```rust
use riscv_core::instruction::*;

// Create instructions using the encoder
let instructions = vec![
    // addi x10, x0, 42
    encode_i_type(0b0010011, 10, 0b000, 0, 42),
    
    // addi x11, x0, 100
    encode_i_type(0b0010011, 11, 0b000, 0, 100),
    
    // add x12, x10, x11  (x12 = 42 + 100 = 142)
    encode_r_type(0b0110011, 12, 0b000, 10, 11, 0b0000000),
    
    // sw x12, -16(x0)  (write to tohost)
    encode_s_type(0b0100011, 0b010, 0, 12, -16),
];

// Convert to byte array (little-endian)
let mut program = Vec::new();
for inst in instructions {
    program.extend_from_slice(&inst.to_le_bytes());
}

// Write and run as before
sim.write_memory_region(START_ADDR, &program);
let result = sim.run(START_ADDR, 100)?;

assert_eq!(result.tohost_value, Some(142));
```

## Loading ELF Files (Traditional Approach)

If you have an ELF file, you can still use the traditional approach:

```rust
use cpu_sim::*;
use std::path::Path;

// Method 1: Using the high-level run_elf function
let result = run_elf(Path::new("program.elf"), 1000, false)?;

// Method 2: Manual control with load_elf
let dram = dram::Dram::new();
let bus = bus::SystemBus::new(dram);
let runtime = riscv_core::create_cpu_runtime()?;

let mut sim = sim::Simulator::new(
    &runtime,
    bus,
    false,
    None::<fn(u32)>,
    None::<fn(&riscv_core::trace::InstructionTrace)>,
)?;

// Load ELF and get entry point
let entry_point = load_elf(&mut sim, Path::new("program.elf"))?;

// Run with the ELF's entry point
let result = sim.run(entry_point, 1000)?;
```

## Mixing Programmatic Code with ELF Loading

You can also load an ELF file and then modify memory:

```rust
// Load base program from ELF
let entry_point = load_elf(&mut sim, Path::new("program.elf"))?;

// Patch a specific instruction at runtime
let nop_instruction: Vec<u8> = vec![0x13, 0x00, 0x00, 0x00]; // addi x0, x0, 0 (NOP)
sim.write_memory_region(0x8000_0100, &nop_instruction);

// Or add a test payload at a specific address
let test_data = vec![0xAA, 0xBB, 0xCC, 0xDD];
sim.write_memory_region(0x8000_2000, &test_data);

// Run normally
let result = sim.run(entry_point, 1000)?;
```

## Benefits of the New API

1. **Testing**: Easily create minimal test cases without compiling ELF files
2. **Debugging**: Patch specific instructions during debugging
3. **Fuzzing**: Generate random instruction sequences for testing
4. **Education**: Demonstrate CPU behavior with simple instruction sequences
5. **Flexibility**: Decouple simulator initialization from program loading

## API Summary

### Core Functions

- `Simulator::new()` - Create simulator (no entry point required)
- `Simulator::write_memory_region(addr, data)` - Write bytes to memory
- `Simulator::dump_memory_region(addr, size)` - Read bytes from memory
- `Simulator::reset(boot_pc)` - Reset CPU with specific boot PC
- `Simulator::run(boot_pc, max_cycles)` - Run simulation from boot PC
- `load_elf(sim, path)` - Load ELF into simulator, returns entry point

### Migration Guide

**Old API:**
```rust
let mut dram = Dram::new();
let entry_point = dram.load_elf(path)?;
let bus = SystemBus::new(dram);
let mut sim = Simulator::new(&runtime, bus, entry_point, ...)?;
sim.reset();
let result = sim.run(max_cycles)?;
```

**New API:**
```rust
let dram = Dram::new();
let bus = SystemBus::new(dram);
let mut sim = Simulator::new(&runtime, bus, ...)?;
let entry_point = load_elf(&mut sim, path)?;
let result = sim.run(entry_point, max_cycles)?;
```

The new API provides the same functionality while being more flexible and decoupled from ELF files.
