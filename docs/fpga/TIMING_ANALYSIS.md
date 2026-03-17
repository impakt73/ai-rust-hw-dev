# iCE40-HX8K Critical Path & Timing Analysis

**Date:** 2026-03-17  
**Target device:** Lattice iCE40-HX8K-CB132 (Alchitry Cu v1)  
**Clock target:** 25.00 MHz (`pll_clk_global`)  
**Build command:** `cd rtl/fpga && make TARGET=ice40_alchitry_cu stats STATS_FORMAT=json`

---

## Fresh-Build Confirmation

This report is based on a fresh local rebuild of the iCE40 target using the
required stats flow:

```bash
cd rtl/fpga
make TARGET=ice40_alchitry_cu stats STATS_FORMAT=json
```

For this run, `rtl/fpga/build/ice40_alchitry_cu/riscv_fpga_timing.rpt` was **not**
generated because the local `nextpnr-ice40` build does not expose `--report`.
Accordingly, the analysis below uses the **final routed timing section** near the
end of `rtl/fpga/build/ice40_alchitry_cu/nextpnr.log` as the authoritative timing
source, with `riscv_fpga_stats.json` used only for cross-checking summary data.

---

## Authoritative Artifacts Used

Fresh artifacts for this analysis:

- `rtl/fpga/build/ice40_alchitry_cu/nextpnr.log` — authoritative routed timing
  details and final post-route timing summary
- `rtl/fpga/build/ice40_alchitry_cu/yosys.log` — synthesis cell counts
- `rtl/fpga/build/ice40_alchitry_cu/riscv_fpga_stats.json` — normalized summary
  cross-check
- `rtl/fpga/build/ice40_alchitry_cu/riscv_fpga_stats.md` — normalized markdown
  cross-check

The top-path ranking in this document comes from the **final routed timing
section** in `nextpnr.log`, not from earlier placement/timing snapshots and not
from older timing-analysis markdown.

---

## Executive Summary

The fresh `ice40_alchitry_cu` build still closes timing comfortably at the
25 MHz target, but the dominant routed synchronous bottleneck is now a
**registered-bus request/decode/control-enable path** rather than the older ALU
compare cone described in the previous report.

- **Authoritative post-route Fmax used for this analysis:** **77.74 MHz**
- **Timing status:** **PASS**
- **Margin vs. 25.00 MHz target:** **+52.74 MHz** (**+210.96% above target**)
- **Worst synchronous routed delay:** **12.9 ns** on `pll_clk_global`

The most important conclusions from the fresh routed dump are:

1. **The top synchronous path is a routed control cone from system-controller
   request valid into the registered bus request-issue enable path**
2. **The secondary synchronous path remains the reset-button debouncer in the
   raw input clock domain, and it is strongly routing-dominated**
3. **The normalized stats JSON again captured an earlier timing snapshot, so the
   final routed `nextpnr.log` must remain the timing source of truth**

### Final Timing Snapshot

| Category | Result |
| --- | ---: |
| `pll_clk_global` max frequency | **77.74 MHz** |
| `clk$SB_IO_IN` max frequency | **162.34 MHz** |
| `<async> -> posedge clk$SB_IO_IN` max delay | **1.75 ns** |
| `<async> -> posedge pll_clk_global` max delay | **2.12 ns** |
| `posedge pll_clk_global -> <async>` max delay | **5.43 ns** |

### Fresh Resource Summary

| Metric | Value | Available | Utilization |
| --- | ---: | ---: | ---: |
| Logic cells (`ICESTORM_LC`) | 6131 | 7680 | 79% |
| BRAM (`ICESTORM_RAM`) | 30 | 32 | 93% |
| Global buffers (`SB_GB`) | 8 | 8 | 100% |
| IOs (`SB_IO`) | 77 | 256 | 30% |
| PLLs (`ICESTORM_PLL`) | 1 | 2 | 50% |

### Fresh Cell Counts

| Cell | Count |
| --- | ---: |
| `SB_LUT4` | 4780 |
| `SB_CARRY` | 960 |
| `SB_DFF` | 363 |
| `SB_DFFE` | 983 |
| `SB_DFFESR` | 1064 |
| `SB_DFFESS` | 4 |
| `SB_DFFSR` | 437 |
| `SB_DFFSS` | 12 |
| `SB_RAM40_4K` | 30 |
| `SB_PLL40_CORE` | 1 |

---

## Cross-Check Note: Normalized Stats vs. Raw Routed Log

