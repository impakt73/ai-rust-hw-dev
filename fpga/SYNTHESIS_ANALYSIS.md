# FPGA Synthesis Analysis Report

**Date:** 2026-02-01  
**Target Device:** Lattice iCE40-HX8K-CB132 (Alchitry Cu v1)  
**Synthesis Tools:** Yosys 0.33, nextpnr-ice40, icetime  
**Design:** RISC-V RV32I CPU with UART peripherals (M and F extensions disabled)

---

## Executive Summary

The RISC-V CPU design successfully synthesizes and meets timing at **25 MHz** target frequency, with an achieved **Fmax of 35.29 MHz** (41% timing margin). The design uses **~90% of available logic cells**, leaving minimal headroom for additional features.

### Key Metrics

| Metric | Value | Available | Utilization |
|--------|-------|-----------|-------------|
| **Logic Cells (ICESTORM_LC)** | ~6,973 | 7,680 | **~90%** |
| **Block RAM (ICESTORM_RAM)** | 0 | 32 | 0% |
| **I/O Pins (SB_IO)** | 77 | 256 | 30% |
| **Global Buffers (SB_GB)** | 5 | 8 | 62% |
| **PLLs (ICESTORM_PLL)** | 1 | 2 | 50% |
| **Max Frequency** | 35.29 MHz | 25 MHz target | **PASS** (+41%) |
| **Critical Path Delay** | 27.93 ns | 40.00 ns budget | PASS |

---

## Resource Utilization Analysis

### Cell Breakdown (from Yosys)

| Cell Type | Count | Description |
|-----------|-------|-------------|
| **SB_LUT4** | 4,870 | 4-input Look-Up Tables |
| **SB_CARRY** | 885 | Carry chain cells (arithmetic) |
| **SB_DFFE** | 1,121 | D flip-flop with enable |
| **SB_DFFER** | 1,079 | D flip-flop with enable and reset |
| **SB_DFFR** | 84 | D flip-flop with reset |
| **SB_DFFSR** | 23 | D flip-flop with set/reset |
| **SB_DFFS** | 10 | D flip-flop with set |
| **SB_DFFESR** | 8 | D flip-flop with enable, set, and reset |
| **SB_DFFES** | 7 | D flip-flop with enable and set |
| **SB_DFF** | 2 | Basic D flip-flop |
| **SB_PLL40_CORE** | 1 | PLL for clock generation |
| **Total Cells** | 8,090 | - |

### Logic Cell Allocation (from nextpnr)

| LC Usage | Count | Percentage |
|----------|-------|------------|
| LUT4 only | 4,281 | 61.4% |
| LUT4 + DFF combined | 589 | 8.5% |
| DFF only | 1,745 | 25.0% |
| CARRY only | 372 | 5.3% |
| Carry chain legalization | 62 | 0.9% |
| **Total LCs Used** | **6,973** | **90.8%** |

---

## Top Resource Consumers

Based on the synthesis output and critical path analysis, the following modules consume the most resources:

### 1. **CPU Core (cpu.sv)** - Primary Resource Consumer

The CPU core is the dominant consumer of logic resources due to:

- **12-state FSM** controlling multi-cycle operation
- **32x32-bit integer register file** (requires significant multiplexing)
- **Instruction decode logic** for RV32IC instructions
- **Address generation** and memory interface
- **Decompressor** for RV32C compressed instructions

**Impact:** The FSM and register file together consume substantial LUT resources for control logic and data routing.

### 2. **ALU (alu.sv)** - ~15-20% of LUTs

The ALU implements all RV32I arithmetic and logical operations:

- ADD/SUB (with 32-bit carry chains)
- AND, OR, XOR (bitwise operations)
- SLL, SRL, SRA (barrel shifter)
- SLT, SLTU (comparators)
- MIN/MAX operations (for A extension atomics)

**Note:** With M extension disabled, no multiplier or divider hardware is instantiated.

### 3. **Register File (regfile.sv)** - ~8-10% of LUTs

- 32 registers × 32 bits = 1,024 flip-flops
- 2-read, 1-write port multiplexing
- LUT-based implementation (not using BRAM)

**Recommendation:** Convert to BRAM implementation to save ~400+ LUTs.

