# FPGA Synthesis

This directory contains the board wrappers, constraints, and build collateral for the supported FPGA targets in this repository.

## Supported targets

- **`TARGET=ecp5_icepi_zero`** *(default)* — iCE Pi Zero (Lattice ECP5-25F) via Yosys + nextpnr-ecp5 + ecppack
- **`TARGET=artix7_alchitry_au`** — Alchitry Au (Xilinx Artix-7) via Vivado batch Tcl
- **`TARGET=cyclonev_analogue_pocket`** — Analogue Pocket bring-up target via Quartus batch Tcl
- **`TARGET=gowin_tang_primer_25k`** — Sipeed Tang Primer 25K bring-up target via Gowin batch Tcl

## Default open-source target

The repository's default FPGA flow is now **`ecp5_icepi_zero`**.

Current baseline characteristics for that target:

- **Board / FPGA**: iCE Pi Zero / ECP5-25F-CABGA256
- **Clocking**: Direct 50 MHz board clock
- **Programming default**: SRAM (`PROGRAM_MODE=sram`)
- **Primary artifact**: `build/ecp5_icepi_zero/riscv_fpga.bit`
- **Host link**: UART-backed host bus through `fpga_common_top`

## Quick start

### Install open-source ECP5 tools

```bash
sudo apt-get update
sudo apt-get install -y yosys fpga-trellis fpga-trellis-database nextpnr-ecp5 openfpgaloader
```

### Build the default target

```bash
cd rtl/fpga
make
```

### Build another supported target

```bash
cd rtl/fpga
make TARGET=artix7_alchitry_au
make TARGET=cyclonev_analogue_pocket
make TARGET=gowin_tang_primer_25k
```

### Program the default target

```bash
cd rtl/fpga
sudo make program

# Equivalent explicit command
sudo openFPGALoader -b icepi-zero -m build/ecp5_icepi_zero/riscv_fpga.bit
```

## Standardized FPGA stats

Use the stats flow when you need concise utilization and max-frequency data for a supported target with stats support.

```bash
cd rtl/fpga
make TARGET=ecp5_icepi_zero stats STATS_FORMAT=text
make TARGET=ecp5_icepi_zero stats STATS_FORMAT=json
make TARGET=artix7_alchitry_au stats STATS_FORMAT=markdown
```

Generated artifacts land under `build/<target>/`:

- `riscv_fpga_stats.json`
- `riscv_fpga_stats.md`

Stats are currently implemented for:

- `ecp5_icepi_zero`
- `artix7_alchitry_au`

## Files

- **`common/fpga_common_top.sv`** — Shared FPGA integration wrapper used by the UART-backed board targets
- **`ecp5_icepi_zero/`** — Default open-source target wrapper, LPF constraints, and timing exceptions
- **`artix7_alchitry_au/`** — Artix-7 wrapper, XDC constraints, and Vivado batch flow
- **`cyclonev_analogue_pocket/`** — Quartus bring-up target and Pocket packaging collateral
- **`gowin_tang_primer_25k/`** — Gowin bring-up target and batch-flow collateral
- **`Makefile`** — Common build entry point for all supported targets
- **`fpga_design_stats.py`** — Normalized timing/utilization summary generator

## Common commands

```bash
cd rtl/fpga
make help
make timing
make utilization
make clean
make check-tools
```

## Toolchain notes

### ECP5 iCE Pi Zero

- Uses Yosys + nextpnr-ecp5 + ecppack
- Uses `ecp5_icepi_zero/ecp5_icepi_zero.lpf` for pin constraints
- Uses `ecp5_icepi_zero/timing_async.sdc` for external timing intent
- Uses `openFPGALoader -b icepi-zero` for programming

### Artix-7 Alchitry Au

- Uses the checked-in Vivado batch Tcl flow in `artix7_alchitry_au/vivado_build.tcl`
- Emits reports and bitstream artifacts under `build/artix7_alchitry_au/`

### Analogue Pocket

- Uses the checked-in Quartus batch Tcl flow in `cyclonev_analogue_pocket/quartus_build.tcl`
- Produces a packaged Pocket deployment artifact during the build flow
- Remains a bring-up target rather than a default CI-validated flow

### Tang Primer 25K

- Uses the checked-in Gowin batch Tcl flow in `gowin_tang_primer_25k/gowin_build.tcl`
- Remains a local bring-up target outside default CI

## CI policy

GitHub Actions now verifies synthesis only for the default **`ecp5_icepi_zero`** target. Vendor-tool flows remain checked-in and reviewable, but they are intentionally outside the default CI environment.
