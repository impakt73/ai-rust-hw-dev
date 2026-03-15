# iCE40-HX8K Timing Analysis

**Date:** 2026-03-13  
**Target device:** Lattice iCE40-HX8K-CB132 (Alchitry Cu v1)  
**Clock target:** 25.00 MHz (`pll_clk_global`)  
**Build command:** `cd rtl/fpga && make TARGET=ice40_alchitry_cu stats STATS_FORMAT=json`

---

## Executive Summary

The fresh `ice40_alchitry_cu` build still closes timing comfortably.

- **Post-route worst synchronous Fmax used for this analysis:** **74.72 MHz**
- **Timing status:** **PASS**
- **Margin vs. 25.00 MHz target:** **+49.72 MHz** (**+198.9%**)

The final detailed timing section in `nextpnr.log` shows that the design's
dominant timing limiter is now firmly inside the CPU datapath:

1. **`pll_clk_global` ALU MIN/MAX compare path** — the clear top synchronous path
2. **`clk$SB_IO_IN` reset-button debouncer path** — the secondary synchronous path
3. **`pll_clk_global -> <async>` LED output route**
4. **`<async> -> pll_clk_global` button synchronizer input**
5. **`<async> -> clk$SB_IO_IN` reset synchronizer input**

That ordering is intentional: the report prioritizes **synchronous paths first**,
because those are the paths that determine clock-closing headroom for the FPGA.

### Final Timing Snapshot

The final post-route timing section reports:

| Category | Result |
| --- | ---: |
| `pll_clk_global` max frequency | **74.72 MHz** |
| `clk$SB_IO_IN` max frequency | **154.34 MHz** |
| `<async> -> posedge pll_clk_global` max delay | **2.39 ns** |
| `<async> -> posedge clk$SB_IO_IN` max delay | **2.37 ns** |
| `posedge pll_clk_global -> <async>` max delay | **3.70 ns** |

The build still has the same two structural constraints that matter for future
timing work:

- **BRAM:** 30 / 32 blocks (**93%**)
- **Global buffers:** 8 / 8 (**100%**)

Those limits do not prevent this build from meeting 25 MHz, but they do reduce
how much placement flexibility remains for future optimizations.

---

## Authoritative Artifacts Used

This report is based on the freshly generated artifacts in:

- `rtl/fpga/build/ice40_alchitry_cu/nextpnr.log`
- `rtl/fpga/build/ice40_alchitry_cu/yosys.log`
- `rtl/fpga/build/ice40_alchitry_cu/riscv_fpga_stats.json`
- `rtl/fpga/build/ice40_alchitry_cu/riscv_fpga_stats.md`

For this iCE40 target:

- **Detailed critical-path analysis comes from the final timing section in**
  `nextpnr.log`
- **Resource utilization comes from** `nextpnr.log`
- **Post-synthesis cell counts come from** `yosys.log`

`riscv_fpga_stats.json` and `riscv_fpga_stats.md` remain useful for the fresh
resource counts above, but the path-by-path timing discussion in this document
is anchored to the **final detailed timing section** of `nextpnr.log`, because
that is the section that contains the actual routed critical-path dumps.

### Fresh Resource Summary

| Metric | Value | Available | Utilization |
| --- | ---: | ---: | ---: |
| Logic cells (`ICESTORM_LC`) | 5570 | 7680 | 72% |
| BRAM (`ICESTORM_RAM`) | 30 | 32 | 93% |
| Global buffers (`SB_GB`) | 8 | 8 | 100% |
| IOs (`SB_IO`) | 77 | 256 | 30% |
| PLLs (`ICESTORM_PLL`) | 1 | 2 | 50% |

### Fresh Cell Counts

| Cell | Count |
| --- | ---: |
| `SB_LUT4` | 4510 |
| `SB_CARRY` | 897 |
| `SB_DFF` | 351 |
| `SB_DFFE` | 72 |
| `SB_DFFESR` | 1625 |
| `SB_DFFESS` | 2 |
| `SB_DFFSR` | 421 |
| `SB_DFFSS` | 12 |
| `SB_RAM40_4K` | 30 |

---

## Critical Path Ranking

