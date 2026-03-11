# ECP5-25F Critical Path & Timing Analysis

**Date:** 2026-03-11  
**Target Device:** Lattice ECP5-25F-CABGA256 (iCE Pi Zero)  
**Synthesis Tools:** Yosys 0.33, nextpnr-ecp5, ecppack  
**Design Configuration:** RV32IMAC + Zicsr CPU (`ENABLE_M_EXT=1`, `ENABLE_F_EXT=0`), direct 50 MHz board clock, no PLL

---

## Executive Summary

This report measures the impact of adding **six skid buffers around `cpu_host_bus_mux`** in `rtl/common/top.sv`, one on each ready/valid interface boundary:

- CPU A-channel into the mux
- CPU D-channel out of the mux
- System A-channel out of the mux
- System D-channel into the mux
- Host A-channel out of the mux
- Host D-channel into the mux

The change achieved its primary goal: the routed ECP5 critical path is **no longer inside `host_bus_mux`**. The previous documented worst path ended at `cpu_host_bus_mux.pending_req_wdata`; after skid insertion, the routed worst path now ends in the new **CPU-side skid buffer register** instead.

The tradeoff is measurable overhead:

- **Routed Fmax dropped from 65.02 MHz to 56.91 MHz**
- **Critical path delay increased from 15.4 ns to 17.6 ns**
- **Resource usage increased materially** in LUT/comb and FF counts

Even with that cost, the ECP5 build still **meets the 50 MHz target** after final routing:

- **2.43 ns positive timing margin**
- **13.8% frequency headroom** above the target

So the skid buffers successfully isolated the mux from the original timing bottleneck, but they did **not** improve absolute ECP5 timing; instead, they moved the bottleneck to the new isolation register boundary.

### Impact of the Host-Bus-Mux Skid Buffer Change

| Metric | Previous documented value | New measured value | Impact |
|--------|---------------------------|--------------------|--------|
| Achieved routed Fmax | 65.02 MHz | 56.91 MHz | **-8.11 MHz** |
| Critical path delay | 15.4 ns | 17.6 ns | **+2.2 ns** |
| TRELLIS_COMB | 10,007 | 11,340 | **+1,333** |
| TRELLIS_FF | 2,436 | 3,020 | **+584** |
| LUT4 | 8,523 | 9,892 | **+1,369** |
| PFUMX | 2,148 | 2,514 | **+366** |
| L6MUX21 | 1,126 | 1,207 | **+81** |
| DP16KD | 9 | 9 | no change |
| MULT18X18D | 0 | 0 | no change |

The skid buffers therefore provide the intended timing isolation, but they do so by adding a non-trivial amount of buffering/control logic and by moving the longest path into the new CPU A-channel skid stage.

### Key Metrics

| Metric | Value | Available | Utilization |
|--------|-------|-----------|-------------|
| **TRELLIS_COMB** | 11,340 | 24,288 | **46%** |
| **TRELLIS_FF** | 3,020 | 24,288 | **12%** |
| **Block RAM (DP16KD)** | 9 | 56 | 16% |
| **DSP (MULT18X18D)** | 0 | 28 | 0% |
| **I/O (TRELLIS_IO)** | 5 | 197 | 2% |
| **PLL (EHXPLLL)** | 0 | 2 | 0% |
| **Achieved Fmax** | 56.91 MHz | 50.00 MHz target | **PASS (+13.8%)** |
| **Critical Path Delay** | 17.6 ns | 20.0 ns budget | **PASS (+2.4 ns slack)** |
| **Async → Clock Max Delay** | 1.54 ns | 20.0 ns budget | PASS |
| **Clock → Async Max Delay** | 2.30 ns | 20.0 ns budget | PASS |

---

## Data Sources

This report is based on the generated build artifacts for the ECP5 target:

- `rtl/fpga/build/ecp5_icepi_zero/yosys.log`
- `rtl/fpga/build/ecp5_icepi_zero/nextpnr.log`

