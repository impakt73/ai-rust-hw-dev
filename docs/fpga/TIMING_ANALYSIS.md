# iCE40-HX8K Timing Analysis

**Date:** 2026-03-15  
**Target device:** Lattice iCE40-HX8K-CB132 (Alchitry Cu v1)  
**Clock target:** 25.00 MHz (`pll_clk_global`)  
**Build command:** `cd rtl/fpga && make TARGET=ice40_alchitry_cu stats STATS_FORMAT=json`

---

## Executive Summary

A fresh `ice40_alchitry_cu` build still closes timing comfortably, but the
highest-value timing work has shifted away from the older ALU narrative.

- **Post-route worst synchronous Fmax used for this analysis:** **73.58 MHz**
- **Timing status:** **PASS**
- **Margin vs. 25.00 MHz target:** **+48.58 MHz** (**+194.3%**)

The fresh routed critical-path dump shows that the dominant synchronous limit is
now a **registered-bus response arbitration / ready-feedback control path** that
ends in the SRAM peripheral's registered D-channel valid control.

The top-five timing stories for the current build are:

1. **`pll_clk_global` registered-bus response arbitration -> SRAM `mem_d_valid_r` clear/hold path**
2. **`clk$SB_IO_IN` reset-button debouncer counter -> debounced output enable path**
3. **`pll_clk_global -> <async>` seven-segment position register -> `io_seg[2]` output path**
4. **`<async> -> pll_clk_global` IO button pad -> first synchronizer stage**
5. **`<async> -> clk$SB_IO_IN` reset button pad -> first synchronizer stage**

That ordering is intentional: **synchronous paths are ranked first**, even when
a later reg-to-output path has a comparable raw delay, because the synchronous
paths are the ones that directly set internal clock-closing headroom.

### Final Timing Snapshot

The final detailed timing section in `nextpnr.log` reports:

| Category | Result |
| --- | ---: |
| `pll_clk_global` max frequency | **73.58 MHz** |
| `clk$SB_IO_IN` max frequency | **169.75 MHz** |
| `<async> -> posedge clk$SB_IO_IN` max delay | **1.75 ns** |
| `<async> -> posedge pll_clk_global` max delay | **2.11 ns** |
| `posedge pll_clk_global -> <async>` max delay | **6.18 ns** |

The design still has two placement-pressure points that matter for future work:

- **BRAM:** 30 / 32 blocks (**93%**)
- **Global buffers:** 8 / 8 (**100%**)

Those limits do not threaten the current 25 MHz target, but they do reduce the
placer/router's flexibility for long, high-fanout control paths.

---

## Fresh-Build Confirmation

This report is based on a fresh local run of:

```bash
cd rtl/fpga
make TARGET=ice40_alchitry_cu stats STATS_FORMAT=json
```

The critical-path narrative below is anchored to the **final detailed timing
section** in the freshly generated `nextpnr.log`, not to the previous markdown
report.

---

## Authoritative Artifacts Used

This analysis uses the fresh artifacts in:

- `rtl/fpga/build/ice40_alchitry_cu/nextpnr.log`
- `rtl/fpga/build/ice40_alchitry_cu/yosys.log`
- `rtl/fpga/build/ice40_alchitry_cu/riscv_fpga_stats.json`
- `rtl/fpga/build/ice40_alchitry_cu/riscv_fpga_stats.md`

For this iCE40 target:

- **Detailed critical-path analysis comes from** the final timing section in
  `nextpnr.log`
- **Resource utilization comes from** `nextpnr.log`
- **Post-synthesis cell counts come from** `yosys.log`
- **Normalized summary cross-checks come from** `riscv_fpga_stats.json` and
  `riscv_fpga_stats.md`

### Fresh Resource Summary

| Metric | Value | Available | Utilization |
| --- | ---: | ---: | ---: |
| Logic cells (`ICESTORM_LC`) | 5711 | 7680 | 74% |
| BRAM (`ICESTORM_RAM`) | 30 | 32 | 93% |
| Global buffers (`SB_GB`) | 8 | 8 | 100% |
| IOs (`SB_IO`) | 77 | 256 | 30% |
| PLLs (`ICESTORM_PLL`) | 1 | 2 | 50% |

### Fresh Cell Counts

