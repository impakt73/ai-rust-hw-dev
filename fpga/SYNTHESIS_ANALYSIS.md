# FPGA Synthesis Analysis Report

**Date:** 2026-02-01 (Updated with BRAM Register File Optimization)  
**Target Device:** Lattice iCE40-HX8K-CB132 (Alchitry Cu v1)  
**Synthesis Tools:** Yosys 0.33, nextpnr-ice40, icetime  
**Design:** RISC-V RV32I CPU with UART peripherals (M and F extensions disabled)

---

## Executive Summary

The RISC-V CPU design successfully synthesizes and meets timing at **25 MHz** target frequency, with an achieved **Fmax of 36.72 MHz** (47% timing margin). After implementing the **dual-copy BRAM register file** optimization, the design uses only **59% of available logic cells**, providing ample headroom for additional features.

### Key Metrics (After BRAM Register File Optimization)

| Metric | Value | Available | Utilization |
|--------|-------|-----------|-------------|
| **Logic Cells (ICESTORM_LC)** | 4,563 | 7,680 | **59%** |
| **Block RAM (ICESTORM_RAM)** | 4 | 32 | 12% |
| **I/O Pins (SB_IO)** | 77 | 256 | 30% |
| **Global Buffers (SB_GB)** | 5 | 8 | 62% |
| **PLLs (ICESTORM_PLL)** | 1 | 2 | 50% |
| **Max Frequency** | 36.72 MHz | 25 MHz target | **PASS** (+47%) |
| **Critical Path Delay** | 27.24 ns | 40.00 ns budget | PASS |

### BRAM Register File Optimization Impact

| Metric | Before | After | Change |
|--------|--------|-------|--------|
| **Logic Cells** | 6,988 (91%) | 4,563 (59%) | **-2,425 LCs (-35%)** |
| **Block RAM** | 0 (0%) | 4 (12%) | +4 BRAM blocks |
| **SB_LUT4** | 4,832 | 3,445 | **-1,387 LUTs (-29%)** |
| **Max Frequency** | 37.33 MHz | 36.72 MHz | -0.61 MHz (-1.6%) |

**Summary:** The dual-copy BRAM register file implementation trades 4 BRAM blocks for a 35% reduction in logic cell usage, with only a 1.6% decrease in maximum frequency. This frees ~2,400 logic cells for future features.

---

## Resource Utilization Analysis

### Cell Breakdown (from Yosys)

| Cell Type | Count | Description |
|-----------|-------|-------------|
| **SB_LUT4** | 3,445 | 4-input Look-Up Tables |
| **SB_CARRY** | 885 | Carry chain cells (arithmetic) |
| **SB_DFFE** | 128 | D flip-flop with enable |
| **SB_DFFER** | 1,175 | D flip-flop with enable and reset |
| **SB_DFFR** | 84 | D flip-flop with reset |
| **SB_DFFSR** | 23 | D flip-flop with set/reset |
| **SB_DFFS** | 10 | D flip-flop with set |
| **SB_DFFESR** | 8 | D flip-flop with enable, set, and reset |
| **SB_DFFES** | 7 | D flip-flop with enable and set |
| **SB_DFF** | 12 | Basic D flip-flop |
| **SB_RAM40_4K** | 4 | 4Kbit Block RAM (for register file) |
| **SB_PLL40_CORE** | 1 | PLL for clock generation |
| **Total Cells** | 5,782 | - |

### Logic Cell Allocation (from nextpnr)

| LC Usage | Count | Percentage |
|----------|-------|------------|
| **Total LCs Used** | **4,563** | **59.4%** |

Note: After the BRAM optimization, flip-flop usage is significantly reduced because the 32×32-bit register file no longer uses LUT-based distributed RAM.

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

### 3. **Register File (regfile_bram.sv)** - Now using BRAM ✅

- **Dual-copy BRAM architecture**: Two identical copies of 32×32-bit register file
- Copy A provides rs1 read data, Copy B provides rs2 read data
- Both copies are written simultaneously to stay in sync
- Uses 4× SB_RAM40_4K blocks (256×16-bit each, 2 per copy for 32-bit width)
- Added S_REG_READ FSM state to handle 1-cycle BRAM read latency

