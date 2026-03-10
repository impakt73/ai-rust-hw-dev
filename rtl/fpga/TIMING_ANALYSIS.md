# iCE40-HX8K Critical Path & Timing Analysis

**Date:** 2026-03-10 (updated)  
**Target Device:** Lattice iCE40-HX8K-CB132 (Alchitry Cu v1)  
**Synthesis Tools:** Yosys 0.33, nextpnr-ice40, icetime  
**Design Configuration:** RV32I CPU (M/F extensions disabled for iCE40 resources), dual-banked BRAM register file

---

## Executive Summary

The RISC-V RV32I CPU design still meets the 25 MHz target frequency with substantial headroom. After implementing **Suggestion 2 (ALU result grouping)**, the latest iCE40-HX8K build reaches **47.30 MHz** (nextpnr) / **47.43 MHz** (icetime), leaving roughly **89–90% timing margin** over the 25 MHz target (21.08 ns critical path vs. 40.0 ns clock budget).

Since the previous analysis, three optimizations have now been implemented and verified:

1. **Writeback Mux Adder Pre-computation** (former Critical Path #4): AUIPC and JAL/JALR return-address additions are now performed during the EXECUTE FSM state and stored into `alu_out_reg`. The `writeback_mux.sv` now selects the pre-computed `alu_result` for those cases, removing the inline carry chains from the writeback mux entirely.

2. **Seven-Segment Modulo-6 Replacement** (former Critical Path #2): The `button_counter % 8'd6` expression in `ice40_alchitry_cu_top.sv` has been replaced with a registered rollover counter (`seg_position_reg`). The 24.37 ns combinational modulo-division chain is eliminated.

3. **ALU Result Operation Grouping** (former Suggestion 2): `rtl/common/cpu/alu.sv` now computes grouped `arith_result`, `bitwise_result`, `shift_result`, `minmax_result`, and `muldiv_result` values before the final result select. This removes the previous flat 10-way arithmetic/logic/shift result tree from the hottest ALU cases and slightly reduces overall logic usage.

Measured full-chip impact of the ALU grouping change on the iCE40 target:

- **Logic cells:** 5,520 → **5,472** (**−48 LCs**, −0.9%)
- **SB_LUT4 cells:** 4,462 → **4,437** (**−25 LUT4s**)
- **Fmax (nextpnr):** 47.70 MHz → **47.30 MHz** (**−0.40 MHz**, −0.8%)
- **Fmax (icetime):** 47.05 MHz → **47.43 MHz** (**+0.38 MHz**, +0.8%)

The grouped ALU structure therefore produced a **small utilization reduction** and a **timing-neutral overall result**. The previous flat post-carry mux is no longer the only obvious hotspot, but the new dominant path still lives in the ALU: it now runs through the **A-extension MIN/MAX compare/select logic plus residual result routing**.

The design still faces two impending **resource saturation** constraints that will limit future development:

- **BRAM utilization: 30/32 blocks (93%)** — only 2 blocks of headroom remain
- **Global buffer utilization: 8/8 (100%)** — all global routing resources exhausted

### Key Metrics

| Metric | Value | Available | Utilization |
|--------|-------|-----------|-------------|
| **Logic Cells (ICESTORM_LC)** | 5,472 | 7,680 | **71%** |
| **Block RAM (ICESTORM_RAM)** | 30 | 32 | **93% ⚠️** |
| **I/O Pins (SB_IO)** | 77 | 256 | 30% |
| **Global Buffers (SB_GB)** | 8 | 8 | **100% ⚠️** |
| **PLLs (ICESTORM_PLL)** | 1 | 2 | 50% |
| **Achieved Fmax (nextpnr)** | 47.30 MHz | 25 MHz target | **PASS (+89%)** |
| **Achieved Fmax (icetime)** | 47.43 MHz | 25 MHz target | **PASS (+90%)** |
| **Critical Path Delay** | 21.08 ns | 40.0 ns budget | PASS |

### Cell Type Breakdown (from Yosys)

| Cell Type | Count | Change vs. Previous | Description |
|-----------|-------|---------------------|-------------|
| SB_LUT4 | 4,437 | −25 vs. previous analysis | 4-input Look-Up Tables |
| SB_CARRY | 814 | 0 vs. previous analysis | Carry chain cells (arithmetic / compare) |
| SB_DFFESR | 1,576 | −31 vs. previous analysis | D flip-flop with enable and set/reset |
| SB_DFF variants | 635 | −10 vs. previous analysis | Various D flip-flop configurations |
| SB_RAM40_4K | 30 | 0 | 4Kbit Block RAM instances |
| SB_PLL40_CORE | 1 | 0 | PLL |

### Optimization History

| Date | Optimization | Fmax Before | Fmax After | Improvement |
|------|-------------|-------------|------------|-------------|
| 2026-03-10 | Pre-compute AUIPC/JAL adders in EXECUTE; remove inline adders from `writeback_mux.sv` | 42.09 MHz | 47.70 MHz | +5.61 MHz (+13.3%) |
| 2026-03-10 | Replace `button_counter % 8'd6` modulo with registered rollover counter in `ice40_alchitry_cu_top.sv` | 41.04 MHz (icetime worst case) | 47.05 MHz (icetime) | +6.01 MHz (+14.6%) |
| 2026-03-10 | Group ALU result selection into arithmetic / bitwise / shift / minmax / muldiv buckets in `alu.sv` | 47.70 MHz (nextpnr), 47.05 MHz (icetime) | 47.30 MHz (nextpnr), 47.43 MHz (icetime) | −0.40 MHz routed, +0.38 MHz icetime |

---

## Critical Path #1 — Atomic MIN/MAX Compare/Select + 32-bit Carry/Compare Chain + Residual Result Routing (Primary)

**Clock domain:** `pll_clk_global` (posedge → posedge)  
**Total delay:** 21.08 ns (9.1 ns logic + 12.1 ns routing) — nextpnr; 21.08 ns — icetime  
**Achieved Fmax:** 47.30 MHz (nextpnr), 47.43 MHz (icetime)  
**Logic levels (icetime):** 43  
**RTL modules involved:** `cpu.sv`, `alu.sv`

### Path Narrative

This is the dominant registered-clock critical path identified by both nextpnr and icetime after implementing ALU result grouping. It launches from the registered atomic decode flag **`is_lr_reg`**, passes through the ALU-side decode/select logic, traverses a full-width compare/carry chain generated from the MIN/MAX compare logic in `alu.sv`, and then continues through residual result-selection/routing LUTs before terminating at `alu_out_reg` / `cpu_to_arb_a_wdata`.

The important change versus the previous analysis is qualitative: the hot path is no longer best described as a flat 10-way ADD/SUB/logic/shift result mux. After grouping, the worst endpoint is now associated with the **`minmax_result` compare/select cone** (`alu.sv` MIN/MAX/MINU/MAXU group), which still maps onto a long carry/comparator structure and several downstream LUT levels.

### Detailed Stage Breakdown

| Stage | Start (ns) | End (ns) | Δ (ns) | Type | Description |
|-------|-----------|---------|-------|------|-------------|
| DFF output (`is_lr_reg`) | 0.00 | 0.64 | 0.64 | Logic | Registered atomic decode flag launch |
| Atomic decode routing + early select LUTs | 0.64 | 3.99 | 3.35 | **Routing + Logic** | `is_lr_reg` fanout into the grouped ALU result network |
| Compare/carry input setup | 3.99 | 5.77 | 1.78 | Logic + Routing | Comparator inputs arrive at the carry chain |
| **30-stage compare/carry chain** | 5.77 | 10.96 | **5.20** | Logic | Full-width comparator/carry propagation across four tile rows |
| Carry-out handoff + bit-31 formatting | 10.96 | 12.77 | 1.81 | Logic + Routing | Carry-out export and high-bit formatting |
| Residual grouped-result routing / select LUTs | 12.77 | 20.68 | 7.91 | Logic + Routing | `minmax_result`/`alu_result` selection and routing into the destination register |
| Setup at destination DFF | 20.68 | 21.08 | 0.40 | Setup | Register setup time |

**Path summary:**
```
is_lr_reg[DFF] (cpu.sv)
  → grouped ALU decode/select LUTs
  → MIN/MAX compare input conditioning
  → 30-stage compare/carry chain (alu.sv)
  → carry-out / bit-31 handoff
  → residual `alu_result` selection + routing
  → alu_out_reg / cpu_to_arb_a_wdata[DFF]
```

### Why This Path Is Slow

1. **Late compare/carry input arrival:** The carry/comparator chain still does not start until nearly 5.8 ns into the path because `is_lr_reg` must route into the grouped ALU select logic first. Grouping removed mux fan-in on many operations, but it did not move the atomic decode registers physically closer to the ALU carry fabric.

2. **MIN/MAX compare still needs a full-width carry structure:** The new worst endpoint is tied to `minmax_result` generation in `alu.sv`. Signed/unsigned MIN/MAX operations still require a 32-bit compare, so the path retains a long carry/comparator chain even though ADD/SUB/logic/shift selection is now grouped.

3. **Residual post-compare LUT depth remains material:** After the carry chain finishes, the result still traverses several LUT levels for final result shaping and destination routing. This network is shallower and lower-fanout than the previous flat mux, but it is still large enough to keep the overall path above 21 ns.

4. **Routing remains dominant:** 12.1 ns of the 21.08 ns total (~57%) is routing. The grouped RTL reduced LUT count, but the placer still spreads decode, compare, and writeback resources across the fabric because the design is already at 71% LC utilization and 100% global-buffer utilization.

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

The `pll_clk_global → <async>` output path via `seg_position_reg` is now only **4.19 ns** (max delay from nextpnr), well within the 40 ns clock budget. The 35-level combinational division chain is completely gone.

---

## Critical Path #2 — Residual ALU Result Routing / Select Depth (Near-Critical)

**Clock domain:** `pll_clk_global` (posedge → posedge)  
**Characterized delay:** ~19–21 ns (multiple endpoints in tight slack range)  
**RTL modules involved:** `alu.sv`, `cpu.sv`

### Path Narrative

The slack histogram still shows a cluster of endpoints in the 19–21 ns range, indicating several near-critical paths in addition to the primary 21.08 ns path. After operation grouping, these paths no longer come from one flat 10-way ALU result tree; instead, they share the **residual grouped-result routing and select structure** described in Critical Path #1.

The primary contributor is now the tail of the grouped ALU output network inside `alu.sv`, especially the MIN/MAX compare/select bucket and the downstream `result` assignment that merges the grouped results. The depth is lower than the previous flat mux, but multiple bits of `alu_result` still travel through 4–5 LUT levels once placement and routing are included.

The near-critical endpoint cluster corresponds to paths that reach the same result-format LUT tree via different carry-chain bits (different ALU result bits) or different source operands. Each bit of `alu_result` may travel a slightly different routing path, causing the cluster to span ~2 ns in the histogram.

### Why This Path Persists

Unlike the previous near-critical paths (which were caused by inline adders in `writeback_mux.sv`, now removed), this cluster arises from the remaining ALU result-tail logic after grouping. Further reduction likely requires either (a) shortening the compare/carry path itself, or (b) further specializing the atomic MIN/MAX datapath so that its compare/select network does not feed the same wide `result` merge structure.

---

## Critical Path #3 — FSM Instruction Decode Depth (Contributing)

**Clock domain:** `pll_clk_global`  
**Characterized delay:** ~3.0–6.0 ns (early preamble of Critical Path #1)  
**RTL modules involved:** `cpu.sv`, `decoder.sv`

### Path Narrative

The CPU's multi-cycle FSM stores decoded control flags (`jump_reg`, `is_lr_reg`, `is_auipc_reg`, etc.) as registered signals latched at the end of the DECODE state. These registered flags are the launch flip-flops for Critical Path #1. In the current build, the worst endpoint launches from `is_lr_reg` and still spends ~2–3 ns crossing the fabric before reaching the first ALU-side select logic, reflecting the same placement pressure between the decoder/register file region and the ALU carry fabric.

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

While global buffer saturation does not directly appear as a timing violation, it creates a "hidden tax" on every high-fanout signal added to the design. The routing congestion forces nextpnr to place related logic farther apart, increasing routing delays across the board. The 12.1 ns routing component in Critical Path #1 is partly attributable to congested placement caused by high-fanout signal distribution without global buffers. In particular, the long decode-to-ALU and ALU-to-writeback hops in the grouped MIN/MAX path reflect this congestion.

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
| 1 | Atomic MIN/MAX Compare/Select + 32-bit Carry Chain + Result Routing | 21.08 ns total (5.20 ns compare/carry + 7.91 ns residual result tail) | ⚠️ Active | Registered atomic decode fanout feeds a full-width compare chain, followed by residual grouped-result routing | `cpu.sv`, `alu.sv` |
| 2 | Cross-Fabric Routing (~57% of critical path) | 12.1 ns routing | ⚠️ Active | Path still crosses decode, ALU, and writeback regions under 71% LC utilization | `cpu.sv`, `alu.sv` |
| 3 | Residual ALU Result Select Depth (near-critical) | ~19–21 ns | ⚠️ Active | Grouping removed the flat mux, but MIN/MAX compare/select and final result merge still cost 4–5 LUT levels when routed | `alu.sv` |
| 4 | FSM Decode Routing | ~2.4 ns (preamble of CP#1) | ⚠️ Active | `jump_reg`/`is_lr_reg` routed far from ALU due to LC congestion | `cpu.sv` |
| 5 | ~~Seven-Segment Modulo Arithmetic~~ | ~~24.37 ns (35 levels)~~ | ✅ **RESOLVED** | Replaced `% 8'd6` with registered rollover counter | `ice40_alchitry_cu_top.sv` |
| 6 | ~~Writeback Mux Adders~~ | ~~~19–21 ns~~ | ✅ **RESOLVED** | Pre-computed AUIPC/JAL results in EXECUTE state; mux now uses registered `alu_result` | `writeback_mux.sv`, `cpu.sv` |
| 7 | Global Buffer Saturation | N/A (resource limit) | ⚠️ Active | All 8 SB_GB consumed; new high-fanout signals must use congested local routing | `top.sv`, system-wide |
| 8 | BRAM Near Capacity | N/A (resource limit) | ⚠️ Active | 12KB SRAM uses ~24/30 BRAM blocks; blocks timing optimizations requiring BRAM | `sram_peripheral.sv`, `sram.sv` |

---

## Suggestions for Addressing Timing Challenges

The following suggestions are ordered within groups from lowest to highest implementation effort. All estimates assume the current iCE40-HX8K target and Yosys/nextpnr toolchain. Suggestions 1, 2, and 4 from the previous analysis have now been implemented.

---

### Tier 1: Low Effort (Days)

#### ~~Suggestion 1 — Replace Modulo-6 with a Registered Rollover Counter~~ (IMPLEMENTED ✅)

**Addresses:** Former Critical Path #2 (Seven-Segment Display Modulo Arithmetic)  
**Result:** Eliminated the 35-level, 24.37 ns icetime critical path. Replaced with a 3-bit registered rollover counter (`seg_position_reg`) that increments on button press or LED output change. The synchronous critical path remains in the ALU/writeback network and is now measured at 21.08 ns by icetime.

---

#### ~~Suggestion 2 — Reduce ALU Result Mux Depth via Operation Grouping~~ (IMPLEMENTED ✅)

**Addresses:** Critical Path #1 post-carry segment (5 LUT levels, ~9.2 ns), Critical Path #2 (near-critical cluster)  
**Expected improvement:** ~2–3 ns reduction in post-carry delay; could push Fmax above 52 MHz  
**Risk:** Low-moderate — requires ALU restructuring but no FSM changes  
**Files:** `rtl/common/cpu/alu.sv`

**Measured result:** Implemented in `rtl/common/cpu/alu.sv` by splitting ALU outputs into `arith_result`, `bitwise_result`, `shift_result`, `minmax_result`, and `muldiv_result` groups before the final `result` merge. Full-chip iCE40 utilization dropped from **5,520 → 5,472 LCs** and **4,462 → 4,437 LUT4s**. Timing was effectively flat overall: **47.70 → 47.30 MHz** in nextpnr and **47.05 → 47.43 MHz** in icetime.

The original post-carry result mux in `alu.sv` selected among ADD/SUB/SLT/SLTU/AND/OR/XOR/SLL/SRL/SRA results in a flat priority-encoded tree. With 10+ operation outputs, that generated 4–5 LUT levels after the carry chain — more expensive than the carry chain itself in absolute path contribution.

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

The implementation achieved the intended structural cleanup, but the new measured hotspot moved into the **atomic MIN/MAX compare/select bucket** rather than disappearing entirely. In other words, grouping removed the flat arithmetic/logic/shift mux as the clearest bottleneck, but the iCE40 placer still maps the MIN/MAX compare and result-tail logic onto a similarly long routed path. A further timing win now likely requires reducing the carry/comparator depth itself (Suggestion 3) or isolating the atomic MIN/MAX result path more aggressively.

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

3. **Reduced LUT Pressure:** Some of the 5,472 LCs are used by the bus routing logic for the SRAM peripheral. Reducing the SRAM footprint slightly reduces LUT pressure, giving nextpnr more placement flexibility and potentially improving routing quality (reducing the 12.1 ns routing component of Critical Path #1).

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
Max frequency for clock 'pll_clk_global': 47.30 MHz (PASS at 25.00 MHz)
Max frequency for clock   'clk$SB_IO_IN': 626.57 MHz (PASS at 25.00 MHz)

Cross-domain path delays:
  <async> -> posedge clk$SB_IO_IN  : 3.58 ns max
  <async> -> posedge pll_clk_global : 3.22 ns max
  posedge pll_clk_global -> <async> : 4.19 ns max
```

The `pll_clk_global → <async>` path remains comfortably short; the registered rollover-counter change keeps the seven-segment output logic well away from the synchronous critical path.

### Critical Path Logic/Routing Breakdown (nextpnr)

```
Critical path for 'pll_clk_global' (posedge -> posedge):
  Source: is_lr_SB_LUT4_O_LC.O  (cpu_core, cpu.sv)
  Dest:   alu_out_reg_SB_DFFESR_Q_2_DFFLC.I0  (cpu.sv / alu.sv result path)
  Logic:   9.1 ns
  Routing: 12.1 ns
  Total:  21.1 ns
```

### icetime Timing Summary (ASC-Level Static Timing)

```
Total number of logic levels: 43
Total path delay: 21.08 ns (47.43 MHz)

Critical path (icetime):
  Source: is_lr_reg (cpu_core, cpu.sv) @ tile (19,9)
  → grouped ALU decode/select logic
  → MIN/MAX compare/carry chain (tiles 20,18–20,22)
  → residual grouped-result routing/select LUTs
  → alu_result[25] → cpu_to_arb_a_wdata[25] register
  Total: 21.084 ns
```

### Slack Histogram (pll_clk_global, 25 MHz target = 40 ns period)

The slack histogram below shows the distribution of timing endpoints relative to the 40 ns target period. The tightest endpoints are now just under **18.9 ns** of slack (endpoints at ~21.1 ns total delay). Compared to the previous analysis state, the histogram is effectively flat overall, matching the timing-neutral nature of the ALU grouping change.

```
Slack range (ps)    Endpoint count (legend: * = 29 endpoints, + = [1,29))
[ 18860,  19865)    |*+
[ 19865,  20870)    |*+
[ 20870,  21875)    |+
[ 21875,  22880)    |+
[ 22880,  23885)    |*+
[ 23885,  24890)    |*+
[ 24890,  25895)    |**+
[ 25895,  26900)    |*+
[ 26900,  27905)    |***+
[ 27905,  28910)    |*****+
[ 28910,  29915)    |********************+
[ 29915,  30920)    |************************+
[ 30920,  31925)    |***********************+
[ 31925,  32930)    |************************************+
[ 32930,  33935)    |****************+
[ 33935,  34940)    |************************************************************ (most endpoints)
[ 34940,  35945)    |*****************+
[ 35945,  36950)    |********************************+
[ 36950,  37955)    |*******************************************************+
[ 37955,  38960)    |******************************************+
```

### Comparison: Previous vs. Current Analysis

| Metric | Previous | Current | Change |
|--------|---------|---------|--------|
| Fmax (nextpnr) | 47.70 MHz | **47.30 MHz** | −0.40 MHz (−0.8%) |
| Fmax (icetime) | 47.05 MHz | **47.43 MHz** | +0.38 MHz (+0.8%) |
| Critical path delay | 21.0 ns | **21.08 ns** | +0.08 ns (+0.4%) |
| Critical path routing | 12.2 ns (58%) | **12.1 ns (57%)** | −0.1 ns |
| ICESTORM_LC count | 5,520 (71%) | **5,472 (71%)** | −48 LCs (−0.9%) |
| SB_LUT4 count | 4,462 | **4,437** | −25 |
| SB_CARRY count | 814 | **814** | 0 |
| Tightest slack (pll_clk_global) | ~19.0 ns | **~18.9 ns** | ~flat |

---

*Analysis generated from synthesis runs using Yosys 0.33 and nextpnr-ice40, targeting the iCE40-HX8K-CB132 device.*