Unlike the iCE40 flow, the current Makefile does not generate a separate standalone timing report for `TARGET=ecp5_icepi_zero`, so the authoritative timing numbers come directly from the final `nextpnr.log`.

The comparison baseline comes from the previously documented ECP5 measurements in this same file before the skid-buffer change.

---

## Resource Utilization Analysis

### Cell Breakdown (from Yosys)

| Cell Type | Count | Description |
|-----------|-------|-------------|
| **LUT4** | 9,892 | 4-input lookup tables used for core logic |
| **PFUMX** | 2,514 | ECP5 LUT-combining mux resources |
| **L6MUX21** | 1,207 | Wider-function mux resources |
| **CCU2C** | 625 | Carry-chain cells |
| **TRELLIS_FF** | 3,020 | Flip-flops |
| **DP16KD** | 9 | 18-kbit block RAMs |
| **Total cells** | 17,267 | Post-synthesis mapped cells |

### Placement / Routing Utilization (from nextpnr)

| Resource | Used | Available | Utilization |
|----------|------|-----------|-------------|
| **TRELLIS_COMB** | 11,340 | 24,288 | 46% |
| **TRELLIS_FF** | 3,020 | 24,288 | 12% |
| **DP16KD** | 9 | 56 | 16% |
| **MULT18X18D** | 0 | 28 | 0% |
| **TRELLIS_IO** | 5 | 197 | 2% |
| **EHXPLLL** | 0 | 2 | 0% |

### Utilization Notes

1. **The design still fits comfortably on ECP5.** Even after adding six skid buffers, comb usage is only **46%** and FF usage is **12%**.
2. **The change is primarily logic/register overhead.** BRAM and DSP usage are unchanged; the cost is almost entirely extra datapath/control buffering.
3. **The FF increase is expected.** The new skid buffers add registered payload and backpressure state on all six mux boundaries.
4. **This is a timing-isolation tradeoff, not a free optimization.** The mux is removed from the top path, but the inserted isolation logic consumes both area and timing budget.

---

## Critical Path #1 — CPU Control / ALU Path into CPU A-Channel Skid Buffer

**Clock domain:** `$glbnet$clk$TRELLIS_IO_IN` (posedge → posedge)  
**Total delay:** **17.6 ns**  
**Breakdown:** **6.4 ns logic + 11.2 ns routing**  
**Achieved Fmax:** **56.91 MHz**  
**RTL modules involved:** `cpu.sv`, `alu.sv`, `mul_unit.sv`, `top.sv`, `primitives/skid_buffer.sv`, `fpga_common_top.sv`

### Path Narrative

The final routed critical path no longer ends in `cpu_host_bus_mux.pending_req_wdata`. Instead, it starts in the CPU core control/datapath, propagates through ALU / multiplier-side logic, crosses into the newly inserted CPU A-channel skid buffer, and ends at the skid buffer output register:

```text
cpu_core.current_state
  → decoder / ALU control-selection logic
  → multiplier-ready / request-generation logic
  → cpu_a_skid_buffer combinational ready/data logic
  → cpu_a_skid_buffer.out_data_current
```

The key confirmation is that the routed path report names **`cpu_a_skid_buffer`** rather than **`cpu_host_bus_mux`** at the endpoint. The worst path still originates in CPU control/ALU logic, but the skid buffer has successfully moved the timing boundary so the host-bus mux itself is no longer on the top routed path.

### Detailed Stage Breakdown

| Stage | Start (ns) | End (ns) | Δ (ns) | Type | Description |
|-------|-----------:|---------:|-------:|------|-------------|
| CPU state/control launch | 0.0 | 2.8 | 2.8 | Logic + Routing | Launch from `cpu_core.current_state` into decoder/control logic |
| Decoder to ALU/multiplier propagation | 2.8 | 12.9 | 10.1 | Logic + Routing | Control/data selection propagates through ALU and multiplier-related logic |
| CPU-to-skid routing | 12.9 | 16.7 | 3.8 | Logic + Routing | Request-generation logic reaches the CPU A-channel skid buffer inputs |
| Skid buffer output-stage logic | 16.7 | 17.6 | 0.9 | Logic + Routing | Data reaches `cpu_a_skid_buffer.out_data_current` capture |
| Final setup | 17.6 | 17.6 | 0.0 | Setup | Capture in skid-buffer FF |

