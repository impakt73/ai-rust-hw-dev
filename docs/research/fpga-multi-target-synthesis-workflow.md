# FPGA Multi-Target Synthesis Workflow: Current Repository State

**Research Document**  
**Context:** Current-state refresh of the repository FPGA synthesis workflow  
**Date:** 2026-03-21

---

## Executive Summary

The repository already has a real multi-target FPGA workflow. The authoritative
entry point is `rtl/fpga/Makefile`, which currently supports **four** named
targets:

1. **`ice40_alchitry_cu`** - Alchitry Cu v1 / Lattice iCE40-HX8K via
   Yosys + nextpnr-ice40 + icepack
2. **`ecp5_icepi_zero`** - iCE Pi Zero / Lattice ECP5-25F via
   Yosys + nextpnr-ecp5 + ecppack
3. **`artix7_alchitry_au`** - Alchitry Au / Xilinx Artix-7 via
   Vivado batch Tcl
4. **`cyclonev_analogue_pocket`** - Analogue Pocket bring-up target /
   Intel Cyclone V via Quartus batch Tcl

The key current-state distinctions are:

- The repository has a stable **`make TARGET=...`** abstraction for FPGA builds.
- The shared hardware model is **board wrapper -> `fpga_common_top` ->
  `rtl/common/top.sv`** for the current Cu / iCE Pi Zero / Au flows.
- The shared FPGA integration remains **UART-centric**, with
  `fpga_common_top` instantiating a 1,000,000 baud UART host link.
- Standard GitHub Actions CI currently verifies only the **open-source**
  synthesis targets: iCE40 and ECP5 iCE Pi Zero.
- The Analogue Pocket target is now a **checked-in supported build target**,
  but it is still explicitly a **bring-up-only** platform integration because
  its repository host path is stubbed rather than connected to the normal UART
  host-bus flow.

This means the main question is no longer whether the repository supports
multiple FPGA targets. It does. The more useful framing is:

- which targets have full runtime parity versus partial bring-up support,
- which flows participate in open-source CI,
- and what structure new platform targets should follow.

---

## 1. Current Supported Target Matrix

The authoritative target list lives in `rtl/fpga/Makefile` and is mirrored in
`rtl/fpga/README.md`.

| Target | Board / FPGA | Tool flow | Primary output | Current status |
|--------|---------------|-----------|----------------|----------------|
| `ice40_alchitry_cu` | Alchitry Cu v1 / iCE40-HX8K-CB132 | Yosys + nextpnr-ice40 + icepack | `riscv_fpga.bin` | Supported; default target; CI-covered |
| `ecp5_icepi_zero` | iCE Pi Zero / ECP5-25F-CABGA256 | Yosys + nextpnr-ecp5 + ecppack | `riscv_fpga.bit` | Supported; CI-covered |
| `artix7_alchitry_au` | Alchitry Au / XC7A35T-FTG256-1 | Vivado batch/Tcl | `riscv_fpga.bit` | Supported; local/vendor-tool flow |
| `cyclonev_analogue_pocket` | Analogue Pocket / 5CEBA4F23C8 | Quartus batch/Tcl | `riscv_fpga.rbf` | Supported bring-up target; local/vendor-tool flow |

Two practical status boundaries matter:

### 1.1 Open-source CI targets

These are the targets validated in the normal GitHub Actions workflow:

- `ice40_alchitry_cu`
- `ecp5_icepi_zero`

Those are also the targets whose build dependencies are installed directly in
CI (`yosys`, `nextpnr-ice40`, `nextpnr-ecp5`, Trellis, openFPGALoader).

### 1.2 Local/vendor-tool targets

These require proprietary vendor tools and are intentionally outside the default
open-source CI path:

- `artix7_alchitry_au` -> Vivado
- `cyclonev_analogue_pocket` -> Quartus

The repository already treats this split as normal rather than exceptional.

---

## 2. Current Multi-Target Architecture

### 2.1 Public build interface

The implemented public interface is:

```bash
cd rtl/fpga
make TARGET=ice40_alchitry_cu
make TARGET=ecp5_icepi_zero
make TARGET=artix7_alchitry_au
make TARGET=cyclonev_analogue_pocket
```

This is the abstraction boundary new platform targets should preserve whenever
possible.

### 2.2 Shared vs. target-specific structure

The repository already uses the decomposition that a scalable FPGA flow needs:

- **Shared FPGA integration:** `rtl/fpga/common/fpga_common_top.sv`
- **Board/platform wrappers:** `rtl/fpga/<target>/`
- **Vendor-neutral CPU/peripheral RTL:** `rtl/common/`

The board wrappers remain intentionally thin. They typically own:

1. board-level ports,
2. clock generation or direct clock selection,
3. reset synchronization / debounce / lock handling,
4. simple board-local I/O mapping such as LEDs,
5. instantiation of the shared FPGA integration top.

### 2.3 Shared runtime assumption: UART-backed host integration

The present shared model is still built around a UART host transport:

- `fpga_common_top` exposes `usb_rx` / `usb_tx`
- it instantiates `uart.sv`
- the baud rate is fixed to **1,000,000**
- `rtl/common/top.sv` uses the host byte-stream path for the host bus /
  external-memory bridge

This matters because it separates current targets into two categories:

- **full shared-model wrappers** that can provide a practical UART path
- **platform bring-up wrappers** that can build the design but do not yet
  provide the full host/runtime path

### 2.4 Current architecture tiers

The repo currently has three effective tiers of FPGA support:

