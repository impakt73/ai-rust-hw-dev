## iCE40-HX8K Timing Analysis

**Date:** 2026-03-13  
**Target device:** Lattice iCE40-HX8K-CB132 (Alchitry Cu v1)  
**Clock target:** 25.00 MHz (`pll_clk_global`)  
**Build command:** `cd rtl/fpga && make TARGET=ice40_alchitry_cu stats STATS_FORMAT=json`

---

## Executive Summary

The current `ice40_alchitry_cu` build closes timing comfortably, but the fresh reports make it clear that the design is now limited by a **host-bus control path**, not by the CPU execute datapath.

- **Normalized routed Fmax (`riscv_fpga_stats.json`):** **70.45 MHz**
- **Detailed worst synchronous path (`nextpnr.log`):** **15.1 ns** = **66.12 MHz**
- **Timing status:** **PASS**
- **Conservative timing margin vs. 25 MHz target:** **+41.12 MHz** (**+164.5%**)

Only **two synchronous critical-path reports** appear in the final `nextpnr` output:

1. the real system bottleneck in `host_bus_interface.sv`, and
2. a much shorter debouncer counter path in the raw input clock domain.

The remaining top reported paths are boundary paths:

- asynchronous input pad to synchronizer,
- asynchronous input pad to synchronizer, and
- synchronous internal logic to output pad.

That matters because the design already shows strong **routing pressure**:

- **BRAM:** 30 / 32 (**93%**)
- **Global buffers:** 8 / 8 (**100%**)

Those utilization numbers line up with what the critical-path reports show: the worst path is **routing-dominated control logic** with long cross-device hops after the compare/control cone is built.

---

## Authoritative Artifacts Used

This report is based on the fresh build artifacts in:

- `rtl/fpga/build/ice40_alchitry_cu/nextpnr.log`
- `rtl/fpga/build/ice40_alchitry_cu/yosys.log`
- `rtl/fpga/build/ice40_alchitry_cu/riscv_fpga_stats.json`
- `rtl/fpga/build/ice40_alchitry_cu/riscv_fpga_stats.md`

For the iCE40 target, the **authoritative timing source is `nextpnr.log`**. The normalized stats artifacts are still useful for compact summaries, but the final detailed path dumps in `nextpnr.log` are the right source for path-level root-cause analysis.

### Resource Summary

| Metric | Value | Available | Utilization |
| --- | ---: | ---: | ---: |
| Logic cells (`ICESTORM_LC`) | 5661 | 7680 | 73% |
| BRAM (`ICESTORM_RAM`) | 30 | 32 | 93% |
| Global buffers (`SB_GB`) | 8 | 8 | 100% |
| IOs (`SB_IO`) | 77 | 256 | 30% |
| PLLs (`ICESTORM_PLL`) | 1 | 2 | 50% |

### Post-Synthesis Cell Counts

| Cell | Count |
| --- | ---: |
| `SB_LUT4` | 4569 |
| `SB_CARRY` | 943 |
| `SB_DFF` | 351 |
| `SB_DFFE` | 72 |
| `SB_DFFESR` | 1624 |
| `SB_DFFESS` | 2 |
| `SB_DFFSR` | 421 |
| `SB_DFFSS` | 12 |
| `SB_RAM40_4K` | 30 |

### Note on the Fmax Numbers

The normalized stats output reports **70.45 MHz**, while the final detailed synchronous path section of `nextpnr.log` reports a **15.1 ns** worst path, equivalent to **66.12 MHz**. For optimization work, the **66.12 MHz figure is the more useful one**, because it is tied directly to the concrete source/destination pair and routed delay breakdown of the actual worst path.

---

## Critical Path #1 — Host Read-Response Burst Control to Host Address Update

**Timing relevance:** **Primary synchronous bottleneck**  
**Domain:** `pll_clk_global` (posedge → posedge)  
**Source:** `host_req_burst_len_m1` register in `host_bus_interface.sv`  
**Destination:** `host_curr_addr` clock-enable path in `host_bus_interface.sv`  
**Delay:** **15.1 ns total**  
**Breakdown:** **6.1 ns logic + 9.1 ns routing**

### RTL Path Narrative

The longest path starts in the stored host-burst metadata and flows into the read-response formatter in `rtl/common/io/host_bus_interface.sv`:

```systemverilog
tx_pkt_start = (host_beats_remaining == ({1'b0, host_req_burst_len_m1} + 17'd1));
tx_pkt_last  = (host_beats_remaining == 17'd1);
tx_pkt_src_fixed = host_req_src_fixed;
```

That control then reconverges with the logic that conditionally advances the host address after a response beat is accepted:

```systemverilog
if (!host_req_src_fixed) begin
    host_curr_addr <= host_curr_addr + {{29{1'b0}}, host_stride};
end
```

At a high level, the path is:

```text
host_req_burst_len_m1[DFF]
  -> burst_len+1 / equality compare for tx_pkt_start
  -> HOST_READ_TX packet-control muxing
  -> tx handshake acceptance logic
  -> host_req_src_fixed / host_state reconvergence
  -> host_curr_addr clock-enable logic[DFF]
```

### Why This Path Is Slow

1. **The "first beat" decision is recomputed combinationally.**  
   `tx_pkt_start` is derived from a full-width compare against `host_req_burst_len_m1 + 1`, so the design pays for an increment and equality test in the hottest control cone instead of reusing a predecoded flag.

2. **The result does not terminate locally.**  
   The compare output keeps traveling through TX packet formatting and acceptance logic before coming back into host-side state/update control. That reconvergence is what turns a modest arithmetic check into the dominant routed control path.

3. **The path crosses a module boundary through handshake logic.**  
   `host_bus_interface` computes packet metadata, but `host_bus_tx` contributes the ready/accept behavior that feeds back into the address-update decision. That makes the cone deeper and harder to place compactly.

4. **Routing is the biggest penalty.**  
   The path is **more routing-limited than logic-limited**: 9.1 ns of the 15.1 ns total is routing. The detailed `nextpnr` dump shows a late long hop from roughly `(26,28)` through `(14,31)` into `(13,32)`, which is exactly the kind of cross-device control route that hurts HX8K timing.

### Actionable Fixes

1. **Precompute and register a "first read beat" flag** when the request is accepted, instead of recomputing  
   `host_beats_remaining == ({1'b0, host_req_burst_len_m1} + 17'd1)` inside `HOST_READ_TX`.

2. **Break the control cone at the read-response boundary.**  
   Register the metadata that `host_bus_tx` needs (`start`, `last`, and any fixed-address attributes) before the TX handshake stage.

3. **Decouple address-update enable from packet-start formatting.**  
   The `host_curr_addr` update decision should not have to wait on the same combinational logic that decides whether the outgoing packet is the first beat.

4. **Keep the read-response bookkeeping physically local.**  
   Any RTL refactor that clusters `host_beats_remaining`, `host_req_src_fixed`, `host_state`, and the `host_curr_addr` enable path should reduce the long routed hop more effectively than micro-optimizing individual LUTs.

---

## Critical Path #2 — Reset Debouncer Terminal Count to Debounced Output Enable

**Timing relevance:** **Real synchronous path, but not the main system limiter**  
**Domain:** `clk$SB_IO_IN` (posedge → posedge)  
**Source:** `stable_counter` in `rst_n_btn_debouncer_inst`  
**Destination:** `dout` clock-enable path in `rst_n_btn_debouncer_inst`  
**Delay:** **6.5 ns total**  
**Breakdown:** **2.3 ns logic + 4.2 ns routing**

### RTL Path Narrative

This path is inside `rtl/common/primitives/debouncer.sv`, which is instantiated for the reset button in `ice40_alchitry_cu_top.sv`:

```systemverilog
else if (stable_counter == STABLE_COUNT_MAX) begin
    stable_counter <= '0;
    dout <= din;
end else begin
    stable_counter <= stable_counter + 1'b1;
end
```

The path is the classic synchronous terminal-count problem:

```text
stable_counter[DFF]
  -> equality compare against STABLE_COUNT_MAX
  -> output-update enable generation
  -> dout clock-enable[DFF]
```

### Why This Path Is Slow

1. **The compare is wide enough to matter.**  
   The debouncer uses a multi-bit counter to enforce the requested stable time, so the terminal-count detect is not free.

2. **It is still routing-heavy.**  
   Even this relatively small path spends more time in routing than logic, which is consistent with the overall placement pressure on the HX8K target.

3. **It is isolated from the main CPU clock domain.**  
   This is a real synchronous path, but it lives in the raw board-clock domain of the reset debouncer. It is not the performance limit of the CPU subsystem.

### Actionable Fixes

1. **Use a saturating or down-counter formulation** so the terminal condition becomes simpler.
2. **Register the terminal-count result** before it drives the `dout` enable if this path ever becomes relevant.
3. **Reduce debounce counter width** only if board-level debounce requirements allow it.

---

## Critical Path #3 — Reset Button Pad to First Synchronizer Stage

**Timing relevance:** **Not a system Fmax bottleneck; top async input path**  
**Domain:** `<async> -> posedge clk$SB_IO_IN`  
**Source:** `rst_n_btn` input pad  
**Destination:** first stage of `rst_n_btn_sync_inst`  
**Delay:** **1.7 ns total**  
**Breakdown:** **0.5 ns logic + 1.3 ns routing**

### RTL Path Narrative

The raw reset button is synchronized directly in `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv`:

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

### Why This Path Is Long

1. **Almost all of the delay is pad routing.**  
   There is essentially no meaningful logic here.

2. **This is the correct structure for an asynchronous input.**  
   The pad feeds the synchronizer directly, which is exactly what should happen for metastability containment.

