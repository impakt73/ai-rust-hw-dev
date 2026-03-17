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

## Executive Summary

The fresh `ice40_alchitry_cu` build still closes timing comfortably at the
25 MHz target, but the dominant routed synchronous bottleneck has changed from
the older host-bus control cone to an **ALU signed-compare / output-select
cone** inside `u_alu`.

- **Authoritative post-route Fmax used for this analysis:** **69.30 MHz**
- **Timing status:** **PASS**
- **Margin vs. 25.00 MHz target:** **+44.30 MHz** (**+177.2% above target**)
- **Worst synchronous routed delay:** **14.4 ns** on `pll_clk_global`

The most important conclusions from the fresh routed dump are:

1. **The top synchronous path is now the ALU signed-compare carry chain feeding the registered ALU output**
2. **The only other reported synchronous hotspot is still the reset-button debouncer, but the endpoint is the counter clear/reset control rather than `dout` enable**
3. **The normalized stats JSON again captured an earlier timing snapshot, so the final routed `nextpnr.log` must remain the source of truth**

### Final Timing Snapshot

| Category | Result |
| --- | ---: |
| `pll_clk_global` max frequency | **69.30 MHz** |
| `clk$SB_IO_IN` max frequency | **154.87 MHz** |
| `<async> -> posedge clk$SB_IO_IN` max delay | **1.06 ns** |
| `<async> -> posedge pll_clk_global` max delay | **2.48 ns** |
| `posedge pll_clk_global -> <async>` max delay | **4.64 ns** |

### Fresh Resource Summary

| Metric | Value | Available | Utilization |
| --- | ---: | ---: | ---: |
| Logic cells (`ICESTORM_LC`) | 6043 | 7680 | 78% |
| BRAM (`ICESTORM_RAM`) | 30 | 32 | 93% |
| Global buffers (`SB_GB`) | 8 | 8 | 100% |
| IOs (`SB_IO`) | 77 | 256 | 30% |
| PLLs (`ICESTORM_PLL`) | 1 | 2 | 50% |

### Fresh Cell Counts

| Cell | Count |
| --- | ---: |
| `SB_LUT4` | 4727 |
| `SB_CARRY` | 929 |
| `SB_DFF` | 363 |
| `SB_DFFE` | 983 |
| `SB_DFFESR` | 1064 |
| `SB_DFFESS` | 4 |
| `SB_DFFSR` | 431 |
| `SB_DFFSS` | 12 |
| `SB_RAM40_4K` | 30 |
| `SB_PLL40_CORE` | 1 |

---

## Cross-Check Note: Normalized Stats vs. Raw Routed Log

The normalized stats artifact and the final routed log do **not** agree on the
headline `pll_clk_global` frequency for this build:

- `riscv_fpga_stats.json` reports **66.53 MHz**
- the earlier timing snapshot in `nextpnr.log` also reports **66.53 MHz**
- the **final routed timing summary** in `nextpnr.log` reports **69.30 MHz**

That **2.77 MHz mismatch** means the normalized stats flow again captured an
earlier clock summary from `nextpnr.log`, not the final routed one. For that
reason:

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
| 1 | ALU signed-compare carry chain -> registered ALU output | `pll_clk_global` | **14.4 ns** | **7.4 ns** | **7.0 ns** |
| 2 | Reset-button debouncer counter feedback -> counter clear control | `clk$SB_IO_IN` | **6.5 ns** | **2.3 ns** | **4.1 ns** |
| 3 | Seven-segment position register -> output pad | `posedge pll_clk_global -> <async>` | **4.64 ns** | **0.9 ns** | **3.7 ns** |
| 4 | IO button pad -> first synchronizer stage | `<async> -> posedge pll_clk_global` | **2.48 ns** | **0.5 ns** | **2.0 ns** |
| 5 | Reset button pad -> first synchronizer stage | `<async> -> posedge clk$SB_IO_IN` | **1.06 ns** | **0.5 ns** | **0.6 ns** |

The key architectural takeaway from the fresh routed build is that the
system-clock limit is now set by a **mixed logic/routing ALU compare path**,
not by host-bus control.

---

## Critical Path #1 — ALU Signed-Compare Cone into Registered `out_data`

**Domain / class:** `pll_clk_global` (posedge -> posedge)  
**Startpoint:** `fpga_common_top_inst.cpu_inst.cpu_core.u_alu.req_b_reg...O`  
**Endpoint:** `fpga_common_top_inst.cpu_inst.cpu_core.u_alu.out_data...I0`  
**Delay:** **14.4 ns total**  
**Breakdown:** **7.4 ns logic + 7.0 ns routing**  
**Classification:** **Mixed synchronous datapath/control path**

