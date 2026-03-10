# iCE40-HX8K Critical Path & Timing Analysis

**Date:** 2026-03-10 (updated)  
**Target Device:** Lattice iCE40-HX8K-CB132 (Alchitry Cu v1)  
**Synthesis Tools:** Yosys 0.33, nextpnr-ice40, icetime  
**Design Configuration:** RV32I CPU (M/F extensions disabled for iCE40 resources), dual-banked BRAM register file

---

## Executive Summary

The RISC-V RV32I CPU design still meets the 25 MHz target frequency with substantial headroom. After implementing a **two-cycle staged atomic MIN/MAX datapath** in `rtl/common/cpu/alu.sv` and reusing the existing `alu_ready` / memory ready-valid handshake in `cpu.sv`, the latest iCE40-HX8K build reaches **55.20 MHz** (nextpnr) / **53.50 MHz** (icetime), leaving roughly **114–121% timing margin** over the 25 MHz target (18.12–18.69 ns critical path vs. 40.0 ns clock budget).

Since the previous analysis, four major optimizations have now been implemented and verified:

1. **Writeback Mux Adder Pre-computation** (former Critical Path #4): AUIPC and JAL/JALR return-address additions are now performed during the EXECUTE FSM state and stored into `alu_out_reg`. The `writeback_mux.sv` now selects the pre-computed `alu_result` for those cases, removing the inline carry chains from the writeback mux entirely.

2. **Seven-Segment Modulo-6 Replacement** (former Critical Path #2): The `button_counter % 8'd6` expression in `ice40_alchitry_cu_top.sv` has been replaced with a registered rollover counter (`seg_position_reg`). The 24.37 ns combinational modulo-division chain is eliminated.

3. **ALU Result Operation Grouping** (former Suggestion 2): `rtl/common/cpu/alu.sv` now computes grouped `arith_result`, `bitwise_result`, `shift_result`, `minmax_result`, and `muldiv_result` values before the final result select. This removed the previous flat 10-way arithmetic/logic/shift result tree from the hottest ALU cases and set up the follow-on MIN/MAX fix.

4. **Atomic MIN/MAX Compare/Select Staging** (new): `ALU_MIN/MAX/MINU/MAXU` now execute across two cycles. The first cycle performs and registers the signed/unsigned comparison; the second cycle selects the winner with a simple mux. `cpu.sv` now waits for `alu_ready` before issuing the AMO write request, preserving the existing upstream handshake contract.

Measured full-chip impact of the staged MIN/MAX change on the iCE40 target:

- **Logic cells:** 5,472 → **5,334** (**−138 LCs**, −2.5%)
- **SB_LUT4 cells:** 4,437 → **4,270** (**−167 LUT4s**, −3.8%)
- **SB_CARRY cells:** 814 → **783** (**−31 carries**, −3.8%)
- **Fmax (nextpnr):** 47.30 MHz → **55.20 MHz** (**+7.90 MHz**, +16.7%)
- **Fmax (icetime):** 47.43 MHz → **53.50 MHz** (**+6.07 MHz**, +12.8%)

The staged MIN/MAX datapath therefore produced a **meaningful utilization reduction** and a **clear timing improvement**. The dominant path still lives in the A-extension MIN/MAX network, but the compare and value-select work are no longer collapsed into one cycle, so the remaining path is materially shorter.

The design still faces two impending **resource saturation** constraints that will limit future development:

- **BRAM utilization: 30/32 blocks (93%)** — only 2 blocks of headroom remain
- **Global buffer utilization: 8/8 (100%)** — all global routing resources exhausted

### Key Metrics

| Metric | Value | Available | Utilization |
|--------|-------|-----------|-------------|
| **Logic Cells (ICESTORM_LC)** | 5,334 | 7,680 | **69%** |
| **Block RAM (ICESTORM_RAM)** | 30 | 32 | **93% ⚠️** |
| **I/O Pins (SB_IO)** | 77 | 256 | 30% |
| **Global Buffers (SB_GB)** | 8 | 8 | **100% ⚠️** |
| **PLLs (ICESTORM_PLL)** | 1 | 2 | 50% |
| **Achieved Fmax (nextpnr)** | 55.20 MHz | 25 MHz target | **PASS (+121%)** |
| **Achieved Fmax (icetime)** | 53.50 MHz | 25 MHz target | **PASS (+114%)** |
| **Critical Path Delay** | 18.69 ns | 40.0 ns budget | PASS |

### Cell Type Breakdown (from Yosys)

| Cell Type | Count | Change vs. Previous | Description |
|-----------|-------|---------------------|-------------|
| SB_LUT4 | 4,270 | −167 vs. previous analysis | 4-input Look-Up Tables |
| SB_CARRY | 783 | −31 vs. previous analysis | Carry chain cells (arithmetic / compare) |
| SB_DFFESR | 1,644 | +68 vs. previous analysis | D flip-flop with enable and set/reset |
| SB_DFF variants | 635 | 0 vs. previous analysis | Various D flip-flop configurations |
| SB_RAM40_4K | 30 | 0 | 4Kbit Block RAM instances |
| SB_PLL40_CORE | 1 | 0 | PLL |

### Optimization History

| Date | Optimization | Fmax Before | Fmax After | Improvement |
|------|-------------|-------------|------------|-------------|
| 2026-03-10 | Pre-compute AUIPC/JAL adders in EXECUTE; remove inline adders from `writeback_mux.sv` | 42.09 MHz | 47.70 MHz | +5.61 MHz (+13.3%) |
| 2026-03-10 | Replace `button_counter % 8'd6` modulo with registered rollover counter in `ice40_alchitry_cu_top.sv` | 41.04 MHz (icetime worst case) | 47.05 MHz (icetime) | +6.01 MHz (+14.6%) |
| 2026-03-10 | Group ALU result selection into arithmetic / bitwise / shift / minmax / muldiv buckets in `alu.sv` | 47.70 MHz (nextpnr), 47.05 MHz (icetime) | 47.30 MHz (nextpnr), 47.43 MHz (icetime) | −0.40 MHz routed, +0.38 MHz icetime |
| 2026-03-10 | Break atomic MIN/MAX into compare and select cycles in `alu.sv`; wait for `alu_ready` before AMO write request in `cpu.sv` | 47.30 MHz (nextpnr), 47.43 MHz (icetime) | 55.20 MHz (nextpnr), 53.50 MHz (icetime) | +7.90 MHz routed, +6.07 MHz icetime |

---

## Critical Path #1 — Staged Atomic MIN/MAX Compare Chain + Residual Result Tail (Primary)

**Clock domain:** `pll_clk_global` (posedge → posedge)  
**Total delay:** 18.12 ns (8.5 ns logic + 9.7 ns routing) — nextpnr; 18.69 ns — icetime  
**Achieved Fmax:** 55.20 MHz (nextpnr), 53.50 MHz (icetime)  
**Logic levels (icetime):** 41  
**RTL modules involved:** `cpu.sv`, `alu.sv`

### Path Narrative

This is the dominant registered-clock critical path identified by both nextpnr and icetime after implementing the two-cycle MIN/MAX staging. The launch point has shifted into the FSM / ALU-control side of the datapath (`current_state` in nextpnr, `alu_start_sent` in icetime), then flows through atomic-state operand conditioning, traverses the full-width MIN/MAX compare carry chain in `alu.sv`, and finally passes through the reduced `alu_result` tail before terminating at `alu_out_reg`.

The key qualitative change versus the previous analysis is that the **compare** and **value selection** work are no longer collapsed into a single cycle. The worst endpoint is still tied to the MIN/MAX network, but the second-cycle winner-select mux is now simpler and the overall routed delay is lower by roughly 2.4 ns.

### Detailed Stage Breakdown

| Stage | Start (ns) | End (ns) | Δ (ns) | Type | Description |
|-------|-----------|---------|-------|------|-------------|
| DFF output (`current_state` / `alu_start_sent`) | 0.00 | 0.50 | 0.50 | Logic | Registered FSM / ALU-control launch |
| Atomic state + operand-routing preamble | 0.50 | 6.80 | 6.30 | **Routing + Logic** | `S_ATOMIC_RMW` control and `alu_a` operand conditioning reach the compare logic |
| **30-stage compare/carry chain** | 6.80 | 11.30 | **4.50** | Logic | Full-width MIN/MAX comparator propagation across the carry fabric |
| Residual staged-result routing / select LUTs | 11.30 | 17.60 | 6.30 | Logic + Routing | Reduced `alu_result` tail after the staged compare result |
| Setup at destination DFF | 17.60 | 18.12 | 0.52 | Setup | Register setup time |

**Path summary:**
```
current_state / alu_start_sent[DFF] (cpu.sv)
  → atomic-state / operand-select logic
  → MIN/MAX compare carry chain (alu.sv)
  → reduced `alu_result` tail
  → alu_out_reg[DFF]
```

### Why This Path Is Slow

1. **MIN/MAX compare still needs a full-width carry structure:** Signed/unsigned MIN/MAX operations still require a 32-bit compare, so the path retains a long carry/comparator chain even though the winner-select mux moved into its own cycle.

2. **Control/operand routing still arrives late:** The critical path still launches from control logic associated with `S_ATOMIC_RMW` and `alu_start_sent`, then crosses into the ALU operand network before the carry chain begins.

3. **Residual post-compare LUT depth remains material:** The two-cycle staging removed part of the old select pressure, but the remaining `alu_result` tail still burns a few routed LUT levels before `alu_out_reg`.

4. **Routing remains slightly dominant:** 9.7 ns of the 18.12 ns nextpnr path (~54%) is routing. The utilization drop helped placement, but the design still sits at 69% LC usage and 100% global-buffer utilization, so cross-fabric hops are still expensive.

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

The `pll_clk_global → <async>` output path via `seg_position_reg` is now only **3.95 ns** (max delay from nextpnr), well within the 40 ns clock budget. The 35-level combinational division chain is completely gone.

---

## Critical Path #2 — Residual ALU Result Routing / Select Depth (Near-Critical)

**Clock domain:** `pll_clk_global` (posedge → posedge)  
**Characterized delay:** ~18–20 ns (multiple endpoints in tight slack range)  
**RTL modules involved:** `alu.sv`, `cpu.sv`

### Path Narrative

The slack histogram still shows a cluster of endpoints in the ~18–20 ns range, indicating several near-critical paths in addition to the primary 18.12/18.69 ns path. After the two-cycle MIN/MAX change, these endpoints still come from the **tail of the staged ALU result network** described in Critical Path #1.

The primary contributor is now the reduced post-compare output network inside `alu.sv`, especially the residual `alu_result` merge fed by the staged MIN/MAX path. The depth is lower than the previous flat mux, but multiple bits of `alu_result` still travel through several LUT levels once placement and routing are included.

The near-critical endpoint cluster corresponds to paths that reach the same result-format LUT tree via different carry-chain bits (different ALU result bits) or different source operands. Each bit of `alu_result` may travel a slightly different routing path, causing the cluster to span ~2 ns in the histogram.

### Why This Path Persists

Unlike the previous near-critical paths (which were caused by inline adders in `writeback_mux.sv`, now removed), this cluster arises from the remaining ALU result-tail logic after MIN/MAX staging. Further reduction likely requires either (a) shortening the compare/carry path itself, or (b) isolating the staged MIN/MAX datapath from the shared `result` merge even more aggressively.

---

## Critical Path #3 — FSM Instruction Decode Depth (Contributing)

**Clock domain:** `pll_clk_global`  
**Characterized delay:** ~1.8–6.8 ns (early preamble of Critical Path #1)  
**RTL modules involved:** `cpu.sv`, `decoder.sv`

### Path Narrative

The CPU's multi-cycle FSM still provides the launch flip-flops for Critical Path #1, but the worst endpoint now originates from the **execute/atomic-control side** (`current_state` in nextpnr, `alu_start_sent` in icetime) rather than a decode flag such as `is_lr_reg`. The path still spends several nanoseconds crossing from control logic into the ALU operand network before the carry chain begins.

The decode/control complexity increases with each instruction class supported. With the A extension enabled (atomic instructions adding `is_lr_reg`, `is_sc_reg`, `is_amo_reg`), the FSM/control region still carries significant fanout. The staging change improved the ALU itself, but it did not physically co-locate the control and ALU regions.

### Why This Path Persists

The registered-flag approach (latching decoded signals in DECODE state) correctly isolates LUT decode depth to earlier cycles. However, the placement challenge remains: the control/FSM region lives near the decoder/register file, while the ALU carry chain still sits where arithmetic placement permits. Even at the improved 69% LC utilization, the placer has limited freedom to co-locate these structures perfectly.

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

While global buffer saturation does not directly appear as a timing violation, it creates a "hidden tax" on every high-fanout signal added to the design. The routing congestion forces nextpnr to place related logic farther apart, increasing routing delays across the board. The still-large 9.7 ns routing component in Critical Path #1 is partly attributable to congested placement caused by high-fanout signal distribution without global buffers. In particular, the long control-to-ALU and ALU-to-writeback hops in the staged MIN/MAX path reflect this congestion.

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
| 1 | Staged Atomic MIN/MAX Compare Chain + Result Tail | 18.12 ns total (4.50 ns compare/carry + 6.30 ns result tail) | ⚠️ Active | Atomic-state control and operand routing still feed a full-width compare chain, followed by a smaller residual result tail | `cpu.sv`, `alu.sv` |
| 2 | Cross-Fabric Routing (~54% of critical path) | 9.7 ns routing | ⚠️ Active | Path still crosses FSM/control, ALU, and writeback regions under 69% LC utilization | `cpu.sv`, `alu.sv` |
| 3 | Residual ALU Result Select Depth (near-critical) | ~18–20 ns | ⚠️ Active | MIN/MAX staging removed part of the old mux depth, but the shared `alu_result` merge still costs routed LUT levels | `alu.sv` |
| 4 | FSM / Execute-Control Routing | ~1.8–6.8 ns (preamble of CP#1) | ⚠️ Active | `current_state` / `alu_start_sent` still route non-trivially into the ALU operand network | `cpu.sv` |
| 5 | ~~Seven-Segment Modulo Arithmetic~~ | ~~24.37 ns (35 levels)~~ | ✅ **RESOLVED** | Replaced `% 8'd6` with registered rollover counter | `ice40_alchitry_cu_top.sv` |
| 6 | ~~Writeback Mux Adders~~ | ~~~19–21 ns~~ | ✅ **RESOLVED** | Pre-computed AUIPC/JAL results in EXECUTE state; mux now uses registered `alu_result` | `writeback_mux.sv`, `cpu.sv` |
| 7 | Global Buffer Saturation | N/A (resource limit) | ⚠️ Active | All 8 SB_GB consumed; new high-fanout signals must use congested local routing | `top.sv`, system-wide |
| 8 | BRAM Near Capacity | N/A (resource limit) | ⚠️ Active | 12KB SRAM uses ~24/30 BRAM blocks; blocks timing optimizations requiring BRAM | `sram_peripheral.sv`, `sram.sv` |

---

## Suggestions for Addressing Timing Challenges

The following suggestions are ordered within groups from lowest to highest implementation effort. All estimates assume the current iCE40-HX8K target and Yosys/nextpnr toolchain. The modulo fix, grouped ALU result selection, writeback-adder precomputation, and staged atomic MIN/MAX datapath have now been implemented.

---

### Tier 1: Low Effort (Days)

#### ~~Suggestion 1 — Replace Modulo-6 with a Registered Rollover Counter~~ (IMPLEMENTED ✅)

**Addresses:** Former Critical Path #2 (Seven-Segment Display Modulo Arithmetic)  
**Result:** Eliminated the 35-level, 24.37 ns icetime critical path. Replaced with a 3-bit registered rollover counter (`seg_position_reg`) that increments on button press or LED output change. The synchronous critical path stayed in the ALU/writeback network and has since been reduced further to **18.69 ns** by the staged atomic MIN/MAX work.

---

#### ~~Suggestion 2 — Reduce ALU Result Mux Depth via Operation Grouping~~ (IMPLEMENTED ✅)

**Addresses:** Former Critical Path #1 post-carry segment and the near-critical ALU result-tail cluster  
**Implementation summary:** `rtl/common/cpu/alu.sv` now groups ALU outputs into arithmetic, bitwise, shift, min/max, and mul/div buckets before the final `result` merge.  
**Observed outcome:** Logic utilization dropped modestly, but timing was effectively neutral overall because the critical path moved into the MIN/MAX compare/select cone rather than disappearing.  
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

#### Implemented Optimization — Stage Atomic MIN/MAX Across Two Cycles (IMPLEMENTED ✅)

**Addresses:** Critical Path #1 (atomic MIN/MAX compare/select cone)  
**Implementation summary:** `rtl/common/cpu/alu.sv` now captures the signed/unsigned MIN/MAX comparison and operands on the first cycle, then selects the winner on the second cycle. `rtl/common/cpu/cpu.sv` now waits for `alu_ready` before issuing the AMO write request so the existing upstream ready/valid protocol remains correct.  
**Files:** `rtl/common/cpu/alu.sv`, `rtl/common/cpu/cpu.sv`

**Measured result:** Relative to the previous grouped-ALU build, full-chip iCE40 utilization dropped from **5,472 → 5,334 LCs**, **4,437 → 4,270 LUT4s**, and **814 → 783 SB_CARRY** cells. Timing improved from **47.30 → 55.20 MHz** in nextpnr and **47.43 → 53.50 MHz** in icetime, with the reported critical path falling from **21.08 ns → 18.69 ns**.

This change delivered the first clear improvement on the MIN/MAX hotspot itself. The path is still dominated by the staged atomic MIN/MAX network, but the carry-chain work is now separated from the value-select mux and the AMO write request is naturally back-pressured through `alu_ready`.

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

3. **Reduced LUT Pressure:** Some of the 5,334 LCs are used by the bus routing logic for the SRAM peripheral. Reducing the SRAM footprint slightly reduces LUT pressure, giving nextpnr more placement flexibility and potentially improving routing quality (reducing the 9.7 ns routing component of Critical Path #1).

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
Max frequency for clock 'pll_clk_global': 55.20 MHz (PASS at 25.00 MHz)
Max frequency for clock   'clk$SB_IO_IN': 626.57 MHz (PASS at 25.00 MHz)

Cross-domain path delays:
  <async> -> posedge clk$SB_IO_IN   : 3.66 ns max
  <async> -> posedge pll_clk_global : 3.49 ns max
  posedge pll_clk_global -> <async> : 3.95 ns max
```

The `pll_clk_global → <async>` path remains comfortably short; the registered rollover-counter change keeps the seven-segment output logic well away from the synchronous critical path.

### Critical Path Logic/Routing Breakdown (nextpnr)

```
Critical path for 'pll_clk_global' (posedge -> posedge):
  Source: current_state / alu_start_sent control flop  (cpu_core, cpu.sv)
  Dest:   alu_out_reg_SB_DFFESR_*_DFFLC.I0  (cpu.sv / alu.sv staged MIN/MAX path)
  Logic:   8.5 ns
  Routing: 9.7 ns
  Total:  18.1 ns
```

### icetime Timing Summary (ASC-Level Static Timing)

```
Total number of logic levels: 41
Total path delay: 18.69 ns (53.50 MHz)

Critical path (icetime):
  Source: alu_start_sent / execute-control state (cpu_core, cpu.sv)
  → atomic-state operand conditioning
  → MIN/MAX compare/carry chain
  → reduced staged-result routing/select LUTs
  → alu_out_reg / cpu_to_arb_a_wdata endpoint
  Total: 18.693 ns
```

### Slack Histogram (pll_clk_global, 25 MHz target = 40 ns period)

The slack histogram below shows the distribution of timing endpoints relative to the 40 ns target period. The tightest endpoints are now just under **21.9 ns** of slack (endpoints at ~18.1 ns total delay). Compared to the previous analysis state, the histogram has shifted materially rightward, matching the timing improvement from staging atomic MIN/MAX.

```
Slack range (ps)    Endpoint count (legend: * = 26 endpoints, + = [1,26))
[ 21883,  22728)    |+
[ 22728,  23573)    |+
[ 23573,  24418)    |*+
[ 24418,  25263)    |**+
[ 25263,  26108)    |**+
[ 26108,  26953)    |***+
[ 26953,  27798)    |***+
[ 27798,  28643)    |***+
[ 28643,  29488)    |*************+
[ 29488,  30333)    |****************+
[ 30333,  31178)    |********************+
[ 31178,  32023)    |******************+
[ 32023,  32868)    |***********************+
[ 32868,  33713)    |*********************************+
[ 33713,  34558)    |************************************************************ (most endpoints)
[ 34558,  35403)    |**************+
[ 35403,  36248)    |******************+
[ 36248,  37093)    |******************************+
[ 37093,  37938)    |***********************************************+
[ 37938,  38783)    |*****************************************+
```

### Comparison: Previous vs. Current Analysis

| Metric | Previous | Current | Change |
|--------|---------|---------|--------|
| Fmax (nextpnr) | 47.30 MHz | **55.20 MHz** | +7.90 MHz (+16.7%) |
| Fmax (icetime) | 47.43 MHz | **53.50 MHz** | +6.07 MHz (+12.8%) |
| Critical path delay | 21.08 ns | **18.69 ns** | −2.39 ns (−11.3%) |
| Critical path routing | 12.1 ns (57%) | **9.7 ns (54%)** | −2.4 ns |
| ICESTORM_LC count | 5,472 (71%) | **5,334 (69%)** | −138 LCs (−2.5%) |
| SB_LUT4 count | 4,437 | **4,270** | −167 |
| SB_CARRY count | 814 | **783** | −31 |
| Tightest slack (pll_clk_global) | ~18.9 ns | **~21.9 ns** | +~3.0 ns |

---

*Analysis generated from synthesis runs using Yosys 0.33 and nextpnr-ice40, targeting the iCE40-HX8K-CB132 device.*
