# Tang Primer 25K Initial Target Support Plan

## 1. Overview

This document defines the initial implementation plan for adding **Sipeed Tang
Primer 25K** support as a new FPGA target in this repository.

The plan should preserve the repository's existing multi-target FPGA structure:

1. **repo-native target integration**
   - add a new target under `rtl/fpga/gowin_tang_primer_25k/`,
   - expose it through `rtl/fpga/Makefile` as
     `TARGET=gowin_tang_primer_25k`,
   - and normalize build outputs under
     `rtl/fpga/build/gowin_tang_primer_25k/`.

2. **vendor-tool-first synthesis flow**
   - use the Gowin vendor toolchain rather than the open-source flow,
   - drive synthesis, place-and-route, and bitstream generation with a
     non-interactive **Tcl** entry point,
   - and follow the existing repository precedent established by the
     `artix7_alchitry_au` Vivado flow and the
     `cyclonev_analogue_pocket` Quartus flow.

The initial Tang target should be treated as a **local vendor-tool target**.
Like the existing Vivado and Quartus targets, it should **not** be added to the
default GitHub Actions CI workflow in its first implementation.

---

## 2. Goals

### 2.1 Primary Goals

1. Add a new FPGA target named **`gowin_tang_primer_25k`**.
2. Keep all Tang-specific wrapper, constraints, and vendor-project collateral
   localized to `rtl/fpga/gowin_tang_primer_25k/`.
3. Reuse the repository's normal board-target structure:
   `board wrapper -> fpga_common_top -> rtl/common/top.sv`.
4. Add a non-interactive Tcl-driven Gowin build flow that can be invoked through
   the existing `make TARGET=...` interface.
5. Preserve the standard artifact convention under
   `rtl/fpga/build/gowin_tang_primer_25k/`.
6. Reuse the repository's UART-backed host/runtime path if the board's onboard
   debugger/UART connection is practically accessible from FPGA fabric.

### 2.2 Secondary Goals

1. Match the existing vendor-flow ergonomics used by the Artix-7 and Quartus
   targets.
2. Keep the initial integration narrowly focused on clock/reset/LED/UART
   bring-up rather than broad board-feature enablement.
3. Document prerequisites, local build commands, artifact locations, and any
   local programming steps in a target-local README.

---

## 3. Non-Goals for the Initial Bring-Up

The first Tang Primer 25K implementation should **not** attempt to solve every
board/platform question at once. The following are intentionally out of scope
for the initial vertical slice:

1. Adding Gowin synthesis to default GitHub Actions CI.
2. Supporting the Tang target in `rtl/fpga/fpga_design_stats.py`.
3. Adopting or validating an open-source Gowin flow as the default path.
4. Enabling all dock-board peripherals beyond the minimum needed for board
   bring-up.
5. Redesigning the repository's host transport unless UART reuse is proven
   impractical.
6. Claiming full runtime parity before host communication and external-memory
   behavior are verified on real hardware.

---

## 4. Current-State Constraints That Shape the Design

### 4.1 Existing Repository FPGA Structure

The repository already has a stable target model that Tang should extend rather
than replace:

- `rtl/common/` contains vendor-neutral CPU and peripheral RTL.
- `rtl/fpga/common/fpga_common_top.sv` contains the shared FPGA-facing
  integration used by the UART-backed board targets.
- `rtl/fpga/<target>/` contains target-specific wrappers and collateral.
- `rtl/fpga/Makefile` is the public entry point for FPGA builds.
- build outputs are normalized under `rtl/fpga/build/<target>/`.

Tang should slot cleanly into that model as another `TARGET=...` choice.

### 4.2 Existing Vendor-Tool Precedent

The repository already supports two local-only vendor-tool flows:

- **`artix7_alchitry_au`** -> Vivado batch Tcl
- **`cyclonev_analogue_pocket`** -> Quartus batch Tcl

That means Tang does **not** need a new top-level build system. It needs a new
target-local Tcl flow that integrates into the existing Makefile conventions.

### 4.3 Architecture Constraint: Shared Runtime Is UART-Centric

The current shared FPGA runtime assumes:

- a board-local UART connection,
- `rtl/fpga/common/fpga_common_top.sv`,
- and the repository host bus serialized over that UART path.

This is the key architectural gate for Tang support. If the Tang Primer 25K can
route a usable UART pair between the FPGA fabric and the onboard JTAG+UART
bridge, Tang can likely follow the same architecture as the existing Cu / iCE
Pi Zero / Au targets. If not, the initial scope may need to be reduced to a
bring-up-only target until transport adaptation is solved.

### 4.4 Board Facts That Must Be Treated as Authoritative Inputs

