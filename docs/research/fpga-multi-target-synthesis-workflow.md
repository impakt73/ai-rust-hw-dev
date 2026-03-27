# FPGA Multi-Target Synthesis Workflow: Current Repository State

**Research Document**  
**Context:** Current-state refresh of the repository FPGA synthesis workflow  
**Date:** 2026-03-27

---

## Executive Summary

The authoritative FPGA build entry point remains `rtl/fpga/Makefile`, but the supported target matrix has changed:

1. **`ecp5_icepi_zero`** — default open-source target, CI-covered
2. **`artix7_alchitry_au`** — vendor-tool board flow via Vivado
3. **`cyclonev_analogue_pocket`** — vendor-tool bring-up flow via Quartus
4. **`gowin_tang_primer_25k`** — vendor-tool bring-up flow via Gowin EDA

The deprecated `ice40_alchitry_cu` target and its related collateral are no longer part of the repository.

The key current-state distinctions are now:

- The repository still exposes a stable **`make TARGET=...`** abstraction.
- **`ecp5_icepi_zero`** is the only open-source target validated in default CI.
- The shared UART-backed wrapper model remains **`board wrapper -> fpga_common_top -> rtl/common/top.sv`** for the ECP5 and Artix-7 board flows.
- The Analogue Pocket and Tang Primer 25K targets remain checked-in bring-up flows that are outside default CI.

---

## 1. Current Supported Target Matrix

| Target | Board / FPGA | Tool flow | Primary output | Current status |
|--------|---------------|-----------|----------------|----------------|
| `ecp5_icepi_zero` | iCE Pi Zero / ECP5-25F-CABGA256 | Yosys + nextpnr-ecp5 + ecppack | `riscv_fpga.bit` | Supported; default target; CI-covered |
| `artix7_alchitry_au` | Alchitry Au / XC7A35T-FTG256-1 | Vivado batch Tcl | `riscv_fpga.bit` | Supported; local/vendor-tool flow |
| `cyclonev_analogue_pocket` | Analogue Pocket / 5CEBA4F23C8 | Quartus batch Tcl | `riscv_fpga.rbf` | Supported bring-up target; local/vendor-tool flow |
| `gowin_tang_primer_25k` | Tang Primer 25K / GW5A-LV25MG121NC1/I0 | Gowin batch Tcl | `riscv_fpga.fs` | Supported bring-up target; local/vendor-tool flow |

### 1.1 Open-source CI target

The normal GitHub Actions workflow now validates exactly one synthesis target:

- `ecp5_icepi_zero`

That is also the only open-source FPGA toolchain installed by default in CI and the Copilot setup workflow (`yosys`, `nextpnr-ecp5`, Trellis, openFPGALoader).

### 1.2 Local/vendor-tool targets

These flows remain repository-native but intentionally outside default CI:

- `artix7_alchitry_au` -> Vivado
- `cyclonev_analogue_pocket` -> Quartus
- `gowin_tang_primer_25k` -> Gowin EDA

---

## 2. Public Build Interface

```bash
cd rtl/fpga
make                          # default: TARGET=ecp5_icepi_zero
make TARGET=artix7_alchitry_au
make TARGET=cyclonev_analogue_pocket
make TARGET=gowin_tang_primer_25k
```

This remains the abstraction boundary that new board targets should preserve.

---

## 3. Shared Architecture Model

The repository still uses the same scalable decomposition:

- **Shared FPGA integration:** `rtl/fpga/common/fpga_common_top.sv`
- **Board/platform wrappers:** `rtl/fpga/<target>/`
- **Vendor-neutral CPU/peripheral RTL:** `rtl/common/`

For the UART-backed board targets, `fpga_common_top` still provides:

- board-facing `usb_rx` / `usb_tx`
- the instantiated `uart.sv` transport
- the shared connection into `rtl/common/top.sv`

The Pocket target remains the main exception because it stubs the normal repository host path during bring-up.

---

## 4. Reporting and Stats

The normalized FPGA stats workflow remains:

```bash
cd rtl/fpga
make TARGET=<target> stats STATS_FORMAT=json
```

Stats are currently implemented for:

- `ecp5_icepi_zero`
- `artix7_alchitry_au`

Output artifacts are written to:

- `rtl/fpga/build/<target>/`

This output convention remains the preferred way to document timing/utilization truth in the repository.

---

## 5. Current CI Coverage

Default GitHub Actions currently verifies:

- SystemVerilog lint
- Rust formatting / clippy / build / tests
- FPGA synthesis for `ecp5_icepi_zero`

The workflow does **not** run:

- Vivado-based Artix-7 synthesis
- Quartus-based Analogue Pocket synthesis
- Gowin-based Tang Primer 25K synthesis

That split is deliberate:

- the default open-source target is CI-verified directly
- vendor-tool flows remain reproducible and reviewable, but are kept outside the default CI environment

---

## 6. Implications for Future FPGA Work

The repository still suggests the same rule set for future targets:

1. Keep `rtl/common/` vendor-neutral
2. Add a target-local wrapper under `rtl/fpga/<target>/`
3. Integrate the target in `rtl/fpga/Makefile` via `TARGET=...`
4. Write outputs to `rtl/fpga/build/<target>/`
5. Document durable procedures in `rtl/fpga/README.md` and `docs/fpga/`
6. Be explicit about whether the target has full runtime parity or only bring-up-level support

The main update is policy, not architecture: the repository now standardizes on **ECP5 iCE Pi Zero** as the default open-source board target.
