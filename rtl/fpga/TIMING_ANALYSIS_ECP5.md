# ECP5-25F Critical Path & Timing Analysis

**Date:** 2026-03-11  
**Target Device:** Lattice ECP5-25F-CABGA256 (iCE Pi Zero)  
**Synthesis Tools:** Yosys 0.33, nextpnr-ecp5, ecppack  
**Design Configuration:** RV32IMAC + Zicsr CPU (`ENABLE_M_EXT=1`, `ENABLE_F_EXT=0`), direct 50 MHz board clock, no PLL

---

## Executive Summary

The ECP5 iCE Pi Zero build meets its **50 MHz** timing target with comfortable margin after full place-and-route. The final routed result from `nextpnr-ecp5` reports an achieved **Fmax of 62.86 MHz**, corresponding to a **15.9 ns** clock period on a **20.0 ns** budget.

This leaves approximately:

- **4.09 ns positive timing margin**
- **25.7% frequency headroom** above the 50 MHz target

One useful detail from the log is that the placer-only estimate was initially below target:

- **46.41 MHz** after simulated-annealing placement (**FAIL** at 50 MHz)
- **62.86 MHz** after final routing/timing optimization (**PASS** at 50 MHz)

That means the ECP5 implementation benefits substantially from the routed result, and the final signoff number to trust is the later **62.86 MHz PASS** report, not the earlier placement estimate.

### Key Metrics

| Metric | Value | Available | Utilization |
|--------|-------|-----------|-------------|
| **Total LUT4s** | 9,769 | 24,288 | **40%** |
| **Logic LUTs** | 8,487 | 24,288 | 34% |
| **Carry LUTs** | 1,282 | 24,288 | 5% |
| **TRELLIS_COMB** | 9,971 | 24,288 | **41%** |
| **TRELLIS_FF** | 2,402 | 24,288 | 9% |
| **Block RAM (DP16KD)** | 9 | 56 | 16% |
| **DSP (MULT18X18D)** | 0 | 28 | 0% |
| **I/O (TRELLIS_IO)** | 5 | 197 | 2% |
| **PLL (EHXPLLL)** | 0 | 2 | 0% |
| **Achieved Fmax** | 62.86 MHz | 50.00 MHz target | **PASS (+25.7%)** |
| **Critical Path Delay** | 15.9 ns | 20.0 ns budget | **PASS (+4.1 ns slack)** |
| **Async → Clock Max Delay** | 1.15 ns | 20.0 ns budget | PASS |
| **Clock → Async Max Delay** | 3.06 ns | 20.0 ns budget | PASS |

---

## Data Sources

This report is based on the generated build artifacts for the ECP5 target:

- `rtl/fpga/build/ecp5_icepi_zero/yosys.log`
- `rtl/fpga/build/ecp5_icepi_zero/nextpnr.log`

Unlike the iCE40 flow, the current Makefile does not generate a separate standalone timing report for `TARGET=ecp5_icepi_zero`, so the authoritative timing numbers come directly from the final `nextpnr.log`.

---

## Resource Utilization Analysis

### Cell Breakdown (from Yosys)

| Cell Type | Count | Description |
|-----------|-------|-------------|
| **LUT4** | 8,487 | 4-input lookup tables used for core logic |
| **PFUMX** | 2,132 | ECP5 LUT-combining mux resources |
| **L6MUX21** | 1,100 | Wider-function mux resources |
| **CCU2C** | 641 | Carry-chain cells |
| **TRELLIS_FF** | 2,402 | Flip-flops |
| **DP16KD** | 9 | 18-kbit block RAMs |
| **Total cells** | 14,771 | Post-synthesis mapped cells |

### Placement / Routing Utilization (from nextpnr)

| Resource | Used | Available | Utilization |
|----------|------|-----------|-------------|
| **TRELLIS_COMB** | 9,971 | 24,288 | 41% |
| **TRELLIS_FF** | 2,402 | 24,288 | 9% |
| **DP16KD** | 9 | 56 | 16% |
| **MULT18X18D** | 0 | 28 | 0% |
| **TRELLIS_IO** | 5 | 197 | 2% |
| **EHXPLLL** | 0 | 2 | 0% |

### Utilization Notes

1. **The design uses light logic resources on ECP5.** At roughly 40-41% LUT/comb utilization, the ECP5-25F has much more headroom than the iCE40-HX8K target.
2. **Block RAM pressure is low.** The design uses only **9/56 DP16KD** blocks, so memory resources are not a near-term constraint on this target.
3. **DSP blocks are unused.** Even with `ENABLE_M_EXT=1`, the current build maps without using `MULT18X18D` primitives, leaving all DSP resources available for future optimization work.
4. **No PLL is required.** The top-level ECP5 wrapper runs directly from the board's 50 MHz oscillator.