### RTL Path Narrative

This is the dominant synchronous path in the fresh routed build. The launch flop
is bit 31 of `req_b_reg` inside `u_alu`. The path then travels through the
signed less-than compare cone used by the ALU's min/max and compare-class logic,
including the mapped iCE40 carry chain that implements the wide signed
comparison, before it lands in the LUT feeding the registered `out_data` path.

At a high level, the path is:

```text
req_b_reg[31][DFF]
  -> signed compare (`minmax_signed_lt`)
  -> wide carry-chain / compare reduction
  -> result-select LUT feeding `out_data`
  -> out_data[DFF input]
```

### Relevant RTL

- `rtl/common/cpu/alu.sv:143-145` — request operand registers
- `rtl/common/cpu/alu.sv:197-198` — `minmax_signed_lt` / `minmax_unsigned_lt`
- `rtl/common/cpu/alu.sv:205-214` — arithmetic result generation, including
  `ALU_SLT` / `ALU_SLTU`
- `rtl/common/cpu/alu.sv:251-265` — `result_next` selection mux
- `rtl/common/cpu/alu.sv:267-369` — registered ALU request/output pipeline and
  `out_data <= result_next`
- `rtl/common/cpu/cpu.sv:1044-1057` — `u_alu` instantiation in the CPU core
- `rtl/common/top.sv:419-456` — CPU instantiation inside the shared RTL top
- `rtl/fpga/common/fpga_common_top.sv:34-80` — `top` wrapper and UART hookup
- `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv:115-128` — iCE40 board
  top-level wrapper

### Why This Path Is Long

1. **A 32-bit compare chain still reaches the final output-select cone in one cycle.**  
   `minmax_signed_lt` is built from a wide compare across `req_a_reg` and
   `req_b_reg`, and the synthesized implementation uses an extended iCE40 carry
   chain before the result is consumed by the final LUT feeding `out_data`.

2. **The compare result is not fully isolated from the generic `result_next` mux.**  
   Even though min/max has extra internal staging, the reported path still ends
   in the general ALU output-select structure rather than in a tiny dedicated
   compare-result register.

3. **Logic and routing are both significant.**  
   The path is **7.4 ns logic** and **7.0 ns routing**, so neither pure logic
   simplification nor pure placement/locality work alone is likely to give the
   full improvement.

4. **The device is already placement-constrained.**  
   The design uses **8 / 8 global buffers** and **30 / 32 BRAMs**, so the
   router has limited flexibility to perfectly localize wide ALU cones.

### Actionable Optimization Suggestions

1. **Register compare-class results before the final `out_data` writeback.**  
   Add a small local register boundary so the signed/unsigned compare result is
   captured before it feeds the final output-select logic in
   `rtl/common/cpu/alu.sv:251-265,362-366`.

2. **Split compare-class output from the generic `result_next` mux.**  
   Give SLT/SLTU and min/max-class operations a narrower dedicated output path
   so the compare chain does not have to reconverge with the broader ALU result
   mux right before `out_data`.

3. **Isolate min/max compare capture from final output selection even further.**  
   The existing min/max pipeline (`rtl/common/cpu/alu.sv:331-355`) already
   stages part of this work; moving more compare-related control fully into that
   staged path would reduce same-cycle coupling into the final output LUT.

4. **If higher Fmax becomes a priority, allow compare-class operations to take an extra cycle.**  
   For an FPGA-focused configuration, an explicitly staged compare/writeback step
   would be the most direct way to reduce this system-clock bottleneck.

---

## Critical Path #2 — Reset-Button Debouncer Counter Feedback into `stable_counter` Clear

**Domain / class:** `clk$SB_IO_IN` (posedge -> posedge)  
**Startpoint:** `rst_n_btn_debouncer_inst.stable_counter...O`  
**Endpoint:** `rst_n_btn_debouncer_inst.stable_counter...SR`  
**Delay:** **6.5 ns total**  
**Breakdown:** **2.3 ns logic + 4.1 ns routing**  
**Classification:** **Mixed, routing-dominated synchronous control path**

### RTL Path Narrative

This is the worst reported synchronous path in the raw board-clock domain. It
starts from a bit of `stable_counter`, runs through the equality / update logic
that decides whether the synchronized reset button has remained stable long
enough, and ends on the synthesized clear/reset control of `stable_counter`
rather than on `dout`.

At a high level, the path is:

```text
stable_counter[DFF]
  -> `stable_counter == STABLE_COUNT_MAX` compare
  -> `din != dout` / debounce update decision
  -> stable_counter clear/reset control[DFF]
```

### Relevant RTL

