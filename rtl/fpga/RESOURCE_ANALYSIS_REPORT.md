# RV32F Resource Consumption Report for iCE40-HX8K

This report investigates why the RV32F implementation cannot currently be enabled on the Alchitry Cu v1 (`ice40_alchitry_cu`) target and identifies the F-extension blocks that dominate FPGA resource usage.

- **Target FPGA:** Lattice iCE40-HX8K-CB132
- **Available logic cells:** 7,680
- **Date:** 2026-03-09
- **Primary conclusion:** the F extension is **logic-cell bound, not BRAM bound**. Enabling RV32F pushes the routed design from **5,580 / 7,680 LCs (72%)** to **14,573 / 7,680 LCs (189%)**, so the build fails in nextpnr with no logic cells remaining.

## Measurement Methodology

Three concrete data sets were collected:

1. **Current shipped ice40 build (RV32F disabled)**
   - Command: `cd rtl/fpga && make TARGET=ice40_alchitry_cu all utilization`
   - Source of truth: `rtl/fpga/build/ice40_alchitry_cu/yosys.log` and `rtl/fpga/build/ice40_alchitry_cu/nextpnr.log`
2. **Experimental ice40 build with RV32F forced on**
   - Command: Yosys `chparam -set ENABLE_F_EXT 1 ice40_alchitry_cu_top` followed by `nextpnr-ice40`
   - Source of truth: `rtl/fpga/build/ice40_alchitry_cu/yosys_f_enabled.log` and `rtl/fpga/build/ice40_alchitry_cu/nextpnr_f_enabled.log`
3. **Standalone per-module synthesis on iCE40 technology mapping**
   - Command family: `cd rtl/fpga/resource_analysis && make synth-<module>`
   - Source of truth: `rtl/fpga/resource_analysis/build/*.log`

### Important Interpretation Rule

The standalone numbers are most useful for **ranking hotspots**.

- `fpu` already contains the arithmetic core, converters, comparator/classifiers, and the shared 48-bit divider path, so **do not add `fpu` to its child modules**.
- `fp_regfile` is instantiated **outside** `fpu` in `rtl/common/cpu/cpu.sv`, so `fp_regfile + fpu` is the best standalone approximation of the RV32F datapath cost before CPU-side decode/control/writeback overhead.
- Child-module totals are therefore used for **attribution**, not exact addition.

## Top-Level Impact on the HX8K Build

### Baseline vs. Experimental RV32F-Enabled Build

| Build | Yosys SB_LUT4 | Yosys DFF | Yosys RAM | nextpnr LCs | nextpnr RAM | Status |
|---|---:|---:|---:|---:|---:|---|
| Current `ice40_alchitry_cu_top` (`ENABLE_F_EXT=0`) | 4,528 | 2,252 | 30 | 5,580 / 7,680 (72%) | 30 / 32 (93%) | Routes successfully |
| Experimental `ENABLE_F_EXT=1` | 12,230 | 3,646 | 30 | 14,573 / 7,680 (189%) | 30 / 32 (93%) | Fails: no logic cells remaining |
| **Delta when enabling RV32F** | **+7,702** | **+1,394** | **+0** | **+8,993 LCs** | **+0** | Logic is the blocker |

### What This Means

- **RV32F does not materially change BRAM usage** on this target. The build already uses 30/32 RAM blocks with RV32F disabled, and it still reports 30/32 with RV32F enabled.
- The overrun is overwhelmingly in **logic cells / LUT fabric**.
- The shipped F-disabled build still has timing margin after place-and-route: **41.40 MHz Fmax** against a **25 MHz** target clock.
- Because the F-enabled build already reaches **189% LC utilization**, timing optimization alone cannot solve this problem.

## RV32F Module Inventory and Hierarchy

### F-extension integration points

- `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv`
  - `ENABLE_F_EXT` defaults to `1'b0` for the HX8K build.
- `rtl/common/cpu/cpu.sv`
  - Instantiates `fp_regfile` and `fpu` when `ENABLE_F_EXT=1`.
- `rtl/common/fpu/fpu.sv`
  - Instantiates the arithmetic, conversion, comparison, and divide helper blocks.