---

## Critical Path #1 — SRAM Peripheral Readback into Registered Bus Response

**Clock domain:** `$glbnet$clk$TRELLIS_IO_IN` (posedge → posedge)  
**Total delay:** **15.9 ns**  
**Breakdown:** **8.2 ns logic + 7.7 ns routing**  
**Achieved Fmax:** **62.86 MHz**  
**RTL modules involved:** `sram_peripheral.sv`, `sram.sv`, `top.sv`, `fpga_common_top.sv`

### Path Narrative

The final routed critical path does **not** run through the ALU or branch datapath. Instead, it starts at the SRAM peripheral's block RAM output and ends at the registered-bus response data register:

```text
sram_periph.sram_inst.mem.*.DOB13
  → sram_periph read-data glue logic
  → split/concat bus formatting in top-level interconnect
  → rtl_registered_bus.pending_resp_rdata
```

In other words, the slowest synchronous path on ECP5 is currently the **SRAM read-response formatting and capture path** rather than the CPU execution core itself.

### Detailed Stage Breakdown

| Stage | Start (ns) | End (ns) | Δ (ns) | Type | Description |
|-------|-----------:|---------:|-------:|------|-------------|
| BRAM data output launch | 0.0 | 5.8 | 5.8 | Logic | Launch from `sram_inst.mem.*.DOB13` |
| BRAM output routing to SRAM read-data LUT | 5.8 | 7.7 | 1.9 | Routing | BRAM output reaches SRAM read-data logic |
| SRAM read-data local logic | 7.7 | 9.0 | 1.3 | Logic + Routing | `rdata_*` LUT chain inside SRAM peripheral |
| Interconnect / split-concat formatting | 9.0 | 14.7 | 5.7 | Logic + Routing | Bus formatting before response capture |
| Final register route + setup | 14.7 | 15.9 | 1.2 | Routing + Setup | Capture in `pending_resp_rdata` FF |

### Why This Path Is Slow

1. **It launches from block RAM output.** BRAM read data already begins with a non-trivial output delay before the downstream logic starts.
2. **The readback path crosses multiple integration boundaries.** The signal travels from the SRAM implementation through peripheral glue and into the registered bus response path.
3. **Bus formatting adds mux depth.** The `split_concat_rdata` and `pending_resp_rdata` logic introduces additional muxing after the SRAM output is available.
4. **Routing is still a major contributor.** Nearly half of the total delay (**7.7 ns of 15.9 ns**) is routing, which means physical distance between the SRAM logic and response register matters.

### Practical Interpretation

This is a healthy result for the ECP5 target:

- The path still closes timing at 50 MHz with margin.
- The bottleneck lives in the on-chip SRAM response path, not the main CPU arithmetic datapath.
- Any future push beyond ~60 MHz will likely need optimization in the **SRAM-peripheral-to-registered-bus** glue logic first.

---

## Pre-Route vs. Final Routed Timing

The `nextpnr-ecp5` log reports two different timing summaries:

| Flow Stage | Reported Fmax | Status |
|------------|---------------|--------|
| After placement refinement | 46.41 MHz | **FAIL** at 50 MHz |
| After final routing | 62.86 MHz | **PASS** at 50 MHz |

This is important because it shows the ECP5 flow's **post-route timing is materially better than the post-placement estimate** for this design. Any future report or regression check should use the **final routed number** near the end of `nextpnr.log`.

---

## Cross-Domain Timing

The ECP5 build also reports comfortable margins on the asynchronous interface paths:

| Path Type | Max Delay | Notes |
|-----------|-----------|-------|
| **`<async>` → `posedge clk`** | 1.15 ns | USB RX / async inputs into synchronized logic |
| **`posedge clk` → `<async>`** | 3.06 ns | Registered outputs to board pins such as LED |

These numbers are far below the 20 ns system-clock budget and do not present an immediate timing concern.

---

## Summary and Recommendations

### Summary

- **Timing target met:** Yes, with **62.86 MHz** achieved vs. **50.00 MHz** required
- **Critical path delay:** **15.9 ns**
- **Dominant path:** SRAM peripheral read data into the registered-bus response register
- **Resource pressure:** Low to moderate; no obvious ECP5 capacity bottlenecks

### Recommendations

1. **No immediate timing work is required** for the current 50 MHz ECP5 target.
2. If a higher target frequency is needed later, focus first on:
   - SRAM read-data glue logic in `sram_peripheral.sv`
   - bus formatting / response capture around `rtl_registered_bus`
   - physical locality between SRAM output logic and the response register
3. If future ECP5 builds begin to stress arithmetic timing, the currently unused **DSP blocks** are available as an additional optimization lever.

Overall, the ECP5 target is in a much healthier timing position than the constrained iCE40 build and has significant room for future feature growth.
