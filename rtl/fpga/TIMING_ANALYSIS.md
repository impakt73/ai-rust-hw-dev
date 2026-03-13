## iCE40-HX8K Timing Analysis

**Date:** 2026-03-13  
**Target device:** Lattice iCE40-HX8K-CB132 (Alchitry Cu v1)  
**Clock target:** 25.00 MHz (`pll_clk_global`)  
**Build command:** `cd rtl/fpga && make TARGET=ice40_alchitry_cu stats STATS_FORMAT=json`

---

## Executive Summary

The current `ice40_alchitry_cu` build still closes timing comfortably, and the
previous top host-bus read-response critical path has now been shortened enough
that it is **no longer the top synchronous path** in the routed design.

- **Current post-fix routed Fmax (nextpnr):** **73.21 MHz**
- **Timing status:** **PASS**
- **Timing margin vs. 25 MHz target:** **+48.21 MHz** (**+192.8%**)

### Verified Before/After Comparison

Both data points below were generated with the same command, once before this
change and once after it:

```bash
cd rtl/fpga
make TARGET=ice40_alchitry_cu stats STATS_FORMAT=json
```

| Metric | Before fix | After fix | Delta |
| --- | ---: | ---: | ---: |
| Routed Fmax | 70.45 MHz | 73.21 MHz | **+2.76 MHz** |
| Timing margin vs. 25 MHz | 45.45 MHz | 48.21 MHz | **+2.76 MHz** |
| Logic cells (`ICESTORM_LC`) | 5661 | 5570 | **-91** |
| BRAM (`ICESTORM_RAM`) | 30 | 30 | 0 |
| Global buffers (`SB_GB`) | 8 | 8 | 0 |

The implemented RTL change was intentionally small: `host_bus_interface.sv`
now registers the **"first read-response beat"** condition in a dedicated
1-bit flag instead of recomputing
`host_beats_remaining == ({1'b0, host_req_burst_len_m1} + 17'd1)` inside the
`HOST_READ_TX` combinational formatter every cycle.

That removes the host read burst-length compare from the
`HOST_READ_TX -> host_bus_tx -> host_beats_remaining` control cone that the
previous report identified as the dominant synchronous path.

Although the RTL adds one state bit, the post-route logic-cell count still
drops overall because Yosys/nextpnr can eliminate much more of the previous
wide compare and reconvergent control logic than the new flag costs.

The design still has the same two structural constraints that may limit future
timing work:

- **BRAM:** 30 / 32 blocks (**93%**)
- **Global buffers:** 8 / 8 (**100%**)

Those constraints still matter because the new worst synchronous path still
contains significant routing cost even after the host-bus cone was shortened.

---

## Authoritative Artifacts Used

This report is based on the generated build artifacts in:

- `rtl/fpga/build/ice40_alchitry_cu/nextpnr.log`
- `rtl/fpga/build/ice40_alchitry_cu/riscv_fpga_timing.rpt`
- `rtl/fpga/build/ice40_alchitry_cu/riscv_fpga_stats.json`
- `rtl/fpga/build/ice40_alchitry_cu/riscv_fpga_stats.md`

Key summarized resource numbers from `riscv_fpga_stats.*`:

| Metric | Value | Available | Utilization |
| --- | ---: | ---: | ---: |
| Logic cells (`ICESTORM_LC`) | 5604 | 7680 | 72% |
| BRAM (`ICESTORM_RAM`) | 30 | 32 | 93% |
| Global buffers (`SB_GB`) | 8 | 8 | 100% |
| IOs (`SB_IO`) | 77 | 256 | 30% |
| PLLs (`ICESTORM_PLL`) | 1 | 2 | 50% |

Cell counts from Yosys:

| Cell | Count |
| --- | ---: |
| `SB_LUT4` | 4540 |
| `SB_CARRY` | 912 |
| `SB_DFF` | 315 |
| `SB_DFFESR` | 1663 |
| `SB_DFFSR` | 451 |
| `SB_RAM40_4K` | 30 |

