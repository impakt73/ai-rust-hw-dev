# iCE40-HX8K Critical Path & Timing Analysis

**Date:** 2026-03-10  
**Target Device:** Lattice iCE40-HX8K-CB132 (Alchitry Cu v1)  
**Synthesis Tools:** Yosys 0.33, nextpnr-ice40, icetime  
**Design Configuration:** RV32IM CPU (M extension enabled, F extension disabled), dual-banked BRAM register file

---

## Executive Summary

The RISC-V RV32IM CPU design meets the 25 MHz target frequency with significant headroom. The achieved Fmax is **42.09 MHz** (nextpnr) / **41.04 MHz** (icetime), providing a **68% timing margin** over the 25 MHz target (23.8 ns path vs. 40.0 ns clock budget).

Despite passing timing comfortably, the design faces two impending **resource saturation** constraints that will limit future development:

- **BRAM utilization: 30/32 blocks (93%)** — only 2 blocks of headroom remain
- **Global buffer utilization: 8/8 (100%)** — all global routing resources exhausted

These constraints, combined with the identified combinational depth bottlenecks, define the major timing and scalability challenges for this design.

### Key Metrics

| Metric | Value | Available | Utilization |
|--------|-------|-----------|-------------|
| **Logic Cells (ICESTORM_LC)** | 5,646 | 7,680 | **73%** |
| **Block RAM (ICESTORM_RAM)** | 30 | 32 | **93% ⚠️** |
| **I/O Pins (SB_IO)** | 77 | 256 | 30% |
| **Global Buffers (SB_GB)** | 8 | 8 | **100% ⚠️** |
| **PLLs (ICESTORM_PLL)** | 1 | 2 | 50% |
| **Achieved Fmax (nextpnr)** | 42.09 MHz | 25 MHz target | **PASS (+68%)** |
| **Achieved Fmax (icetime)** | 41.04 MHz | 25 MHz target | **PASS (+64%)** |
| **Critical Path Delay** | 23.8 ns | 40.0 ns budget | PASS |

### Cell Type Breakdown (from Yosys)

| Cell Type | Count | Description |
|-----------|-------|-------------|
| SB_LUT4 | 4,560 | 4-input Look-Up Tables |
| SB_CARRY | 898 | Carry chain cells (arithmetic) |
| SB_DFFESR | 1,611 | D flip-flop with enable and set/reset |
| SB_DFF variants | ~718 | Various D flip-flop configurations |
| SB_RAM40_4K | 30 | 4Kbit Block RAM instances |
| SB_PLL40_CORE | 1 | PLL |

---

## Critical Path #1 — ALU Ripple-Carry Through Cross-Module Decode Chain (Primary)

**Clock domain:** `pll_clk_global` (posedge → posedge)  
**Total delay:** 23.8 ns (9.1 ns logic + 14.6 ns routing)  
**Achieved Fmax:** 42.09 MHz  
**RTL modules involved:** `cpu.sv`, `mem_interface.sv`, `system_controller_peripheral.sv`, `alu.sv`, `host_bus_mux.sv`

### Path Narrative

This is the dominant registered-clock critical path identified by nextpnr. It originates from the `opcode_reg` flip-flop output and terminates at a setup check on a register inside `cpu_host_bus_mux`. The path crosses four distinct RTL modules, accumulating significant routing delay at each module boundary before arriving at the 32-bit ALU carry chain.

### Detailed Stage Breakdown