- `rtl/common/primitives/debouncer.sv:26-35` — counter width and
  `STABLE_COUNT_MAX`
- `rtl/common/primitives/debouncer.sv:50-63` — counter update, terminal-count
  detection, and `dout` update logic
- `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv:58-75` — reset button
  synchronizer and debouncer instantiation

### Why This Path Is Long

1. **The terminal-count test sits directly in the update loop.**  
   The debouncer checks `stable_counter == STABLE_COUNT_MAX` every cycle while
   `din != dout`, so the equality compare remains part of the hot path.

2. **The synthesized destination is counter clear/reset control.**  
   That makes the path a feedback-control problem rather than a simple data-input
   path into `dout`.

3. **Routing dominates more than logic here.**  
   The path is **4.1 ns routing** versus **2.3 ns logic**, so locality is a
   bigger factor than comparator depth.

4. **It is still far from threatening the design target.**  
   At **6.5 ns**, this path is well inside the 25 MHz budget and is clearly a
   secondary concern relative to the 14.4 ns ALU path.

### Actionable Optimization Suggestions

1. **Use a saturating counter or registered terminal flag.**  
   In `rtl/common/primitives/debouncer.sv:55-61`, a saturating counter plus a
   one-bit `stable_done` flag would shorten the terminal-count compare cone.

2. **Predecode the counter-clear condition.**  
   Separating the “counter reset to zero” decision from the `dout` update branch
   would shorten the reconvergent feedback control that now lands on the
   synthesized `stable_counter...SR` path.

3. **Keep this as a lower-priority cleanup item.**  
   It is only worth touching if button logic grows or if the main ALU path has
   already been optimized.

---

## Async Paths (Informational, Not Ranked Ahead of Synchronous Bottlenecks)

### Reset Button Pad -> First Synchronizer Stage

- **Domain / class:** `<async> -> posedge clk$SB_IO_IN`
- **Delay:** **1.06 ns total** = **0.5 ns logic + 0.6 ns routing**
- **RTL grounding:** `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv:58-65`,
  `rtl/common/primitives/ff_sync.sv:15-50`
- **Interpretation:** this is a normal pad-to-first-stage synchronizer route; it
  is informative, but not an internal synchronous bottleneck.

### IO Button Pad -> First Synchronizer Stage

- **Domain / class:** `<async> -> posedge pll_clk_global`
- **Delay:** **2.48 ns total** = **0.5 ns logic + 2.0 ns routing**
- **RTL grounding:** `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv:151-158`,
  `rtl/common/primitives/ff_sync.sv:15-50`
- **Interpretation:** architecturally clean CDC capture path; not a current
  optimization target.

### Seven-Segment Position Register -> Output Pad

- **Domain / class:** `posedge pll_clk_global -> <async>`
- **Delay:** **4.64 ns total** = **0.9 ns logic + 3.7 ns routing**
- **RTL grounding:** `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv:172-186,194-200`
- **Interpretation:** this is a reg-to-pad route dominated by pad locality. It
  does not set the internal Fmax ceiling.

---

## Structural Timing Pressure

Even though timing is healthy, the resource summary still highlights two layout
constraints that matter for future optimization work:

### 1. Global-buffer saturation

`SB_GB` usage is **8 / 8**. Additional high-fanout control or reset nets are
therefore more likely to stay on ordinary routing resources, which makes it
harder to localize wide ALU/control cones at higher frequencies.

### 2. BRAM saturation

`ICESTORM_RAM` usage is **30 / 32**. The design still fits, but there is limited
room to solve timing by adding more buffering or local scratch storage.

---

## Recommended Optimization Order

If more iCE40 timing margin is needed later, the highest-value next steps are:

1. **Split and stage compare-class ALU results before the final `out_data` mux/writeback**
2. **Reduce reconvergence between the wide signed-compare chain and the generic `result_next` output path**
3. **Only then consider simplifying the debouncer terminal-count feedback path**
4. **Treat the async IO paths as placement-locality observations, not core timing-closure priorities**

---

## Bottom Line

The fresh iCE40 routed build still closes timing with approximately **2.8x** system-clock
headroom relative to the 25 MHz target, but the highest-value optimization focus
has changed.

The current routed bottleneck is:

- **not** the older host-bus first-beat / address-update control cone
- **not** the older registered-bus / SRAM ready-feedback path
- **but** a **mixed ALU signed-compare and output-select path** from
  `req_b_reg[31]` into the registered `out_data` writeback cone

So the clearest next optimization for this target is to **decompose and stage
the compare-class ALU output path first**, while treating the debouncer as a
smaller secondary synchronous path and the reported async paths as informative
locality checks.
