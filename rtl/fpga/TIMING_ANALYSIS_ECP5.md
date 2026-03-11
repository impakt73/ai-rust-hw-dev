# ECP5-25F Critical Path & Timing Analysis

**Date:** 2026-03-11  
**Target Device:** Lattice ECP5-25F-CABGA256 (iCE Pi Zero)  
**Synthesis Tools:** Yosys 0.33, nextpnr-ecp5, ecppack  
**Design Configuration:** RV32IMAC + Zicsr CPU (`ENABLE_M_EXT=1`, `ENABLE_F_EXT=0`), direct 50 MHz board clock, no PLL

---

## Executive Summary

The ECP5 iCE Pi Zero build continues to meet its **50 MHz** timing target with comfortable margin after full place-and-route. After changing the SRAM block to a **2-cycle registered read path** and updating the SRAM peripheral to match, the final routed result from `nextpnr-ecp5` reports an achieved **Fmax of 65.02 MHz**, corresponding to a **15.4 ns** clock period on a **20.0 ns** budget.

This leaves approximately:

- **4.62 ns positive timing margin**
- **30.0% frequency headroom** above the 50 MHz target

Most importantly, the previous top critical path through the **SRAM peripheral readback path** is no longer the routed worst case. The routed critical path now lives in the **CPU control / ALU / host-bus request-data path**, which confirms that the SRAM latency change removed the path that was previously documented as the limiting ECP5 path.

One useful detail from the log is that the placer-only estimate still starts below target:

- **45.91 MHz** after simulated-annealing placement (**FAIL** at 50 MHz)
- **65.02 MHz** after final routing/timing optimization (**PASS** at 50 MHz)

That means the ECP5 implementation still benefits substantially from the routed result, and the final signoff number to trust is the later **65.02 MHz PASS** report, not the earlier placement estimate.

### Impact of the SRAM 2-Cycle Read Change

| Metric | Previous documented value | New measured value | Impact |
|--------|---------------------------|--------------------|--------|
| Achieved routed Fmax | 62.86 MHz | 65.02 MHz | **+2.16 MHz** |
| Critical path delay | 15.9 ns | 15.4 ns | **-0.5 ns** |
| TRELLIS_COMB | 9,971 | 10,007 | +36 |
| TRELLIS_FF | 2,402 | 2,436 | +34 |
| LUT4 | 8,487 | 8,523 | +36 |
| PFUMX | 2,132 | 2,148 | +16 |
| L6MUX21 | 1,100 | 1,126 | +26 |
| DP16KD | 9 | 9 | no change |

The SRAM change therefore traded a small amount of extra control/pipeline logic for a measurable post-route timing improvement, while keeping block RAM usage unchanged.

### Key Metrics

| Metric | Value | Available | Utilization |
|--------|-------|-----------|-------------|
| **Total LUT4s** | 9,805 | 24,288 | **40%** |
| **Logic LUTs** | 8,523 | 24,288 | 35% |
| **Carry LUTs** | 1,282 | 24,288 | 5% |
| **TRELLIS_COMB** | 10,007 | 24,288 | **41%** |
| **TRELLIS_FF** | 2,436 | 24,288 | 10% |
| **Block RAM (DP16KD)** | 9 | 56 | 16% |
| **DSP (MULT18X18D)** | 0 | 28 | 0% |
| **I/O (TRELLIS_IO)** | 5 | 197 | 2% |
| **PLL (EHXPLLL)** | 0 | 2 | 0% |
| **Achieved Fmax** | 65.02 MHz | 50.00 MHz target | **PASS (+30.0%)** |
| **Critical Path Delay** | 15.4 ns | 20.0 ns budget | **PASS (+4.6 ns slack)** |
| **Async → Clock Max Delay** | 2.48 ns | 20.0 ns budget | PASS |
| **Clock → Async Max Delay** | 2.51 ns | 20.0 ns budget | PASS |

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
| **LUT4** | 8,523 | 4-input lookup tables used for core logic |
| **PFUMX** | 2,148 | ECP5 LUT-combining mux resources |
| **L6MUX21** | 1,126 | Wider-function mux resources |
| **CCU2C** | 641 | Carry-chain cells |
| **TRELLIS_FF** | 2,436 | Flip-flops |
| **DP16KD** | 9 | 18-kbit block RAMs |
| **Total cells** | 14,883 | Post-synthesis mapped cells |

