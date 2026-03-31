# Sipeed Tang Primer 25K FPGA Target

This directory contains the initial **local-only vendor-tool bring-up target**
for the **Sipeed Tang Primer 25K** (`GW5A-LV25MG121NC1/I0`).

The target follows the repository's normal board-wrapper structure:

- `gowin_tang_primer_25k_top.sv` adapts the Tang board ports
- `../common/fpga_common_top.sv` provides the shared FPGA runtime integration
- shared CPU/peripheral RTL remains in `../../common/`

## Current Support Level

This is an initial bring-up target intended to validate:

1. 50 MHz board clock integration
2. reset-button handling
3. onboard LED visibility
4. UART connectivity through the onboard debugger bridge

The target is intentionally **not** added to default CI yet, and standardized
FPGA stats are **not** implemented for it in this initial revision.

## Board Signals Used

The initial checked-in constraints are based on Sipeed's public example
projects for the Tang Primer 25K:

- `clk` -> `E2` (50 MHz onboard clock)
- `rst_n_btn` -> `K6` (active-low user key)
- `usb_rx` -> `B3` (fabric UART RX from onboard debugger bridge)
- `usb_tx` -> `C3` (fabric UART TX to onboard debugger bridge)
- `led` -> `L6`
- `led_done` -> `D7`
- `led_ready` -> `E8`

## Required Tools

- Gowin EDA with `gw_sh` available in `PATH`
- `openFPGALoader` for `make program`

Sipeed's public Tang Primer 25K documentation notes that the board requires a
recent Gowin IDE release (their examples reference 1.9.9-beta-era tools or
newer). Keep the installed toolchain aligned with current vendor guidance.

## Build Commands

From `rtl/fpga/`:

```bash
make TARGET=gowin_tang_primer_25k check-tools
make TARGET=gowin_tang_primer_25k
make TARGET=gowin_tang_primer_25k timing
make TARGET=gowin_tang_primer_25k utilization
```

The non-interactive batch build is driven by:

- `gowin_build.tcl`

Generated outputs are normalized under:

- `build/gowin_tang_primer_25k/riscv_fpga.fs`
- `build/gowin_tang_primer_25k/riscv_fpga_timing.rpt`
- `build/gowin_tang_primer_25k/riscv_fpga_timing_summary.rpt` (when available)
- `build/gowin_tang_primer_25k/riscv_fpga_utilization.rpt`

The underlying Gowin project workspace remains under:

- `build/gowin_tang_primer_25k/project/`

## Programming

The checked-in Makefile uses the existing repository openFPGALoader flow:

```bash
make TARGET=gowin_tang_primer_25k program
make TARGET=gowin_tang_primer_25k program PROGRAM_MODE=flash
```

Equivalent manual commands:

```bash
openFPGALoader -b tangprimer25k -m build/gowin_tang_primer_25k/riscv_fpga.fs
openFPGALoader -b tangprimer25k -f build/gowin_tang_primer_25k/riscv_fpga.fs
```

## Notes / Limitations

- This target assumes the onboard debugger UART path (`B3` / `C3`) is usable
  for the repository's shared UART-backed runtime.
- If local hardware testing shows the UART bridge is not viable for the shared
  host/runtime path, this target should be treated as a narrower bring-up-only
  target until transport adaptation is added.
- Some Tang Primer 25K users report debugger-firmware sensitivity with
  openFPGALoader; if programming fails, verify the board debugger firmware and
  fall back to the official Gowin programmer when needed.
