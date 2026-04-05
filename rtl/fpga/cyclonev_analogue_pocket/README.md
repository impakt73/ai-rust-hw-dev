# Analogue Pocket (Cyclone V) Bring-Up Target

This target adds an openFPGA-style scaffold for `TARGET=cyclonev_analogue_pocket`.
It keeps the Pocket-facing source tree local to `rtl/fpga/cyclonev_analogue_pocket/` while instantiating the shared repository RTL from `rtl/common/`.

## Status

This is an **initial bring-up target**.

- `apf_top.v` remains the Pocket platform-facing top.
- `core_top.v` is the user-core integration seam.
- `cyclonev_analogue_pocket_top.sv` instantiates `rtl/fpga/common/fpga_common_top.sv` to reuse the standard UART-backed host path.
- `core_top.v` routes the Pocket link-port SI/SO pins to the shared UART RX/TX path.

The Pocket link port is enabled in `core.json`, so external-memory workflows can use the same UART-backed host bus path as the other FPGA targets.

## Tooling

The checked-in build flow expects Intel Quartus tools in `PATH`, especially `quartus_sh`.

## Build

```bash
cd rtl/fpga
make TARGET=cyclonev_analogue_pocket
```

Build artifacts are written under `rtl/fpga/build/cyclonev_analogue_pocket/`.
The regular build now also runs `pkt`, so each successful synthesis leaves a deployable Pocket core zip in that build directory.
As part of that packaging flow, the release `test_pocket_demo` Rust program is converted to a raw `.bin` with `objcopy` and staged under the Pocket `Assets/` tree before `pkt` folds it into the final zip as a core-specific asset.

To deploy the generated zip into a user-selected directory:

```bash
make TARGET=cyclonev_analogue_pocket program POCKET_DEPLOY_DIR=/path/to/pocket
```

`make program` extracts the generated zip into `POCKET_DEPLOY_DIR` with `unzip -o`, so existing files with the same names are overwritten.

## Notes

- The target directory keeps openFPGA-style metadata/configuration files alongside target-local FPGA sources.
- The regular build produces the Quartus `.rbf` output, a staged `test_pocket_demo.bin` Pocket asset, and a deployable Pocket core zip.
- `pkt` expands `Assets/ex_platform/ex_core_name/...` using `core.json` metadata so core-specific assets land under `/Assets/<platform>/<author>.<shortname>/...` in the deployed zip.