| Stage | Start (ns) | End (ns) | Δ (ns) | Type | Description |
|-------|-----------|---------|-------|------|-------------|
| DFF output (`opcode_reg`) | 0.0 | 0.5 | 0.5 | Logic | Flip-flop Q propagation |
| `opcode_reg[3]` net routing | 0.5 | 1.1 | 0.6 | **Routing** | LUT fanout, local routing |
| `is_ecall_reg` LUT cascade (3 levels) | 1.1 | 3.5 | 2.4 | Logic + Routing | Instruction type decode in `cpu.sv` |
| `is_atomic_rmw` LUT decode | 3.5 | 5.1 | 1.6 | **Routing** | Long net: `cpu_core` → `u_mem_interface` |
| `sysctrl.cpu_boot` LUT decode | 5.1 | 6.8 | 1.7 | Logic + **Routing** | Cross-module net into `system_controller_peripheral.sv` |
| `imm_s_reg/alu_src_reg` decode | 6.8 | 8.1 | 1.3 | Logic + Routing | Return to `cpu_core` scope |
| `alu_b[3]` computation + routing | 8.1 | 10.5 | 2.4 | **Routing** | Long net carrying ALU B operand to carry chain |
| **32-bit ripple carry chain (bits 3–31)** | 10.5 | 14.8 | **4.3** | Logic | 30 chained `SB_CARRY` cells in `alu.sv` |
| Post-carry result mux (2 LUT levels) | 14.8 | 16.6 | 1.8 | Logic + Routing | `alu_result` selection/formatting in `alu.sv` |
| `alu_result[3]` net routing | 16.6 | 23.3 | **6.7** | **Routing** | Long net: `cpu_core` → `cpu_host_bus_mux` |
| Setup at `cpu_mem_a_wdata` register | 23.3 | 23.8 | 0.4 | Setup | Register setup time |

**Path summary:**
```
opcode_reg[DFF] → is_ecall_reg (decode, cpu.sv) 
  → is_atomic_rmw (mem_interface.sv, 1.6 ns net)
  → sysctrl.cpu_boot (system_controller_peripheral.sv, 1.3 ns net)
  → alu_src_reg → alu_b mux (cpu.sv, 2.4 ns net)
  → 32-bit SB_CARRY chain (alu.sv, 4.3 ns, ~30 cells)
  → alu_result (post-carry mux)
  → cpu_host_bus_mux.cpu_mem_a_wdata[DFF] (1.9 ns net)
```

### Why This Path Is Slow

1. **Early arrival problem:** The carry chain receives its inputs late (at ~10.5 ns) because the preceding decode chain has 7+ combinational LUT levels to traverse. Even a fast carry chain cannot compensate for a late carry-in arrival.
2. **Cross-module routing dominance:** 14.6 ns of the 23.8 ns total (61%) is routing delay. The path crosses `cpu.sv` → `mem_interface.sv` → `system_controller_peripheral.sv` → back to `cpu.sv` → `alu.sv` → `host_bus_mux.sv`. Each module boundary introduces routing wire delay because the placer distributes modules across the small 33×33 LC grid.
3. **Ripple carry propagation:** The 32-bit adder in `alu.sv` (line ~225) uses a single linear carry chain. The iCE40's SB_CARRY chain delay is approximately 0.126 ns per stage (local within a tile) but the carry chain is split across tiles, adding inter-tile routing overhead.

---

## Critical Path #2 — Seven-Segment Display Modulo Arithmetic (Peripheral Output)

**Clock domain:** `pll_clk_global` → `<async>` (output path)  
**Total delay:** 24.37 ns (9.2 ns logic + 15.1 ns routing), icetime: 24.37 ns  
**Logic levels:** 35  
**RTL module involved:** `ice40_alchitry_cu_top.sv`

### Path Narrative

This path is reported as the **worst-case path** by icetime (the iCE40 static timing analyzer used after place-and-route). It originates from `button_counter[2]` (a flip-flop inside `ice40_alchitry_cu_top.sv`) and terminates at an IO pad driving the `io_seg[0]` seven-segment display output.

The root cause is the use of a hardware modulo operation:

```systemverilog
// ice40_alchitry_cu_top.sv, line 196
seg_position = 3'(button_counter % 8'd6);
```