| Rank | Path class | Domain | Total delay | Logic | Routing |
| --- | --- | --- | ---: | ---: | ---: |
| 1 | ALU MIN/MAX compare -> result select | `pll_clk_global` | **13.4 ns** | **7.1 ns** | **6.3 ns** |
| 2 | Reset-button debouncer counter -> output enable | `clk$SB_IO_IN` | **6.5 ns** | **2.3 ns** | **4.2 ns** |
| 3 | LED control -> output pad | `posedge pll_clk_global -> <async>` | **3.7 ns** | **0.5 ns** | **3.2 ns** |
| 4 | IO button pad -> synchronizer | `<async> -> posedge pll_clk_global` | **2.4 ns** | **0.5 ns** | **1.9 ns** |
| 5 | Reset button pad -> synchronizer | `<async> -> posedge clk$SB_IO_IN` | **2.4 ns** | **0.5 ns** | **1.9 ns** |

The large gap between Path #1 and the rest of the design makes the main
takeaway straightforward: **if more iCE40 timing margin is ever needed, the ALU
MIN/MAX path is the first place to spend effort.**

---

## Critical Path #1 — CPU ALU MIN/MAX Compare to Final Result Mux

**Domain:** `pll_clk_global` (posedge -> posedge)  
**Source:** `fpga_common_top_inst.cpu_inst.cpu_core.u_alu.req_b_reg...O`  
**Destination:** `fpga_common_top_inst.cpu_inst.cpu_core.u_alu.result_next...I0`  
**Delay:** **13.4 ns total**  
**Breakdown:** **7.1 ns logic + 6.3 ns routing**

### RTL Path Narrative

This is the dominant synchronous path in the current build. It starts from the
latched ALU operand register, runs through the signed/unsigned MIN/MAX compare
network, then reconverges into the shared `result_next` mux that selects the
final ALU output.

At a high level, the path is:

```text
u_alu.req_b_reg[DFF]
  -> minmax_compare_lt carry-chain compare
  -> compare result reconstruction in LUTs
  -> minmax_result / result_next select logic
  -> result_next input[DFF]
```

### Relevant RTL

- `rtl/common/cpu/alu.sv:179-182` — `minmax_compare_lt`
- `rtl/common/cpu/alu.sv:223-228` — `minmax_result`
- `rtl/common/cpu/alu.sv:242-255` — `result_next`

### Detailed Stage Breakdown

| Stage | Delay contribution | What it means |
| --- | ---: | --- |
| Launch from `req_b_reg` | 0.5 ns | Registered operand launches the path |
| Route into compare fabric | 2.0 ns | The first hop from `(4,8)` to `(13,1)` is already significant |
| Long `SB_CARRY` chain for `<` compare | ~5.5 ns additional | The signed/unsigned compare expands into a deep carry chain |
| Post-compare LUT reconstruction | ~2.8 ns additional | The carry result is converted back into control/data terms |
| Final route into `result_next` select input | 1.0 ns | The last hop into the shared output-select LUT is still non-trivial |
| Setup at destination | 0.5 ns | Final register setup requirement |

### Why This Path Is Long

1. **MIN/MAX is using a full-width compare.**  
   `minmax_compare_lt` is computed from the registered operands using either a
   signed or unsigned `<` operator. On iCE40 this maps naturally into a long
   carry chain.

2. **The compare does not terminate locally.**  
   After the carry chain finishes, the result still feeds `minmax_result`, and
   then the shared `result_next` mux. That means one timing path contains both
   the compare **and** the final ALU result selection.

3. **Signed and unsigned handling share the same cone.**  
   The path is not just a raw magnitude compare; it also includes the control
   needed to support both `MIN/MAX` and `MINU/MAXU`, which increases LUT
   reconstruction after the carry chain.

4. **This is both logic-heavy and route-heavy.**  
   Unlike the other reported paths, this one is not just a placement problem.
   It has substantial delay in both the arithmetic mapping and the final
   reconvergent muxing.

### Actionable Optimization Suggestions

1. **Split compare and final result selection across a register boundary.**  
   If more headroom is required, the highest-leverage fix is to register either
   `minmax_result` or the compare outcome so the carry chain and the final
   result mux do not sit in one cycle.

2. **Special-case signed MIN/MAX before the full compare.**  
   Separating sign handling from magnitude compare can shorten the amount of
   logic that must sit after the carry chain.

3. **Localize the MIN/MAX output path instead of feeding the shared mux late.**  
   A dedicated registered MIN/MAX result path would reduce reconvergence at
   `result_next`.

4. **Avoid adding new control fan-in to this cone.**  
   Because this is already the critical path, future ALU feature growth should
   not reuse the same final select structure without rechecking timing.

---

## Critical Path #2 — Reset-Button Debouncer Counter to Output Enable

