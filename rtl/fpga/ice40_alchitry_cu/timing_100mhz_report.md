# iCE40 Alchitry Cu 100 MHz Timing Report

## Scope

This report captures the first synthesis/timing attempt after updating the
`ice40_alchitry_cu` target to run directly from the board's 100 MHz oscillator.

## Commands Run

From `/home/runner/work/ai-rust-hw-dev/ai-rust-hw-dev/rtl/fpga`:

```bash
make TARGET=ice40_alchitry_cu stats STATS_FORMAT=json
make TARGET=ice40_alchitry_cu timing
python3 fpga_design_stats.py --target ice40_alchitry_cu --format json
```

## Overall Result

- **Clock target:** 100.00 MHz
- **nextpnr routed Fmax:** 65.87 MHz
- **icetime cross-check:** 64.05 MHz
- **Timing status:** FAIL
- **Shortfall vs target:** 34.13 MHz (~5.61 ns of period deficit versus a 10.00 ns target)

### Build-flow impact

`make TARGET=ice40_alchitry_cu stats STATS_FORMAT=json` fails as a top-level
command because nextpnr returns non-zero when the 100 MHz constraint is not met.
Even so, the run still produces enough intermediate artifacts for analysis:

- `build/ice40_alchitry_cu/yosys.log`
- `build/ice40_alchitry_cu/nextpnr.log`
- `build/ice40_alchitry_cu/riscv_fpga_timing.rpt`
- `build/ice40_alchitry_cu/riscv_fpga_stats.json`

## Resource Snapshot

Post-route utilization from `build/ice40_alchitry_cu/riscv_fpga_stats.json`:

| Resource | Used | Available | Utilization |
| --- | ---: | ---: | ---: |
| ICESTORM_LC | 5690 | 7680 | 74.0% |
| ICESTORM_RAM | 30 | 32 | 93.0% |
| SB_GB | 8 | 8 | 100.0% |
| SB_IO | 77 | 256 | 30.0% |

Observations:

- Logic utilization is not saturated, but it is already substantial at 74%.
- BRAM usage is very high at 93%, which limits options for structural buffering
  or memory reorganization on this part.
- All 8 global buffers are consumed, so additional global-clock or global-enable
  strategies will likely require reclaiming an existing global net first.
- The design no longer uses an iCE40 PLL (`ICESTORM_PLL = 0/2`) because the
  target now runs directly from the board clock.

## Critical Timing Path

### Top-level summary

Both nextpnr and icetime identify the same worst synchronous path on clock
`clk$SB_IO_IN_$glb_clk`:

- **Launch register:** `fpga_common_top_inst.cpu_inst.cpu_core.a_reg[0]`
- **Capture side:** `fpga_common_top_inst.cpu_inst.cpu_core.imem_addr_internal[0]`
- **Total logic levels:** 41
- **Total path delay:** 15.61 ns (`riscv_fpga_timing.rpt`)

### Functional interpretation

The failing path is dominated by branch-decision arithmetic and the next-PC
selection cone:

1. **Registered branch operands**
   - `a_reg` and `b_reg` are fed into the branch decision logic in
     `rtl/common/cpu/cpu.sv:895-902`.
2. **32-bit branch comparison**
   - `rtl/common/cpu/branch_unit.sv:21-29` implements the signed/unsigned
     branch comparisons (`BLT`, `BGE`, `BLTU`, `BGEU`) directly on 32-bit
     operands.
3. **Next-PC selection**
   - `rtl/common/cpu/cpu.sv:532-540` uses `take_branch` to choose the next
     program counter during `S_BRANCH`.
4. **Instruction memory address fanout**
   - `rtl/common/cpu/cpu.sv:561` drives `imem_addr_internal = pc`, pulling the
     path into the instruction fetch side.

### Why this path is slow

The delay is not concentrated in a single LUT. It is a long arithmetic/control
cone built from:

- An initial decode/compare stage from `a_reg`
- A wide carry-chain traversal inside the branch comparison network
- Additional logic stages feeding the PC/imem control path

From `build/ice40_alchitry_cu/riscv_fpga_timing.rpt`, the carry-chain portion
alone spans multiple rows of logic cells:

- ~1.55 ns reached by branch compare bit 0
- ~3.25 ns by bit 7
- ~4.46 ns by bit 15
- ~5.66 ns by bit 23
- ~6.74 ns by bit 30
- ~7.64 ns by bit 31

After the carry chain, more decode/control logic extends the same path:

- ~8.68 ns at `u_decoder.branch...I1[0]`
- ~9.58 ns at `u_decoder.branch...I0[0]`
- ~10.57 ns at `u_decoder.branch...I2[2]`
- ~12.50 ns at `u_decoder.branch...I3_O[2]`
- ~13.40 ns at `pc...I2[0]`
- ~15.61 ns at the capture endpoint on `imem_addr_internal[0]`

This means the design is currently paying for both:

- a **wide branch comparison** on 32-bit operands, and
- a **same-cycle dependency** from branch decision into the next fetch address.

## Primary Bottlenecks Identified

1. **Branch comparator width**
   - The worst path clearly traverses a long carry chain in the branch
     comparison network.
2. **Branch-decision-to-PC coupling**
   - Even with precomputed branch targets, the `take_branch` decision still
     lands in the next-PC/imem address cone in the same cycle.
3. **Global routing pressure**
   - `SB_GB` is already at 100%, which reduces flexibility for improving high
     fanout timing distribution.
4. **Memory-heavy placement pressure**
   - 30/32 BRAMs in use leaves limited placement freedom around the CPU core,
     which can indirectly hurt critical routing.

## Recommended Next Investigation Areas

These are the most likely high-value places to improve 100 MHz timing on HX8K:

1. **Pipeline or register the branch decision path**
   - Break the `a_reg/b_reg -> take_branch -> next_pc_value/imem_addr_internal`
     dependency across an additional cycle.
2. **Special-case branch comparisons**
   - Consider whether all branch forms need the full current compare structure,
     or whether signed/unsigned compares can be restructured to shorten the
     longest carry path.
3. **Reduce fanout/logic sharing on the PC enable path**
   - The tail end of the path enters PC/imem control logic after the compare
     chain, so localized register duplication or logic partitioning may help.
4. **Free routing/global-buffer headroom**
   - The design already uses all global buffers and nearly all BRAMs, which
     limits physical implementation flexibility on HX8K.

## Bottom Line

The target now correctly requests and analyzes a **100 MHz** clock, but the
current ice40 implementation is **not timing-clean at that frequency**. The
first concrete bottleneck is the **branch comparison / next-PC path**, which
routes at roughly **64-66 MHz** on the HX8K.