The normalized stats artifact and the final routed log do **not** agree on the
headline `pll_clk_global` frequency for this build:

- `riscv_fpga_stats.json` reports **77.42 MHz**
- an earlier timing snapshot in `nextpnr.log` also reports **77.42 MHz**
- the **final routed timing summary** in `nextpnr.log` reports **77.74 MHz**

That **0.32 MHz mismatch** means the normalized stats flow again captured an
earlier clock summary from `nextpnr.log`, not the final routed one. For that
reason:

- use **`riscv_fpga_stats.json` / `.md`** for resource and build-summary checks
- use the **final routed timing section in `nextpnr.log`** for Fmax and
  path-by-path critical-path analysis

The raw routed log is the timing source of truth for this report.

---

## Critical Path Ranking

Because this iCE40 build fell back to the final `nextpnr.log` timing section
instead of a standalone `riscv_fpga_timing.rpt`, the flow exposes the worst
reported path per timing class/domain. The ranking below therefore lists the
reported **synchronous** paths first, followed by the async paths separately.

| Rank | Path class | Domain | Total delay | Logic | Routing |
| --- | --- | --- | ---: | ---: | ---: |
| 1 | Registered-bus request-valid / decode / issue-enable cone | `pll_clk_global` | **12.9 ns** | **3.6 ns** | **9.2 ns** |
| 2 | Reset-button debouncer terminal-flag feedback -> counter clear control | `clk$SB_IO_IN` | **6.2 ns** | **1.0 ns** | **5.1 ns** |
| 3 | Seven-segment position register -> output pad | `posedge pll_clk_global -> <async>` | **5.43 ns** | **0.9 ns** | **4.5 ns** |
| 4 | IO button pad -> first synchronizer stage | `<async> -> posedge pll_clk_global` | **2.12 ns** | **0.5 ns** | **1.7 ns** |
| 5 | Reset button pad -> first synchronizer stage | `<async> -> posedge clk$SB_IO_IN` | **1.75 ns** | **0.5 ns** | **1.3 ns** |

The key architectural takeaway from the fresh routed build is that the
system-clock limit is now set by a **routing-heavy registered-bus control path**
instead of the earlier ALU path.

---

## Critical Path #1 — Registered-Bus Request Decode / Issue-Enable Cone

**Domain / class:** `pll_clk_global` (posedge -> posedge)  
**Startpoint:** `fpga_common_top_inst.cpu_inst.sysctrl.mem_a_valid_SB_LUT4_I2_LC.O`  
**Endpoint:** `fpga_common_top_inst.cpu_inst.rtl_registered_bus.slave_mem_a_wdata_int[0]_SB_DFFE_Q_17_DFFLC.CEN`  
**Delay:** **12.9 ns total**  
**Breakdown:** **3.6 ns logic + 9.2 ns routing**  
**Classification:** **Routing-dominated synchronous control path**

### RTL Path Narrative

This is the dominant synchronous path in the fresh routed build. The launch side
originates from the system-controller request-valid path that is packed into
`registered_slave_mem_a_valid[0]` wiring in `top.sv`. From there, the path moves
through the registered-bus slave decode and request-issue control cone, including
the combinational logic that derives `clock_mem_a_valid`, reconverges with the
registered-bus `decoded_slave_idx` / issue logic, and ultimately feeds the clock
enable on a register inside `rtl_registered_bus`.

At a high level, the path is:

```text
system_controller mem_a_valid
  -> registered_slave_mem_a_valid[0]
  -> top-level slave-valid wiring / clock-peripheral-valid decode
  -> registered_bus decoded_slave_idx / issue-control LUT cone
  -> slave request storage register clock enable
```

Even though the endpoint lands on `slave_mem_a_wdata_int[0]`, the reported sink
is the **clock-enable control** for that register rather than a wide data-input
cone. This is why the path behaves like a control/locality problem instead of a
32-bit datapath problem.

### Relevant RTL

- `rtl/common/top.sv:293-321` — unpacking `registered_slave_mem_a_*` signals to
  `sysctrl_*`, `clock_*`, and other peripheral interfaces
- `rtl/common/top.sv:340-370` — `registered_bus` instantiation that receives the
  packed slave-valid and slave-ready vectors
- `rtl/common/top.sv:487-501` — `clock_peripheral` interface using
  `clock_mem_a_valid` / `clock_mem_a_ready`
- `rtl/common/top.sv:525-537` — `system_controller` interface using
  `sysctrl_mem_a_valid` / `sysctrl_mem_a_ready`