### Placement / Routing Utilization (from nextpnr)

| Resource | Used | Available | Utilization |
|----------|------|-----------|-------------|
| **TRELLIS_COMB** | 10,007 | 24,288 | 41% |
| **TRELLIS_FF** | 2,436 | 24,288 | 10% |
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

## Critical Path #1 — CPU Control / ALU Path into Host-Bus Request Capture

**Clock domain:** `$glbnet$clk$TRELLIS_IO_IN` (posedge → posedge)  
**Total delay:** **15.4 ns**  
**Breakdown:** **5.8 ns logic + 9.6 ns routing**  
**Achieved Fmax:** **65.02 MHz**  
**RTL modules involved:** `cpu.sv`, `alu.sv`, `writeback_mux.sv`, `top.sv`, `fpga_common_top.sv`

### Path Narrative

The final routed critical path no longer starts at the SRAM peripheral block RAM output. Instead, it begins at a decoder/control register inside the CPU core, propagates through ALU/control selection logic, and ends at the host-bus request-data capture register:

```text
cpu_core.u_decoder.jump_reg
  → decoder / ALU control-selection logic
  → cpu_host_bus_mux request-data formatting
  → cpu_host_bus_mux.pending_req_wdata
```

In other words, the SRAM/peripheral read-response path is no longer the worst synchronous path on ECP5. The slowest path has shifted back into the CPU-side datapath/control fabric.

### Detailed Stage Breakdown

| Stage | Start (ns) | End (ns) | Δ (ns) | Type | Description |
|-------|-----------:|---------:|-------:|------|-------------|
| Decoder/control register launch | 0.0 | 1.8 | 1.8 | Logic + Routing | Launch from `u_decoder.jump_reg` into downstream control logic |
| Decoder-to-ALU control propagation | 1.8 | 7.4 | 5.6 | Logic + Routing | Control selection reaches ALU-side min/max and multiplier control logic |
| Carry/control chain through ALU logic | 7.4 | 14.0 | 6.6 | Logic + Routing | Multi-level control/carry propagation inside the CPU datapath |
| Host-bus mux formatting | 14.0 | 15.4 | 1.4 | Logic + Routing | Request data reaches `pending_req_wdata` capture |
| Final setup | 15.4 | 15.4 | 0.0 | Setup | Capture in `pending_req_wdata` FF |

### Why This Path Is Slow

1. **The path crosses multiple CPU sub-blocks.** It travels from decoder state into ALU-side control selection and then out through the host-bus mux.
2. **There is still significant routing cost.** Routing now contributes **9.6 ns of 15.4 ns**, so physical locality remains a major factor.
3. **The path mixes control and datapath logic.** Even though it is not a pure arithmetic critical path, it still accumulates mux and carry-chain depth before request capture.
4. **The previous SRAM readback bottleneck is gone.** The SRAM change successfully removed that path from the top slot, revealing the next-most-critical CPU-side path.

### Practical Interpretation

This is a healthy result for the ECP5 target:

- The path still closes timing at 50 MHz with margin.
- The SRAM/peripheral path is no longer the timing bottleneck.
- Any future push materially beyond ~65 MHz will likely need optimization in the **CPU control / ALU / host-bus mux** path first.

---

## Pre-Route vs. Final Routed Timing

The `nextpnr-ecp5` log reports two different timing summaries:

| Flow Stage | Reported Fmax | Status |
|------------|---------------|--------|
| After placement refinement | 45.91 MHz | **FAIL** at 50 MHz |
| After final routing | 65.02 MHz | **PASS** at 50 MHz |