---

## Former Critical Path #1 — Host Read Response Start/Count Control (Resolved)

**Status:** resolved and no longer the top synchronous path  
**Measured improvement:** **+2.76 MHz Fmax**, **-91 logic cells**

### RTL Path Narrative

The former worst registered path started from the stored host burst length
metadata and fanned into the `HOST_READ_TX` combinational formatter:

```systemverilog
tx_pkt_start = (host_beats_remaining == ({1'b0, host_req_burst_len_m1} + 17'd1));
tx_pkt_last  = (host_beats_remaining == 17'd1);
tx_pkt_src_fixed = host_req_src_fixed;
```

That logic fed the `host_bus_tx` instance. The timing reports showed that the
path then propagated through `host_bus_tx` control around `packet_start` /
`packet_ready` / `beat_bytes_reg`, reconverged into host-side sequencing, and
finally landed on the **clock-enable** controlling updates to
`host_beats_remaining`.

At a high level, the path is:

```text
host_req_burst_len_m1[DFF]
  -> "burst_len + 1" compare / start-of-burst detection
  -> tx packet control muxing in HOST_READ_TX
  -> host_bus_tx packet acceptance / beat formatting control
  -> host_state + host_req_src_fixed reconvergence
  -> host_beats_remaining clock enable[DFF]
```

### Detailed Stage Breakdown

The report makes the structure of the path very clear:

| Stage | Delay contribution | What it means |
| --- | ---: | --- |
| Launch from `host_req_burst_len_m1` flop | 0.64 ns | Registered burst metadata starts the path |
| Carry-chain segment on `cpu_cap_we...` synthesized compare logic | ~2.8 ns by 16-bit point | Yosys mapped the equality / increment logic into a long carry chain |
| Post-compare LUT reconstruction | ~4.3 ns additional | The carry output is re-encoded back into control terms |
| `tx_buf.beat_bytes_reg` / `host_state` control reconvergence | ~4.8 ns additional | Packet formatting and host state logic both sit on the same control cone |
| Long route: `host_req_src_fixed...` from `(7,7)` to `(27,7)` | **2.5 ns** | First major cross-chip route |
| Long return route back to `host_beats_remaining` enable at `(4,8)` | **2.8 ns** | Second major cross-chip route |
| Setup at destination | 0.1 ns | Final register requirement |

### Why This Path Is Slow

1. **A wide equality test is rebuilt combinationally every cycle.**  
   `tx_pkt_start` checks whether `host_beats_remaining` is equal to `host_req_burst_len_m1 + 1`. That means the design performs a width extension, an increment, and a full-width equality test inside the hot combinational path instead of reusing a registered "first beat" flag.

2. **The compare result is not consumed locally.**  
   Instead of terminating near the compare, the result continues through the host TX packet-control logic, then returns into host-side sequencing. That reconvergence is what turns a reasonable arithmetic path into a control path with deep fan-in.

3. **The path crosses module boundaries with handshake logic in the middle.**  
   `host_bus_interface` computes packet metadata, but `host_bus_tx` decides packet acceptance based on `packet_start` and TX state. That creates a combinational interface boundary on the hottest path.

4. **Routing dominates logic.**  
   `nextpnr` reports **10.5 ns routing vs. 6.1 ns logic**. The biggest single penalties are the two long routes on `host_req_src_fixed`-derived logic, not the carry chain alone.

### Implemented Fix

The first actionable fix from the original analysis was enough:

1. **Precompute and register the "first read-response beat" condition.**  
   `host_bus_interface.sv` now uses a dedicated `host_read_first_beat` flag
   that is:
   - set when a host read burst is accepted,
   - consumed as `tx_pkt_start` in `HOST_READ_TX`, and
   - cleared on the first successful TX beat handshake.

This preserves the existing protocol semantics for `host_bus_tx` while
removing the burst-length increment/equality compare from the hottest control
path.