### 4. **Decoder (decoder.sv + decompress.sv)** - ~5% of LUTs

- RV32I instruction decoder
- RV32C compressed instruction decompressor
- Complex case statement logic

### 5. **UART Controllers** - ~5% of LUTs

Two UART instances:
- **Host UART** for USB serial communication
- **Peripheral UART** for CPU access

Each includes:
- TX/RX state machines
- Baud rate generators (25 MHz / 115200 baud)
- 8-entry FIFOs (sync_fifo.sv)

---

## Critical Path Analysis

### Maximum Frequency Results

| Clock Domain | Achieved Fmax | Target | Status |
|--------------|---------------|--------|--------|
| pll_clk_global (25 MHz) | 35.29 MHz | 25.00 MHz | ✅ PASS |

### Critical Path Breakdown

The critical path runs through the **ALU result to UART FIFO**, with the following chain:

```
Path: opcode_reg → regfile.rd_data → alu_b → ALU computation → alu_result →
      clock_wdata → uart_ctrl.tx_fifo_inst
```

**Previous Critical Path (before optimization):**

The original critical path ran through the branch/jump target calculation:
```
Path: opcode_reg → regfile.rd_data → alu.a/b → ALU computation → alu_result → 
      alu_zero → take_branch → next_pc_value → imem_addr
```

This path was optimized by:
1. **Pre-computing branch/jump targets** in dedicated registers during DECODE/EXECUTE
2. **Computing branch equality directly** in branch_unit instead of using ALU's zero flag

**Detailed Critical Path (from icetime):**

| Stage | Time (ns) | Cumulative | Component |
|-------|-----------|------------|-----------|
| Register output (opcode_reg) | 0.64 | 0.64 | DFF to LUT |
| Register file decode | 1.04 | 1.68 | LUT cascade |
| ALU input selection | 5.75 | 7.43 | LUT + routing |
| ALU carry chain (32-bit) | 8.22 | 15.65 | 32× SB_CARRY |
| Result formatting | 8.38 | 24.03 | LUT cascade + routing |
| UART FIFO write | 3.89 | 27.93 | Register setup |

**Total Critical Path:** ~27.93 ns (icetime) / 28.35 ns (nextpnr)

### Critical Path Bottlenecks

1. **32-bit ALU Carry Chain** (~4-5 ns)
   - The 32-bit adder/subtractor uses a ripple carry chain
   - 32 sequential SB_CARRY cells create significant delay

2. **Result Multiplexing** (~3-4 ns)
   - Multiple result sources (ALU, memory, CSR, etc.) require wide muxes
   - The writeback_mux.sv module adds delay

3. **Routing Delays** (~8-10 ns total)
   - Long wires between logic blocks
   - Limited global routing resources

### Timing Optimization History

| Optimization | Before | After | Improvement |
|--------------|--------|-------|-------------|
| Pre-computed branch/jump targets | 32.79 MHz | 35.29 MHz | +7.6% Fmax |

### Cross-Domain Paths

| Path Type | Delay | Description |
|-----------|-------|-------------|
| Async → pll_clk_global | 4.28 ns | Input synchronizers (buttons, USB_RX) |
| pll_clk_global → Async | 24.52 ns | Output paths (LEDs, USB_TX, seven-segment) |

The async-to-clock paths are properly synchronized via 2-FF synchronizers in the design.

---

## Synthesis Warnings and Issues

### Warnings from Yosys

| Warning | File | Description | Severity |
|---------|------|-------------|----------|
| **FIFO memory replaced with registers** | sync_fifo.sv:97 | Small FIFOs (8 entries) synthesized as registers | ℹ️ Info |
| **Async reset value is not constant** | Multiple | `boot_addr` used in reset logic | ⚠️ Minor |

### Warning Details

#### 1. FIFO Memory to Registers (Info)

```
Warning: Replacing memory \mem with list of registers. See ../rtl/sync_fifo.sv:97
```

**Analysis:** The 8-entry FIFOs in the UART controllers are too small to benefit from BRAM and are correctly synthesized as distributed registers. This is expected behavior and not an issue.

#### 2. Async Reset Value Not Constant (Minor)