Yosys synthesizes `% 8'd6` (modulo-6 of an 8-bit value) as a multi-stage combinational divider/comparator circuit. Unlike modulo by a power of 2 (which is a simple bit-select), modulo by 6 requires iterative subtraction/comparison logic, generating approximately 8–10 levels of LUT logic followed by carry chains for the comparison and subtraction. This then feeds the `case (seg_position)` decoder and the inverted output, resulting in **35 total logic levels** and a path that icetime identifies as the worst case at 24.37 ns.

### Detailed Stage Summary

| Stage | Cumulative (ns) | Description |
|-------|----------------|-------------|
| `button_counter[2]` DFF | 0.64 | FF output |
| LUT cascade (modulo-6 partial computations) | ~5.0 | 7-segment arithmetic, 3 carry chain segments |
| Carry chain segments (modulo subtraction) | ~10.5 | Multiple 4–8 bit carry sub-chains |
| Additional LUT decode + carry chain levels | ~16.5 | Intermediate arithmetic |
| `io_seg_SB_LUT4` decode chain | ~21.0 | seg_pattern case statement decode |
| IO buffer routing + setup | 24.37 | Final IO pad with setup/hold |

### Why This Path Is Slow

The `%` operator applied to a non-power-of-2 divisor (6) forces the synthesizer to infer general-purpose integer division hardware. Since there is no hardware divider primitive on iCE40, this expands to a tree of subtractors and comparators mapped to LUT+CARRY chains. A simple registered counter counting 0→5 and rolling over would eliminate this path entirely.

---

## Critical Path #3 — FSM Instruction Decode Depth