| Cell | Count |
| --- | ---: |
| `SB_LUT4` | 4545 |
| `SB_CARRY` | 928 |
| `SB_DFF` | 359 |
| `SB_DFFE` | 515 |
| `SB_DFFESR` | 1246 |
| `SB_DFFESS` | 3 |
| `SB_DFFSR` | 424 |
| `SB_DFFSS` | 12 |
| `SB_RAM40_4K` | 30 |
| `SB_PLL40_CORE` | 1 |

---

## Cross-Check Note: Normalized Stats vs. Raw Routed Log

The normalized stats artifacts report **73.07 MHz** for `pll_clk_global`, while
the final detailed timing section in `nextpnr.log` reports **73.58 MHz**.

That difference is expected with the current tooling: `fpga_design_stats.py`
parses all matching clock summaries from `nextpnr.log` and then selects the
first matching preferred clock entry, while this document intentionally uses the
**final routed timing section** because it also contains the authoritative
critical-path dumps.

For this reason:

- use **`riscv_fpga_stats.json` / `.md`** for resource and build-summary checks
- use the **final detailed timing section in `nextpnr.log`** for Fmax and path-by-path timing analysis

---

## Critical Path Ranking

| Rank | Path class | Domain | Total delay | Logic | Routing |
| --- | --- | --- | ---: | ---: | ---: |
| 1 | Registered-bus response arbitration -> SRAM D-channel valid control | `pll_clk_global` | **13.6 ns** | **4.7 ns** | **8.9 ns** |
| 2 | Reset-button debouncer counter -> output enable | `clk$SB_IO_IN` | **5.9 ns** | **2.3 ns** | **3.6 ns** |
| 3 | Seven-segment position register -> output pad | `posedge pll_clk_global -> <async>` | **6.2 ns** | **0.9 ns** | **5.3 ns** |
| 4 | IO button pad -> synchronizer | `<async> -> posedge pll_clk_global` | **2.1 ns** | **0.5 ns** | **1.6 ns** |
| 5 | Reset button pad -> synchronizer | `<async> -> posedge clk$SB_IO_IN` | **1.7 ns** | **0.5 ns** | **1.3 ns** |

The most important conclusion from this ranking is that the dominant timing
pressure is now **global control routing around response handling**, not an ALU
arithmetic cone.

---

## Critical Path #1 — Registered-Bus Response Arbitration to SRAM `mem_d_valid_r` Control

**Domain:** `pll_clk_global` (posedge -> posedge)  
**Source:** `fpga_common_top_inst.cpu_inst.rtl_registered_bus.slave_response_pending...O`  
**Destination:** `fpga_common_top_inst.cpu_inst.sram_periph.mem_d_valid_r...CEN`  
**Delay:** **13.6 ns total**  
**Breakdown:** **4.7 ns logic + 8.9 ns routing**  
**Classification:** **Mixed, but clearly routing-dominated**

### RTL Path Narrative

This is the new dominant synchronous path in the fresh build. It starts from the
registered bus fabric's `slave_response_pending` bookkeeping, passes through the
central response-selection and ready-generation logic, then reaches the SRAM
peripheral's `mem_d_valid_r` clear/hold control.

At a high level, the path is:

```text
registered_bus.slave_response_pending[DFF]
  -> fixed-priority response selection
  -> slave_mem_d_ready generation
  -> top-level SRAM D-channel ready wiring
  -> sram_peripheral mem_d_handshake / mem_d_valid_r hold-clear logic
  -> mem_d_valid_r clock-enable[DFF]
```

### Relevant RTL

- `rtl/common/memory/registered_bus.sv:126-139` — fixed-priority response selection
- `rtl/common/memory/registered_bus.sv:141-187` — `slave_mem_d_ready` generation and response accept conditions
- `rtl/common/memory/registered_bus.sv:217-223` — clearing `slave_response_pending` when a response is accepted
- `rtl/common/top.sv:293-331` — top-level wiring for system controller, clock peripheral, and SRAM peripheral response channels
- `rtl/common/peripherals/sram_peripheral.sv:96-101` — `mem_d_handshake`
- `rtl/common/peripherals/sram_peripheral.sv:303-329` — `mem_d_valid_r` hold/clear behavior in read/write response states
- `rtl/common/peripherals/system_controller_peripheral.sv:65-69,93-95,132,168` — system-controller response-pending interface
- `rtl/common/peripherals/clock_peripheral.sv:58-62,224-245` — clock-peripheral response-pending interface

