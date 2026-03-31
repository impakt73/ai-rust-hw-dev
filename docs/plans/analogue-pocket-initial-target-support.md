# Analogue Pocket Initial Target Support Plan

## 1. Overview

This document defines the initial implementation plan for adding **Analogue Pocket** support as a new FPGA target in this repository. The goal is to make the Pocket target feel native to the existing `rtl/fpga/` workflow while respecting the structure expected by Analogue's openFPGA ecosystem.

The implementation should follow two simultaneous constraints:

1. **Repository-native target integration**
   - the Pocket target should fit the existing `rtl/fpga/<target>/` pattern,
   - be selectable from `rtl/fpga/Makefile` via `TARGET=cyclonev_analogue_pocket`,
   - and emit build artifacts under `rtl/fpga/build/cyclonev_analogue_pocket/`.

2. **openFPGA-native core structure**
   - the Pocket target-specific folder should contain the **entire openFPGA core source/metadata tree** derived from the official `open-fpga/core-template`,
   - the template's `apf_top.v` remains the platform-facing top,
   - and the template's `core_top` becomes the integration seam where repository RTL is instantiated.

For the **initial vertical slice**, the UART path from the repository's existing FPGA common module should **not** be implemented on Pocket. The UART-facing signals should instead be driven with explicit no-op values at the repository/Pocket integration boundary.

---

## 2. Goals

### 2.1 Primary Goals

1. Add a new target named **`cyclonev_analogue_pocket`** under `rtl/fpga/`.
2. Place the full Pocket/openFPGA core source tree inside the target-specific folder.
3. Use the official template structure as the baseline so the target looks like a normal openFPGA core rather than a one-off Quartus project.
4. Convert the template `core_top.v` to **SystemVerilog** (`core_top.sv`).
5. Instantiate a repository-owned Pocket target wrapper from inside `core_top.sv`.
6. Preserve the existing repository RTL in `rtl/common/` as vendor-neutral shared logic.
7. Defer host/UART transport integration by explicitly stubbing it for the first implementation.

### 2.2 Secondary Goals

1. Keep the Quartus/openFPGA-specific collateral isolated to the new target directory.
2. Minimize changes to existing Lattice and Artix-7 targets.
3. Establish a clean path for later APF bridge, storage, video, audio, and controller integration.

---

## 3. Non-Goals for the Initial Bring-Up

The first Pocket implementation should **not** attempt to solve the full platform problem. The following are intentionally out of scope for the initial vertical slice:

1. Replacing the repository's UART host bus with a Pocket-native transport.
2. Mapping the host/debug protocol onto APF bridge commands.
3. Supporting host-backed DRAM access through the current external-memory path.
4. Implementing full audio/video output from the RISC-V system.
5. Integrating Pocket controller input into CPU software.
6. Extending CI to build Quartus/openFPGA outputs.
7. Normalizing Quartus timing/resource reports in `rtl/fpga/fpga_design_stats.py`.

---

## 4. Current-State Constraints That Shape the Design

### 4.1 Existing Repository FPGA Structure

The repository already uses a stable multi-target model:

- `rtl/common/` contains vendor-neutral CPU/peripheral RTL.
- `rtl/fpga/<target>/` contains target-specific wrappers and collateral.
- `rtl/fpga/Makefile` selects a target with `TARGET=...`.
- at the time this plan was written, the existing supported FPGA board targets
  were `ecp5_icepi_zero` and `artix7_alchitry_au`; consult
  `rtl/fpga/Makefile` for the current list of supported targets.

This existing structure should remain intact. The Pocket target should be added as another target, not as a special alternate build system detached from `rtl/fpga/`.

### 4.2 Existing Shared FPGA Integration Is UART-Centric

Today, `rtl/fpga/common/fpga_common_top.sv` instantiates `rtl/common/top.sv` and wraps it with a UART transport using board-facing `usb_rx` / `usb_tx` pins. That is appropriate for the current boards, but it does **not** map naturally to the Pocket's APF/openFPGA structure.

The key implication is:

- the initial Pocket target should **not** route through `fpga_common_top.sv`,
- and should instead instantiate `rtl/common/top.sv` from a new Pocket-specific wrapper that can tie off or stub the host/UART signals explicitly.

### 4.3 openFPGA Template Structure

The official `open-fpga/core-template` establishes a target shape that the repository should preserve inside the new Pocket target directory:

- root metadata files such as `core.json`, `video.json`, `audio.json`, `input.json`, `interact.json`, `variants.json`, `data.json`, and `info.txt`,
- `src/fpga/apf/apf_top.v` as the platform-facing APF top,
- `src/fpga/core/core_top.v` as the user-core top instantiated by `apf_top.v`,
- and Quartus project files such as `src/fpga/ap_core.qpf` / `src/fpga/ap_core.qsf`.

That means the natural place to integrate this repository's RTL is **inside `core_top`**, not by replacing `apf_top`.

---

## 5. Proposed Target Directory Layout

Create a new target directory:

```text
rtl/fpga/cyclonev_analogue_pocket/
├── audio.json
├── core.json
├── data.json
├── info.txt
├── input.json
├── interact.json
├── variants.json
├── video.json
├── src/
│   └── fpga/
│       ├── ap_core.qpf
│       ├── ap_core.qsf
│       ├── apf/
│       │   ├── apf_top.v
│       │   ├── common.v
│       │   ├── io_bridge_peripheral.v
│       │   ├── io_pad_controller.v
│       │   └── other required template/APF support files
│       └── core/
│           ├── core_top.sv
│           ├── core_bridge_cmd.v
│           ├── core_constraints.sdc
│           ├── analogue_pocket_repo_top.sv
│           └── any small Pocket-specific helper modules
├── quartus_build.tcl
├── README.md
└── .gitignore
```

### 5.1 Folder Content Policy

The target directory should contain the **complete openFPGA core source/metadata tree** needed to build a Pocket core, but should **not** commit generated Quartus outputs such as `output_files/` or packaged release artifacts. Those generated files should remain build outputs under:

- `rtl/fpga/build/cyclonev_analogue_pocket/`

This preserves the repository's existing build-artifact convention while still keeping the full Pocket core definition and sources localized to the target-specific directory.

---

## 6. Target Architecture

### 6.1 Top-Level Ownership Model

The initial Pocket architecture should have three clear layers:

1. **APF/platform layer**
   - `src/fpga/apf/apf_top.v`
   - owns the Pocket platform ports and APF bridge wiring

2. **Pocket user-core integration layer**
   - `src/fpga/core/core_top.sv`
   - continues to be the module instantiated by `apf_top.v` after conversion to SystemVerilog
   - owns only Pocket-to-repository adaptation

3. **Repository RTL wrapper layer**
   - `src/fpga/core/analogue_pocket_repo_top.sv`
   - new repository-owned target wrapper instantiated from `core_top.sv`
   - instantiates `rtl/common/top.sv`
   - adapts clocks, reset, and stubbed host/UART behavior

This matches the openFPGA template's expectations and keeps repository-specific behavior out of the platform top.

### 6.2 Why `core_top.sv` Is the Right Integration Seam

The official template already treats `core_top` as the user-owned core entry point. Converting it to SystemVerilog and instantiating a repository-specific wrapper from there is the most natural way to integrate the repository RTL because it:

- preserves the template's `apf_top.v` ownership model,
- keeps APF-specific code and repository-specific code separated,
- matches the existing repository pattern where target wrappers are thin and platform-facing,
- and avoids forcing APF concepts into `rtl/common/`.

---

## 7. Detailed Module Plan

### 7.1 `apf_top.v`

For the initial implementation, keep `src/fpga/apf/apf_top.v` as close to the template version as possible.

Required changes should be limited to:

1. ensuring it instantiates `core_top` from `core_top.sv`,
2. updating file references in the Quartus project if needed,
3. and only making minimal edits required to compile the repository-owned integration module tree.

### 7.2 `core_top.sv`

Rename/convert:

- `src/fpga/core/core_top.v` -> `src/fpga/core/core_top.sv`

This converted module should remain the template's user-core top from the APF point of view, but internally it should instantiate the repository's actual Pocket target wrapper.

Conceptually:

```systemverilog
module core_top (... Pocket/APF ports ...);
    analogue_pocket_repo_top repo_top_inst (... adapted Pocket/APF signals ...);
endmodule
```

The conversion to SystemVerilog is important because the repository-owned wrapper and future adaptation logic will be cleaner and more consistent when written in `.sv` using `logic`, `always_ff`, and `always_comb` where needed.

### 7.3 `analogue_pocket_repo_top.sv`

Create a new module in the target-specific `core/` directory that acts as the Pocket equivalent of the existing board wrappers.

Responsibilities:

1. accept the Pocket-side clock/reset handed down from `core_top.sv`,
2. convert reset to the repository's internal synchronous active-high `rst` convention,
3. instantiate `rtl/common/top.sv`,
4. expose any immediately useful status outputs (for example, LEDs or APF-visible debug placeholders),
5. and stub the host/UART path explicitly.

