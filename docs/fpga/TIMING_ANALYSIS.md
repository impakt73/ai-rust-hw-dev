# iCE40-HX8K Critical Path & Timing Analysis

**Date:** 2026-03-16  
**Target device:** Lattice iCE40-HX8K-CB132 (Alchitry Cu v1)  
**Clock target:** 25.00 MHz (`pll_clk_global`)  
**Build command:** `cd rtl/fpga && make TARGET=ice40_alchitry_cu stats STATS_FORMAT=json`

---

## Executive Summary

The fresh `ice40_alchitry_cu` build still closes timing comfortably at the
25 MHz target, but the dominant routed synchronous bottleneck has shifted again.
The current limiter is now a **host-bus control/enable cone** inside
`host_bus_interface`, not the previously documented registered-bus / SRAM
response path.

- **Authoritative post-route Fmax used for this analysis:** **74.64 MHz**
- **Timing status:** **PASS**
- **Margin vs. 25.00 MHz target:** **+49.64 MHz** (**+198.6%**)
- **Worst synchronous routed delay:** **13.4 ns** on `pll_clk_global`

The most important conclusion from the fresh routed dump is:

1. **The top synchronous path is routing-dominated host-bus address-update control**
2. **The only other reported synchronous hotspot is the reset-button debouncer**
3. **All remaining reported paths are async-input or reg-to-output locality paths**

### Final Timing Snapshot

| Category | Result |
| --- | ---: |
| `pll_clk_global` max frequency | **74.64 MHz** |
| `clk$SB_IO_IN` max frequency | **179.34 MHz** |
| `<async> -> posedge clk$SB_IO_IN` max delay | **4.57 ns** |
| `<async> -> posedge pll_clk_global` max delay | **1.75 ns** |
| `posedge pll_clk_global -> <async>` max delay | **4.40 ns** |

### Fresh Resource Summary

| Metric | Value | Available | Utilization |
| --- | ---: | ---: | ---: |
| Logic cells (`ICESTORM_LC`) | 5841 | 7680 | 76% |
| BRAM (`ICESTORM_RAM`) | 30 | 32 | 93% |
| Global buffers (`SB_GB`) | 8 | 8 | 100% |
| IOs (`SB_IO`) | 77 | 256 | 30% |
| PLLs (`ICESTORM_PLL`) | 1 | 2 | 50% |

### Fresh Cell Counts

| Cell | Count |
| --- | ---: |
| `SB_LUT4` | 4465 |
| `SB_CARRY` | 929 |
| `SB_DFF` | 359 |
| `SB_DFFE` | 689 |
| `SB_DFFESR` | 1216 |
| `SB_DFFESS` | 5 |
| `SB_DFFSR` | 430 |
| `SB_DFFSS` | 12 |
| `SB_RAM40_4K` | 30 |
| `SB_PLL40_CORE` | 1 |

---

## Fresh-Build Confirmation

This report is based on a fresh local rebuild of the iCE40 target using the
required stats flow:

```bash
cd rtl/fpga
make TARGET=ice40_alchitry_cu stats STATS_FORMAT=json
```

For this run, `rtl/fpga/build/ice40_alchitry_cu/riscv_fpga_timing.rpt` was **not**
generated, so the analysis below uses the **final routed timing section** near the
end of `rtl/fpga/build/ice40_alchitry_cu/nextpnr.log` as the authoritative timing
source.

---

## Authoritative Artifacts Used

Fresh artifacts for this analysis:

- `rtl/fpga/build/ice40_alchitry_cu/nextpnr.log` — authoritative routed timing
  details and post-route resource summary
- `rtl/fpga/build/ice40_alchitry_cu/yosys.log` — synthesis cell counts
- `rtl/fpga/build/ice40_alchitry_cu/riscv_fpga_stats.json` — normalized summary
  cross-check
- `rtl/fpga/build/ice40_alchitry_cu/riscv_fpga_stats.md` — normalized markdown
  cross-check

The top synchronous-path ranking in this document comes from the **final routed
timing section** in `nextpnr.log`, not from earlier placement summaries or from
older markdown reports.