- `rtl/common/memory/registered_bus.sv:142-154` — slave decode for
  `next_decoded_slave_idx` / `next_decoded_slave_valid`
- `rtl/common/memory/registered_bus.sv:171-184` — issue-side selection between
  registered decode state and new decode results
- `rtl/common/memory/registered_bus.sv:205-212` — `decode_load`,
  `slave_req_load`, `slave_req_accept`, and request-output activity control
- `rtl/common/memory/registered_bus.sv:261-268` — request register write/enable
  point where the endpoint clock-enable is generated
- `rtl/fpga/common/fpga_common_top.sv:34-62` — shared FPGA wrapper around `top`
- `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv:115-128` — iCE40 board
  top-level wrapper

### Why This Path Is Long

1. **The cone reconverges multiple control decisions in one cycle.**  
   The path threads through top-level slave-valid wiring, registered-bus slave
   decode, and request-issue gating before it reaches the destination register
   clock enable. That creates a multi-stage control cone even though no wide data
   arithmetic is involved.

2. **Routing dominates the delay.**  
   The path is **9.2 ns routing** versus **3.6 ns logic**, which is a strong
   sign that high fanout and physical locality dominate more than Boolean depth.

3. **Packed vector wiring encourages cross-module locality pressure.**  
   `top.sv` packs four peripheral interfaces into shared vectors, then
   `registered_bus.sv` unpacks and reselects them. That organization is compact
   in RTL, but it increases the chance that decode/control nets stretch across
   the placed design.

4. **The endpoint is a clock-enable pin, which nextpnr treats as timing-critical
   control.**  
   Feeding a DFFE/DFF enable through a reconvergent cone can be expensive on
   iCE40 because the enable network still consumes general routing resources and
   must arrive cleanly at the destination register.

### Actionable Optimization Suggestions

1. **Register the slave decode result earlier inside `registered_bus`.**  
   Splitting the `pending_req_valid -> decode_load -> issue_decoded_* ->
   slave_req_load` chain with an additional register boundary would directly
   shorten the control cone in `rtl/common/memory/registered_bus.sv:171-212`.

2. **Reduce reconvergence between top-level peripheral-valid wiring and bus issue
   control.**  
   If `clock_mem_a_valid` and sibling signals in `rtl/common/top.sv:293-321` are
   only needed after a selected-slave register stage, move that fanout later so
   the issue logic does not depend on broad packed-valid distribution in the same
   cycle.

3. **Prefer a data-write pulse over a complex register clock-enable when
   practical.**  
   The endpoint currently lands on a DFFE clock enable. Restructuring the local
   request register update to reduce enable complexity in
   `rtl/common/memory/registered_bus.sv:261-268` could lower both enable fan-in
   and routing pressure.

4. **Localize the system-controller/clock-peripheral decode path.**  
   The reported path explicitly traverses signals associated with
   `sysctrl_mem_a_valid`, `clock_mem_a_valid`, and `decoded_slave_idx`. Any
   refactor that keeps selected-slave bookkeeping closer to the registered-bus
   state registers should help more than generic "optimize logic" cleanup.

---

## Critical Path #2 — Reset-Button Debouncer Terminal-Flag Feedback

**Domain / class:** `clk$SB_IO_IN` (posedge -> posedge)  
**Startpoint:** `rst_n_btn_debouncer_inst.stable_counter_is_max_SB_DFFSR_Q_D_SB_LUT4_O_LC.O`  
**Endpoint:** `rst_n_btn_debouncer_inst.stable_counter_is_max_SB_DFFSR_Q_D_SB_LUT4_O_LC.SR`  
**Delay:** **6.2 ns total**  
**Breakdown:** **1.0 ns logic + 5.1 ns routing**  
**Classification:** **Routing-dominated synchronous control path**

### RTL Path Narrative

This is the worst reported synchronous path in the raw input-clock domain used by
the reset button conditioning logic. It starts from the registered terminal flag
`stable_counter_is_max`, runs through the small feedback logic that determines
whether the debouncer should clear/restart, and lands on the synthesized clear
control for the same flag/register structure.

At a high level, the path is:

```text
stable_counter_is_max[DFF]
  -> debounce feedback decision
  -> synthesized clear/reset control
```

Compared with the system-clock bottleneck, this path is short and clearly
secondary, but it is still useful because it shows the raw button-conditioning
logic is mostly limited by placement/locality rather than comparator depth.

### Relevant RTL