This is important because it shows the ECP5 flow's **post-route timing is materially better than the post-placement estimate** for this design. Any future report or regression check should use the **final routed number** near the end of `nextpnr.log`.

---

## Cross-Domain Timing

The ECP5 build also reports comfortable margins on the asynchronous interface paths:

| Path Type | Max Delay | Notes |
|-----------|-----------|-------|
| **`<async>` → `posedge clk`** | 2.48 ns | USB RX / async inputs into synchronized logic |
| **`posedge clk` → `<async>`** | 2.51 ns | Registered outputs to board pins such as LED |

These numbers are far below the 20 ns system-clock budget and do not present an immediate timing concern.

---

## Summary and Recommendations

### Summary

- **Timing target met:** Yes, with **65.02 MHz** achieved vs. **50.00 MHz** required
- **Critical path delay:** **15.4 ns**
- **Dominant path:** CPU control / ALU logic into host-bus request-data capture
- **SRAM-path result:** Previous SRAM readback critical path resolved
- **Resource pressure:** Low to moderate; no obvious ECP5 capacity bottlenecks

### Recommendations

1. **No immediate timing work is required** for the current 50 MHz ECP5 target.
2. If a higher target frequency is needed later, focus first on:
   - CPU control-selection depth around `cpu.sv` / `alu.sv`
   - request-data muxing and capture around the CPU host-bus mux
   - physical locality of those CPU-side datapath/control registers
3. The SRAM change appears worthwhile on ECP5: it preserved BRAM usage while improving routed timing and removing the previously documented SRAM bottleneck.
4. If future ECP5 builds begin to stress arithmetic timing further, the currently unused **DSP blocks** remain available as an additional optimization lever.

Overall, the ECP5 target remains in a healthy timing position, and the SRAM read-latency change improved routed timing while shifting the top critical path away from the SRAM peripheral.

---

## Addendum — Host-Bus Mux Registered-Output Change (2026-03-11)

After the follow-up change that registers all outputs of `rtl/common/io/host_bus_mux.sv`, I rebuilt the ECP5 target and checked the final routed timing report in `rtl/fpga/build/ecp5_icepi_zero/nextpnr.log`.

### Current Routed Timing

- **Achieved routed Fmax:** **60.36 MHz**
- **Critical path delay:** **16.6 ns**
- **Breakdown:** **4.2 ns logic + 12.3 ns routing**

### Current Critical Path

The worst synchronous path has **moved** from the CPU control / ALU / host-bus request-capture path documented above.

The current routed critical path is:

```text
rtl_registered_bus.pending_req_addr[29]
  → registered_bus address decode / slave-select carry chain
  → clock_periph.mem_a_handshake logic
  → rtl_registered_bus.slave_response_pending logic
  → sysctrl.response_pending / cpu_boot gating
  → sysctrl.boot_addr_reg CE
```

In terms of RTL blocks, the path now runs primarily through:

- `registered_bus.sv`
- `clock_peripheral.sv`
- `system_controller_peripheral.sv`
- integration wiring in `top.sv` / `fpga_common_top.sv`

### Comparison Against the Originally Documented Path

| Item | Originally documented in this report | Current post-`host_bus_mux` result |
|------|--------------------------------------|------------------------------------|
| Routed Fmax | **65.02 MHz** | **60.36 MHz** |
| Critical path delay | **15.4 ns** | **16.6 ns** |
| Dominant path | CPU control / ALU → `cpu_host_bus_mux.pending_req_wdata` | Registered-bus decode / peripheral handshake → `sysctrl.boot_addr_reg` |
| Did the path move? | n/a | **Yes** |

So the answer is **yes**: after registering `host_bus_mux` outputs, the routed ECP5 critical path is no longer the CPU/ALU-to-host-bus-capture path described in the main body of this document. The new worst path has shifted into the **registered bus / peripheral handshake / system-controller boot gating** logic.