### FPU internal hierarchy

| Module type | Instances inside `fpu` | Purpose |
|---|---:|---|
| `fpu_classifier` | 2 | Operand classification for `fs1` and `fs2` |
| `fpu_comparator` | 1 | `FEQ.S` / `FLT.S` / `FLE.S` support |
| `fpu_fma` | 1 | Shared arithmetic core for `FADD.S`, `FSUB.S`, `FMUL.S`, and all fused ops |
| `fpu_sqrt` | 1 | `FSQRT.S` |
| `fpu_int_to_float` | 2 | Signed and unsigned `FCVT.S.W*` paths |
| `fpu_float_to_int` | 2 | Signed and unsigned `FCVT.W*.S` paths |
| `fpu_div_setup` | 1 | `FDIV.S` special-case handling and operand preparation |
| `div_unit` (`WIDTH=48`) | 1 | Shared mantissa divider for `FDIV.S` |
| `fpu_div_assemble` | 1 | Re-normalizes divider output into IEEE-754 format |

## Per-Module Resource Breakdown

### Top-level RV32F blocks synthesized standalone

| Module | Standalone SB_LUT4 | % of HX8K | DFFs | Carry cells | Notes |
|---|---:|---:|---:|---:|---|
| `fpu` | 4,081 | 53.1% | 444 | 491 | Integrated floating-point execution block |
| `fp_regfile` | 694 | 9.0% | 335 | 3 | Separate FP register file instantiated in `cpu.sv` |

**Key standalone estimate:** `fpu + fp_regfile = 4,775 LUT4`, or **62.2% of the entire device**, before accounting for the CPU-side decode/control/writeback logic that also becomes active when RV32F is enabled.

### Internal `fpu` hotspot breakdown (standalone synthesis of module types)

| Module type | Instances in `fpu` | LUT4 per module | Approx. replicated subtotal | % of HX8K per module | Why it matters |
|---|---:|---:|---:|---:|---|
| `fpu_fma` | 1 | 2,300 | 2,300 | 29.9% | Dominant arithmetic block; shared by add/sub/mul/fused ops |
| `div_unit` (`WIDTH=48`) | 1 | 553 | 553 | 7.2% | Shared mantissa divider used by `FDIV.S` |
| `fpu_div_assemble` | 1 | 513 | 513 | 6.7% | Division result normalization and packing |
| `fpu_int_to_float` | 2 | 327 | 654 | 4.3% | Two instances are used even though the module already has `is_signed` input |
| `fpu_float_to_int` | 2 | 220 | 440 | 2.9% | Same duplication pattern as integer-to-float conversion |
| `fpu_comparator` | 1 | 97 | 97 | 1.3% | Needed for ordered FP comparisons |
| `fpu_div_setup` | 1 | 61 | 61 | 0.8% | Prepares divide operands and handles special cases |
| `fpu_classifier` | 2 | 33 | 66 | 0.4% | Small, duplicated helper for NaN/Inf/Zero decode |
| `fpu_sqrt` | 1 | 32 | 32 | 0.4% | Very small placeholder-style implementation |

### Hotspot Ranking

1. **`fpu_fma`** is the largest individual consumer by a wide margin.
2. **The `FDIV.S` path** (`div_unit` + `fpu_div_assemble` + `fpu_div_setup`) is the next-largest functional cluster at roughly **1,127 LUT4** by standalone composition.
3. **Converter duplication** (`2 × fpu_int_to_float` + `2 × fpu_float_to_int`) is another significant cluster at roughly **1,094 LUT4** by standalone composition.
4. **`fp_regfile`** is a meaningful external cost at **694 LUT4**, and it is structurally difficult to map into iCE40 BRAM because the current CPU/FPU interface expects **3 asynchronous read ports**.

## Root-Cause Observations

### 1. The arithmetic core is fundamentally too large for HX8K in its current form

`fpu_fma` is a fully combinational multiply-align-add-normalize datapath. It is shared effectively, but even after that consolidation it still costs **2,300 LUT4** standalone.