The research identified several board facts that should be treated as required
inputs before implementation details are finalized:

- FPGA family/device: **Gowin `GW5A-LV25MG121`**
- onboard JTAG + UART debug connectivity over USB-C
- board-specific clock, reset, LED, and UART mappings still need confirmation
  from authoritative Sipeed documentation and schematics

Those inputs should drive final pin constraints, clocking decisions, reset
conditioning, and programming/deployment decisions.

---

## 5. Proposed Target Directory Layout

Create a new target-local directory:

```text
rtl/fpga/gowin_tang_primer_25k/
├── gowin_tang_primer_25k_top.sv
├── gowin_build.tcl
├── README.md
├── <gowin project / constraint collateral>
└── <optional programming collateral kept under source control when needed>
```

### 5.1 Folder Content Policy

The target directory should contain the checked-in files required to describe
and build the target:

- board wrapper RTL,
- constraints,
- any Gowin project template files the Tcl flow depends on,
- the Tcl build driver,
- and target-local documentation.

Generated outputs should **not** be committed there. All generated artifacts
should continue to land under:

- `rtl/fpga/build/gowin_tang_primer_25k/`

This matches the existing repository convention for both open-source and
vendor-tool FPGA targets.

---

## 6. Target Architecture

### 6.1 Preferred Top-Level Structure

The preferred Tang architecture is the same thin-wrapper pattern already used by
the existing board targets:

1. `gowin_tang_primer_25k_top.sv` accepts raw board ports
2. it performs board-local clock/reset handling
3. it maps LEDs and the UART pins
4. it instantiates `rtl/fpga/common/fpga_common_top.sv`

That keeps the Tang-specific code focused on board adaptation and preserves the
shared system integration model.

### 6.2 Clock / Reset Responsibilities

The Tang wrapper should own board-local clock and reset adaptation, including:

- selecting the authoritative system clock input,
- instantiating any required Gowin clock-generation primitive or PLL wrapper if
  the board clock must be transformed,
- converting reset/button behavior into the repository's internal synchronous
  active-high `rst` convention as close to the board boundary as practical,
- and handling any vendor-lock / clock-stable gating needed before releasing the
  shared FPGA integration.

Even if the board-level reset input arrives active-low, the Tang wrapper should
convert it near the boundary and preserve the repository's normal internal
synchronous active-high reset style for shared RTL.

### 6.3 UART / Host Responsibilities

If board documentation confirms a practical fabric-facing UART connection to the
onboard debugger bridge, the wrapper should connect that UART directly into
`fpga_common_top`.

That is the desired outcome because it preserves:

- the existing host bus transport,
- the host-backed external-memory model,
- and the normal FPGA user workflow already used by the other board targets.

### 6.4 Fallback Scope if UART Reuse Fails

If the UART path is not actually usable from the fabric, the initial Tang target
should be explicitly scoped more narrowly:

- either as a board bring-up target with limited runtime parity,
- or as a Tang-specific integration wrapper below the Makefile target seam.

That contingency should remain explicit in both the plan and target-local docs
so that the support level is accurately described.

---

## 7. Build-System Integration Plan

### 7.1 New Makefile Target Block

Add a new target block to `rtl/fpga/Makefile`:

```make
ifeq ($(TARGET),gowin_tang_primer_25k)
...
endif
```

This block should define the same style of target-local metadata used by the
existing vendor-tool targets, including:

- `FPGA_DIR`
- `TOP_MODULE`
- `DEFAULT_PROGRAM_MODE`
- a Tang-specific Tcl path variable such as `GOWIN_TCL`
- any vendor executable variable(s)
- `OPENFPGALOADER_BOARD := tangprimer25k`
- `OUTPUT_EXT`
- and, if practical, `PROGRAM_CMD`

### 7.2 Preserve Existing Makefile Conventions

Tang should fit the current Makefile model rather than inventing a separate
build interface. The plan should keep Tang aligned with the standard variables
and output paths already used across targets:

- `BUILD_DIR := build/$(TARGET)`
- `BIN := $(BUILD_DIR)/riscv_fpga.$(OUTPUT_EXT)`
- `TIMING := $(BUILD_DIR)/riscv_fpga_timing.rpt`
- `TIMING_SUMMARY := $(BUILD_DIR)/riscv_fpga_timing_summary.rpt`
- `UTILIZATION := $(BUILD_DIR)/riscv_fpga_utilization.rpt`

The `help`, unsupported-target error message, `check-tools`, and target
selection documentation should also be updated to include Tang.

### 7.3 Tool Checks

`make check-tools` should gain a Tang-specific branch that verifies the required
Gowin command-line tools are in `PATH` before attempting a build.