This module should be treated as the Pocket target's true repository integration wrapper.

---

## 8. UART / Host-Bus Strategy for the Initial Implementation

### 8.1 Required Initial Behavior

The initial implementation should **not** attempt to connect the repository's UART host bus to Pocket hardware.

Instead, the Pocket target wrapper should instantiate `rtl/common/top.sv` directly and hardcode the host interface to no-op values such as:

- `host_rx_data = '0`
- `host_rx_valid = 1'b0`
- `com_err = 1'b0`
- `host_tx_ready = 1'b1`

and ignore:

- `host_tx_data`
- `host_tx_valid`
- `host_rx_ready`

This makes the lack of transport explicit and local to the Pocket wrapper.

### 8.2 Why Not Reuse `fpga_common_top.sv`

Although tying `usb_rx` high and ignoring `usb_tx` would be the smallest literal UART stub, it is not the preferred initial Pocket architecture because:

1. it preserves a board-serial abstraction that the Pocket does not actually expose,
2. it hides the transport limitation inside the wrong architectural layer,
3. and it conflicts with the goal of making `core_top.sv` the natural openFPGA integration seam.

The Pocket target should therefore bypass `fpga_common_top.sv` for the first implementation.

### 8.3 Functional Limitation of the UART Stub

This stubbed configuration has an important limitation: accesses that depend on the external host-memory path will not work.

Therefore the initial bring-up must be explicitly limited to workloads that remain within repository-local RTL resources such as:

- boot/reset behavior,
- LED peripheral,
- clock peripheral,
- system controller,
- and on-chip SRAM peripheral.

The plan and follow-up documentation should state clearly that the first Pocket target is a **bring-up target**, not a full parity replacement for the existing UART-backed FPGA runtime.

---

## 9. Build-System Plan

### 9.1 New Makefile Target

Extend `rtl/fpga/Makefile` with:

- `TARGET=cyclonev_analogue_pocket`

The Pocket target should follow the existing Artix-7 precedent more closely than the Lattice targets:

- proprietary vendor flow,
- repository-owned non-interactive script,
- and predictable artifacts under `build/<target>/`.

### 9.2 Quartus Driver Script

Add a checked-in Tcl batch script, e.g.:

- `rtl/fpga/cyclonev_analogue_pocket/quartus_build.tcl`

Responsibilities:

1. point Quartus at the target-local openFPGA project files,
2. add/override repository source-file references,
3. ensure `core_top.sv` is treated as SystemVerilog,
4. emit logs/reports into `rtl/fpga/build/cyclonev_analogue_pocket/`,
5. and produce the intermediate/programming/package artifacts required by the Pocket flow.

### 9.3 Source Inclusion Strategy

The Quartus project should reference two categories of files:

1. **Target-local Pocket/openFPGA files** from `rtl/fpga/cyclonev_analogue_pocket/`
2. **Shared repository RTL** from `rtl/common/`

The repository should not duplicate `rtl/common/` into the Pocket target directory. Only the openFPGA core/template collateral belongs entirely inside the target directory.

---

## 10. Implementation Sequence

### Phase 1 - Scaffold the Target

1. Create `rtl/fpga/cyclonev_analogue_pocket/`
2. Import the open-fpga/core-template source/metadata tree into that directory.
3. Add a target-local `.gitignore` to exclude Quartus-generated files.
4. Add a target-local README describing tool prerequisites and build entry points.

### Phase 2 - Establish the Integration Seam

1. Convert `src/fpga/core/core_top.v` to `core_top.sv`.
2. Update Quartus project references from `.v` to `.sv`.
3. Add `src/fpga/core/analogue_pocket_repo_top.sv`.
4. Instantiate `analogue_pocket_repo_top` from `core_top.sv`.

### Phase 3 - Connect Repository RTL

1. Instantiate `rtl/common/top.sv` from `analogue_pocket_repo_top.sv`.
2. Pass Pocket clock/reset into repository-compatible `clk` / `rst`.
3. Route any immediately useful status outputs to simple Pocket-visible placeholders as needed.
4. Stub the host/UART-facing signals with explicit no-op values.

### Phase 4 - Add Build Flow Integration

1. Add `TARGET=cyclonev_analogue_pocket` handling to `rtl/fpga/Makefile`.
2. Add the Quartus batch/Tcl script.
3. Standardize outputs in `rtl/fpga/build/cyclonev_analogue_pocket/`.
4. Document the expected developer invocation.