1. **Open-source, CI-verified board targets**  
   `ice40_alchitry_cu`, `ecp5_icepi_zero`
2. **Vendor-tool, repo-native board targets**  
   `artix7_alchitry_au`
3. **Vendor-tool, repo-native platform bring-up targets**  
   `cyclonev_analogue_pocket`

That distinction is more accurate than describing every target as equally
complete.

---

## 3. Current Target Profiles

### 3.1 `ice40_alchitry_cu`

- **Flow:** Yosys + nextpnr-ice40 + icepack
- **Programming:** openFPGALoader
- **Clocking:** board 100 MHz input -> PLL-derived 25 MHz system clock
- **Default program mode:** `flash`
- **Build artifacts:** `rtl/fpga/build/ice40_alchitry_cu/`

This remains the default open-source board target and the smallest fully
documented reproduction path.

### 3.2 `ecp5_icepi_zero`

- **Flow:** Yosys + nextpnr-ecp5 + ecppack
- **Programming:** openFPGALoader
- **Clocking:** direct 50 MHz board clock
- **Default program mode:** `sram`
- **Build artifacts:** `rtl/fpga/build/ecp5_icepi_zero/`

This is a real, supported open-source ECP5 target, not future work.

### 3.3 `artix7_alchitry_au`

- **Flow:** Vivado batch/Tcl
- **Programming:** openFPGALoader
- **Clocking:** board-specific Artix-7 wrapper flow
- **Default program mode:** `sram`
- **Build artifacts:** `rtl/fpga/build/artix7_alchitry_au/`

This target establishes the repository pattern for proprietary-tool support:
checked-in target directory, scripted batch flow, standardized output location,
and Makefile integration.

### 3.4 `cyclonev_analogue_pocket`

- **Flow:** Quartus batch/Tcl
- **Deployment model:** package-oriented rather than direct board programming
- **Default program mode:** `package`
- **Build artifacts:** `rtl/fpga/build/cyclonev_analogue_pocket/`

This target is important because it is **implemented**, but it is not yet at
runtime parity with the UART-hosted board targets.

Current Pocket-specific facts:

- the target keeps an openFPGA-style source tree under
  `rtl/fpga/cyclonev_analogue_pocket/`
- `analogue_pocket_repo_top.sv` instantiates `rtl/common/top.sv` directly
- the normal repository host/UART path is intentionally stubbed
- the target is currently suitable for SRAM/peripheral-oriented bring-up rather
  than full host-backed runtime workflows

That makes the Pocket a current platform target with **partial integration**,
not merely a hypothetical future board.

---

## 4. Reporting, Stats, and Artifacts

### 4.1 Standardized stats workflow

For the non-Pocket supported stats targets, the repo provides:

```bash
cd rtl/fpga
make TARGET=<target> stats STATS_FORMAT=json
```

Today, the standardized stats workflow is implemented for:

- `ice40_alchitry_cu`
- `ecp5_icepi_zero`
- `artix7_alchitry_au`

It is **not implemented yet** for `cyclonev_analogue_pocket`.

### 4.2 Artifact conventions

The current build system already standardizes output directories well:

- `rtl/fpga/build/<target>/`

Within that directory, targets emit their native logs, bitstreams, and timing /
utilization reports. This is the right model for future targets because it lets
repo docs point to artifact locations rather than hard-coded timing numbers that
age quickly.

### 4.3 Preferred reporting model

The current repo is strongest when documentation explains:

- where timing/utilization truth comes from,
- which artifacts are authoritative,
- and how to regenerate them,

rather than embedding static performance claims in research docs.

---

## 5. Current CI Coverage

The GitHub Actions workflow currently verifies:

- SystemVerilog lint
- Rust formatting / clippy / build / tests
- iCE40 synthesis
- ECP5 iCE Pi Zero synthesis

The workflow does **not** currently run:

- Vivado-based Artix-7 synthesis
- Quartus-based Analogue Pocket synthesis

This is the current repository policy and should be treated as deliberate:

- open-source FPGA targets are CI-verified directly
- proprietary-tool targets are kept scriptable, reproducible, and reviewable,
  but remain outside the default CI environment

---

## 6. Implications for New FPGA Targets

The current repository suggests a clear rule set for future targets:

1. **Keep `rtl/common/` vendor-neutral**
2. **Add a target-local wrapper under `rtl/fpga/<target>/`**
3. **Integrate the target in `rtl/fpga/Makefile` via `TARGET=...`**
4. **Write outputs to `rtl/fpga/build/<target>/`**
5. **Document durable procedures in `rtl/fpga/README.md` and `docs/fpga/`**
6. **Be explicit about whether the target has full host/runtime parity or only
   bring-up-level support**

The Pocket target demonstrates why the last rule matters. A target can be
genuinely integrated into the repo without yet participating in the full shared
UART-host runtime model.

---

## 7. Key Current Conclusions

The refreshed current-state picture is:

- The repo supports **four** FPGA targets today, not three.
- The multi-target Makefile and wrapper architecture are already established and
  working.
- The shared FPGA integration remains **UART-host-centric**.
- The iCE40 and iCE Pi Zero targets are the current **open-source CI-backed**
  implementations.
- The Artix-7 target is the current **fully supported vendor-tool board flow**.
- The Analogue Pocket target is the current **vendor-tool bring-up platform
  flow**, with packaging support and Quartus integration already checked in, but
  with the host/runtime path still stubbed.

That is the accurate baseline any new FPGA platform work should start from.