Because this is a vendor-tool target, the check should mirror the existing
Vivado and Quartus pattern rather than the open-source Yosys/nextpnr flow.

---

## 8. Tcl-Driven Gowin Flow Plan

### 8.1 Entry Point

The initial implementation should use a target-local non-interactive Tcl script,
for example:

- `rtl/fpga/gowin_tang_primer_25k/gowin_build.tcl`

The Makefile should invoke that script in batch mode from
`make TARGET=gowin_tang_primer_25k`.

### 8.2 Required Tcl Responsibilities

The Tcl script should own the full vendor build orchestration:

1. receive the normalized build directory and target metadata from the Makefile
2. load target-local project/constraint collateral
3. add all shared RTL sources from `rtl/common/`
4. add `rtl/fpga/common/fpga_common_top.sv`
5. add `gowin_tang_primer_25k_top.sv`
6. run synthesis, place-and-route, and bitstream generation non-interactively
7. emit or copy authoritative outputs into
   `rtl/fpga/build/gowin_tang_primer_25k/`

### 8.3 Accepted Tcl Implementation Styles

The repo already contains two acceptable vendor-flow patterns:

1. **Vivado-style direct output**
   - the Tcl script reads explicit RTL/constraint inputs
   - runs the flow in the normalized build directory
   - and writes reports/bitstreams there directly

2. **Quartus-style project-copy flow**
   - the Tcl script works from checked-in vendor project collateral
   - compiles in a vendor-project context
   - and copies authoritative outputs back into the normalized build directory

Tang may use either pattern, depending on which Gowin CLI/project model is more
stable and scriptable. The important requirement is that the final artifact
surface exposed to the repository stays normalized.

### 8.4 Artifact Conventions

At minimum, the Tang flow should normalize these outputs when available:

- `build/gowin_tang_primer_25k/riscv_fpga.<bitstream-ext>`
- `build/gowin_tang_primer_25k/riscv_fpga_timing.rpt`
- `build/gowin_tang_primer_25k/riscv_fpga_utilization.rpt`

If the Gowin tools provide a stable summary report, also normalize:

- `build/gowin_tang_primer_25k/riscv_fpga_timing_summary.rpt`

The exact bitstream extension should remain an implementation detail until the
vendor tools and programming workflow are confirmed. The plan should therefore
avoid hardcoding the final `OUTPUT_EXT` until that detail is verified.

---

## 9. Programming and Deployment Plan

### 9.1 Initial Expectation

The first implementation should document local programming/deployment steps, but
it does not need to guarantee the same level of polished programming automation
as the mature open-source targets on day one.

### 9.2 Makefile Programming Integration

The Tang target should use **openFPGALoader** for programming, with the board
parameter set to:

- `-b tangprimer25k`

The Makefile integration should therefore follow the repository's existing
openFPGALoader pattern and wire programming through:

- `DEFAULT_PROGRAM_MODE`
- `PROGRAM_MODE`
- `OPENFPGALOADER_BOARD`
- `PROGRAM_CMD`

The intended programming command shape is:

```bash
openFPGALoader -b tangprimer25k <program-mode-flag> build/gowin_tang_primer_25k/riscv_fpga.<bitstream-ext>
```

The target-local README should also document the equivalent manual programming
command using `openFPGALoader -b tangprimer25k`.

### 9.3 SRAM vs Flash Policy

The plan should explicitly verify:

- what artifact format is used for SRAM programming,
- what artifact format is used for flash/persistent programming,
- and which mode should become the default for `DEFAULT_PROGRAM_MODE`.

Those details should be based on confirmed Gowin tooling behavior rather than
assumptions borrowed from the current Lattice or Quartus targets, but the
programmer frontend should be treated as settled: use `openFPGALoader` with
board parameter `tangprimer25k`.

---

## 10. Documentation Plan

### 10.1 Target-Local Documentation

Add `rtl/fpga/gowin_tang_primer_25k/README.md` documenting:

- required vendor tools,
- expected environment setup,
- build commands,
- artifact locations,
- programming workflow,
- and any current functional limitations.

### 10.2 Shared FPGA Docs

Update `rtl/fpga/README.md` so Tang appears in:

- the supported target list,
- the quick-start build examples,
- the toolchain notes for vendor-tool flows,
- and any target-specific support caveats.

### 10.3 Current Research / Planning Lifecycle

Once this implementation plan exists, the source research document should be
removed from `docs/research/` in keeping with the documented research-to-plan
lifecycle.

---

## 11. Validation Plan

Because Tang is a vendor-tool target, validation should be **local-only** for
the initial implementation.

### 11.1 Required Build Validation

The implementation should document and support:

```bash
cd rtl/fpga
make TARGET=gowin_tang_primer_25k check-tools
make TARGET=gowin_tang_primer_25k
make TARGET=gowin_tang_primer_25k timing
make TARGET=gowin_tang_primer_25k utilization
```

### 11.2 Initial Functional Validation Scope

The initial validation target should be a successful bitstream build plus basic
hardware bring-up for:

1. clocking
2. reset behavior
3. LED mapping
4. UART electrical/path validation

Only after UART communication is verified should Tang be treated as a candidate
for full runtime parity with the existing board targets.

### 11.3 Explicit CI Policy

The initial Tang target should **not** be added to default CI.

The plan should state this plainly:

- Tang support is local/vendor-tool-only in the initial revision
- default GitHub Actions CI remains limited to the existing open-source
  synthesis targets
- any later CI consideration is follow-on work after the flow is stable and
  reproducible

---

## 12. Phased Implementation Plan

### Phase 1 - Confirm Board and Tool Facts

Before writing constraints or automating the Makefile path, confirm:

1. authoritative board clock source and frequency
2. reset button polarity and whether extra debounce is required
3. LED pin mapping
4. FPGA-fabric UART pin mapping to the onboard debugger/UART bridge
5. vendor-tool command-line entry point(s) suitable for Tcl automation
6. bitstream/programming artifact types for SRAM and flash workflows

### Phase 2 - Add the Smallest Repo-Native Vertical Slice

Implement the smallest useful Tang target:

1. add `rtl/fpga/gowin_tang_primer_25k/`
2. add the thin board wrapper
3. add target-local constraints/project collateral
4. add the Tcl build driver
5. add a new `TARGET=gowin_tang_primer_25k` Makefile block
6. emit normalized artifacts into `build/gowin_tang_primer_25k/`

The goal of this phase is a successful local vendor-tool bitstream build.

### Phase 3 - Validate Runtime Reuse

Once the build works:

1. verify UART host communication on hardware
2. verify host-bus transactions
3. verify CPU boot/reset behavior
4. verify SRAM/peripheral behavior
5. verify host-backed external-memory behavior

This phase determines whether Tang reaches full board-target parity or remains a
more limited bring-up target.

### Phase 4 - Follow-On Polish

After the target is stable locally:

1. update shared FPGA documentation
2. decide whether Makefile `program` should be fully automated
3. evaluate whether Gowin report formats are stable enough for stats support
4. consider whether any future CI or containerized vendor-flow strategy is worth
   pursuing

---

## 13. Completion Criteria

Tang Primer 25K should not be considered a supported initial target until all of
the following are true:

- `make TARGET=gowin_tang_primer_25k` works locally with the required vendor
  tools installed
- the target uses a checked-in Tcl build entry point
- the wrapper cleanly converts board-local clock/reset/UART signals into the
  repository's internal conventions
- build outputs land under `rtl/fpga/build/gowin_tang_primer_25k/`
- the target-local README documents local prerequisites and build/program usage
- the support level is documented accurately as either full shared-runtime
  support or narrower bring-up support

Anything short of that should be described more narrowly as **bring-up** rather
than full supported-board parity.

---

## 14. Risks and Open Questions

The following should remain explicit until verified:

1. **UART reuse**  
   Can the FPGA fabric actually access a usable UART pair tied to the onboard
   debugger/USB path?

2. **Clocking model**  
   What is the authoritative board clock, and does the target require a PLL or
   other vendor-specific clock conditioning?

3. **Reset behavior**  
   What is the board reset polarity, and is additional debounce/synchronization
   needed beyond the normal wrapper conventions?

4. **Constraint/project collateral**  
   What exact Gowin project and constraint files are required for a stable
   Tcl-driven build?

5. **Programming UX**  
   Is there a reliable CLI programming path worth wiring into `make program`, or
   should programming remain manually documented at first?

6. **Artifact/report normalization**  
   Can the Gowin flow copy timing/utilization reports into the repository's
   standard names as cleanly as the Vivado and Quartus flows?

7. **Target support level**  
   If UART reuse fails, is the initial target still valuable as a narrower
   bring-up target?

---

## 15. Bottom Line

The Tang Primer 25K should be integrated as a **new Gowin-family target** that:

- follows the existing `rtl/fpga/<target>/` structure,
- uses a **Tcl-driven vendor flow**,
- keeps artifacts normalized under `rtl/fpga/build/gowin_tang_primer_25k/`,
- stays **out of default CI** initially,
- and treats UART/runtime reuse as the key architectural gate.

That plan is the smallest path that matches the repository's current
multi-target architecture while keeping the shared CPU/SoC RTL vendor-neutral.
