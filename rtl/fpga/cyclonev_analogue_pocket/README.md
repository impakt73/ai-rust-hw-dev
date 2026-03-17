# Analogue Pocket (Cyclone V) Bring-Up Target

This target adds an openFPGA-style scaffold for `TARGET=cyclonev_analogue_pocket`.
It keeps the Pocket-facing source tree local to `rtl/fpga/cyclonev_analogue_pocket/` while instantiating the shared repository RTL from `rtl/common/`.

## Status

This is an **initial bring-up target**.

- `apf_top.v` remains the Pocket platform-facing top.
- `core_top.sv` is the user-core integration seam.
- `analogue_pocket_repo_top.sv` instantiates `rtl/common/top.sv` directly.
- The repository UART/host path is intentionally stubbed with explicit no-op values.

Because the host path is stubbed, this target is currently suitable only for SRAM/peripheral-oriented bring-up work.
External-memory workflows that depend on the UART-backed host bus are intentionally deferred.

## Tooling

The checked-in build flow expects Intel Quartus tools in `PATH`, especially `quartus_sh`.

## Build

```bash
cd rtl/fpga
make TARGET=cyclonev_analogue_pocket
```

Build artifacts are written under `rtl/fpga/build/cyclonev_analogue_pocket/`.

## Notes

- The target directory keeps openFPGA-style metadata/configuration files alongside target-local FPGA sources.
- Packaging/deployment automation beyond the Quartus batch compile is follow-on work.