### Detailed Stage Breakdown

| Stage | Delay contribution | What it means |
| --- | ---: | --- |
| Launch from `slave_response_pending` | 0.5 ns | Registered bus response bookkeeping starts the path |
| Cross-peripheral response-pending fan-in | ~5.9 ns additional | Response-valid information propagates through the bus-wide control cone spanning system controller, clock peripheral, and SRAM response logic |
| Response-selection / accept reconstruction | ~2.5 ns additional | The fixed-priority selection and accept logic rebuild the winning response and associated control terms |
| Final route into SRAM `mem_d_valid_r` enable | **2.2 ns additional** | The last hop into the SRAM valid register control is one of the largest single route segments |
| Setup at destination | 0.1 ns | Final clock-enable setup at the destination flop |

### Why This Path Is Long

1. **A bus-wide response decision is feeding a local ready/clear decision.**  
   `registered_bus` scans all slave response sources with a fixed-priority loop,
   generates a central `slave_mem_d_ready`, and feeds that back into the SRAM
   peripheral's `mem_d_handshake` logic. That makes one timing path span
   multiple logical blocks instead of staying local.

2. **The path combines arbitration, routing, and state-hold control.**  
   The SRAM peripheral does not just consume a data-ready pulse; it uses
   `mem_d_handshake` to decide whether `mem_d_valid_r` remains asserted or
   clears back to idle in `S_WRITE_RESP`, `S_READ_RESP`, and
   `S_READ_SPLIT_RESP`.

3. **Routing dominates more than logic depth.**  
   The log attributes **8.9 ns** of the **13.6 ns** total to routing. That is a
   strong signal that the real issue is the physical spread of this shared
   control network, not just the number of LUT levels.

4. **High-fanout control is being timed under full global-buffer pressure.**  
   With `SB_GB` already at **8 / 8**, additional long control nets are more
   likely to stay on ordinary fabric routing, which makes shared ready/valid
   control paths expensive on HX8K.

### Actionable Optimization Suggestions

1. **Break the response-pop / ready-feedback loop with a register boundary.**  
   The highest-leverage fix is to register the response grant or pop event in
   `registered_bus` so the current same-cycle path does not have to include both
   central arbitration and SRAM valid-state clearing.

2. **Replace the linear response scan with a more localized registered scheme.**  
   A registered one-hot grant or a small staged response queue would reduce both
   LUT depth and long control routing compared with the current
   fixed-priority-all-slaves-every-cycle scan.

3. **Decouple SRAM response clearing from raw combinational `mem_d_ready`.**  
   Let `sram_peripheral` clear `mem_d_valid_r` from a registered consume pulse
   instead of directly from the same-cycle combinational ready path.

4. **Avoid adding more slave-wide fan-in to this response cone.**  
   Any future peripheral that plugs into the same shared response logic should be
   evaluated carefully, because this cone is already the top timing limiter.

---

## Critical Path #2 — Reset-Button Debouncer Counter to Debounced Output Enable

**Domain:** `clk$SB_IO_IN` (posedge -> posedge)  
**Source:** `rst_n_btn_debouncer_inst.stable_counter...O`  
**Destination:** `rst_n_btn_debouncer_inst.dout...CEN`  
**Delay:** **5.9 ns total**  
**Breakdown:** **2.3 ns logic + 3.6 ns routing**  
**Classification:** **Mixed, slightly routing-dominated**

### RTL Path Narrative

This is the worst synchronous path in the raw board-clock domain. It starts in
the reset-button debouncer's `stable_counter`, runs through the terminal-count
and update decision logic, and lands on the debounced output register enable.

At a high level, the path is:

```text
stable_counter[DFF]
  -> stable_counter == STABLE_COUNT_MAX compare
  -> dout update decision
  -> dout clock-enable[DFF]
```

### Relevant RTL

- `rtl/common/primitives/debouncer.sv:26-33` — debounce interval sizing and `STABLE_COUNT_MAX`
- `rtl/common/primitives/debouncer.sv:50-63` — counter update and `dout` update logic
- `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv:54-75` — reset-button synchronizer and debouncer instance

### Detailed Stage Breakdown