---

## Cross-Check Note: Normalized Stats vs. Raw Routed Log

The normalized stats artifact and the final routed log do **not** agree on the
headline `pll_clk_global` frequency for this build:

- `riscv_fpga_stats.json` reports **75.69 MHz**
- the final routed timing summary in `nextpnr.log` reports **74.64 MHz**

That **1.05 MHz mismatch** means the normalized stats flow captured an earlier
clock summary from `nextpnr.log`, not the final routed one. For that reason:

- use **`riscv_fpga_stats.json` / `.md`** for resource and build-summary checks
- use the **final routed timing section in `nextpnr.log`** for Fmax and
  path-by-path critical-path analysis

The raw routed log is the source of truth for this report.

---

## Critical Path Ranking

Because this iCE40 build fell back to the final `nextpnr.log` timing section
instead of a standalone timing report, the flow exposes only the worst reported
path per timing class/domain. The ranking below therefore lists the reported
**synchronous** paths first, followed by the async paths separately.

| Rank | Path class | Domain | Total delay | Logic | Routing |
| --- | --- | --- | ---: | ---: | ---: |
| 1 | Host-bus first-beat / address-update enable cone | `pll_clk_global` | **13.4 ns** | **2.8 ns** | **10.6 ns** |
| 2 | Reset-button debouncer counter -> output-enable cone | `clk$SB_IO_IN` | **5.6 ns** | **2.3 ns** | **3.3 ns** |
| 3 | Reset button pad -> first synchronizer stage | `<async> -> posedge clk$SB_IO_IN` | **4.6 ns** | **0.5 ns** | **4.1 ns** |
| 4 | LED register -> output pad | `posedge pll_clk_global -> <async>` | **4.4 ns** | **0.5 ns** | **3.9 ns** |
| 5 | IO button pad -> first synchronizer stage | `<async> -> posedge pll_clk_global` | **1.7 ns** | **0.5 ns** | **1.3 ns** |

The key architectural takeaway is that the internal system-clock limit is now
set by a **routing-heavy host-bus control path**, not by arithmetic or by the
registered-bus response logic that dominated an older build.

---

## Critical Path #1 — Host-Bus First-Beat Control into `host_curr_addr` Enable

**Domain / class:** `pll_clk_global` (posedge -> posedge)  
**Startpoint:** `fpga_common_top_inst.cpu_inst.host_bus_if.host_read_first_beat...O`  
**Endpoint:** `fpga_common_top_inst.cpu_inst.host_bus_if.host_curr_addr...CEN`  
**Delay:** **13.4 ns total**  
**Breakdown:** **2.8 ns logic + 10.6 ns routing**  
**Classification:** **Routing-dominated synchronous control path**

### RTL Path Narrative

This is the dominant synchronous path in the fresh routed build. The launch flop
is `host_read_first_beat`, a host-read framing flag in `host_bus_interface`. The
path then crosses a reconvergent control cone that touches transmit framing,
receive-side packet state, host-state sequencing, and fixed-address handling
before it lands on the **clock enable** of `host_curr_addr`.

At a high level, the path is:

```text
host_read_first_beat[DFF]
  -> HOST_READ_TX packet framing (`tx_pkt_start`)
  -> host_bus_tx payload-enable / serializer control
  -> host_bus_rx packet output state interaction
  -> host_state / fixed-address control reconstruction
  -> host_curr_addr increment-enable[DFF]
```

### Relevant RTL

- `rtl/common/io/host_bus_interface.sv:241-253` — `HOST_READ_TX` transmit packet
  framing uses `host_read_first_beat`
- `rtl/common/io/host_bus_interface.sv:333-357` — host read request setup and
  initialization of `host_read_first_beat` and `host_curr_addr`
- `rtl/common/io/host_bus_interface.sv:407-419` — `HOST_READ_TX` clears
  `host_read_first_beat` and conditionally increments `host_curr_addr`
- `rtl/common/io/host_bus_interface.sv:367-376` — write-side address increment
  logic shares the same `host_curr_addr` enable structure