Because `fpu_fma` serves:
- `FADD.S`
- `FSUB.S`
- `FMUL.S`
- `FMADD.S`
- `FMSUB.S`
- `FNMSUB.S`
- `FNMADD.S`

there is no low-cost way to keep the current arithmetic instruction set while removing this block.

### 2. The divide path is expensive enough to justify being optional on HX8K

`fpu_div_setup` is small, but the shared **48-bit** `div_unit` plus `fpu_div_assemble` consume another **1,066 LUT4** standalone.

That makes `FDIV.S` a good candidate for feature-gating or software emulation on resource-constrained targets.

### 3. The conversion blocks are duplicated even though the modules are already parameterized by `is_signed`

In `rtl/common/fpu/fpu.sv`, the design instantiates:
- two `fpu_int_to_float` blocks
- two `fpu_float_to_int` blocks

but both module types already accept an `is_signed` input.

That duplication is one of the clearest localized opportunities to reclaim LUTs without changing the external ISA surface.

### 4. The FP register file is architecturally expensive on iCE40

`rtl/common/fpu/fp_regfile.sv` explicitly documents why it stays LUT-based today:
- depth is only 32 entries,
- reads are asynchronous,
- three simultaneous read ports are required for fused operations.

That combination blocks straightforward iCE40 BRAM inference and makes the FP register file a persistent LUT consumer.

## Actionable Resource-Reduction Suggestions

The table below focuses on changes that directly target the measured hotspots.

| Suggestion | Targeted blocks | Expected benefit | Development effort | Technical complexity | Rationale |
|---|---|---|---|---|---|
| **Share one signed/unsigned converter instance per direction** | `2 × fpu_int_to_float`, `2 × fpu_float_to_int` | Moderate | **Low** | **Low** | The converter modules already take `is_signed`; replacing the duplicated instances in `fpu.sv` with one instance per direction is a localized change and should remove a noticeable chunk of duplicated LUT logic. |
| **Add an HX8K-specific reduced-F profile that omits `FDIV.S`** | `fpu_div_setup`, `div_unit`, `fpu_div_assemble` | High | **Low** | **Low-Medium** | The divide path is more than 1.1k LUT standalone and is cleanly isolated in the current hierarchy. It is the easiest large feature slice to disable while keeping the rest of the FPU structure intact. |
| **Extend the reduced-F profile to drop `FCVT.*` on HX8K if software fallback is acceptable** | `fpu_int_to_float`, `fpu_float_to_int` | Moderate to High | **Low-Medium** | **Medium** | The conversion logic is already isolated and duplicated. If an HX8K profile can rely on software conversion sequences, this removes another sizable logic cluster. |
| **Serialize or prefetch the third FP source operand for fused ops** | `fp_regfile` | Moderate | **Medium** | **Medium-High** | The present 3-read-port asynchronous FP register file costs 694 LUTs and blocks BRAM inference. Allowing an extra cycle for fused ops could reduce the read-port requirement and make a denser implementation practical. |
| **Replace the current `FDIV.S` implementation with a slower, narrower, or multi-pass divider** | `div_unit`, `fpu_div_assemble` | Moderate | **Medium** | **High** | The existing divider uses a 48-bit non-restoring datapath. A higher-latency divider could trade cycles for less fabric, but it is a more invasive arithmetic redesign. |
| **Re-architect `fpu_fma` into a multi-cycle or iterative arithmetic datapath** | `fpu_fma` | Very High | **High** | **High** | `fpu_fma` is the dominant area consumer. Any serious attempt to fit a meaningful arithmetic subset on HX8K eventually has to reduce this block, but it requires a substantial redesign of the current combinational datapath. |
| **Rework the CPU/FPU interface to tolerate synchronous FP register reads and BRAM-style storage** | `fp_regfile`, CPU execute/decode timing | Moderate to High | **High** | **High** | This is the architectural fix hinted at in `fp_regfile.sv`, but it requires extra latency handling, hazard management, and likely changes to the multi-cycle CPU sequencing. |

## Recommended Prioritization

### Best near-term experiments

1. **Remove converter duplication first**
   - Lowest-risk localized optimization.
   - Good candidate for an immediate measurable win.
