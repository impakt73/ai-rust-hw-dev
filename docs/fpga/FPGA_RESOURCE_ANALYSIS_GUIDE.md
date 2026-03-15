# FPGA Resource Analysis Reproduction Guide

This guide documents the exact process used to regenerate:

- `docs/fpga/FPGA_TOP_RESOURCE_BREAKDOWN.md`

It is written so future updates can be reproduced consistently after new RTL merges.

---

## 1) Scope and outputs

The analysis document combines **two kinds of data** for each FPGA target:

1. **Packed/post-route metrics** from nextpnr logs
   - utilization (% used / available)
   - post-route Fmax
2. **Hierarchical module attribution** from Yosys `-noflatten` statistics
   - board top -> `fpga_common_top` -> `rtl/common/top.sv` -> `cpu` -> `alu`

### Targets covered

- `TARGET=ice40_alchitry_cu`
- `TARGET=ecp5_icepi_zero`

---

## 2) Prerequisites

From repository root:

```bash
export REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"
```

Required tools:

- `yosys`
- `nextpnr-ice40`
- `nextpnr-ecp5`
- `icepack`
- `ecppack`
- `rg` (ripgrep, used for metric extraction checks)

If `rg` is unavailable, use `grep -E` with the same pattern.

Check availability:

```bash
cd "$REPO_ROOT/rtl/fpga"
make check-tools TARGET=ice40_alchitry_cu
make check-tools TARGET=ecp5_icepi_zero
```

---

## 3) Regenerate synthesis outputs (fresh)

Run both target flows from `rtl/fpga`:

```bash
cd "$REPO_ROOT/rtl/fpga"

make clean TARGET=ice40_alchitry_cu
make TARGET=ice40_alchitry_cu all utilization

make clean TARGET=ecp5_icepi_zero
make TARGET=ecp5_icepi_zero all utilization
```

Generated artifacts:

- `rtl/fpga/build/ice40_alchitry_cu/*`
- `rtl/fpga/build/ecp5_icepi_zero/*`

---

## 4) Extract packed utilization and Fmax

### ICE40 (from nextpnr)

File:

- `rtl/fpga/build/ice40_alchitry_cu/nextpnr.log`

Fields used in report:

- `ICESTORM_LC`
- `ICESTORM_RAM`
- `SB_IO`
- `SB_GB`
- `ICESTORM_PLL`
- Fmax from clock line containing `pll_clk_global`

Quick check:

```bash
rg -n "ICESTORM_LC|ICESTORM_RAM|SB_IO|SB_GB|ICESTORM_PLL|Max frequency for clock 'pll_clk_global'" \
  "$REPO_ROOT/rtl/fpga/build/ice40_alchitry_cu/nextpnr.log"
```

### ECP5 (from nextpnr)

File:

- `rtl/fpga/build/ecp5_icepi_zero/nextpnr.log`

Fields used in report:

- `TRELLIS_COMB`
- `TRELLIS_FF`
- `DP16KD`
- `TRELLIS_IO`
- `DCCA`
- `EHXPLLL`
- Fmax from the **last** line containing:
  - `Max frequency for clock '$glbnet$clk$TRELLIS_IO_IN'`

Quick check:

```bash
rg -n "TRELLIS_COMB|TRELLIS_FF|DP16KD|TRELLIS_IO|DCCA|EHXPLLL|Max frequency for clock '\\$glbnet\\$clk\\$TRELLIS_IO_IN'" \
  "$REPO_ROOT/rtl/fpga/build/ecp5_icepi_zero/nextpnr.log"
```

---

## 5) Generate hierarchical stats used for module breakdown tables

The Makefile synthesis flattens for implementation outputs, so the report additionally uses Yosys `-noflatten` runs to build hierarchical attribution tables.

Run:

```bash
cd "$REPO_ROOT/rtl/fpga"

# ICE40 hierarchical stats JSON
yosys -q -p " \
  read_verilog -sv +/ice40/cells_sim.v \
    ../common/cpu/alu.sv ../common/cpu/branch_unit.sv \
    ../common/cpu/csr_file.sv ../common/cpu/decoder.sv \
    ../common/cpu/decompress.sv ../common/cpu/div_unit.sv ../common/primitives/ff_sync.sv \
    ../common/primitives/phase_accumulator.sv ../common/primitives/activity_indicator.sv \
    ../common/primitives/square_wave_generator.sv \
    ../common/io/host_bus_mux.sv ../common/cpu/fetch_buffer.sv ../common/fpu/fp_regfile.sv \
    ../common/fpu/fpu.sv ../common/fpu/fpu_classifier.sv ../common/fpu/fpu_comparator.sv \
    ../common/fpu/fpu_div_assemble.sv ../common/fpu/fpu_div_setup.sv ../common/fpu/fpu_float_to_int.sv \
    ../common/fpu/fpu_fma.sv ../common/fpu/fpu_int_to_float.sv ../common/fpu/fpu_sqrt.sv \
    ../common/io/host_bus_interface.sv ../common/io/host_bus_rx.sv ../common/io/host_bus_tx.sv \
    ../common/io/sys_led_controller.sv ../common/cpu/mem_interface.sv ../common/cpu/mul_unit.sv \
    ../common/memory/registered_bus.sv ../common/memory/sync_dpram.sv ../common/cpu/regfile.sv \
    ../common/primitives/sync_fifo.sv ../common/primitives/reset_controller.sv ../common/io/uart.sv \
    ../common/memory/sram.sv ../common/cpu/writeback_mux.sv ../common/cpu/cpu.sv \
    ../common/peripherals/clock_peripheral.sv ../common/peripherals/led_controller_peripheral.sv \
    ../common/peripherals/sram_peripheral.sv ../common/peripherals/system_controller_peripheral.sv \
    ../common/top.sv common/fpga_common_top.sv ice40_alchitry_cu/ice40_alchitry_cu_top.sv; \
  hierarchy -check -top ice40_alchitry_cu_top; \
  synth_ice40 -top ice40_alchitry_cu_top -noflatten; \
  tee -o build/ice40_alchitry_cu/hier_stat.json stat -json"

# ECP5 hierarchical stats JSON
yosys -q -p " \
  read_verilog -sv \
    ../common/cpu/alu.sv ../common/cpu/branch_unit.sv \
    ../common/cpu/csr_file.sv ../common/cpu/decoder.sv \
    ../common/cpu/decompress.sv ../common/cpu/div_unit.sv ../common/primitives/ff_sync.sv \
    ../common/primitives/phase_accumulator.sv ../common/primitives/activity_indicator.sv \
    ../common/primitives/square_wave_generator.sv \
    ../common/io/host_bus_mux.sv ../common/cpu/fetch_buffer.sv ../common/fpu/fp_regfile.sv \
    ../common/fpu/fpu.sv ../common/fpu/fpu_classifier.sv ../common/fpu/fpu_comparator.sv \
    ../common/fpu/fpu_div_assemble.sv ../common/fpu/fpu_div_setup.sv ../common/fpu/fpu_float_to_int.sv \
    ../common/fpu/fpu_fma.sv ../common/fpu/fpu_int_to_float.sv ../common/fpu/fpu_sqrt.sv \
    ../common/io/host_bus_interface.sv ../common/io/host_bus_rx.sv ../common/io/host_bus_tx.sv \
    ../common/io/sys_led_controller.sv ../common/cpu/mem_interface.sv ../common/cpu/mul_unit.sv \
    ../common/memory/registered_bus.sv ../common/memory/sync_dpram.sv ../common/cpu/regfile.sv \
    ../common/primitives/sync_fifo.sv ../common/primitives/reset_controller.sv ../common/io/uart.sv \
    ../common/memory/sram.sv ../common/cpu/writeback_mux.sv ../common/cpu/cpu.sv \
    ../common/peripherals/clock_peripheral.sv ../common/peripherals/led_controller_peripheral.sv \
    ../common/peripherals/sram_peripheral.sv ../common/peripherals/system_controller_peripheral.sv \
    ../common/top.sv common/fpga_common_top.sv ecp5_icepi_zero/ecp5_icepi_zero_top.sv; \
  hierarchy -check -top ecp5_icepi_zero_top; \
  synth_ecp5 -top ecp5_icepi_zero_top -noflatten; \
  tee -o build/ecp5_icepi_zero/hier_stat.json stat -json"
```

Notes:

- ICE40 hierarchical synthesis explicitly loads `+/ice40/cells_sim.v` because `ice40_alchitry_cu_top` instantiates `SB_PLL40_CORE`.
  - In Yosys, `+/` means "resolve from Yosys's built-in share/data directory".
- ECP5 hierarchical synthesis does not require an equivalent explicit cell sim file in this flow.

---

## 6) Build the markdown tables

Use:

- `build/<target>/hier_stat.json` for hierarchical module-area attribution
- `build/<target>/nextpnr.log` for packed utilization and Fmax

Populate sections in `docs/fpga/FPGA_TOP_RESOURCE_BREAKDOWN.md`:

1. ICE40 summary table
2. ICE40 hierarchy tables (board top, `fpga_common_top`, `top`, `cpu`, ALU)
3. ECP5 summary table
4. ECP5 hierarchy tables (board top, `fpga_common_top`, `top`, `cpu`, ALU)
5. Notes on primitive vocabulary differences

---

## 7) Consistency checks before commit

Confirm report numbers match logs:

```bash
# ICE40 utilization/Fmax references
rg -n "ICESTORM_LC|ICESTORM_RAM|SB_IO|SB_GB|ICESTORM_PLL|Max frequency for clock 'pll_clk_global'" \
  "$REPO_ROOT/rtl/fpga/build/ice40_alchitry_cu/nextpnr.log"

# ECP5 utilization/Fmax references
rg -n "TRELLIS_COMB|TRELLIS_FF|DP16KD|TRELLIS_IO|DCCA|EHXPLLL|Max frequency for clock '\\$glbnet\\$clk\\$TRELLIS_IO_IN'" \
  "$REPO_ROOT/rtl/fpga/build/ecp5_icepi_zero/nextpnr.log"
```

Check only intended report/docs files are modified:

```bash
cd "$REPO_ROOT"
git --no-pager status --short
```

If build artifacts are staged accidentally, remove them before commit.

---

## 8) Update checklist for future refreshes

When repeating this process after a merge from `main`:

- [ ] Rebuild ICE40
- [ ] Rebuild ECP5
- [ ] Refresh hierarchical `hier_stat.json` for both
- [ ] Update all report tables and timestamp
- [ ] Re-verify utilization/Fmax values against `nextpnr.log`
- [ ] Confirm ALU detailed breakdown is still present for both targets