- `rtl/common/primitives/debouncer.sv:37-38` — `stable_counter` and
  `stable_counter_is_max`
- `rtl/common/primitives/debouncer.sv:53-69` — counter update, terminal-flag
  generation, and output update logic
- `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv:67-75` — reset button
  debouncer instantiation

### Why This Path Is Long

1. **It is almost entirely a routing/locality problem.**  
   The path is **5.1 ns routing** and only **1.0 ns logic**, so logic
   simplification alone will not move it much.

2. **The feedback target is a control pin, not just a data pin.**  
   As in the top path, the destination is synthesized control on a sequential
   element, which tends to be sensitive to routing fanout/locality on iCE40.

3. **The path is far from threatening the target frequency.**  
   At **6.2 ns**, it is comfortably inside the 25 MHz budget and is only a
   cleanup candidate after higher-value system-clock work.

### Actionable Optimization Suggestions

1. **Use a saturating terminal flag rather than feedbacking the clear path
   through the same control structure.**  
   A one-way `stable_done` style flag in `rtl/common/primitives/debouncer.sv:53-69`
   would reduce the amount of control reconvergence on the current path.

2. **Keep the debouncer physically local and avoid unnecessary fanout growth.**  
   Because routing is the dominant cost, preserving compact placement around the
   button-conditioning logic is likely more effective than micro-optimizing the
   compare expression.

3. **Treat this as lower priority than the registered-bus path.**  
   This path is useful to monitor, but it is not the current system Fmax limiter.

---

## Async Paths (Informational, Not Ranked Ahead of Synchronous Bottlenecks)

### Reset Button Pad -> First Synchronizer Stage

- **Domain / class:** `<async> -> posedge clk$SB_IO_IN`
- **Delay:** **1.75 ns total** = **0.5 ns logic + 1.3 ns routing**
- **RTL grounding:** `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv:58-65`,
  `rtl/common/primitives/ff_sync.sv`
- **Interpretation:** normal pad-to-first-stage synchronizer route; informative,
  but not an internal synchronous bottleneck.

### IO Button Pad -> First Synchronizer Stage

- **Domain / class:** `<async> -> posedge pll_clk_global`
- **Delay:** **2.12 ns total** = **0.5 ns logic + 1.7 ns routing**
- **RTL grounding:** `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv:151-158`,
  `rtl/common/primitives/ff_sync.sv`
- **Interpretation:** architecturally clean CDC capture path; not a current
  optimization target.

### Seven-Segment Position Register -> Output Pad

- **Domain / class:** `posedge pll_clk_global -> <async>`
- **Delay:** **5.43 ns total** = **0.9 ns logic + 4.5 ns routing**
- **RTL grounding:** `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv:172-186`
- **Interpretation:** reg-to-pad path dominated by pad locality; it does not set
  the internal Fmax ceiling.

---

## Structural Timing Pressure

Even though timing is healthy, the resource summary still highlights two layout
constraints that matter for future optimization work:

### 1. Global-buffer saturation

`SB_GB` usage is **8 / 8**. Additional high-fanout control or reset nets are
therefore more likely to stay on ordinary routing resources, which makes it
harder to localize wide control cones at higher frequencies.

### 2. BRAM saturation

`ICESTORM_RAM` usage is **30 / 32**. The design still fits, but there is limited
room to solve timing by adding more local storage or buffering structures.

---

## Recommended Optimization Order

If more iCE40 timing margin is needed later, the highest-value next steps are:

1. **Stage the registered-bus decode / issue-enable cone before the destination
   request-register enable**
2. **Reduce same-cycle reconvergence between packed top-level peripheral-valid
   wiring and registered-bus issue control**
3. **Only then consider simplifying the debouncer terminal-flag feedback path**
4. **Treat the async IO paths as placement-locality observations, not core
   timing-closure priorities**

---

## Bottom Line

The fresh iCE40 routed build still closes timing with a little over **3.1x**
system-clock headroom relative to the 25 MHz target, but the highest-value
optimization focus has shifted.

The current routed bottleneck is:

- **not** the earlier ALU compare/output-select path
- **not** an async pad or reg-to-output path
- **but** a **routing-dominated registered-bus request/decode/control-enable
  cone** from system-controller-side valid signaling into the request register
  enable logic in `registered_bus`

So the clearest next optimization for this target is to **decompose and stage the
registered-bus request issue path first**, while treating the debouncer as a
smaller secondary synchronous path and the async paths as informative locality
checks only.
