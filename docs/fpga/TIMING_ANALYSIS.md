# iCE40-HX8K Timing Analysis

**Date:** 2026-03-15  
**Target device:** Lattice iCE40-HX8K-CB132 (Alchitry Cu v1)  
**Clock target:** 25.00 MHz (`pll_clk_global`)  
**Build command:** `cd rtl/fpga && make TARGET=ice40_alchitry_cu stats STATS_FORMAT=json`

---

## Executive Summary

This document reflects a fresh before/after timing study around the top
synchronous path called out in the previous revision of this report.

The implemented fix was to **break the same-cycle slave-response ready feedback**
in `registered_bus` by capturing the winning slave response first and then
issuing a **registered one-cycle slave pop pulse** on the following cycle. That
removed the old routed bottleneck from the top spot.

### Implemented Fix

- **File:** `rtl/common/memory/registered_bus.sv:83-85,171-179,187-236`
- **Change:** Added `slave_response_pop` and now drive `slave_mem_d_ready` from
  that registered pulse instead of directly from the same-cycle combinational
  response-arbitration cone.
- **Intent:** Preserve master-visible response capture while shortening the
  `registered_bus -> sram_peripheral` ready-feedback path that previously ended
  in `sram_periph.mem_d_valid_r` control.

### Before / After Summary

| Metric | Before fix | After fix | Delta |
| --- | ---: | ---: | ---: |
| Raw routed `pll_clk_global` Fmax (`nextpnr.log` final section) | **73.58 MHz** | **75.85 MHz** | **+2.27 MHz** |
| Normalized stats Fmax (`riscv_fpga_stats.json`) | **73.07 MHz** | **77.15 MHz** | **+4.08 MHz** |
| Raw routed top synchronous path delay | **13.6 ns** | **13.2 ns** | **-0.4 ns** |
| Raw routed top synchronous path | Response arbitration -> SRAM `mem_d_valid_r` control | Request hold/release -> SRAM request / BRAM control | Top path changed |
| Logic cells (`ICESTORM_LC`) | 5711 | 5729 | +18 |

The important outcome is not only the Fmax gain, but that the **old critical
feedback arc is no longer the worst synchronous path**.

### Final Timing Snapshot (Post-Fix Build)

The final detailed timing section in `nextpnr.log` reports:

| Category | Result |
| --- | ---: |
| `pll_clk_global` max frequency | **75.85 MHz** |
| `clk$SB_IO_IN` max frequency | **163.51 MHz** |
| `<async> -> posedge clk$SB_IO_IN` max delay | **3.76 ns** |
| `<async> -> posedge pll_clk_global` max delay | **2.81 ns** |
| `posedge pll_clk_global -> <async>` max delay | **5.18 ns** |

The design still closes timing comfortably against the 25 MHz target.

---

## Fresh-Build Confirmation

Two fresh iCE40 builds were run with the same command during this analysis:

```bash
cd rtl/fpga
make TARGET=ice40_alchitry_cu stats STATS_FORMAT=json
```

1. **Pre-fix baseline build** on the unmodified RTL
2. **Post-fix validation build** after the `registered_bus` change

All path ranking below is based on the **final detailed timing section** in the
fresh post-fix `rtl/fpga/build/ice40_alchitry_cu/nextpnr.log`.

---

## Authoritative Artifacts Used

### Post-fix artifacts

- `rtl/fpga/build/ice40_alchitry_cu/nextpnr.log`
- `rtl/fpga/build/ice40_alchitry_cu/yosys.log`
- `rtl/fpga/build/ice40_alchitry_cu/riscv_fpga_stats.json`
- `rtl/fpga/build/ice40_alchitry_cu/riscv_fpga_stats.md`

### Pre-fix comparison data

- Fresh pre-change run of the same build command in this session
- Previous revision of this document for the matching pre-fix raw routed path
  summary

For this iCE40 target:

- use the **final timing section in `nextpnr.log`** for authoritative path-by-path timing
- use **`riscv_fpga_stats.json` / `.md`** for normalized build summaries and utilization

### Fresh Resource Summary (Post-Fix Build)

| Metric | Value | Available | Utilization |
| --- | ---: | ---: | ---: |
| Logic cells (`ICESTORM_LC`) | 5729 | 7680 | 74% |
| BRAM (`ICESTORM_RAM`) | 30 | 32 | 93% |
| Global buffers (`SB_GB`) | 8 | 8 | 100% |
| IOs (`SB_IO`) | 77 | 256 | 30% |
| PLLs (`ICESTORM_PLL`) | 1 | 2 | 50% |

### Fresh Cell Counts (Post-Fix Build)