**Clock domain:** `pll_clk_global`  
**Characterized delay:** ~7.0 ns (first 7 stages of Critical Path #1)  
**RTL modules involved:** `cpu.sv`, `decoder.sv`, `mem_interface.sv`

### Path Narrative

This sub-path is not independently reported by nextpnr but is the preamble to Critical Path #1 and is a pervasive issue. The CPU's multi-cycle FSM stores `opcode_reg` (the raw instruction opcode bits) as a flip-flop, then derives all control signals combinationally in `always_comb` blocks. Because many control signals depend on multiple instruction fields (opcode + funct3 + funct7 + extension-specific bits), they form deep LUT trees.

The critical path specifically shows:
- `opcode_reg[3]` → `is_ecall_reg` → `is_ecall_reg_SB_LUT4_I0_O_SB_LUT4_O_I2_SB_LUT4_O_I0_SB_LUT4_I3_LC` — a **4-LUT cascade** just to identify an ECALL/EBREAK instruction
- This feeds into `is_atomic_rmw` detection in `mem_interface.sv`, which requires checking multiple opcode fields
- The result routes back to influence `alu_src_reg`, which then selects the ALU B operand

Each additional instruction class (M extension, A extension, C extension) adds more branches to these case/if-else trees, increasing the LUT depth. With M extension enabled (adding 8 new instruction encodings), the decode trees are deeper than in the RV32I-only configuration.

### Why This Path Is Slow

The decode logic in `cpu.sv` uses combinational signals derived every clock cycle from registered `opcode_reg`. Unlike a pipelined design that registers decoded signals in DECODE state for use in EXECUTE state, this design re-computes all decode signals from raw opcode bits continuously. This means the full opcode→control signal propagation is part of the critical clock-to-clock path, rather than a one-time decode cost.

---

## Critical Path #4 — Writeback Multiplexer Fan-In (Setup-Critical)

**Clock domain:** `pll_clk_global` (near-critical path)  
**Characterized delay:** Estimated ~19–21 ns (based on slack histogram)  
**RTL modules involved:** `writeback_mux.sv`, `cpu.sv`

### Path Narrative

The `writeback_mux.sv` module selects among 8 result sources for the register file write-back:

```systemverilog
// writeback_mux.sv
if (fp_to_int)          rd_data = fpu_result;      // F extension
else if (is_amo)        rd_data = formatted_load_data;
else if (is_sc)         rd_data = {31'b0, ~sc_success};
else if (opcode == LUI) rd_data = imm_u;
else if (opcode == AUIPC) rd_data = pc + imm_u;     // adder!
else if (jump)          rd_data = pc + 32'd4;        // adder!
else if (is_csr)        rd_data = csr_rdata;
else                    rd_data = alu_result;
```

The `AUIPC` case (`pc + imm_u`) and the `JAL/JALR` case (`pc + 32'd4`) both instantiate adders within the writeback mux. These combinational adders, while only 32-bit, occur late in the pipeline after the `pc` and `imm_u` registers must be routed to the mux. Additionally, the 8-way priority mux structure itself requires 3 LUT levels for the condition tree.

This path contributes heavily to the tight slack entries in the 16–20 ns histogram range (many endpoints visible in the slack histogram).

### Why This Path Is Slow

The `AUIPC` and `JAL` return-address calculations are performed in the writeback mux rather than pre-computed during the EXECUTE state. The `pc + imm_u` operation involves a 32-bit addition that executes in the same clock cycle as the mux select logic, creating a dependent carry chain. Pre-computing these values during an earlier FSM state (into `jal_target_reg`, which is already partially done for jump targets) could move this work out of the combinational path.

---

## Critical Path #5 — Global Buffer Saturation (Routing Congestion)

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

While global buffer saturation does not directly appear as a timing violation, it creates a "hidden tax" on every high-fanout signal added to the design. The routing congestion also forces nextpnr to place related logic farther apart, increasing routing delays across the board. The 14.6 ns routing component in Critical Path #1 is partly attributable to the congested placement caused by high-fanout signal distribution without global buffers.

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

| Rank | Challenge | Path Delay | Root Cause | Modules Involved |
|------|-----------|-----------|------------|-----------------|
| 1 | ALU 32-bit Ripple-Carry Chain | 4.3 ns (of 23.8 ns) | Sequential SB_CARRY propagation for full 32-bit add/sub | `alu.sv` (line ~225) |
| 2 | Cross-Module Routing (61% of critical path) | 14.6 ns routing | Path crosses 4+ module boundaries; 73% LC utilization forces distant placement | `cpu.sv`, `mem_interface.sv`, `system_controller_peripheral.sv`, `host_bus_mux.sv` |
| 3 | Deep FSM Decode Chain | ~7.0 ns (pre-ALU) | Combinational re-decode every cycle from raw opcode_reg; M extension adds case branches | `cpu.sv`, `decoder.sv` |
| 4 | Seven-Segment Modulo Arithmetic | 24.37 ns (35 levels) | Hardware modulo-6 synthesis from `%` operator on non-power-of-2 divisor | `ice40_alchitry_cu_top.sv` (line 196) |
| 5 | Writeback Mux Adders | ~19–21 ns (est.) | AUIPC/JAL return-address adders inside combinational writeback mux | `writeback_mux.sv` |
| 6 | Global Buffer Saturation | N/A (resource limit) | All 8 SB_GB consumed; new high-fanout signals must use congested local routing | `top.sv`, system-wide |
| 7 | BRAM Near Capacity | N/A (resource limit) | 12KB SRAM uses ~24/30 BRAM blocks; blocks timing optimizations requiring BRAM | `sram_peripheral.sv`, `sram.sv` |

---

## Suggestions for Addressing Timing Challenges

The following 5 suggestions are ordered within groups from lowest to highest implementation effort. All estimates assume the current iCE40-HX8K target and Yosys/nextpnr toolchain.

---

### Tier 1: Low Effort (Days)

#### Suggestion 1 — Replace Modulo-6 with a Registered Rollover Counter

**Addresses:** Critical Path #2 (Seven-Segment Display Modulo Arithmetic)  
**Expected improvement:** Eliminates the 35-level, 24.37 ns icetime critical path entirely  
**Risk:** Very low — purely additive/peripheral change  
**File:** `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv`

The line:
```systemverilog
seg_position = 3'(button_counter % 8'd6);
```
synthesizes a hardware divider because the divisor (6) is not a power of 2. Replace it with a dedicated 3-bit registered counter that counts 0→5 and rolls over. This eliminates the entire combinational division chain:

```systemverilog
// Replace the combinational seg_position with a registered counter
logic [2:0] seg_position_reg;
always_ff @(posedge sys_clk) begin
    if (!rst_n_core) begin
        seg_position_reg <= '0;
    end else if (button_counter != button_counter_prev) begin  // on change
        seg_position_reg <= (seg_position_reg == 3'd5) ? '0 : seg_position_reg + 1'b1;
    end
end
```

This replaces 35 combinational levels with a single flip-flop transition, moving the segment pattern computation to the registered domain entirely.

---

#### Suggestion 2 — Pre-Register Decoded Control Signals in the DECODE FSM State

**Addresses:** Critical Path #3 (FSM Instruction Decode Depth), contributes to Critical Path #1  
**Expected improvement:** ~2–3 ns reduction in pre-ALU preamble; reduces the ~7 ns decode chain  
**Risk:** Low — requires one additional cycle in the DECODE state (which already exists)  
**Files:** `rtl/common/cpu/cpu.sv`

The CPU already spends time in `S_DECODE` before `S_EXECUTE`. However, many control signals (e.g., `alu_src_reg`, `is_ecall_reg`, `is_atomic_rmw`) are re-derived combinationally from `opcode_reg` on every cycle, including during `S_EXECUTE` when the ALU is active. Register these signals at the end of `S_DECODE` so they are stable registered values when `S_EXECUTE` begins:

```systemverilog
// In S_DECODE always_ff block:
alu_op_reg    <= decoded_alu_op;    // register decoded ALU op
alu_src_reg   <= decoded_alu_src;   // register source selection
is_ecall_staged <= is_ecall;        // register instruction-type flags
is_atomic_staged <= is_atomic_rmw;
```

This moves the LUT decode depth (`opcode_reg` → `is_ecall_reg` → `is_atomic_rmw` → `alu_src_reg`) out of the `S_EXECUTE` critical path, replacing those ~7 ns of combinational work with a single DFF→LUT transition. The M extension instruction decode case-statements, which currently add LUT depth every time a new ALU operation is added, would no longer be on the critical path.

---

### Tier 2: Medium Effort (Weeks)

#### Suggestion 3 — Carry-Select Adder for ALU Addition/Subtraction

**Addresses:** Critical Path #1 (32-bit ALU Ripple-Carry Chain, 4.3 ns)  
**Expected improvement:** ~1.5–2.0 ns reduction in carry chain delay (~35–45% improvement)  
**Risk:** Moderate — requires carefully preserving ALU result correctness for all operations (ADD, SUB, SLT, SLTU, AUIPC, etc.)  
**Files:** `rtl/common/cpu/alu.sv`

The iCE40's SB_CARRY chain is extremely efficient for sequential carry propagation within a tile (0.126 ns/stage). The issue is that 32 sequential carry cells create an unavoidable 4+ ns chain, and this chain receives its inputs late (after the pre-ALU decode chain).

A **carry-select adder** partitions the 32-bit adder into N groups (e.g., 4 × 8-bit groups). Each group computes two sums simultaneously — one assuming carry-in = 0, one assuming carry-in = 1 — and selects the correct result once the actual carry-in propagates from the previous group. This halves the critical carry chain length:

- **Current:** 1 × 32-bit ripple carry = ~4.3 ns
- **After:** 4 × 8-bit groups + 3 mux levels ≈ ~8-stage carry + ~3 mux levels ≈ ~2.0–2.5 ns

Implementation involves writing `alu.sv` to instantiate explicit partial-sum modules rather than relying on the synthesizer's automatic `+` operator mapping. The trade-off is ~50–100 additional LUTs for the parallel partial-sum computation and carry-select muxes.

---

#### Suggestion 4 — Pre-Compute Writeback Mux Adders in the EXECUTE Stage

**Addresses:** Critical Path #4 (Writeback Mux Fan-In, ~19–21 ns near-critical paths)  
**Expected improvement:** Moves `pc + imm_u` and `pc + 32'd4` off the writeback mux combinational path; could improve Fmax by 2–4 MHz  
**Risk:** Moderate — requires careful FSM state sequencing to ensure correct values are staged  
**Files:** `rtl/common/cpu/writeback_mux.sv`, `rtl/common/cpu/cpu.sv`

The `writeback_mux.sv` contains two inline adders for AUIPC (`pc + imm_u`) and JAL/JALR return-address (`pc + 32'd4`). Both adders execute within the same clock cycle as the 8-way mux selection logic. Since `S_EXECUTE` already computes the ALU result and the FSM transitions to `S_WRITEBACK` afterward, the return-address and AUIPC results can be pre-computed during `S_EXECUTE` and stored in an intermediate register:

```systemverilog
// In S_EXECUTE always_ff:
auipc_result_reg <= pc + imm_u;     // pre-compute AUIPC result
jal_ret_addr_reg <= pc + 32'd4;     // pre-compute JAL return address
```

Then `writeback_mux.sv` selects from registered values (no adders), removing the carry chains from the writeback mux's combinational depth. This approach is consistent with how `branch_target_reg`, `jal_target_reg`, and `jalr_target_reg` are already pre-computed in the existing design.

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

3. **Reduced LUT Pressure:** Some of the 5,646 LCs are used by the bus routing logic for the SRAM peripheral. Reducing the SRAM footprint slightly reduces LUT pressure, giving nextpnr more placement flexibility and potentially improving routing quality (reducing the 14.6 ns routing component of Critical Path #1).

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
Max frequency for clock 'pll_clk_global': 42.09 MHz (PASS at 25.00 MHz)
Max frequency for clock   'clk$SB_IO_IN': 626.57 MHz (PASS at 25.00 MHz)

Cross-domain path delays:
  <async> -> posedge clk$SB_IO_IN  : 3.58 ns max
  <async> -> posedge pll_clk_global : 3.49 ns max
  posedge pll_clk_global -> <async> : 24.33 ns max
```

### icetime Timing Summary (ASC-Level Static Timing)

```
Total number of logic levels: 35
Total path delay: 24.37 ns (41.04 MHz)
```

### Slack Histogram (pll_clk_global, 25 MHz target = 40 ns period)

The slack histogram below shows the distribution of timing endpoints relative to the 40 ns target period. Most endpoints have 31–39 ns of slack (easily meeting 25 MHz), with the tightest cluster around 15–17 ns slack (near 42 MHz Fmax).

```
Slack range (ps)    Endpoint count
[ 15666, 16830)     |+                  (~1–31 endpoints)
[ 16830, 17994)     |*+
[ 17994, 19158)     |+
[ 20322, 21486)     |+
[ 21486, 22650)     |+
[ 22650, 23814)     |+
[ 23814, 24978)     |**+
[ 24978, 26142)     |*+
[ 26142, 27306)     |*****+
[ 27306, 28470)     |*************+
[ 28470, 29634)     |************+
[ 29634, 30798)     |***********+
[ 30798, 31962)     |*************+
[ 31962, 33126)     |************************************************************ (most endpoints)
[ 33126, 34290)     |**********************************+
[ 34290, 35454)     |******************+
[ 35454, 36618)     |***********************+
[ 36618, 37782)     |************************************************+
[ 37782, 38946)     |*******************************************+
```

---

*Analysis generated from synthesis runs using Yosys 0.33 and nextpnr-ice40, targeting the iCE40-HX8K-CB132 device.*