3. **It is not timing-critical for system performance.**  
   This path only appears because it is the longest async-to-clocked path in that domain after the synchronous reports are listed.

### Actionable Fixes

1. No immediate RTL change is recommended.
2. If the path ever grows, **keep the first synchronizer flop physically close to the pad**.
3. Continue to avoid any logic in front of the synchronizer.

---

## Critical Path #4 — IO Button Pad to System-Clock Synchronizer

**Timing relevance:** **Not a system Fmax bottleneck; top async input path into `pll_clk_global`**  
**Domain:** `<async> -> posedge pll_clk_global`  
**Source:** `io_button[1]` input pad  
**Destination:** first stage of `io_button_sync_inst`  
**Delay:** **2.9 ns total**  
**Breakdown:** **0.5 ns logic + 2.4 ns routing**

### RTL Path Narrative

The five IO-shield buttons are synchronized before they enter the rest of the system logic:

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

1. **The path is pad-to-flop routing dominated.**
2. **The source pad and synchronizer are not adjacent.**  
   The detailed route spans roughly `(33,10)` to `(31,26)`, which is still harmless at 25 MHz but explains why this is the top async path into the system domain.
3. **The shared 5-bit synchronizer is convenient RTL, but placement is still per-bit physical routing.**

### Actionable Fixes

1. If this ever matters, **split the button synchronizers per bit** so the placer has more freedom to keep each first-stage flop close to its pad.
2. Preserve the current direct pad-to-synchronizer structure.
3. Add placement guidance only if future builds show these input paths becoming materially worse.

---

## Critical Path #5 — LED Fanout from Internal Logic to Output Pad

**Timing relevance:** **Not a system Fmax bottleneck; top registered-to-output boundary path**  
**Domain:** `posedge pll_clk_global -> <async>`  
**Source:** LED-control logic in `fpga_common_top_inst.cpu_inst.led_ctrl`  
**Destination:** `io_led[*]` output pad  
**Delay:** **4.7 ns total**  
**Breakdown:** **0.5 ns logic + 4.2 ns routing**

### RTL Path Narrative

The top-level wrapper replicates one internal 8-bit LED value onto three 8-bit IO-shield banks:

```systemverilog
assign io_led[7:0]   = led_out;
assign io_led[15:8]  = led_out;
assign io_led[23:16] = led_out;
```

The timing dump identifies one of those replicated sinks as the longest output route. This is a fanout problem, not a logic-depth problem.

### Why This Path Is Long

1. **A single internal source fans out to many distant pads.**
2. **Routing dominates the delay.**  
   The detailed path shows a long cross-device route from around `(6,14)` to `(33,28)`.
3. **The path is benign at the current target frequency.**  
   It is included because it is the top output-boundary path, not because it threatens 25 MHz closure.

### Actionable Fixes

1. **Register or duplicate LED fanout drivers** closer to the output banks if additional fanout is added later.
2. Keep the current simple direct assignments unless the IO wrapper grows further.
3. Treat this path as a placement/fanout signal, not as an immediate optimization priority.

---

## Structural Timing Pressure

Even though the current build passes comfortably, the fresh reports still show two structural limits that will matter for future work.

### 1. Global-buffer saturation

The routed design uses **all 8 global buffers** on the HX8K.

That matters because once `SB_GB` is fully consumed, any new high-fanout control has to use ordinary fabric routing. The worst host-bus path is already routing-limited, so additional high-fanout control would make long reconvergent paths even more fragile.

### 2. BRAM saturation

The design uses **30 / 32 BRAMs**.

That leaves very little freedom for architectural timing fixes that rely on extra buffering or replicated local storage. Timing work on this target will need to focus on **control decomposition and placement-friendly RTL**, not on broad structural buffering.

---

## Recommended Optimization Order

If more timing margin is needed on the iCE40 target, the best next steps are:

1. **Register the host read-response first/last-beat metadata**
2. **Break the combinational control boundary on the `HOST_READ_TX` path**
3. **Localize `host_beats_remaining`, `host_req_src_fixed`, `host_state`, and `host_curr_addr` update control**
4. **Avoid introducing new high-fanout control nets while `SB_GB` remains fully utilized**
5. **Only then consider secondary cleanup on the reset debouncer terminal-count path**

That priority order follows directly from the report data: the current design is limited by a **routed host-control cone**, not by arithmetic datapath logic or wrapper IO logic.

---

## Bottom Line

The current iCE40 build is healthy and comfortably exceeds the 25 MHz requirement, but the dominant timing constraint is now clearly the **host read-response control plane**:

- **not** the CPU ALU,
- **not** the seven-segment wrapper,
- **not** the LED output network,
- and **not** the asynchronous input synchronizers.

The most valuable future timing improvement would be to **replace recomputed burst-boundary decisions with registered local flags** so the host TX formatter and the host address-update path no longer share one long routed combinational cone.