| Cell | Count |
| --- | ---: |
| `SB_LUT4` | 4559 |
| `SB_CARRY` | 928 |
| `SB_DFF` | 359 |
| `SB_DFFE` | 515 |
| `SB_DFFESR` | 1246 |
| `SB_DFFESS` | 3 |
| `SB_DFFSR` | 428 |
| `SB_DFFSS` | 12 |
| `SB_RAM40_4K` | 30 |
| `SB_PLL40_CORE` | 1 |

---

## Cross-Check Note: Normalized Stats vs. Raw Routed Log

The normalized post-fix stats artifact reports **77.15 MHz** for
`pll_clk_global`, while the final detailed timing section in `nextpnr.log`
reports **75.85 MHz**.

As noted in prior analysis, `fpga_design_stats.py` selects the first matching
preferred clock summary from `nextpnr.log`, while this document deliberately
uses the **final routed timing section** because it contains the authoritative
critical-path dumps.

For this reason:

- use **`riscv_fpga_stats.json` / `.md`** for build-summary and utilization checks
- use the **final routed timing section in `nextpnr.log`** for Fmax and critical-path ranking

---

## Critical Path Ranking (Post-Fix Build)

Synchronous paths are ranked first because they determine internal clock-closing
headroom. Async and reg-to-output paths are kept below them even when the raw
absolute delay is comparable.

| Rank | Path class | Domain | Total delay | Logic | Routing |
| --- | --- | --- | ---: | ---: | ---: |
| 1 | Registered-bus same-slave request hold/release -> SRAM request / BRAM control | `pll_clk_global` | **13.2 ns** | **3.3 ns** | **9.9 ns** |
| 2 | Reset-button debouncer counter -> output enable | `clk$SB_IO_IN` | **6.1 ns** | **2.3 ns** | **3.8 ns** |
| 3 | Seven-segment position register -> `io_seg[2]` output pad | `posedge pll_clk_global -> <async>` | **5.2 ns** | **0.9 ns** | **4.3 ns** |
| 4 | Reset button pad -> raw-clock synchronizer | `<async> -> posedge clk$SB_IO_IN` | **3.8 ns** | **0.5 ns** | **3.3 ns** |
| 5 | IO button pad -> system-clock synchronizer | `<async> -> posedge pll_clk_global` | **2.8 ns** | **0.5 ns** | **2.3 ns** |

The old top path — registered-bus response arbitration -> SRAM
`mem_d_valid_r` control — no longer holds the #1 slot after the fix.

---

## Critical Path #1 — Registered-Bus Same-Slave Request Hold/Release to SRAM Request / BRAM Control

**Domain:** `pll_clk_global` (posedge -> posedge)  
**Source:** `fpga_common_top_inst.cpu_inst.rtl_registered_bus.slave_response_pending...O`  
**Destination:** `fpga_common_top_inst.cpu_inst.sram_periph.sram_inst....SR`  
**Delay:** **13.2 ns total**  
**Breakdown:** **3.3 ns logic + 9.9 ns routing**  
**Classification:** **Routing-dominated**

### What changed

Before the fix, the worst path ran from `registered_bus` response arbitration
through `slave_mem_d_ready` and into the SRAM peripheral's registered D-channel
valid clear logic.

After the fix, the new top synchronous path is different:

```text
registered_bus.slave_response_pending[DFF]
  -> pending/slave-accept gating for same-slave request release
  -> top-level SRAM A-channel wiring
  -> sram_peripheral request/address selection
  -> inferred SRAM read-pipeline / BRAM-adjacent control
```

That change in path identity is the key proof that the original feedback arc was
successfully removed as the top limiter.

### RTL grounding

- `rtl/common/memory/registered_bus.sv:83-85` — `slave_response_pending` and the new `slave_response_pop`
- `rtl/common/memory/registered_bus.sv:163-179` — request dispatch gating and registered slave pop drive
- `rtl/common/memory/registered_bus.sv:187-194` — `slave_req_accept`, `slave_resp_accept`, and response/request acceptance conditions
- `rtl/common/memory/registered_bus.sv:196-236` — sequential update of pending request/response state and the new registered pop pulse
- `rtl/common/top.sv:323-331` — SRAM A/D channel wiring to the registered bus
- `rtl/common/top.sv:340-371` — `registered_bus` instantiation
- `rtl/common/top.sv:507-519` — `sram_peripheral` instantiation
- `rtl/common/peripherals/sram_peripheral.sv:96-101` — SRAM peripheral A/D handshake definitions
- `rtl/common/peripherals/sram_peripheral.sv:178-222` — request-side `sram_waddr` / `sram_raddr` selection
- `rtl/common/memory/sram.sv:51-65` — inferred BRAM read pipeline

### Why this is now the worst path

1. **The response-ready feedback path was shortened, so the next shared control cone surfaced.**  
   `slave_response_pending` still participates in preventing a new request from
   being issued to the same slave until the prior response has been captured.