### Result

After this change, the old `host_req_burst_len_m1 -> HOST_READ_TX ->
host_bus_tx -> host_beats_remaining` cone no longer appears as the top detailed
`pll_clk_global` path in `nextpnr.log`. In the final post-fix build, the
current top synchronous path has moved back into the CPU ALU's min/max compare
logic.

---

## Critical Path #1 — CPU ALU Min/Max Compare

**Domain:** `pll_clk_global` (posedge → posedge)  
**Primary source:** `u_alu.req_b_reg` register  
**Primary destination:** `u_alu.result_next` input logic  
**Delay:** **13.4 ns total** (`nextpnr` detailed path)  
**Breakdown:** **7.1 ns logic + 6.3 ns routing** (`nextpnr`)

### RTL Path Narrative

The current top synchronous path is no longer in `host_bus_interface`. In the
final routed build it starts in the ALU operand register and traverses the
`minmax_compare_lt` carry-chain / LUT network before landing in the ALU result
selection logic.

At a high level, the path is:

```text
u_alu.req_b_reg[DFF]
  -> minmax_compare_lt carry-chain compare logic
  -> ALU result select / reconstruction
  -> u_alu.result_next input[DFF]
```

This path is more logic-heavy than the previous host-bus path, which is
consistent with the detailed `nextpnr` report showing a long compare chain in
`rtl/common/cpu/alu.sv`.

---

## Critical Path #2 — Asynchronous Button Input to System-Clock Synchronizer

**Domain:** `<async> -> posedge pll_clk_global`  
**Source:** `io_button[2]` input pad  
**Destination:** first stage of `io_button_sync_inst`  
**Delay:** **3.24 ns** (`nextpnr` headline) / **3.5 ns** (`nextpnr` detailed report)  
**Breakdown:** **0.5 ns logic + 3.0 ns routing**

### Path Narrative

This is the longest asynchronous input path reported into the main 25 MHz system domain. The path goes directly from the physical `io_button[2]` pad to the first synchronizer stage instantiated in `ice40_alchitry_cu_top.sv`:

```systemverilog
ff_sync #(
    .WIDTH(5)
) io_button_sync_inst (
    .clk(sys_clk),
    .rst_n(rst_n_core),
    .din(io_button),
    .dout(io_button_sync2)
);
```

### Why This Path Is Long

1. **It is overwhelmingly a routing path.**  
   Nearly the entire delay is pad-to-flop routing, not logic.

2. **The IO pad and synchronizer are physically separated.**  
   On a small device like the HX8K, a few long span wires are enough to dominate an async path.

3. **The button bus shares a wide synchronizer instance.**  
   The five-bit synchronizer is convenient RTL, but the placer does not necessarily keep every synchronizer bit adjacent to its corresponding pad.

### Actionable Fixes

1. **Keep the first synchronizer stage near the IO pads.**  
   If this path ever becomes problematic, separate button synchronizers per bit could give the placer more freedom to keep each first-stage flop close to its pad.

2. **Do not add logic in front of the synchronizer.**  
   The current structure is good: raw pad directly into `ff_sync`. Preserving that property keeps metastability handling clean.

3. **If board-level latency matters, consider pin-local buffering only after synchronization.**  
   Any debouncing or edge detection should stay after the synchronizer, which is already the case.

---

## Critical Path #3 — System-Clock Register to LED Output Pad

**Domain:** `posedge pll_clk_global -> <async>`  
**Source:** LED control logic in `fpga_common_top_inst.cpu_inst.led_ctrl`  
**Destination:** `io_led[6]` output pad  
**Delay:** **3.92 ns** (`nextpnr` headline) / **4.19 ns** (`nextpnr` detailed report)  
**Breakdown:** **0.5 ns logic + 3.7 ns routing**

### Path Narrative