2. **Create an HX8K `F-lite` profile without `FDIV.S`**
   - Biggest low-effort area reduction available from a cleanly isolated functional block.
3. **If still over budget, also remove `FCVT.*` in the HX8K profile**
   - This attacks another clearly isolated logic cluster before touching the arithmetic core.

### If full RV32F on HX8K remains a hard requirement

The current measurements strongly suggest that **feature trimming alone is unlikely to be enough** unless the arithmetic core itself is also redesigned. In practice, fitting “full” RV32F on HX8K likely requires at least one of:

- a substantially smaller multi-cycle `fpu_fma`,
- an architectural change to the FP register file interface,
- or both.

## Reproduction Commands

### Baseline ice40 build

```bash
cd rtl/fpga
make TARGET=ice40_alchitry_cu all utilization
```

### Standalone F-extension module analysis

```bash
cd rtl/fpga/resource_analysis
make synth-fp_regfile \
     synth-fpu \
     synth-fpu_fma \
     synth-fpu_sqrt \
     synth-fpu_div_setup \
     synth-fpu_div_assemble \
     synth-fpu_float_to_int \
     synth-fpu_int_to_float \
     synth-fpu_classifier \
     synth-fpu_comparator \
     synth-div_unit
```

### Experimental RV32F-enabled HX8K synthesis

```bash
cd rtl/fpga
yosys -p "read_verilog -sv ../common/cpu/alu.sv ../common/cpu/branch_unit.sv \
  ../common/memory/bus.sv ../common/memory/bus_arbiter.sv ../common/cpu/csr_file.sv \
  ../common/cpu/decoder.sv ../common/cpu/decompress.sv ../common/cpu/div_unit.sv \
  ../common/primitives/ff_sync.sv ../common/primitives/phase_accumulator.sv \
  ../common/primitives/activity_indicator.sv ../common/primitives/square_wave_generator.sv \
  ../common/io/bus_bridge.sv ../common/io/host_bus_mux.sv ../common/cpu/fetch_buffer.sv \
  ../common/fpu/fp_regfile.sv ../common/fpu/fpu.sv ../common/fpu/fpu_classifier.sv \
  ../common/fpu/fpu_comparator.sv ../common/fpu/fpu_div_assemble.sv \
  ../common/fpu/fpu_div_setup.sv ../common/fpu/fpu_float_to_int.sv ../common/fpu/fpu_fma.sv \
  ../common/fpu/fpu_int_to_float.sv ../common/fpu/fpu_sqrt.sv ../common/io/host_bus_interface.sv \
  ../common/io/host_bus_rx.sv ../common/io/host_bus_tx.sv ../common/io/sys_led_controller.sv \
  ../common/cpu/mem_interface.sv ../common/cpu/mul_unit.sv ../common/memory/registered_bus.sv \
  ../common/memory/sync_dpram.sv ../common/cpu/regfile.sv ../common/primitives/sync_fifo.sv \
  ../common/primitives/reset_controller.sv ../common/io/uart.sv ../common/memory/sram.sv \
  ../common/cpu/writeback_mux.sv ../common/cpu/cpu.sv ../common/peripherals/clock_peripheral.sv \
  ../common/peripherals/led_controller_peripheral.sv ../common/peripherals/sram_peripheral.sv \
  ../common/peripherals/system_controller_peripheral.sv ../common/top.sv \
  common/fpga_common_top.sv ice40_alchitry_cu/ice40_alchitry_cu_top.sv; \
  chparam -set ENABLE_F_EXT 1 ice40_alchitry_cu_top; \
  hierarchy -top ice40_alchitry_cu_top; \
  synth_ice40 -top ice40_alchitry_cu_top -json build/ice40_alchitry_cu/riscv_fpga_f_enabled.json"

nextpnr-ice40 --hx8k --package cb132 \
  --json build/ice40_alchitry_cu/riscv_fpga_f_enabled.json \
  --pcf ice40_alchitry_cu/ice40_alchitry_cu.pcf \
  --asc build/ice40_alchitry_cu/riscv_fpga_f_enabled.asc \
  --freq 25
```