2. **The cone spans bus control and local SRAM request steering.**  
   The path now couples registered-bus request release logic to the SRAM
   peripheral's address/control selection and BRAM-adjacent logic.

3. **Routing dominates the remaining delay.**  
   The routed report attributes **9.9 ns** of the **13.2 ns** total to routing,
   which indicates locality/fanout pressure more than raw LUT depth.

### Actionable next optimizations

If more iCE40 headroom is needed beyond this fix, the highest-value next steps are:

1. **Precompute per-slave request eligibility closer to the bus state.**  
   Reduce long-distance dependence on `slave_response_pending` when deciding
   whether the buffered request may reissue to SRAM.

2. **Localize the SRAM dispatch cone.**  
   Keep the request-release condition and the SRAM A-channel drive physically and
   logically closer together instead of recomputing them through broader shared
   control terms.

3. **Avoid introducing more cross-block fan-in on the same request-release path.**  
   The old response-ready bottleneck is gone, so this request-side cone is now
   the main place where extra bus-wide control logic would hurt.

---

## Critical Path #2 — Reset-Button Debouncer Counter to Output Enable

**Domain:** `clk$SB_IO_IN` (posedge -> posedge)  
**Delay:** **6.1 ns total** (**2.3 ns logic + 3.8 ns routing**)  
**RTL:** `rtl/common/primitives/debouncer.sv:50-62`

This remains the worst path in the raw board-clock domain. It is still the same
terminal-count/update-enable cone in the debouncer's `stable_counter -> dout`
logic, and it is still materially smaller than the main system-clock path.

---

## Critical Path #3 — Seven-Segment Position Register to `io_seg[2]` Output Pad

**Domain:** `posedge pll_clk_global -> <async>`  
**Delay:** **5.2 ns total** (**0.9 ns logic + 4.3 ns routing**)  
**RTL:** `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv:172-231`

This is still a routing-heavy reg-to-output path dominated by board-IO locality,
not by deep internal datapath logic.

---

## Critical Path #4 — Reset Button Pad to Raw-Clock Synchronizer

**Domain:** `<async> -> posedge clk$SB_IO_IN`  
**Delay:** **3.8 ns total** (**0.5 ns logic + 3.3 ns routing**)  
**RTL:** `rtl/common/primitives/ff_sync.sv:15-50`, `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv:60-75`

This remains an asynchronous input-locality path into the first synchronizer
stage. It matters for placement quality, but it is not the architectural timing
limit for the CPU datapath or bus fabric.

---

## Critical Path #5 — IO Button Pad to System-Clock Synchronizer

**Domain:** `<async> -> posedge pll_clk_global`  
**Delay:** **2.8 ns total** (**0.5 ns logic + 2.3 ns routing**)  
**RTL:** `rtl/common/primitives/ff_sync.sv:15-50`, `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv:151-169`

Like Path #4, this is a synchronizer locality path, not a deep synchronous
combinational bottleneck.

---

## Structural Timing Pressure

Two structural constraints still shape the iCE40 implementation:

1. **Global buffers remain saturated (`SB_GB` = 8 / 8).**  
   Long, high-fanout control nets are still expensive because they cannot rely
   on spare global resources.

2. **BRAM usage remains high (`ICESTORM_RAM` = 30 / 32).**  
   That leaves limited room for solving timing by adding buffering structures or
   duplicating local storage.

The existing RTL guideline still matters here: **do not reset datapath-only
payload registers when a separate `valid`/`pending` flag already guarantees the
payload is ignored while invalid.** Keeping wide payload buses off reset reduces
routing pressure on this device.

---

## Recommended Optimization Order

If more timing margin is needed after this fix, the best order of attack is:

1. **Localize the new request-side `slave_response_pending` -> SRAM dispatch cone**
2. **Simplify the reset-button debouncer terminal-count / enable cone**
3. **Only then consider seven-segment output staging or synchronizer locality cleanup**

That order matters because the first item is now the dominant synchronous path,
and the remaining reported paths are either smaller synchronous housekeeping
logic or IO/CDC locality effects.

---

## Bottom Line

The targeted `registered_bus` change resolved the previous top critical path from
this document.

After a fresh post-fix build:

- the old **response arbitration -> SRAM `mem_d_valid_r` control** path is no
  longer the worst synchronous limiter
- the raw routed `pll_clk_global` Fmax improved from **73.58 MHz** to
  **75.85 MHz**
- the next dominant path is a **different request-side cone**, which confirms
  the original feedback arc was successfully shortened

So the timing-analysis conclusion is now clear: the highest-value fix from the
previous report has been implemented successfully, the design still has roughly
**3x** clock headroom versus the 25 MHz target, and any future timing work
should move on to the new request-release path rather than revisiting the old
SRAM D-channel ready-feedback bottleneck.
