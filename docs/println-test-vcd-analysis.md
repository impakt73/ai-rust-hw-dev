# VCD Analysis: println_test.elf Simulation

This document contains a detailed analysis of the VCD waveform dump generated from running the `println_test.elf` test program through the RISC-V CPU simulator.

## Overview

The `println_test.elf` program was executed using the `cpu-sim` simulator with VCD waveform dumping enabled. The program demonstrates the RISC-V CPU's ability to execute a simple program that uses the FIFO-based debug packet protocol to print messages to the console.

### Test Program Output

```
[INFO] Hello from RISC-V CPU!

[INFO] The answer is 42

[INFO] Testing println macro
```

## Simulation Configuration

- **Test Program**: `test_programs/println_test.elf`
- **Maximum Cycles**: 100,000 (configured limit)
- **VCD Output**: `/tmp/println_test.vcd`
- **VCD File Size**: 6.5 MB
- **Simulator**: cpu-sim (RISC-V RV32IM CPU with Zicsr)

## Execution Statistics

### Overall Performance

| Metric | Value |
|--------|-------|
| **Total Clock Cycles** | 16,951 |
| **Total Instructions Executed** | 4,236 |
| **Cycles Per Instruction (CPI)** | 4.00 |
| **Instructions Per Cycle (IPC)** | 0.25 |
| **Simulation Time** | 16,951 ps (picoseconds) |
| **CPU Time (host)** | 17.86 ms |

### Memory Access Statistics

| Operation | Count |
|-----------|-------|
| **Memory Reads** | 405 |
| **Memory Writes** | 341 |
| **Total Memory Accesses** | 746 |
| **Memory Access Rate** | ~0.18 accesses per instruction |

### Program Counter (PC) Analysis

| Metric | Value |
|--------|-------|
| **Minimum PC** | 0x00000000 |
| **Maximum PC** | 0x800023d4 |
| **Unique PC Values** | 697 |
| **PC Address Range** | 0x800023d4 bytes (~9 KB) |

The PC range from 0x00000000 to 0x800023d4 indicates that the program:
- Starts at address 0x00000000 (likely a boot/initialization sequence)
- Executes code up to address 0x800023d4
- The 697 unique PC values suggest significant code coverage across the loaded program

## Performance Analysis

### Cycles Per Instruction (CPI)

The CPI of **4.00** indicates that, on average, each instruction takes 4 clock cycles to complete. This is expected for a multi-cycle RISC-V implementation where:

1. **Instruction Fetch** - 1 cycle
2. **Instruction Decode** - 1 cycle
3. **Execute** - 1 cycle
4. **Memory Access** (if needed) - 1 cycle
5. **Write Back** - included in execution cycle

The exact cycle count per instruction may vary based on:
- Instruction type (ALU operations, branches, loads/stores)
- Memory access patterns
- Control flow dependencies

### Instructions Per Cycle (IPC)

The IPC of **0.25** is the inverse of CPI and confirms that the CPU executes approximately one instruction every 4 cycles. This is characteristic of a simple multi-cycle processor without pipelining.

### Memory Access Patterns

With 746 total memory accesses across 4,236 instructions:
- **Memory access frequency**: ~17.6% of instructions involve memory operations
- **Read/Write ratio**: 405 reads to 341 writes (1.19:1 ratio)
- This suggests a relatively balanced mix of loads and stores, which is typical for:
  - Stack operations (function calls/returns)
  - Variable access
  - FIFO communication for debug output

## Program Execution Characteristics

### Code Coverage

The program executed code from 697 unique program counter locations, which represents:
- **Instruction density**: 4,236 instructions / 697 unique PCs = ~6.08 instructions per unique location on average
- This indicates that the program has loops or frequently executed code paths (hot spots)

### Address Range Analysis

The address space from 0x00000000 to 0x800023d4:
- **Low addresses (0x00000000 - 0x00000FFF)**: Likely initialization code and vectors
- **High addresses (0x80000000 - 0x800023d4)**: Main program code and data (standard RISC-V memory map)
- The program spans approximately 9 KB of addressable memory

## VCD Signal Analysis

The VCD file contains the following key signals:

### Control Signals
- `clk` - System clock
- `rst_n` - Active-low reset
- `halted` - CPU halt indicator
- `instr_complete` - Instruction completion signal

### Memory Interface
- `imem_addr`, `imem_data` - Instruction memory interface
- `dmem_addr`, `dmem_wdata`, `dmem_rdata` - Data memory interface
- `dmem_we`, `dmem_re` - Memory write/read enable
- `dmem_size` - Memory access size

### Debug Signals
- `debug_pc` - Current program counter value
- `debug_instruction` - Current instruction word
- `debug_rs1_data`, `debug_rs2_data` - Source register values
- `debug_rd_data` - Destination register value
- `debug_fsm_state` - FSM state for multi-cycle control

## Conclusion

The `println_test.elf` program successfully executed on the RISC-V CPU simulator, demonstrating:

1. **Correct instruction execution**: 4,236 instructions completed without errors
2. **Memory subsystem functionality**: 746 memory operations completed successfully
3. **FIFO debug protocol**: Successfully transmitted debug messages to the host
4. **Multi-cycle operation**: Consistent 4 CPI performance across the program execution
5. **Program termination**: Clean exit via tohost mechanism (value 0x0000002a = 42)

The VCD waveform file provides a complete record of all CPU signals throughout the execution, enabling:
- Detailed timing analysis
- Signal-level debugging
- Performance profiling
- Verification of RTL implementation

## Appendix: How to Generate VCD Files

To generate VCD files for other test programs:

```bash
# Run cpu-sim with VCD dump enabled
cargo run --package cpu-sim -- test_programs/your_program.elf --vcd output.vcd

# Analyze the VCD file
cargo run --package cpu-sim --bin analyze_vcd -- output.vcd
```

## Appendix: Tools Used

- **CPU Simulator**: `cpu-sim` - RISC-V RV32IM simulator with Verilator backend
- **VCD Analyzer**: Custom analyzer tool (`analyze_vcd`) built using the `vcd` crate
- **Waveform Viewer**: GTKWave (optional, for visual inspection)

---

*Generated from VCD analysis of println_test.elf execution*  
*Date: 2026-01-08*  
*VCD File: /tmp/println_test.vcd (6.5 MB)*