- `rtl/common/io/host_bus_tx.sv:71-73,94-101,154-181` — payload-enabled state
  machine and capture logic that reconverges with first-beat framing
- `rtl/common/io/host_bus_rx.sv:85-99,187-220` — output buffering and packet
  emission state that also participates in the control cone seen by the router
- `rtl/common/top.sv:373-414` — `host_bus_interface` instantiation inside the
  shared RTL top
- `rtl/fpga/common/fpga_common_top.sv:34-80` — `top` wrapper and UART hookup
- `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv:115-128` — iCE40 board
  top-level wrapper

### Why This Path Is Long

1. **The endpoint is a clock-enable input, not a plain D input.**  
   The tool has to rebuild the full host address-update decision before reaching
   `host_curr_addr...CEN`, which creates a deeper and harder-to-place control
   cone than a simple datapath register input.

2. **Several host-side control decisions reconverge in one cycle.**  
   `host_read_first_beat`, `host_state`, the fixed-address flags, TX payload
   handling, and RX packet buffering all influence whether `host_curr_addr`
   should advance.

3. **Routing clearly dominates the delay.**  
   The path is **10.6 ns routing** versus only **2.8 ns logic**, so the main
   problem is long control-net travel and locality, not a large number of LUT
   levels.

4. **The design is already under routing pressure.**  
   The target is at **8 / 8 global buffers** and **30 / 32 BRAMs**, so the
   placer/router has limited flexibility for long, high-fanout control nets.

### Actionable Optimization Suggestions

1. **Split `host_curr_addr` control into explicit local enables.**  
   In the `HOST_IDLE`, `HOST_WRITE_D`, and `HOST_READ_TX` control points
   (`rtl/common/io/host_bus_interface.sv:342-343,373-375,416-417`), predecode
   separate signals such as `host_curr_addr_load`, `host_curr_addr_inc_read`,
   `host_curr_addr_inc_write`, and a final short `host_curr_addr_en`. That
   reduces reconvergence at the destination clock enable.

2. **Decouple `host_read_first_beat` from address-update control.**  
   Move first-beat/header tracking further into `host_bus_tx`, or register a
   local framing flag there, so the same flop no longer fans into both packet
   framing and address-update decisions.

3. **Insert a small register boundary around read-burst advance.**  
   The logic in `HOST_READ_TX` (`rtl/common/io/host_bus_interface.sv:407-419`)
   can be staged so TX handshake completion and `host_curr_addr` advancement do
   not have to reconverge in the same cycle.

4. **Avoid adding more control fan-in to this cone.**  
   Any future host-bus features should avoid feeding additional conditions into
   the `host_curr_addr` enable path unless they are first localized or staged.

---

## Critical Path #2 — Reset-Button Debouncer Counter to `dout` Enable

**Domain / class:** `clk$SB_IO_IN` (posedge -> posedge)  
**Startpoint:** `rst_n_btn_debouncer_inst.stable_counter...O`  
**Endpoint:** `rst_n_btn_debouncer_inst.dout...CEN`  
**Delay:** **5.6 ns total**  
**Breakdown:** **2.3 ns logic + 3.3 ns routing**  
**Classification:** **Mixed, slightly routing-dominated synchronous control path**

### RTL Path Narrative

This is the worst reported synchronous path in the raw board-clock domain. It
starts from a bit of `stable_counter`, runs through the equality / update logic
that decides whether the synchronized reset button has remained stable long
enough, and ends at the debounced output register enable.

At a high level, the path is:

```text
stable_counter[DFF]
  -> `stable_counter == STABLE_COUNT_MAX` compare
  -> `din != dout` / update decision
  -> `dout` clock-enable[DFF]
```

### Relevant RTL

- `rtl/common/primitives/debouncer.sv:26-33` — counter width and
  `STABLE_COUNT_MAX`
- `rtl/common/primitives/debouncer.sv:50-63` — counter update, terminal-count
  detection, and `dout` update logic
- `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv:58-75` — reset button
  synchronizer and debouncer instantiation

### Why This Path Is Long