The longest registered-to-output path is the fanout from the internal LED register to one of the replicated IO-shield LED outputs. In `ice40_alchitry_cu_top.sv`, the same 8-bit value is driven onto all three LED banks:

```systemverilog
assign io_led[7:0]   = led_out;
assign io_led[15:8]  = led_out;
assign io_led[23:16] = led_out;
```

### Why This Path Is Long

1. **One source drives many physical pads.**  
   Replicating `led_out` across 24 outputs forces some destinations to be physically far from the LED-control source logic.

2. **Again, routing dominates.**  
   There is essentially no meaningful combinational logic here; this is mostly a net-length problem.

3. **This path is benign for the 25 MHz target, but it is still the top output path.**  
   It is useful as a placement signal: if future features add more IO replication, output routing delay will rise first.

### Actionable Fixes

1. **Register LED-bank outputs locally if more fanout is added.**  
   If the IO shield logic grows, inserting a small output register bank near the pads would shorten long pad routes.

2. **Preserve direct assignments for now.**  
   At under 4.2 ns, this path is not a timing risk at 25 MHz and does not justify extra area today.

---

## Critical Path #4 — Asynchronous Reset Button to UART-Clock Synchronizer

**Domain:** `<async> -> posedge clk$SB_IO_IN`  
**Source:** `rst_n_btn` input pad  
**Destination:** first stage of `rst_n_btn_sync_inst`  
**Delay:** **1.68 ns** (`nextpnr` headline) / **1.75 ns** (`nextpnr` detailed report)

### Path Narrative

The reset button input is synchronized in the raw board clock domain before it is used to control the PLL/reset chain:

```systemverilog
ff_sync #(
    .WIDTH(1)
) rst_n_btn_sync_inst (
    .clk(clk),
    .rst_n(1'b1),
    .din(rst_n_btn),
    .dout(rst_n_btn_sync2)
);
```

This is a healthy path with ample margin and very little logic. It is included here only because it is one of the top reported cross-domain paths in the design.

### Actionable Fixes

No immediate change is recommended. The structure is already correct: asynchronous input directly into a synchronizer with no pre-logic.

---

## Structural Timing Pressure

Even though the current build passes comfortably, the reports show two structural limits that will shape future work.

### 1. Global-buffer saturation

`nextpnr.log` reports:

- `SB_GB: 8 / 8`
- promoted high-fanout nets including:
  - reset control (`fanout 1199`)
  - CPU register-write control (`fanout 565`)
  - decoder/ALU control enables (`fanout 67-86`)

This matters because once all global buffers are consumed, any additional high-fanout control must use ordinary fabric routing. That increases wire delay and makes long control reconvergence paths like Critical Path #1 harder to place.

### 2. BRAM saturation

At **30 / 32 BRAMs**, there is very little memory headroom for architectural changes that might otherwise help timing, such as larger buffering or more local staging storage. The routed design still fits, but future optimizations will need to be selective.

---

## Recommended Optimization Order

If more timing margin is needed on the iCE40 target, the best next steps are:

1. **Register the host read-response "first beat" / "last beat" metadata**
2. **Break the combinational boundary between `host_bus_interface` and `host_bus_tx` on the read-response path**
3. **Localize `host_state`, `host_req_src_fixed`, and `host_beats_remaining` control to reduce the two long routed hops**
4. **Avoid introducing any new high-fanout control nets while `SB_GB` remains 100% utilized**

That ordering is important: the reports indicate the design is currently **routing-limited control logic**, not arithmetic-limited datapath logic.

---

## Bottom Line

The current iCE40 build is healthy and comfortably exceeds the 25 MHz requirement, but the dominant critical path has moved into the **host bus control plane**:

- **not** CPU execute/ALU logic,
- **not** the seven-segment display,
- **not** the writeback network.

The longest path is now the **host read-response start/count decision path**, and the best future optimization is to **replace recomputed wide burst-control comparisons with registered local flags** so the TX formatter and beat counter do not have to share one long combinational cone.
