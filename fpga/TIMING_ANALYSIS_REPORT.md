# FPGA Timing Analysis Report

**Date:** 2026-02-07  
**Target Device:** Lattice iCE40-HX8K-CB132 (Alchitry Cu v1)  
**Target Frequency:** 25 MHz  
**Tools:** Yosys 0.33, nextpnr-ice40, icetime

---

## Executive Summary

This report documents the timing analysis performed to address reduced FPGA timing slack caused by recent PRs, particularly the addition of the system controller. Two simple fixes were implemented to improve Fmax from **28.21 MHz to 31.12 MHz** (+10.3%), restoring healthy timing margin at the 25 MHz target. Remaining complex timing issues are documented for future optimization.

### Timing Improvement Summary

| Metric | Before Fixes | After Fixes | Change |
|--------|-------------|-------------|--------|
| **Fmax (nextpnr)** | 28.21 MHz | 31.12 MHz | **+10.3%** |
| **Fmax (icetime)** | 27.93 MHz | 30.72 MHz | **+10.0%** |
| **Critical Path** | 35.4 ns | 32.1 ns | **-3.3 ns** |
| **Timing Margin** | 12.8% | 24.5% | **+11.7%** |
| **Logic Cells** | 82% | 82% | No change |

---

## Fixes Implemented

### Fix 1: Register div_unit Inputs

**File:** `rtl/div_unit.sv`  
**Impact:** ~0.8 MHz improvement

**Problem:** The division unit received its `dividend` and `divisor` inputs as live combinational signals from the ALU input muxes. Inside div_unit, these inputs immediately fed into:
- Absolute value computation (`~divisor + 1'b1`) — 32-bit negate
- Overflow detection (`dividend == MIN_INT && divisor == -1`) — 32-bit comparison with carry chain
- Zero check (`divisor == 0`) — 32-bit comparison

This created a long combinational path from `opcode_reg` through the ALU b mux, into div_unit, and through the comparison carry chains.

**Solution:** Added registered copies of all inputs (`dividend_reg`, `divisor_reg`, `is_signed_reg`, `rem_sel_reg`) that are captured in the `DIV_IDLE` state when `start` is asserted. All internal logic now operates on the registered copies.

**Trade-off:** One additional clock cycle per division operation (~35 cycles instead of ~34). Negligible impact since division is already multi-cycle.

### Fix 2: Register system_controller Write Data

**File:** `rtl/peripherals/system_controller.sv`  
**Impact:** ~2.1 MHz improvement

**Problem:** The system controller performed 32-bit equality comparisons on `wdata` (`wdata == 32'h00000001` for RESET_SYSTEM, `wdata == 32'h00000002` for RESET_CPU) in the same clock cycle that data arrived from the ALU through the bus. This created the tail end of the critical path:

```
opcode_reg → ALU b mux → ALU carry chain → alu_result →
bus.sv wdata broadcast → system_controller.wdata →
32-bit comparison → cpu_reset_trigger register enable
```

**Solution:** Added a pipeline register stage for write operations:
- **Cycle 1:** Detect write intent (`write_reset`/`write_boot`) and register `wdata` into `wdata_reg`
- **Cycle 2:** Process the registered `wdata_reg` for reset trigger comparisons

**Trade-off:** One additional clock cycle for system controller write operations. Acceptable since reset/boot operations are infrequent startup operations.

---

## Current Critical Path (After Fixes)

The critical path (32.1 ns = 11.1 ns logic + 21.0 ns routing) is now:

```
opcode_reg → ALU b input mux (LUT chain) → ALU computation (carry chain) →
alu_result → bus.sv wdata broadcast → arbiter wdata → uart_ctrl.tx_fifo write
```

### Path Breakdown

| Stage | Delay | Description |
|-------|-------|-------------|
| opcode_reg output | 0.5 ns | Register to first LUT |
| ALU b input mux | ~8.2 ns | 4-5 LUT stages selecting ALU operand b |
| ALU computation | ~12 ns | Carry chain through comparisons/arithmetic |
| alu_result → bus | ~4 ns | Routing through bus.sv to peripheral |
| Bus → UART FIFO | ~7 ns | Routing to UART FIFO write port |

---

## Complex Issues (Not Fixed — Require Architectural Changes)

### Issue 1: ALU Input Mux Combinational Depth

**Severity:** Medium  
**Estimated Timing Impact:** 8-10 ns of the critical path

The ALU `b` input selection (in `cpu.sv`, line ~1059-1093) involves complex state-dependent muxing:

```systemverilog
// Current implementation (simplified):
alu_b = alu_src_reg ? ((opcode_reg == STORE || opcode_reg == FSW) ? imm_s_reg : imm_i_reg) : b_reg;

if (current_state == S_MEM_ADDR && (is_amo_reg || is_lr_reg || is_sc_reg))
    alu_b = 32'h0;
else if (current_state == S_EXECUTE)
    case (opcode_reg)
        AUIPC: alu_b = imm_u_reg;
        JAL/JALR: alu_b = 32'd4;
    endcase
else if (current_state == S_ATOMIC_RMW)
    alu_b = b_reg;
```

This creates multiple levels of LUT muxing because:
1. The base mux depends on `alu_src_reg` AND `opcode_reg` (nested ternary)
2. State-dependent overrides add more mux levels
3. Each mux level requires routing between LUTs

**Why Not Fixed:** Resolving this requires restructuring the CPU's ALU operand selection to pre-compute the operand in an earlier FSM state and register it. This is a significant architectural change that affects multiple FSM states and the overall multi-cycle timing.

**Recommended Approach:**
1. Add an `alu_b_reg` register in the CPU
2. Compute and register the ALU b operand during `S_DECODE`/`S_REG_READ` states
3. Have `S_EXECUTE` and `S_MEM_ADDR` use the pre-registered value
4. This would eliminate ~8 ns from the critical path but requires careful FSM restructuring

### Issue 2: ALU Result Fan-Out Through Bus

**Severity:** Low-Medium  
**Estimated Timing Impact:** 5-7 ns routing

The ALU result (`alu_result`) feeds through `mem_interface` → CPU memory interface → bus arbiter → bus.sv → all peripherals simultaneously. The fan-out and routing delay from ALU result to the farthest peripheral (UART FIFO) contributes significant routing delay.

**Why Not Fixed:** This is inherent to the bus architecture where all peripherals are connected in parallel. Fixing it would require:
- Registering the bus outputs in bus.sv (adding latency to all bus transactions)
- Or restructuring the bus to use a registered output stage with backpressure

The current multi-cycle CPU design already handles variable-latency memory, so adding a registered bus output stage is feasible but would increase all peripheral access latency by one cycle and require careful verification.

### Issue 3: Combinational Address Decode in bus.sv

**Severity:** Low  
**Estimated Timing Impact:** 2-3 ns

The address decoder in `bus.sv` performs range comparisons on the full 32-bit address:
```systemverilog
if (master_addr >= LED_BASE && master_addr < LED_LIMIT)
    sel_led = 1'b1;
else if (master_addr >= CLOCK_BASE && master_addr < CLOCK_LIMIT)
    sel_clock = 1'b1;
// ... etc
```

Each range comparison involves 32-bit subtraction (carry chains). With 5 peripherals (LED, Clock, UART, System Controller, External Memory), the priority-encoded if-else chain adds LUT depth.

**Why Not Fixed:** The current timing impact is relatively small (~2-3 ns). Optimization options include:
- Using upper address bits only for coarse decode (e.g., bits [27:24] could distinguish all peripherals)
- Registering the address decode output (adds 1 cycle latency to all bus transactions)

These require careful verification since changing address decode timing affects all bus transactions.

---

## Resource Utilization (After Fixes)

| Resource | Used | Available | Utilization |
|----------|------|-----------|-------------|
| **Logic Cells (ICESTORM_LC)** | ~6,359 | 7,680 | **82%** |
| **Block RAM (ICESTORM_RAM)** | 4 | 32 | 12% |
| **I/O Pins (SB_IO)** | 77 | 256 | 30% |
| **Global Buffers (SB_GB)** | 6 | 8 | 75% |
| **PLLs (ICESTORM_PLL)** | 1 | 2 | 50% |

---

## Historical Timing Comparison

| Version | Fmax | Status | Notes |
|---------|------|--------|-------|
| Pre-system-controller | ~34.91 MHz | ✅ | Previous baseline |
| Post-system-controller (before fixes) | 28.21 MHz | ✅ (reduced margin) | System controller added combinational paths |
| After Fix 1 (div_unit registered inputs) | 29.03 MHz | ✅ | +0.8 MHz |
| **After Fix 2 (system_controller pipelined writes)** | **31.12 MHz** | ✅ | **+2.1 MHz** |

---

## Recommendations

### Short-Term (Next Sprint)
1. **Register ALU b operand** in the CPU FSM — would recover ~3-5 MHz by reducing mux depth
2. **Simplify bus address decode** — use upper bits only for coarse peripheral selection

### Medium-Term
3. **Register bus outputs** in bus.sv — breaks routing-dominated paths to peripherals
4. **Consider 30 MHz operation** — current 31 MHz Fmax provides ~3% margin, sufficient for a stable 30 MHz clock

### Long-Term (Architectural)
5. **Pipelined CPU** — would fundamentally eliminate multi-stage combinational paths
6. **Registered peripheral interfaces** — standard AXI-lite style registered outputs

---

*Report generated by automated FPGA timing analysis*