| Stage | Delay contribution | What it means |
| --- | ---: | --- |
| Launch from `stable_counter` | 0.5 ns | Counter bit starts the path |
| Route into terminal-count logic | 0.6 ns | Counter output fans into compare/update logic |
| LUT reconstruction of debounce decision | ~3.4 ns additional | Equality and update conditions are rebuilt across several LUT levels |
| Final route into `dout` enable | **1.3 ns additional** | Final clock-enable route remains the largest single late-stage segment |
| Setup at destination | 0.1 ns | Final setup at the destination flop |

### Why This Path Is Long

1. **The counter terminal-count test sits directly in the hot loop.**  
   The debouncer must check `stable_counter == STABLE_COUNT_MAX` every cycle
   that the input differs from `dout`, so the equality test is unavoidable in
   the current structure.

2. **The destination is a clock-enable input.**  
   The path ends at `CEN`, not a simple data input, so the tool rebuilds control
   terms that can be harder to place than a plain D-path.

3. **This path is important, but still small compared with Path #1.**  
   At **5.9 ns**, it is well inside the 25 MHz requirement; it only appears so
   high in the ranking because most of the rest of the design is comfortable.

### Actionable Optimization Suggestions

1. **Use a saturating counter plus a registered `stable_done` flag.**  
   That removes the need to rebuild the full terminal-count compare on the cycle
   that updates `dout`.

2. **Register the terminal-count pulse before updating `dout`.**  
   Adding one cycle of latency to the debounce decision is usually harmless and
   cleanly splits the compare from the output-enable cone.

3. **Only optimize this if button-handling logic grows.**  
   The path is healthy today, so it should stay a secondary priority behind the
   registered-bus response cone.

---

## Critical Path #3 — Seven-Segment Position Register to `io_seg[2]` Output Pad

**Domain:** `posedge pll_clk_global -> <async>`  
**Source:** `seg_position_reg...O`  
**Destination:** `io_seg[2]$sb_io.D_OUT_0`  
**Delay:** **6.2 ns total**  
**Breakdown:** **0.9 ns logic + 5.3 ns routing**  
**Classification:** **Routing-dominated**

### RTL Path Narrative

This is the longest register-to-output path in the current build. It starts from
`seg_position_reg`, passes through the simple seven-segment decode case tree,
and then drives the physical `io_seg` pad.

At a high level, the path is:

```text
seg_position_reg[DFF]
  -> seg_pattern decode
  -> io_seg inversion
  -> output pad
```

### Relevant RTL

- `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv:145-149` — `seg_position_reg`
- `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv:172-189` — segment-position update logic
- `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv:216-231` — seven-segment decode and `io_seg` output

### Why This Path Is Long

1. **This is mostly pad routing, not logic depth.**  
   The path only has **0.9 ns** of logic and **5.3 ns** of routing, so the cost
   comes from reaching the physical segment pin rather than from the decode.

2. **The source and sink are physically far apart.**  
   The routed path crosses a large portion of the device, which is why the
   reg-to-output delay is higher than the debouncer path despite having much
   less logic.

3. **It does not set the internal Fmax ceiling.**  
   This path matters for output timing locality, but it is not the reason the
   system clock tops out around the low-70 MHz range.

### Actionable Optimization Suggestions

1. **Leave it alone unless output timing becomes externally relevant.**  
   This is not the first place to spend effort for internal timing margin.

2. **Register the segment outputs closer to the pads if IO timing ever matters.**  
   A bank-local staging register would trade a few flops for shorter pad routes.

3. **Keep the decode shallow if more seven-segment features are added.**  
   Future expansion should avoid turning this currently routing-dominated path
   into a logic-and-routing path.

---

## Critical Path #4 — IO Button Pad to System-Clock Synchronizer

**Domain:** `<async> -> posedge pll_clk_global`  
**Source:** `io_button[3]$sb_io.D_IN_0`  
**Destination:** `io_button_sync_inst.sync_regs[0]...I0`  
**Delay:** **2.1 ns total**  
**Breakdown:** **0.5 ns logic + 1.6 ns routing**  
**Classification:** **Routing-dominated CDC capture path**

### RTL Path Narrative

This is the normal pad-to-first-stage synchronizer path for the external button
bus entering the system clock domain.

### Relevant RTL

- `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv:151-158` — system-clock button synchronizer instance
- `rtl/common/primitives/ff_sync.sv:15-50` — synchronizer implementation