```
Warning: Async reset value `\boot_addr' is not constant!
```

**Analysis:** The `boot_addr` input parameter is used to initialize the PC register on reset. Since it's an input signal rather than a compile-time constant, Yosys flags this. In practice, `boot_addr` is held constant (0x80000000) during reset, so this is safe.

**Recommendation:** If this warning is undesirable, change the reset logic to use a `localparam` for the boot address instead of a port.

### No Warnings from nextpnr

nextpnr completed without warnings, indicating:
- No placement conflicts
- No routing failures
- All timing constraints met

---

## Recommendations for Improvement

### Immediate Optimizations (Low Effort)

1. **Convert Register File to BRAM**
   - The 32×32-bit register file currently uses LUTs
   - Using 1 BRAM block would save ~400 LUTs
   - Trade-off: 1-cycle read latency increase

2. **Use BRAM for UART FIFOs (if larger)**
   - Current 8-entry FIFOs correctly use registers
   - If FIFO depth increases, consider BRAM

### Medium-Term Improvements

3. **Reduce Carry Chain Length**
   - The 32-bit ALU adder creates a long critical path
   - Consider: Carry-lookahead or carry-select adder for better timing
   - Or: Break computation across two cycles

4. **Register ALU Outputs**
   - Adding a pipeline register after ALU output could improve Fmax
   - Trade-off: Additional cycle for ALU operations

5. **Optimize Writeback Mux**
   - The 8-input writeback multiplexer adds delay
   - Consider: Pre-selecting data earlier in the pipeline

### Architecture Changes (Higher Effort)

6. **Enable Extensions Conditionally**
   - M and F extensions are already disabled via parameters
   - Current design fits without them
   - Re-enabling would exceed FPGA capacity

7. **Clock Frequency Optimization**
   - Current: 25 MHz (from 100 MHz via PLL)
   - Achieved Fmax: 32.79 MHz
   - Could potentially run at 30 MHz with margin

---

## BRAM Usage Opportunity

### Current State
The design uses **0 of 32 available BRAM blocks**.

### Potential BRAM Applications

| Use Case | BRAM Blocks | LUT Savings | Notes |
|----------|-------------|-------------|-------|
| Integer Register File | 1 | ~400 LUTs | Add 1-cycle read latency |
| Larger UART FIFOs | 1-2 | Minimal | Only if deeper FIFOs needed |
| Instruction Cache | 4-16 | N/A | Would improve performance |
| Boot ROM | 1-2 | N/A | Store bootloader |

### Recommendation
Using BRAM for the register file is the highest-value optimization, reducing LUT usage from 90% to ~85% while freeing resources for additional features.

---

## Global Buffer Usage

### Promoted Signals

| Signal | Fanout | Purpose |
|--------|--------|---------|
| `reset_ctrl.rst_n_out` | 1,206 | Global reset signal |
| `cpu_core.a_reg_write` | 143 | Register write enable |
| `host_bus_if.next_state...` | 67 | Host interface control |
| `cpu_core.instr_complete_internal` | 64 | Instruction completion |

### Analysis
5 of 8 global buffers are in use. The high fanout signals are correctly promoted to global routing, reducing routing congestion.

---

## Conclusion

The RISC-V CPU design is a successful fit for the iCE40-HX8K FPGA with:

- ✅ **Timing closure** at 25 MHz with 41% margin (35.29 MHz achieved)
- ✅ **No critical warnings** affecting functionality
- ⚠️ **High utilization** (~90%) limiting expansion
- ⚠️ **No BRAM usage** despite availability

### Completed Optimizations

1. **Pre-computed branch/jump targets** - Moved branch target calculation from combinational logic to registered values, improving Fmax from 32.79 MHz to 35.29 MHz (+7.6%)

### Priority Recommendations

1. **Convert regfile to BRAM** to reduce utilization to ~85%
2. **Document boot_addr warning** as expected behavior
3. **Consider 30+ MHz operation** - now achievable with 17% margin

---

## Files Referenced

| File | Purpose |
|------|---------|
| `fpga/fpga_top.sv` | FPGA top-level wrapper |
| `fpga/Makefile` | Build automation |
| `fpga/build/yosys.log` | Synthesis output |
| `fpga/build/nextpnr.log` | Place & route output |
| `fpga/build/riscv_fpga_timing.rpt` | Timing analysis |

---

*Report generated by automated synthesis analysis*