### Phase 5 - Validate the Initial Vertical Slice

1. Confirm Quartus can elaborate/synthesize the imported openFPGA project with repository RTL.
2. Confirm the generated Pocket core still follows the template/APF top hierarchy.
3. Confirm the repository-owned wrapper compiles with UART unconnected.
4. Confirm the resulting target is suitable for SRAM/peripheral-only bring-up.

---

## 11. Validation Plan

### 11.1 Required Local Validation

For the first implementation, validation should focus on build integrity rather than feature completeness:

1. **Quartus compile/elaboration succeeds** for `TARGET=cyclonev_analogue_pocket`.
2. **Existing repository targets remain unchanged**.
3. **Pocket top hierarchy is preserved**:
   - `apf_top.v` instantiates `core_top.sv`
   - `core_top.sv` instantiates `analogue_pocket_repo_top.sv`
   - `analogue_pocket_repo_top.sv` instantiates `rtl/common/top.sv`
4. **Stubbed host/UART path is explicit and reviewable**.

### 11.2 Initial Runtime Expectations

Success for the first runtime milestone should be limited to proving that the target can boot repository RTL without attempting the full external-memory workflow.

Reasonable first demonstrations:

1. reset release works,
2. system clock is stable,
3. `top.sv` is alive,
4. an SRAM-resident test program can run,
5. and a simple RTL-visible behavior such as LED state change can be observed through Pocket-facing debug or temporary integration signals.

---

## 12. Risks and Open Questions

### 12.1 APF Bridge vs. Repository Host Bus

The largest deferred question is how the current repository host-bus/UART model should map onto the Pocket's APF bridge environment. The initial plan intentionally postpones that decision.

### 12.2 Clocking Details

The initial bring-up implementation must confirm which APF/platform clock should drive the repository system and whether a Pocket-specific PLL configuration is required. Even if the first version uses a simple direct clocking choice, that decision needs to be explicit during initial integration rather than deferred implicitly.

### 12.3 Reset Ownership

The template `apf_top.v` currently contains its own reset-generation behavior. The implementation must define a clean handoff from that platform reset behavior into the repository's synchronous active-high `rst` convention.

### 12.4 Packaging Boundaries

The exact boundary between target-local source metadata and generated deployable core packaging still needs to be finalized. The implementation should prefer checked-in source/configuration plus generated outputs in `rtl/fpga/build/`.

---

## 13. Recommended Naming and Conventions

### 13.1 Target Name

Use:

- **`cyclonev_analogue_pocket`**

This matches the repository's existing target naming style and keeps the device/platform identity explicit.

### 13.2 Repository-Owned Pocket Wrapper Name

Use a clearly repository-scoped module name such as:

- `analogue_pocket_repo_top`

This avoids confusion with the template's `apf_top` and `core_top` modules.

### 13.3 Reset Convention

Inside repository-owned SystemVerilog modules:

- use synchronous active-high `rst`,
- convert any Pocket/APF reset conventions at the wrapper boundary,
- and keep target/platform exceptions out of `rtl/common/`.

---

## 14. Completion Criteria for the Initial Implementation

The initial Pocket support work should be considered complete when all of the following are true:

1. `rtl/fpga/cyclonev_analogue_pocket/` exists and contains the complete openFPGA core source/metadata tree required for the target.
2. `rtl/fpga/Makefile` supports `TARGET=cyclonev_analogue_pocket`.
3. the template `core_top` has been converted to `core_top.sv`.
4. `core_top.sv` instantiates a repository-owned Pocket target wrapper.
5. the repository-owned Pocket wrapper instantiates `rtl/common/top.sv` directly.
6. the UART/host signals are intentionally stubbed with explicit no-op values for the first vertical slice.
7. the build flow produces reproducible Quartus/openFPGA outputs under `rtl/fpga/build/cyclonev_analogue_pocket/`.
8. the resulting target is documented as an initial bring-up target with SRAM/peripheral-only expectations until a real Pocket-native transport strategy is added.

---

## 15. Follow-On Work After the Initial Bring-Up

After the initial target exists and compiles, the next work items should be prioritized in this order:

1. define the real Pocket/APF transport bridge strategy,
2. add a proper program-loading/runtime story that does not rely on UART,
3. expose practical debug/status signals through APF-visible mechanisms,
4. integrate Pocket-appropriate video/audio output paths,
5. add packaging/deployment automation,
6. and extend stats/report tooling for Quartus-generated timing and utilization outputs.

This sequencing keeps the first implementation narrow and achievable while still leaving a clean path to a full Pocket-native target.
