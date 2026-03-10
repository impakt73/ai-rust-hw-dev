# iCE40-HX8K Critical Path & Timing Analysis

**Date:** 2026-03-10 (updated)  
**Target Device:** Lattice iCE40-HX8K-CB132 (Alchitry Cu v1)  
**Synthesis Tools:** Yosys 0.33, nextpnr-ice40, icetime  
**Design Configuration:** RV32I CPU (M/F extensions disabled for iCE40 resources), dual-banked BRAM register file

---

## Executive Summary

The RISC-V RV32I CPU design meets the 25 MHz target frequency with substantial headroom. The achieved Fmax is **47.70 MHz** (nextpnr) / **47.05 MHz** (icetime), providing a **91% timing margin** over the 25 MHz target (21.0 ns critical path vs. 40.0 ns clock budget).

Since the previous analysis, two major optimizations have been implemented and verified:

1. **Writeback Mux Adder Pre-computation** (former Critical Path #4): AUIPC and JAL/JALR return-address additions are now performed during the EXECUTE FSM state and stored into `alu_out_reg`. The `writeback_mux.sv` now selects the pre-computed `alu_result` for those cases, removing the inline carry chains from the writeback mux entirely.

2. **Seven-Segment Modulo-6 Replacement** (former Critical Path #2): The `button_counter % 8'd6` expression in `ice40_alchitry_cu_top.sv` has been replaced with a registered rollover counter (`seg_position_reg`). The 24.37 ns combinational modulo-division chain is eliminated; icetime now reports the dominant path as the ALU carry chain at 21.25 ns.

Together these changes improved routed Fmax from **42.09 MHz → 47.70 MHz** (+5.61 MHz, **+13.3%**) and reduced the critical path from **23.8 ns → 21.0 ns** (−2.8 ns, **−11.8%**). Logic cell count decreased from 5,646 to **5,520** (−126 LCs).

The design still faces two impending **resource saturation** constraints that will limit future development:

- **BRAM utilization: 30/32 blocks (93%)** — only 2 blocks of headroom remain
- **Global buffer utilization: 8/8 (100%)** — all global routing resources exhausted

### Key Metrics

| Metric | Value | Available | Utilization |
|--------|-------|-----------|-------------|
| **Logic Cells (ICESTORM_LC)** | 5,520 | 7,680 | **71%** |
| **Block RAM (ICESTORM_RAM)** | 30 | 32 | **93% ⚠️** |
| **I/O Pins (SB_IO)** | 77 | 256 | 30% |
| **Global Buffers (SB_GB)** | 8 | 8 | **100% ⚠️** |
| **PLLs (ICESTORM_PLL)** | 1 | 2 | 50% |
| **Achieved Fmax (nextpnr)** | 47.70 MHz | 25 MHz target | **PASS (+91%)** |
| **Achieved Fmax (icetime)** | 47.05 MHz | 25 MHz target | **PASS (+88%)** |
| **Critical Path Delay** | 21.0 ns | 40.0 ns budget | PASS |

### Cell Type Breakdown (from Yosys)

| Cell Type | Count | Change vs. Previous | Description |
|-----------|-------|---------------------|-------------|
| SB_LUT4 | 4,462 | −98 | 4-input Look-Up Tables |
| SB_CARRY | 814 | −84 | Carry chain cells (arithmetic) |
| SB_DFFESR | 1,607 | −4 | D flip-flop with enable and set/reset |
| SB_DFF variants | 645 | −73 | Various D flip-flop configurations |
| SB_RAM40_4K | 30 | 0 | 4Kbit Block RAM instances |
| SB_PLL40_CORE | 1 | 0 | PLL |

### Optimization History

| Date | Optimization | Fmax Before | Fmax After | Improvement |
|------|-------------|-------------|------------|-------------|
| 2026-03-10 | Pre-compute AUIPC/JAL adders in EXECUTE; remove inline adders from `writeback_mux.sv` | 42.09 MHz | 47.70 MHz | +5.61 MHz (+13.3%) |
| 2026-03-10 | Replace `button_counter % 8'd6` modulo with registered rollover counter in `ice40_alchitry_cu_top.sv` | 41.04 MHz (icetime worst case) | 47.05 MHz (icetime) | +6.01 MHz (+14.6%) |

---

## Critical Path #1 — ALU B-Input Decode + Full 32-bit Ripple-Carry + Post-Carry Result Mux (Primary)

**Clock domain:** `pll_clk_global` (posedge → posedge)  
**Total delay:** 21.0 ns (8.8 ns logic + 12.2 ns routing) — nextpnr; 21.25 ns — icetime  
**Achieved Fmax:** 47.70 MHz (nextpnr), 47.05 MHz (icetime)  
**Logic levels (icetime):** 42  
**RTL modules involved:** `cpu.sv`, `alu.sv`

### Path Narrative

This is the dominant registered-clock critical path identified by both nextpnr and icetime. It originates from a registered CPU control flag (`jump_reg` / `is_lr_reg` — the decoded jump or load-reserved atomic instruction flag), traverses two LUT levels that compute the ALU B input (`alu_b`), enters a 30-stage SB_CARRY ripple-carry chain, and then passes through multiple post-carry LUT levels that format the `alu_result` output before terminating at a register (`alu_out_reg` / `cpu_to_arb_a_wdata`) in the host-bus arbiter interface.

Compared to the previous critical path, this path is significantly more compact: it no longer crosses into `mem_interface.sv`, `system_controller_peripheral.sv`, or `host_bus_mux.sv`. The previous cross-module routing accounted for 6.7 ns alone; the equivalent final hop is now only ~1.0 ns because nextpnr can place the destination register adjacent to the ALU output LUTs.

### Detailed Stage Breakdown

| Stage | Start (ns) | End (ns) | Δ (ns) | Type | Description |
|-------|-----------|---------|-------|------|-------------|
| DFF output (`jump_reg` or `is_lr_reg`) | 0.0 | 0.64 | 0.64 | Logic | Flip-flop Q propagation |
| `jump_reg` net routing (18,8 → 22,21) | 0.64 | 3.00 | 2.36 | **Routing** | Long cross-fabric net for jump flag |
| ALU B mux LUT level 1 | 3.00 | 3.45 | 0.45 | Logic | LUT computing `alu_b` bit selection |
| ALU B mux LUT level 2 | 3.45 | 4.48 | 1.03 | Logic + Routing | Second LUT + short net (~0.5 ns) |
| ALU B carry input setup | 4.48 | 6.01 | 1.53 | Logic + Routing | LUT → `alu_b[2]` carry mux path |
| **30-stage SB_CARRY chain (tiles 20,18–20,21)** | 6.01 | 10.96 | **4.94** | Logic | 30 `SB_CARRY` cells across 4 tiles (1 carry-start + 29 carry-propagate) |
| Carry-out buffer + bit-31 LUT | 10.96 | 11.85 | 0.89 | Logic + Routing | Tile crossing + carry bit-31 formatting |
| `alu_result` format LUT → routing (20,21→18,26) | 11.85 | 14.91 | 3.06 | **Routing** | Span-12 cross-fabric route to result mux |
| Post-carry result mux (5 LUT levels, tiles 18,30–17,32) | 14.91 | 20.26 | 5.35 | Logic + Routing | `alu_result[27]` selection from multi-op mux |
| `alu_result[27]` routing to DFF (17,32→17,31) | 20.26 | 20.85 | 0.59 | **Routing** | Short final net to destination register |
| Setup at destination DFF | 20.85 | 21.25 | 0.40 | Setup | Register setup time |

**Path summary:**
```
jump_reg[DFF] (cpu.sv)
  → 2 LUT levels: ALU B mux (cpu.sv, 2.4 ns cross-fabric net)
  → alu_b[2] → carry chain input mux
  → 30-stage SB_CARRY chain (alu.sv, 4.94 ns, tiles 20,18–20,21)
  → carry bit-31 LUT (0.89 ns)
  → alu_result format mux (5 LUT levels, 3.06 ns cross-fabric + 5.35 ns logic/routing)
  → cpu_to_arb_a_wdata[27][DFF] (0.59 ns net + 0.40 ns setup)
```

### Why This Path Is Slow

1. **Late carry-chain input arrival:** The carry chain receives its inputs at ~6.0 ns because `jump_reg` must route across a significant distance (18,8 → 22,21 in the LC grid, ~2.4 ns) and then traverse two LUT levels before reaching the carry input mux. Even though the carry chain itself is only ~4.9 ns, it cannot start until the late inputs arrive.

2. **Post-carry result formatting depth:** After the carry chain completes at ~11.0 ns, the result still passes through **5 LUT levels** for `alu_result` output selection. This ~9.2 ns post-carry segment exceeds the carry chain itself in total contribution. The result mux in `alu.sv` must select between many operation outputs (ADD/SUB/SLT/SLTU/AND/OR/XOR/SLL/SRL/SRA), producing a deep priority-encoded mux tree.

3. **Residual routing dominance:** 12.2 ns of the 21.0 ns total (58%) is routing. While improved from the previous 14.6 ns (61%), routing still accounts for the majority of path delay. The 3.06 ns cross-fabric net after the carry chain is the single largest routing hop.

4. **32-bit ripple carry propagation:** The 32-bit adder in `alu.sv` (line ~225) uses a single linear carry chain across 4 tile rows. The carry chain alone accounts for 4.94 ns — 24% of the total path. The chain comprises 30 cells (1 carry-start cell at 0.231 ns and 29 carry-propagate cells at 0.126 ns each), split across three tile-row boundaries (each requiring an ICE_CARRY_IN_MUX at ~0.196 ns).

---

## ~~Critical Path #2 — Seven-Segment Display Modulo Arithmetic~~ (RESOLVED)

**Status:** ✅ **ELIMINATED** by replacing `button_counter % 8'd6` with a registered rollover counter.  
**Previous worst-case:** 24.37 ns (icetime), 35 logic levels  
**Current contribution:** ~5.4 ns (clock-to-async output via `seg_position_reg → io_seg` decode, within budget)

### What Changed

The `seg_position = 3'(button_counter % 8'd6)` combinational expression in `ice40_alchitry_cu_top.sv` was the icetime worst-case path due to hardware modulo-6 synthesis (Yosys generates iterative subtraction/comparison circuits for non-power-of-2 divisors). This produced 35 logic levels and 24.37 ns of combinational depth.

The fix replaced it with a dedicated registered counter:

```systemverilog
// Advance segment position on button press OR led_out change
if ((|(io_button_sync2 & ~io_button_prev)) || (led_out != led_out_prev)) begin
    seg_position_reg <= (seg_position_reg == 3'd5) ? 3'd0 : (seg_position_reg + 3'd1);
end
```

The new `pll_clk_global → <async>` output path via `seg_position_reg` is only **5.36 ns** (max delay from nextpnr), well within the 40 ns clock budget. The 35-level combinational division chain is completely gone.

---

## Critical Path #2 — Post-Carry ALU Result Mux Depth (Near-Critical)

**Clock domain:** `pll_clk_global` (posedge → posedge)  
**Characterized delay:** ~19–21 ns (multiple endpoints in tight slack range)  
**RTL modules involved:** `alu.sv`, `cpu.sv`

### Path Narrative

The slack histogram shows a cluster of endpoints in the 19–21 ns range (slack 19–21 ns out of a 40 ns clock period), indicating several near-critical paths in addition to the primary 21.0 ns path. These paths share the same post-carry ALU result mux structure described in Critical Path #1 but arrive slightly earlier at the carry chain or bypass it.

The primary contributor is the multi-way result selection mux inside `alu.sv`. The ALU must select among outputs for: ADD/SUB, SLT, SLTU, AND, OR, XOR, SLL, SRL, SRA, LUI-passthrough, and AUIPC. In the current implementation, all these results are computed in parallel and combined in a priority-encoded mux tree. The tree depth is approximately 4–5 LUT levels, consistently appearing on multiple near-critical paths.

The near-critical endpoint cluster corresponds to paths that reach the same result-format LUT tree via different carry-chain bits (different ALU result bits) or different source operands. Each bit of `alu_result` may travel a slightly different routing path, causing the cluster to span ~2 ns in the histogram.

### Why This Path Persists

Unlike the previous near-critical paths (which were caused by inline adders in `writeback_mux.sv` now removed), this cluster arises from the inherent structural depth of the ALU's result selection mux. Since the ALU cannot know which operation will be needed at elaboration time, all operation results are always computed and muxed, regardless of the actual instruction in flight. Reducing this requires either a smaller ALU operation set or explicit encoding techniques.

---

## Critical Path #3 — FSM Instruction Decode Depth (Contributing)

**Clock domain:** `pll_clk_global`  
**Characterized delay:** ~3.0–6.0 ns (early preamble of Critical Path #1)  
**RTL modules involved:** `cpu.sv`, `decoder.sv`

### Path Narrative

The CPU's multi-cycle FSM stores decoded control flags (`jump_reg`, `is_lr_reg`, `is_auipc_reg`, etc.) as registered signals latched at the end of the DECODE state. These registered flags are the launch flip-flops for Critical Path #1. The 2.36 ns routing from `jump_reg` (at tile 18,8) to the first ALU B mux LUT (at tile 22,21) reflects placement pressure: the CPU decode registers and the ALU carry chain are placed more than 4 tile-rows apart.

The decode complexity increases with each instruction class supported. With the A extension enabled (atomic instructions adding `is_lr_reg`, `is_sc_reg`, `is_amo_reg`), each new instruction flag adds fanout to the existing decode LUT trees. This is the same structural decode pressure identified in the previous analysis, but its impact is now limited to the 2.4 ns cross-fabric routing hop rather than the previous ~7 ns decode chain before the ALU.

### Why This Path Persists

The registered-flag approach (latching decoded signals in DECODE state) correctly isolates the LUT decode depth to the DECODE clock cycle. However, the placement challenge remains: the decoded flags live near the decoder and register file, while the ALU carry chain lives wherever the placer puts the arithmetic. With 71% LC utilization, the placer has limited freedom to co-locate these structures.

---

## ~~Critical Path #4 — Writeback Multiplexer Fan-In~~ (RESOLVED)

**Status:** ✅ **ELIMINATED** by pre-computing AUIPC and JAL/JALR return addresses in the EXECUTE stage.  
**Previous impact:** Near-critical paths at ~19–21 ns, multiple endpoints in tight slack range  
**Current contribution:** No inline adders in `writeback_mux.sv`; AUIPC and jump return-address results use the registered `alu_result` directly

### What Changed

The `writeback_mux.sv` previously instantiated two inline 32-bit adders:
- `pc + imm_u` for AUIPC
- `pc + 32'd4` for JAL/JALR return address

Both adders are now removed. During the EXECUTE FSM state, `alu_b` is loaded with `imm_u_reg` (for AUIPC) or `32'd4` (for jump return address), and the ALU computes the result into `alu_out_reg`. The writeback mux now selects the pre-registered `alu_result` for those cases:

```systemverilog
end else if (opcode == 7'b0010111) begin
    // AUIPC - Use pre-computed PC-relative result from EXECUTE
    rd_data = alu_result;
end else if (jump) begin
    // JAL/JALR - Use pre-computed return address from EXECUTE
    rd_data = alu_result;
end
```

This is consistent with how `branch_target_reg`, `jal_target_reg`, and `jalr_target_reg` were already pre-computed. The 8-way writeback mux now contains no carry chains, and the previous near-critical cluster in the 16–20 ns slack range has been relieved.

---

## Critical Path #4 — Global Buffer Saturation (Routing Congestion)

**Clock domain:** `pll_clk_global`  
**Affected signals:** All high-fanout control signals not on global routing  
**RTL modules involved:** All modules, most severely `cpu.sv`, `top.sv`

### Path Narrative

All 8 iCE40-HX8K global buffer resources (SB_GB) are fully utilized at 100%. These buffers feed global routing tracks that span the entire FPGA fabric with low, uniform delay. They are ideal for clocks, resets, and high-fanout control signals (typically signals with fanout > 100).

The existing 8 global buffers are consumed by:
- 1× system clock (`pll_clk_global`)
- 1× UART raw input clock (`clk$SB_IO_IN`)
- At least 2× reset signals (e.g., `rst_n_out` with ~1,206 fanout, per previous analysis)
- At least 4× high-fanout CPU control signals (e.g., `a_reg_write` ~143 fanout, `instr_complete_internal` ~64 fanout, and 2 others)

With all global buffers consumed, nextpnr must route **any new high-fanout signal through local span-4 and span-12 routing tracks**. This causes:

1. **Longer routing delays** for any new enable or reset signals reaching many LCs
2. **Increased routing congestion** in the center of the chip where global tracks otherwise provide bypass
3. **Higher variability in timing** between different endpoints driven by the same signal
4. **A hard ceiling** on the number of distinct clock domains (only 2 usable currently)

As the design grows (adding new instructions, peripherals, or extensions), more signals will exceed the ~100-fanout threshold and require global routing that is no longer available.

### Why This Is a Timing Concern

While global buffer saturation does not directly appear as a timing violation, it creates a "hidden tax" on every high-fanout signal added to the design. The routing congestion forces nextpnr to place related logic farther apart, increasing routing delays across the board. The 12.2 ns routing component in Critical Path #1 is partly attributable to congested placement caused by high-fanout signal distribution without global buffers. In particular, the 3.06 ns cross-fabric net from the carry-chain output to the result-format mux reflects this congestion.

---

## Resource Constraint: BRAM Saturation (93%)

**Not a timing path per se, but directly enables/blocks timing optimizations**  
**RTL modules involved:** `sram_peripheral.sv`, `sram.sv`, `regfile.sv`

### Current BRAM Allocation

| Consumer | BRAM Blocks | Capacity | Notes |
|----------|-------------|----------|-------|
| 12KB SRAM Peripheral (`sram_peripheral.sv`) | ~24 | 3072 × 32-bit words | Largest consumer |
| Integer Register File (`regfile.sv` via `sync_dpram.sv`) | 4 | 32 × 32-bit (dual-banked) | 2 banks × 2 BRAMs |
| FP Register File (`fp_regfile.sv`) | 0 | N/A | Disabled (ENABLE_F_EXT=0) |
| **Total Used** | **30** | — | **93% of 32 available** |
| **Remaining** | **2** | — | Effectively no headroom |

The 12KB SRAM peripheral uses approximately 24 of the 30 BRAM blocks — 80% of all BRAM usage. This allocation directly prevents several architectural improvements that would reduce timing pressure:

- **FP register file** (needed for F extension): ~26 BRAM blocks required — impossible with 2 remaining
- **Instruction cache** (would reduce memory latency by eliminating external DRAM round-trips): 16+ BRAM blocks needed
- **Larger UART FIFOs** (for higher-throughput host communication): 1–2 BRAM blocks each
- **Any data buffer or intermediate storage**: No BRAM budget remaining

---

## Summary Table: Top Timing Challenges

| Rank | Challenge | Path Delay | Status | Root Cause | Modules Involved |
|------|-----------|-----------|--------|------------|-----------------|
| 1 | ALU B-Input Decode + 32-bit Ripple-Carry + Post-Carry Mux | 21.0 ns total (4.9 ns carry + 9.2 ns post-carry mux) | ⚠️ Active | Late carry-chain input arrival + deep result-select mux after carry chain | `cpu.sv`, `alu.sv` |
| 2 | Cross-Fabric Routing (58% of critical path) | 12.2 ns routing | ⚠️ Active | Path crosses fabric diagonally; 71% LC utilization limits co-placement of ALU and carry chain | `cpu.sv`, `alu.sv` |
| 3 | Post-Carry ALU Result Mux Depth (near-critical) | ~19–21 ns | ⚠️ Active | 5 LUT levels for multi-operation result selection after carry chain | `alu.sv` |
| 4 | FSM Decode Routing | ~2.4 ns (preamble of CP#1) | ⚠️ Active | `jump_reg`/`is_lr_reg` routed far from ALU due to LC congestion | `cpu.sv` |
| 5 | ~~Seven-Segment Modulo Arithmetic~~ | ~~24.37 ns (35 levels)~~ | ✅ **RESOLVED** | Replaced `% 8'd6` with registered rollover counter | `ice40_alchitry_cu_top.sv` |
| 6 | ~~Writeback Mux Adders~~ | ~~~19–21 ns~~ | ✅ **RESOLVED** | Pre-computed AUIPC/JAL results in EXECUTE state; mux now uses registered `alu_result` | `writeback_mux.sv`, `cpu.sv` |
| 7 | Global Buffer Saturation | N/A (resource limit) | ⚠️ Active | All 8 SB_GB consumed; new high-fanout signals must use congested local routing | `top.sv`, system-wide |
| 8 | BRAM Near Capacity | N/A (resource limit) | ⚠️ Active | 12KB SRAM uses ~24/30 BRAM blocks; blocks timing optimizations requiring BRAM | `sram_peripheral.sv`, `sram.sv` |

---

## Suggestions for Addressing Timing Challenges

The following suggestions are ordered within groups from lowest to highest implementation effort. All estimates assume the current iCE40-HX8K target and Yosys/nextpnr toolchain. Suggestions 1 and 4 from the previous analysis have already been implemented.

---

### Tier 1: Low Effort (Days)

#### ~~Suggestion 1 — Replace Modulo-6 with a Registered Rollover Counter~~ (IMPLEMENTED ✅)

**Addresses:** Former Critical Path #2 (Seven-Segment Display Modulo Arithmetic)  
**Result:** Eliminated the 35-level, 24.37 ns icetime critical path. Replaced with a 3-bit registered rollover counter (`seg_position_reg`) that increments on button press or LED output change. The icetime worst-case path is now the ALU carry chain at 21.25 ns.

---

#### Suggestion 2 — Reduce ALU Result Mux Depth via Operation Grouping

**Addresses:** Critical Path #1 post-carry segment (5 LUT levels, ~9.2 ns), Critical Path #2 (near-critical cluster)  
**Expected improvement:** ~2–3 ns reduction in post-carry delay; could push Fmax above 52 MHz  
**Risk:** Low-moderate — requires ALU restructuring but no FSM changes  
**Files:** `rtl/common/cpu/alu.sv`

The post-carry result mux in `alu.sv` selects among ADD/SUB/SLT/SLTU/AND/OR/XOR/SLL/SRL/SRA results in a flat priority-encoded tree. With 10+ operation outputs, this generates 4–5 LUT levels after the carry chain — more expensive than the carry chain itself (4.9 ns) in terms of absolute path contribution.

Restructuring the mux into two-level groups can reduce depth:

```systemverilog
// Group 1: arithmetic results (require carry chain)
logic [31:0] arith_result;
always_comb begin
    case (alu_op)
        ALU_ADD, ALU_SUB: arith_result = sum;    // SB_CARRY output
        ALU_SLT:          arith_result = {31'b0, $signed(a) < $signed(b)};
        ALU_SLTU:         arith_result = {31'b0, a < b};
        default:          arith_result = '0;
    endcase
end

// Group 2: bitwise results (no carry)
logic [31:0] bitwise_result;
always_comb begin
    case (alu_op)
        ALU_AND: bitwise_result = a & b;
        ALU_OR:  bitwise_result = a | b;
        ALU_XOR: bitwise_result = a ^ b;
        default: bitwise_result = '0;
    endcase
end

// Final selection (1 LUT level)
assign result = (is_arith_op) ? arith_result : 
                (is_shift_op) ? shift_result : bitwise_result;
```

This restructures the flat 10-way mux into a 2-level hierarchy. The critical path from the carry chain only needs to traverse 1 additional mux level instead of the current 5, reducing post-carry LUT depth from ~9.2 ns to ~3–4 ns.

---

### Tier 2: Medium Effort (Weeks)

#### Suggestion 3 — Carry-Select Adder for ALU Addition/Subtraction

**Addresses:** Critical Path #1 (32-bit ALU Ripple-Carry Chain, 4.94 ns)  
**Expected improvement:** ~2.0–2.5 ns reduction in carry chain delay (~40–50% improvement)  
**Risk:** Moderate — requires carefully preserving ALU result correctness for all operations (ADD, SUB, SLT, SLTU)  
**Files:** `rtl/common/cpu/alu.sv`

The iCE40's SB_CARRY chain is extremely efficient for sequential carry propagation within a tile (0.126 ns/stage). The issue is that 30 carry cells across 4 tile rows create a ~4.9 ns chain that must complete before the post-carry mux can begin.

A **carry-select adder** partitions the 32-bit adder into N groups (e.g., 4 × 8-bit groups). Each group computes two sums simultaneously — one assuming carry-in = 0, one assuming carry-in = 1 — and selects the correct result once the actual carry-in propagates from the previous group. This approximately halves the critical carry chain length:

- **Current:** 1 × 30-stage ripple carry = ~4.9 ns (30 cells across 4 tiles)
- **After:** 4 × 8-bit groups + 3 mux levels ≈ ~8-stage carry + ~3 mux levels ≈ ~2.0–2.5 ns

Implementation involves writing `alu.sv` to instantiate explicit partial-sum modules rather than relying on the synthesizer's automatic `+` operator mapping. The trade-off is ~50–100 additional LUTs for the parallel partial-sum computation and carry-select muxes. Combined with Suggestion 2 (result mux grouping), the combined improvement could push Fmax toward 55–60 MHz.

---

#### ~~Suggestion 4 — Pre-Compute Writeback Mux Adders in the EXECUTE Stage~~ (IMPLEMENTED ✅)

**Addresses:** Former Critical Path #4 (Writeback Mux Fan-In, ~19–21 ns near-critical paths)  
**Result:** Implemented by updating `writeback_mux.sv` to reuse the already-registered `alu_result` for AUIPC and JAL/JALR writeback. The inline `pc + imm_u` and `pc + 32'd4` adders are removed. Improved routed Fmax from **42.09 MHz → 47.70 MHz** (+5.61 MHz, **+13.3%**).

---

### Tier 3: High Effort (Months — Architecture-Level)

#### Suggestion 5 — SRAM Peripheral Downsizing and Freed BRAM Allocation

**Addresses:** BRAM Saturation (93%), Global Buffer Saturation (100%), enables future architectural improvements  
**Expected improvement:** Unlocks 16–22 BRAM blocks for FP register file, instruction cache, or larger FIFOs; reduces routing congestion by enabling better placement  
**Risk:** High — requires co-design with the Rust testbench (memory map changes) and careful validation that all existing tests still pass  
**Files:** `rtl/common/peripherals/sram_peripheral.sv`, `rtl/common/memory/sram.sv`, `rtl/common/top.sv`, corresponding Rust tests

The 12KB SRAM peripheral (`DEPTH = 3072` words) consumes approximately 24 of the 30 available BRAM blocks, leaving only 2 free. This prevents three major timing/performance improvements:

1. **FP Register File (F extension):** Requires ~26 BRAM blocks for 32×32-bit dual-banked FP file — impossible with the current BRAM budget.

2. **Instruction Cache:** Even a minimal 512-entry direct-mapped instruction cache (2KB) would require 4 BRAM blocks and could reduce the effective memory latency for instruction fetches, improving throughput by 20–30% for cache-hitting workloads.

3. **Reduced LUT Pressure:** Some of the 5,520 LCs are used by the bus routing logic for the SRAM peripheral. Reducing the SRAM footprint slightly reduces LUT pressure, giving nextpnr more placement flexibility and potentially improving routing quality (reducing the 12.2 ns routing component of Critical Path #1).

**Implementation path:**

- Reduce SRAM `DEPTH` from 3072 to 1024 (4KB) or 2048 (8KB)
- This frees 16 or 8 BRAM blocks respectively
- Use freed BRAMs to enable the F extension (`ENABLE_F_EXT=1`) and its BRAM-based FP register file
- Alternatively, implement a small instruction prefetch buffer (4 BRAMs = 2KB) to reduce the effective FETCH cycle count for hot loops

This is an architecture-level decision requiring agreement on the memory map, validation of the host-side Rust test infrastructure, and re-tuning of the `fpga_common_top` parameters for the iCE40 target.

---

## Appendix: Full Synthesis Timing Summary

### nextpnr Timing Summary (Post Place-and-Route)

> **Note on signal names:** `clk$SB_IO_IN` is a synthesizer-generated clock name automatically assigned by nextpnr to the UART RX input pin's raw clock domain. The `$` character is a Yosys/nextpnr internal naming convention and does not correspond to any source RTL signal. In the source RTL it maps to the direct input of the `SB_IO` primitive used for `usb_rx`.

```
Max frequency for clock 'pll_clk_global': 47.70 MHz (PASS at 25.00 MHz)
Max frequency for clock   'clk$SB_IO_IN': 626.57 MHz (PASS at 25.00 MHz)

Cross-domain path delays:
  <async> -> posedge clk$SB_IO_IN  : 1.06 ns max
  <async> -> posedge pll_clk_global : 4.01 ns max
  posedge pll_clk_global -> <async> : 5.36 ns max
```

The `pll_clk_global → <async>` path (5.36 ns) originates from `seg_position_reg` propagating through the `io_seg` combinational decode to the IO output pad — the registered rollover counter now makes this path trivially short.

### Critical Path Logic/Routing Breakdown (nextpnr)

```
Critical path for 'pll_clk_global' (posedge -> posedge):
  Source: is_lr_SB_LUT4_O_LC.O  (cpu_core, cpu.sv)
  Dest:   alu_out_reg_SB_DFFESR_Q_4_DFFLC.I0  (alu.sv:15)
  Logic:   8.8 ns
  Routing: 12.2 ns
  Total:  21.0 ns
```

### icetime Timing Summary (ASC-Level Static Timing)

```
Total number of logic levels: 42
Total path delay: 21.25 ns (47.05 MHz)

Critical path (icetime):
  Source: jump_reg (cpu_core, cpu.sv) @ tile (18,8)
  → ALU B mux (2 LUT levels, tile 22,21 area)
  → alu_b[2] → 29-stage SB_CARRY chain (tiles 20,18–20,21)
  → post-carry result mux (5 LUT levels, tiles 20,22 and 18,26–17,32)
  → alu_result[27] → cpu_to_arb_a_wdata[27] register @ tile (17,31)
  Total: 21.253 ns
```

### Slack Histogram (pll_clk_global, 25 MHz target = 40 ns period)

The slack histogram below shows the distribution of timing endpoints relative to the 40 ns target period. The tightest endpoints now have ~19 ns of slack (endpoints at 21 ns total delay), compared to ~15.7 ns slack in the previous analysis. The overall distribution shifted right by ~3.4 ns, reflecting the Fmax improvement from 42.09 MHz → 47.70 MHz.

```
Slack range (ps)    Endpoint count (legend: * = 29 endpoints, + = [1,29))
[ 19034,  20030)    |*+
[ 20030,  21026)    |+
[ 21026,  22022)    |+
[ 22022,  23018)    |+
[ 23018,  24014)    |*+
[ 24014,  25010)    |*+
[ 25010,  26006)    |*+
[ 26006,  27002)    |+
[ 27002,  27998)    |*+
[ 27998,  28994)    |*********+
[ 28994,  29990)    |************+
[ 29990,  30986)    |****************+
[ 30986,  31982)    |**************************+
[ 31982,  32978)    |**************************************+
[ 32978,  33974)    |************************************************************ (most endpoints)
[ 33974,  34970)    |*************+
[ 34970,  35966)    |****************+
[ 35966,  36962)    |*******************************+
[ 36962,  37958)    |***********************************************+
[ 37958,  38954)    |************************************+
```

### Comparison: Previous vs. Current Analysis

| Metric | Previous | Current | Change |
|--------|---------|---------|--------|
| Fmax (nextpnr) | 42.09 MHz | **47.70 MHz** | +5.61 MHz (+13.3%) |
| Fmax (icetime) | 41.04 MHz | **47.05 MHz** | +6.01 MHz (+14.6%) |
| Critical path delay | 23.8 ns | **21.0 ns** | −2.8 ns (−11.8%) |
| Critical path routing | 14.6 ns (61%) | **12.2 ns (58%)** | −2.4 ns |
| ICESTORM_LC count | 5,646 (73%) | **5,520 (71%)** | −126 LCs (−2.2%) |
| SB_LUT4 count | 4,560 | **4,462** | −98 |
| SB_CARRY count | 898 | **814** | −84 |
| Tightest slack (pll_clk_global) | ~15.7 ns | **~19.0 ns** | +3.3 ns |

---

*Analysis generated from synthesis runs using Yosys 0.33 and nextpnr-ice40, targeting the iCE40-HX8K-CB132 device.*
