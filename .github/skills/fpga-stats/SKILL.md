---
name: fpga-stats
description: Standard workflow for generating concise FPGA utilization and max-frequency stats for supported targets.
---

# FPGA Design Stats Workflow

Use this skill whenever you need **resource utilization** or **max-frequency (Fmax)** numbers for an FPGA target in this repository.
The workflow requires **Python 3.10+**.

## Standard Command

From the repository root:

```bash
cd rtl/fpga
make TARGET=<target> stats STATS_FORMAT=json
```

Supported targets:

- `ecp5_icepi_zero`
- `artix7_alchitry_au`

Examples:

```bash
cd rtl/fpga
make TARGET=ecp5_icepi_zero stats STATS_FORMAT=text
```

## What the Workflow Does

The `stats` target runs the normal synthesis flow for the selected target and then invokes:

```bash
./fpga_design_stats.py --target <target> --build --format <format>
```

The script standardizes the output by:

1. Running the target build when requested
2. Parsing the authoritative timing artifact for that target
3. Parsing post-route resource utilization plus post-synthesis cell counts
4. Writing normalized artifacts into `rtl/fpga/build/<target>/`

`--build-dir` is only for re-parsing existing artifacts. Do not combine it with
`--build`, because the synthesis flow writes to the standard `build/<target>/`
directory.

Generated artifacts:

- `riscv_fpga_stats.json`
- `riscv_fpga_stats.md`

## Authoritative Sources Per Target

- **`ecp5_icepi_zero`**
  - Routed timing report: prefer `build/ecp5_icepi_zero/riscv_fpga_timing.rpt`; fall back to `build/ecp5_icepi_zero/nextpnr.log`
  - Resource utilization: `build/ecp5_icepi_zero/nextpnr.log`
  - Synthesis cell counts: `build/ecp5_icepi_zero/yosys.log`

- **`artix7_alchitry_au`**
  - Routed timing report: `build/artix7_alchitry_au/riscv_fpga_timing.rpt`
  - Timing summary: `build/artix7_alchitry_au/riscv_fpga_timing_summary.rpt`
  - Utilization report: `build/artix7_alchitry_au/riscv_fpga_utilization.rpt`

## Agent Guidance

- Prefer `STATS_FORMAT=json` when answering with compact machine-readable data.
- Prefer `STATS_FORMAT=text` when you need a short human-readable summary in the terminal.
- When interpreting regressions, remember that this repo prioritizes **high-frequency operation**: prefer recommendations that add register stages or shorten combinational cones rather than preserving single-cycle logic at all costs.
- Ready/valid-style handshake returns that cannot be registered without major architectural changes are an allowed exception; mention that constraint explicitly when it affects the Fmax trade-off.
- If you already have up-to-date build artifacts and only need to reformat them, run this from `rtl/fpga`:

```bash
cd rtl/fpga
python3 fpga_design_stats.py --target <target> --format json
```

- When comparing revisions, keep the same target and compare the generated `riscv_fpga_stats.json` files.
