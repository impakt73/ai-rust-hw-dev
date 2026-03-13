## iCE40-HX8K Timing Analysis

**Date:** 2026-03-13  
**Target device:** Lattice iCE40-HX8K-CB132 (Alchitry Cu v1)  
**Clock target:** 25.00 MHz (`pll_clk_global`)  
**Build command:** `cd rtl/fpga && make TARGET=ice40_alchitry_cu stats STATS_FORMAT=json`

---

## Executive Summary

The current `ice40_alchitry_cu` build closes timing comfortably.

- **Routed Fmax (nextpnr):** **62.42 MHz**
- **Cross-check Fmax (icetime):** **61.88 MHz**
- **Timing status:** **PASS**
- **Timing margin vs. 25 MHz target:** **+37.42 MHz** (**+149.7%**)

The most important conclusion from the fresh reports is that the design's worst path is **no longer in the CPU ALU**. The dominant synchronous path now lives in the **host bus interface / host TX formatting control logic**, specifically in the logic that decides whether a read response beat is the **first beat of a burst** and whether `host_beats_remaining` should be decremented or held.

That path is not limited by one single arithmetic operator. It is long because it combines:

1. a **16-bit carry-chain compare**,
2. multiple layers of **packet-control muxing**,
3. reconvergence through the `host_bus_tx` handshake logic, and
4. **two very long routed hops** across the iCE40 fabric.

The design still has two structural constraints that will make future timing closure harder even though this build passes:

- **BRAM:** 30 / 32 blocks (**93%**)
- **Global buffers:** 8 / 8 (**100%**)

Those constraints matter because the worst synchronous path is already **routing-dominated**.

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

## Critical Path #1 — Host Read Response Start/Count Control

**Domain:** `pll_clk_global` (posedge → posedge)  
**Primary source:** `host_req_burst_len_m1` register in `host_bus_interface.sv`  
**Primary destination:** `host_beats_remaining` clock-enable logic in `host_bus_interface.sv`  
**Delay:** **16.16 ns** (`icetime`) / **16.5 ns total** (`nextpnr` detailed path)  
**Equivalent Fmax:** **61.88 MHz** (`icetime`)  
**Logic levels:** **25** (`icetime`)  
**Breakdown:** **6.1 ns logic + 10.5 ns routing** (`nextpnr`)

### RTL Path Narrative

The worst registered path starts from the stored host burst length metadata and fans into the `HOST_READ_TX` combinational formatter:

```systemverilog
tx_pkt_start = (host_beats_remaining == ({1'b0, host_req_burst_len_m1} + 17'd1));
tx_pkt_last  = (host_beats_remaining == 17'd1);
tx_pkt_src_fixed = host_req_src_fixed;
```

This logic lives in `rtl/common/io/host_bus_interface.sv` and feeds the `host_bus_tx` instance. The timing reports show that the path then propagates through `host_bus_tx` control around `packet_start` / `packet_ready` / `beat_bytes_reg`, reconverges into `host_state` / `host_req_src_fixed` dependent logic, and finally lands on the **clock-enable** controlling updates to `host_beats_remaining`.

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

The `icetime` report makes the structure of the path very clear:

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

### Actionable Fixes

The most effective fixes are control-structure changes, not generic LUT shaving:

1. **Precompute and register the "first read-response beat" condition.**  
   Add a dedicated flag when entering `HOST_READ_TX` instead of recomputing  
   `host_beats_remaining == ({1'b0, host_req_burst_len_m1} + 17'd1)` combinationally.

2. **Register packet metadata at the `HOST_READ_D -> HOST_READ_TX` boundary.**  
   Move `tx_pkt_start`, `tx_pkt_last`, and any read-response metadata needed by `host_bus_tx` into local registers so the host-state counter logic no longer feeds directly through the TX formatter in one cycle.

3. **Decouple the `host_beats_remaining` decrement-enable from `tx_pkt_start`.**  
   Right now "is this the first beat?" and "should the beat counter update?" are entangled by control reconvergence. Splitting those concerns should cut both LUT depth and routing distance.

4. **Keep the read-response control cone physically local.**  
   The two multi-nanosecond routes show that nextpnr placed producer and consumer logic far apart. Any RTL restructuring that keeps `host_req_src_fixed`, `host_state`, and the beat-counter update in one local control island should help more than micro-optimizing the carry chain.

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