### Why This Path Is Long

1. **There is no meaningful pre-logic.**  
   The pad feeds the synchronizer directly, which is architecturally correct for
   a CDC capture path.

2. **The reported delay is just placement locality.**  
   The only real cost is the route from the IO pad to the first synchronizer
   register.

3. **This is informational, not a current optimization target.**  
   It appears in the top-five because the rest of the design is already fairly
   timing-clean at 25 MHz.

### Actionable Optimization Suggestions

1. **Keep the first synchronizer stage near the button pads if floorplanning is ever added.**
2. **Do not insert any combinational logic before the synchronizer.**
3. **Split synchronizer instances per button only if pad-local placement becomes a future concern.**

---

## Critical Path #5 — Reset Button Pad to Raw-Clock Synchronizer

**Domain:** `<async> -> posedge clk$SB_IO_IN`  
**Source:** `rst_n_btn$sb_io.D_IN_0`  
**Destination:** `rst_n_btn_sync_inst.sync_regs[0]...I0`  
**Delay:** **1.7 ns total**  
**Breakdown:** **0.5 ns logic + 1.3 ns routing**  
**Classification:** **Routing-dominated CDC capture path**

### RTL Path Narrative

This is the matching asynchronous input path for the reset button in the raw
board-clock domain. The structure is already correct: the external reset button
feeds the first synchronizer stage directly and is only debounced after
synchronization.

### Relevant RTL

- `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv:54-75` — reset button synchronizer and debouncer hookup
- `rtl/common/primitives/ff_sync.sv:15-50` — synchronizer implementation
- `rtl/common/primitives/debouncer.sv:50-63` — downstream debounce logic

### Why This Path Is Long

1. **Routing dominates again.**  
   The timing cost is mostly the physical pad-to-flop route.

2. **The front of the synchronizer is already structurally clean.**  
   There is no avoidable logic before the metastability hardening stage.

3. **This path is not a current bottleneck.**  
   Its presence in the report is useful for documenting CDC locality, not for
   identifying a near-term closure problem.

### Actionable Optimization Suggestions

1. **Leave the first synchronizer stage free of pre-logic.**
2. **If placement constraints are ever added, keep the first stage close to the reset pad.**
3. **Do not spend optimization effort here before fixing synchronous control paths.**

---

## Structural Timing Pressure

Even though the build is healthy, the resource summary still highlights two
practical constraints for future timing work.

### 1. Global-buffer saturation

`SB_GB` usage is **8 / 8**. That means new high-fanout control signals are more
likely to stay on general fabric routing, which makes long shared ready/valid or
reset/control nets harder to close.

One existing RTL guideline helps here: **do not reset datapath-only payload
registers when a separate `valid` or `pending` flag already guarantees the
payload is ignored while invalid.** Reset the control flag, write or refresh the
payload when new data is captured, and keep unnecessary wide datapath buses off
the reset network.

### 2. BRAM saturation

`ICESTORM_RAM` usage is **30 / 32**. The design still fits comfortably, but
there is limited room for architectural changes that would add more local SRAM
or buffering structures to solve timing indirectly.

---

## Recommended Optimization Order

If more iCE40 timing margin is needed in the future, the best order of attack is:

1. **Break up the registered-bus response arbitration / SRAM ready-feedback control path**
2. **Simplify the reset-button debouncer terminal-count / enable cone**
3. **Only then consider seven-segment output staging or CDC locality cleanup**

That order matters because the first two items are the only meaningful
**synchronous** hotspots, and Path #1 is substantially larger than every other
reported path.

---

## Bottom Line

The fresh iCE40 timing data still shows a design that is comfortably inside the
25 MHz target, with nearly **3x** headroom on the main system clock.

The important conclusion is not just that timing passes — it is **why** timing
passes and **what would matter next** if more headroom were required:

- the main limiter is now the **registered-bus response arbitration / SRAM
  response-valid control cone**
- the secondary synchronous hotspot is the much smaller
  **reset-button debouncer control path**
- the remaining reported paths are mostly **IO routing / synchronizer locality**
  paths, not deep internal logic bottlenecks

So the highest-value future timing work on the iCE40 target is clear:
**optimize the shared response-control path first, revisit the debouncer only if
needed, and treat the IO-related paths as lower-priority cleanup.**