**Implementation:** `regfile_bram.sv` with `USE_BRAM_REGFILE` parameter in CPU

**Trade-off:** +1 cycle per instruction decode, -1,387 LUTs, +4 BRAM blocks

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
| pll_clk_global (25 MHz) | 37.14 MHz | 25.00 MHz | ✅ PASS |

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
| Result formatting | 7.00 | 22.65 | LUT cascade + routing |
| UART FIFO write | 3.78 | 26.43 | Register setup |

**Total Critical Path:** ~26.43 ns (icetime) / 26.88 ns (nextpnr)

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
| Pre-computed branch/jump targets + direct equality | 32.79 MHz | 37.14 MHz | +13.3% Fmax |
| BRAM register file (dual-copy) | 37.33 MHz | 36.72 MHz | -1.6% Fmax (acceptable trade-off for 35% LUT savings) |

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

### Completed Optimizations ✅

1. **Convert Register File to BRAM** ✅ (Completed)
   - Implemented dual-copy BRAM register file (`regfile_bram.sv`)
   - Uses 4 BRAM blocks, saves ~1,387 LUTs (29% reduction)
   - Added S_REG_READ state to handle 1-cycle read latency
   - Max frequency impact: -1.6% (37.33 → 36.72 MHz)

### Remaining Optimizations (Low Effort)

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

## BRAM Usage

### Current State
The design uses **4 of 32 available BRAM blocks** for the dual-copy register file.

### BRAM Block Allocation

| Use Case | BRAM Blocks | Purpose |
|----------|-------------|---------|
| Register File Copy A (rs1 port) | 2 | Lower 16-bit + Upper 16-bit |
| Register File Copy B (rs2 port) | 2 | Lower 16-bit + Upper 16-bit |
| **Total Used** | **4** | 12% of available BRAM |
| **Remaining** | **28** | Available for future use |

### Potential Future BRAM Applications

| Use Case | BRAM Blocks | Notes |
|----------|-------------|-------|
| Larger UART FIFOs | 1-2 | Only if deeper FIFOs needed |
| Instruction Cache | 4-16 | Would improve performance |
| Boot ROM | 1-2 | Store bootloader |
| Data Cache | 4-16 | Would improve memory performance |

### Completed Optimization
The register file BRAM conversion is now complete. The dual-copy architecture successfully provides 2-read, 1-write capability using iCE40's single-port BRAM blocks.

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

- ✅ **Timing closure** at 25 MHz with 47% margin (36.72 MHz achieved)
- ✅ **No critical warnings** affecting functionality
- ✅ **Low utilization** (~59%) providing ample expansion headroom
- ✅ **BRAM utilized** for register file (4 of 32 blocks)

### Completed Optimizations

1. **Pre-computed branch/jump targets with direct equality** - Moved branch target calculation from combinational logic to registered values and removed ALU dependency for BEQ/BNE, improving Fmax from 32.79 MHz to 37.14 MHz (+13.3%)

2. **Dual-copy BRAM register file** - Implemented `regfile_bram.sv` with two BRAM copies to provide 2-read, 1-write capability on iCE40's single-port BRAM. Added S_REG_READ FSM state for 1-cycle read latency. Reduced LUT usage from 91% to 59% (-35%) at the cost of 4 BRAM blocks and minor Fmax reduction (-1.6%).

### Remaining Priority Recommendations

1. **Document boot_addr warning** as expected behavior
2. **Consider 35 MHz operation** - achievable with 5% margin
3. **Enable M extension** if needed - now have ~2,400 spare logic cells

---

## Files Referenced

| File | Purpose |
|------|---------|
| `fpga/fpga_top.sv` | FPGA top-level wrapper |
| `fpga/Makefile` | Build automation |
| `fpga/build/yosys.log` | Synthesis output |
| `fpga/build/nextpnr.log` | Place & route output |
| `fpga/build/riscv_fpga_timing.rpt` | Timing analysis |
| `rtl/regfile_bram.sv` | BRAM-based register file (new) |
| `rtl/cpu.sv` | CPU with USE_BRAM_REGFILE parameter |

---

*Report generated by automated synthesis analysis*