1. **The terminal-count test sits directly in the update loop.**  
   The debouncer checks `stable_counter == STABLE_COUNT_MAX` every cycle while
   `din != dout`, so the equality compare is part of the hot path.

2. **The path ends on a clock enable.**  
   Like Path #1, the destination is a `CEN` input, which makes the result a
   control-path timing problem rather than a simple register D-input path.

3. **It is still far from threatening the design target.**  
   At **5.6 ns**, this path is well inside the 25 MHz budget and is clearly a
   secondary concern relative to the 13.4 ns host-bus path.

### Actionable Optimization Suggestions

1. **Use a saturating counter or registered terminal flag.**  
   In `rtl/common/primitives/debouncer.sv:55-61`, a saturating counter plus a
   one-bit `stable_done` flag would shorten the terminal-count compare cone.

2. **Register the terminal-count pulse before updating `dout`.**  
   If one extra debounce cycle is acceptable, staging the compare result would
   split the equality logic from the output-enable decision.

3. **Keep this as a lower-priority cleanup item.**  
   It is only worth touching if button logic grows or if the main host-bus path
   has already been optimized.

---

## Async Paths (Informational, Not Ranked Ahead of Synchronous Bottlenecks)

### Reset Button Pad -> First Synchronizer Stage

- **Domain / class:** `<async> -> posedge clk$SB_IO_IN`
- **Delay:** **4.57 ns total** = **0.5 ns logic + 4.1 ns routing**
- **RTL grounding:** `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv:58-65`,
  `rtl/common/primitives/ff_sync.sv:22-37`
- **Interpretation:** this is a normal pad-to-first-stage synchronizer route; the
  cost is almost entirely physical IO-to-flop locality.

### LED Register -> Output Pad

- **Domain / class:** `posedge pll_clk_global -> <async>`
- **Delay:** **4.40 ns total** = **0.5 ns logic + 3.9 ns routing**
- **RTL grounding:** `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv:108-139`
- **Interpretation:** this is a reg-to-pad route dominated by pad locality. It
  does not set the internal Fmax ceiling.

### IO Button Pad -> First Synchronizer Stage

- **Domain / class:** `<async> -> posedge pll_clk_global`
- **Delay:** **1.75 ns total** = **0.5 ns logic + 1.3 ns routing**
- **RTL grounding:** `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv:153-158`,
  `rtl/common/primitives/ff_sync.sv:22-37`
- **Interpretation:** architecturally clean CDC capture path; not a current
  optimization target.

---

## Structural Timing Pressure

Even though timing is healthy, the resource summary still highlights two layout
constraints that matter for future optimization work:

### 1. Global-buffer saturation

`SB_GB` usage is **8 / 8**. Additional high-fanout control or reset nets are
therefore more likely to stay on ordinary routing resources, which makes the
routing-dominated host-bus enable cone harder to close at higher frequencies.

### 2. BRAM saturation

`ICESTORM_RAM` usage is **30 / 32**. The design still fits, but there is limited
room to solve timing by adding more buffering or local scratch storage.

---

## Recommended Optimization Order

If more iCE40 timing margin is needed later, the highest-value next steps are:

1. **Localize and stage the `host_curr_addr` enable cone in `host_bus_interface`**
2. **Move first-beat framing state closer to `host_bus_tx` so it no longer
   reconverges with address-update control**
3. **Only then consider simplifying the debouncer terminal-count path**
4. **Treat the async IO paths as placement-locality observations, not core
   timing-closure priorities**

---

## Bottom Line

The fresh iCE40 routed build still closes timing with almost **3x** system-clock
headroom relative to the 25 MHz target, but the highest-value optimization focus
has changed.

The current routed bottleneck is:

- **not** an ALU datapath cone
- **not** the older registered-bus / SRAM ready-feedback path
- **but** a **routing-heavy host-bus control path** from `host_read_first_beat`
  into the `host_curr_addr` enable logic

So the clearest next optimization for this target is to **decompose and localize
that host-bus control cone first**, while treating the debouncer as a smaller
secondary synchronous path and the remaining reported async paths as
informational locality checks.