### Why This Path Is Slow

1. **The launch side is still the CPU core.** The mux isolation did not shorten the upstream CPU control/ALU logic depth.
2. **Routing dominates the path.** Routing contributes **11.2 ns of 17.6 ns**, which is even more pronounced than before.
3. **The skid buffer becomes the new timing endpoint.** This is expected: once the mux is isolated, the inserted register stage becomes the first synchronous capture point after the CPU request-generation logic.
4. **The mux is no longer the bottleneck.** The routed report no longer identifies `cpu_host_bus_mux` or `pending_req_wdata` on the top path.

### Confirmation Relative to the Original Report

The original pre-change report documented the worst path as:

```text
cpu_core.u_decoder.jump_reg
  → decoder / ALU control-selection logic
  → cpu_host_bus_mux request-data formatting
  → cpu_host_bus_mux.pending_req_wdata
```

After the skid-buffer change, the worst path is now:

```text
cpu_core.current_state
  → decoder / ALU / multiplier logic
  → cpu_a_skid_buffer
  → cpu_a_skid_buffer.out_data_current
```

That confirms the requested outcome: **the critical timing path has moved out of the host-bus mux on ECP5.**

---

## Pre-Route vs. Final Routed Timing

The `nextpnr-ecp5` log again reports two timing summaries:

| Flow Stage | Reported Fmax | Status |
|------------|---------------|--------|
| After placement refinement | 35.66 MHz | **FAIL** at 50 MHz |
| After final routing | 56.91 MHz | **PASS** at 50 MHz |

As before, the routed result is materially better than the placement-only estimate, so the final signoff number to trust is the later **56.91 MHz PASS** result near the end of `nextpnr.log`.

---

## Cross-Domain Timing

The ECP5 build also reports comfortable margins on the asynchronous interface paths:

| Path Type | Max Delay | Notes |
|-----------|-----------|-------|
| **`<async>` → `posedge clk`** | 1.54 ns | USB RX / async inputs into synchronized logic |
| **`posedge clk` → `<async>`** | 2.30 ns | Registered outputs to board pins such as USB TX / LED |

These are far below the 20 ns system-clock budget and do not present an immediate timing concern.

---

## Summary and Recommendations

### Summary

- **Timing target met:** Yes, with **56.91 MHz** achieved vs. **50.00 MHz** required
- **Critical path delay:** **17.6 ns**
- **Dominant path:** CPU control / ALU / multiplier logic into the new CPU A-channel skid buffer
- **Host-bus-mux result:** The worst routed path is **no longer inside `host_bus_mux`**
- **Resource impact:** Noticeable LUT/FF increase, but still well within ECP5 capacity

### Recommendations

1. **The skid buffers accomplished the isolation goal.** The mux has been removed from the top routed critical path.
2. **Do not treat this change as a pure timing optimization.** On ECP5, it reduced routed Fmax by **8.11 MHz** while still preserving timing closure at 50 MHz.
3. If additional frequency margin is needed later, focus first on:
   - CPU request-generation depth around `cpu.sv`
   - ALU / multiplier-side control propagation in `alu.sv` and `mul_unit.sv`
   - the new CPU A-channel skid-buffer boundary in `top.sv`
4. Resource headroom remains healthy, so future timing work can still spend some additional registers if necessary, but the current data suggests the next optimization should target **upstream CPU logic**, not the mux.

Overall, the ECP5 target remains timing-clean at 50 MHz, and the skid-buffer insertion successfully moved the critical path out of `host_bus_mux`, exactly as requested. The cost is modest-to-moderate area growth and reduced routed headroom, not a timing improvement.