**Domain:** `clk$SB_IO_IN` (posedge -> posedge)  
**Source:** `rst_n_btn_debouncer_inst.stable_counter...O`  
**Destination:** `rst_n_btn_debouncer_inst.dout...CEN`  
**Delay:** **6.5 ns total**  
**Breakdown:** **2.3 ns logic + 4.2 ns routing**

### RTL Path Narrative

This is the worst synchronous path in the raw board-clock domain. It starts in
the debouncer's `stable_counter`, passes through the "has the input stayed
stable long enough?" decision logic, and lands on the clock-enable controlling
the debounced output register.

At a high level, the path is:

```text
stable_counter[DFF]
  -> terminal-count / hold-update logic
  -> dout enable generation
  -> dout clock-enable[DFF]
```

### Relevant RTL

- `rtl/common/primitives/debouncer.sv:26-33` — counter sizing and max count
- `rtl/common/primitives/debouncer.sv:50-63` — debounce decision logic
- `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv:67-75` — reset-button debouncer instance

### Detailed Stage Breakdown

| Stage | Delay contribution | What it means |
| --- | ---: | --- |
| Launch from `stable_counter` | 0.5 ns | Counter register starts the path |
| First route into decision logic | 0.6 ns | Counter bit fans into debounce condition logic |
| Three LUT levels of decision logic | ~1.6 ns additional | Equality / update / enable conditions are rebuilt in LUT fabric |
| Final route into `dout` clock-enable | **1.8 ns** | The largest single segment is the last hop into `CEN` |
| Setup at destination | 0.1 ns | Final enable setup |

### Why This Path Is Long

1. **Terminal-count detection is in the hot loop.**  
   The debouncer must decide between reset, hold, and update every cycle, so
   `stable_counter == STABLE_COUNT_MAX` sits directly on the path.

2. **The destination is a clock-enable input.**  
   The logic cone does not end at a simple data input; it has to generate a CE
   condition, which often creates deeper control logic than a plain D-path.

3. **Routing dominates even in this small block.**  
   The path is only modestly logical, but the CE route still contributes most of
   the delay.

### Actionable Optimization Suggestions

1. **Use a saturating counter plus a registered "done" flag.**  
   That removes the need to rebuild the full equality test on the hottest cycle.

2. **Consider simpler D-input update logic instead of a deeper CE cone.**  
   On iCE40, moving complexity from CE generation to D-input muxing can
   sometimes place and route better.

3. **Debounce on a prescaled tick if more margin is needed.**  
   A slower enable would shrink the active counter/control network without
   changing the external button behavior.

---

## Critical Path #3 — LED Control to IO Output Pad

**Domain:** `posedge pll_clk_global -> <async>`  
**Source:** `fpga_common_top_inst.cpu_inst.led_ctrl.mem_a_wdata...O`  
**Destination:** `io_led[6]$sb_io.D_OUT_0`  
**Delay:** **3.7 ns total**  
**Breakdown:** **0.5 ns logic + 3.2 ns routing**

### RTL Path Narrative

This is the longest register-to-output path in the design. It is not a deep
logic path; it is mainly the cost of carrying one internal LED value across the
device to multiple physical IO pins.

At a high level, the path is:

```text
LED control source
  -> replicated LED bank routing
  -> distant IO pad
```

### Relevant RTL

- `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv:133-139`
- `rtl/common/peripherals/led_controller_peripheral.sv`

### Why This Path Is Long

1. **One 8-bit value is replicated onto three LED banks.**  
   The top-level drives:

   ```systemverilog
   assign io_led[7:0]   = led_out;
   assign io_led[15:8]  = led_out;
   assign io_led[23:16] = led_out;
   ```

   That fanout makes at least some destinations physically far away.

2. **This is almost entirely a routing problem.**  
   The log shows only 0.5 ns of logic and 3.2 ns of routing.

3. **The worst sink is simply far from the source.**  
   The critical segment spans from `(7,27)` to `(33,28)`, which is a long
   cross-device output route for an HX8K.

### Actionable Optimization Suggestions

1. **Add bank-local staging registers if LED routing ever becomes a problem.**  
   That would trade a few flops for shorter output routes.

2. **Duplicate LED drivers per bank rather than relying on one high-fanout net.**  
   Logic replication is often cheaper than long routing on small FPGAs.

3. **Keep this low priority unless IO fanout grows.**  
   At 3.7 ns this path is healthy today, so it only becomes worth touching if
   new output features increase the route cost further.

---

## Critical Path #4 — IO Button Pad to System-Clock Synchronizer

**Domain:** `<async> -> posedge pll_clk_global`  
**Source:** `io_button[0]$sb_io.D_IN_0`  
**Destination:** `io_button_sync_inst.sync_regs[0]...I0`  
**Delay:** **2.4 ns total**  
**Breakdown:** **0.5 ns logic + 1.9 ns routing**

### RTL Path Narrative

This is the longest asynchronous input path into the system clock domain. It is
the normal pad-to-first-synchronizer-flop route for the external button bus.

### Relevant RTL

- `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv:151-158`
- `rtl/common/primitives/ff_sync.sv`

### Why This Path Is Long

1. **The delay is almost entirely pad routing.**  
   There is no substantial combinational logic before the synchronizer.

2. **A shared 5-bit synchronizer gives the placer less freedom per bit.**  
   The RTL is clean, but not every synchronizer bit is guaranteed to land close
   to its associated IO pin.

3. **This is a CDC locality issue, not a functional hotspot.**  
   It matters mainly as a placement signal for future board-input growth.

### Actionable Optimization Suggestions

1. **Use per-button synchronizer instances if placement ever becomes tighter.**
2. **Keep all debouncing and edge detection after the synchronizer chain.**
3. **Prefer IO-adjacent first-stage flops if future constraints are added.**

---

## Critical Path #5 — Reset Button Pad to Raw-Clock Synchronizer

**Domain:** `<async> -> posedge clk$SB_IO_IN`  
**Source:** `rst_n_btn$sb_io.D_IN_0`  
**Destination:** `rst_n_btn_sync_inst.sync_regs[0]...I0`  
**Delay:** **2.4 ns total**  
**Breakdown:** **0.5 ns logic + 1.9 ns routing**

### RTL Path Narrative

This is the matching asynchronous input path for the reset button in the raw
board clock domain. The structure is already correct: the physical reset input
feeds the first synchronizer stage directly, with no extra logic inserted
before metastability hardening.

### Relevant RTL

- `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv:58-65`
- `rtl/common/primitives/ff_sync.sv`

### Why This Path Is Long

1. **Again, routing dominates.**  
   The critical segment is just the pad-to-flop route from `(17,0)` to `(6,1)`.

2. **This path is structurally healthy already.**  
   There is no avoidable logic depth on the front of the synchronizer.

3. **Its significance is mostly informational.**  
   It appears in the top-five list because the rest of the design is timing-clean
   at 25 MHz, not because reset synchronization is currently at risk.

### Actionable Optimization Suggestions

1. **Keep the first synchronizer stage close to the reset pin if placement constraints are ever added.**
2. **Do not insert any pre-logic before this synchronizer.**
3. **Leave this path alone unless future floorplanning changes make it worse.**

---

## Structural Timing Pressure

Even though the build is healthy, the resource summary highlights two practical
placement constraints.

### 1. Global-buffer saturation

`SB_GB` usage is **8 / 8**. That means any future high-fanout control signal
that cannot use a dedicated global route will need to live on ordinary fabric
routing, which makes long control cones harder to close.

One practical RTL rule that helps here is to avoid resetting datapath-only
payload registers when a separate `valid`/`pending` flag already guarantees the
payload is ignored while invalid. Reset the control flag, initialize the
payload when the flag asserts, and keep unnecessary payload buses off the reset
network.

### 2. BRAM saturation

`ICESTORM_RAM` usage is **30 / 32**. The design still fits, but there is very
little slack for timing-driven architectural changes that would add extra local
buffering or staging memories.

---

## Recommended Optimization Order

If more iCE40 timing margin is required in the future, the best order of attack
is:

1. **Break up the ALU MIN/MAX compare and result-select path**
2. **Simplify the reset debouncer's terminal-count / CE cone**
3. **Only then consider LED output replication or CDC placement tweaks**

That order matters because the first two items are the only reported
**synchronous** hotspots, and the ALU path is much larger than everything else.

---

## Bottom Line

The fresh iCE40 timing data shows a design that is still comfortably inside the
25 MHz target, with almost **3x** headroom on the main system clock.

The important conclusion is not just that timing passes — it is **why** timing
passes:

- the design's only truly significant timing limiter is now the
  **CPU ALU MIN/MAX compare/result-select cone**
- the secondary synchronous hotspot is the much smaller
  **reset-button debouncer control path**
- the remaining reported paths are primarily **IO routing/locality** paths, not
  deep logic bottlenecks

So the most effective future timing work on the iCE40 target is clear:
**optimize the ALU MIN/MAX datapath first, then revisit the debouncer, and only
after that worry about IO-path cleanup.**
