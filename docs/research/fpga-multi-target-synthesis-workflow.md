# FPGA Multi-Target Synthesis Workflow: Current State and Analogue Pocket Future Work

**Research Document**  
**Context:** Current-state refresh for the repository's FPGA synthesis workflow, with future-work focus on the Analogue Pocket  
**Date:** 2026-03-16

---

## Executive Summary

This repository no longer has a single-target FPGA flow. The current `rtl/fpga/`
implementation already supports **three concrete FPGA targets**:

1. **`ice40_alchitry_cu`** - Alchitry Cu v1 (Lattice iCE40-HX8K) using the
   open-source Yosys + nextpnr-ice40 + IceStorm flow.
2. **`ecp5_icepi_zero`** - iCE Pi Zero (Lattice ECP5-25F) using the
   open-source Yosys + nextpnr-ecp5 + Project Trellis flow.
3. **`artix7_alchitry_au`** - Alchitry Au (Xilinx Artix-7 XC7A35T) using a
   **proprietary Vivado batch/Tcl flow** driven by the repository Makefile.

That means the main architectural question in this document is no longer *how to
make the repository multi-target*. It already is. The more useful questions are:

- what the current multi-target architecture looks like,
- which parts of the workflow are now stable implementation rather than research,
- and how a **future Analogue Pocket target** should fit into the same model.

The key conclusions are:

- The repository has already converged on a good multi-target structure:
  **`make TARGET=...`**, thin board wrappers, and a shared FPGA integration
  module in `rtl/fpga/common/fpga_common_top.sv`.
- The **open-source flows are currently iCE40 and ECP5**.
- The **supported proprietary flow is the Alchitry Au** via
  `artix7_alchitry_au/vivado_build.tcl`.
- The **primary future-work target is now the Analogue Pocket**, and the most
  realistic path should be assumed to look more like the existing Alchitry Au
  flow than the old open-source Cyclone V / Mistral research path.
- The Pocket should be treated as a **platform integration project**, not just a
  new synthesis backend. The likely work includes a proprietary Quartus-driven
  build, an openFPGA/APF-facing wrapper, packaging/deployment tooling, and a
  decision about how the current UART-oriented host/debug path maps onto the
  Pocket platform model.

---

## 1. Current Repository State

### 1.1 Supported FPGA Targets

The authoritative FPGA workflow is implemented in `rtl/fpga/Makefile` and
`rtl/fpga/README.md`. The repository currently supports the following targets:

| Target | Board / FPGA | Build flow | Status |
|--------|---------------|------------|--------|
| `ice40_alchitry_cu` | Alchitry Cu v1 / iCE40-HX8K-CB132 | Yosys + nextpnr-ice40 + icepack | Supported |
| `ecp5_icepi_zero` | iCE Pi Zero / ECP5-25F-CABGA256 | Yosys + nextpnr-ecp5 + ecppack | Supported |
| `artix7_alchitry_au` | Alchitry Au / XC7A35T-FTG256-1 | Vivado batch/Tcl | Supported |

This is a meaningful change from the earlier research framing that treated ECP5
and Artix-7 as future work.

### 1.2 Documentation Boundary

A second important change is documentation maturity. The repository now has
stable FPGA documentation under `docs/fpga/`, while `docs/research/` is meant
for transient investigation material. As a result:

- **implemented build procedures and analysis workflows** belong in stable docs
  such as `rtl/fpga/README.md` and `docs/fpga/`,
- while **unimplemented future-target exploration** belongs here.

This document therefore mixes a concise current-state snapshot with a focused
future-work section for the Analogue Pocket.

### 1.3 Historical Note on Artix-7

Earlier versions of this research explored an open-source Artix-7 flow based on
openXC7 / nextpnr-xilinx. That path is now historical-only for this repository.
The supported Alchitry Au implementation uses **Vivado in batch mode** and emits
bitstream, timing, and utilization reports into `rtl/fpga/build/artix7_alchitry_au/`.

---

## 2. Current Multi-Target Workflow Architecture

### 2.1 Makefile-Driven Target Selection

The repository now uses a single FPGA Makefile with a `TARGET` variable to
select the appropriate build flow, top module, constraint file, and output
format.

Representative usage:

```bash
cd rtl/fpga
make TARGET=ice40_alchitry_cu
make TARGET=ecp5_icepi_zero
make TARGET=artix7_alchitry_au
```

This is the core abstraction boundary for the implemented FPGA workflow. New
hardware targets should fit into this interface whenever possible.

### 2.2 Shared vs. Target-Specific RTL

The current design already implements the decomposition that older research was
proposing:

- **Shared FPGA integration logic:** `rtl/fpga/common/fpga_common_top.sv`
- **Board-specific wrappers:** one wrapper per target under `rtl/fpga/<target>/`
- **Vendor-neutral CPU/peripheral RTL:** `rtl/common/`

The board wrappers are intentionally thin. They handle:

1. board-level ports,
2. board-specific clock generation or direct clock usage,
3. reset synchronization / debounce / PLL-lock handling,
4. simple board-specific output mapping (for example, LED presentation).

The shared `fpga_common_top` currently owns the common FPGA-side system
integration:

- instantiation of `top.sv`,
- host-bus connectivity,
- UART transport,
- LED outputs,
- parameterization of `CLK_FREQ_HZ` and reset duration.

### 2.3 Important Current Assumption: UART-Centric Shared Integration

`fpga_common_top` currently assumes that FPGA-host communication is provided by a
UART instance configured for **1,000,000 baud**. That assumption works well for
current boards because each supported target exposes a straightforward serial
link.

This is a crucial architectural fact for future Pocket work: the Analogue Pocket
is unlikely to be a drop-in replacement for the current UART-host model. Any
Pocket integration effort must decide whether to:

- emulate the same logical host bus over a Pocket-specific bridge,
- introduce a new transport adapter layer below the existing shared logic,
- or define a Pocket-specific shared top that preserves CPU/peripheral behavior
  while changing the external transport contract.

---

## 3. Current Target Profiles

### 3.1 iCE40 - Alchitry Cu v1

**Target:** `ice40_alchitry_cu`  
**Toolchain:** Yosys + nextpnr-ice40 + icepack  
**Programming:** `openFPGALoader`  
**Clocking:** `SB_PLL40_CORE` wrapper path, 100 MHz input -> 25 MHz system clock

This remains the default target and the most lightweight reproduction path for
open-source FPGA development in the repository.

Current workflow highlights:

- Build artifacts live in `rtl/fpga/build/ice40_alchitry_cu/`
- Standardized stats are generated with:

```bash
cd rtl/fpga
make TARGET=ice40_alchitry_cu stats STATS_FORMAT=json
```

- The project maintains stable timing-analysis documentation in `docs/fpga/`
  based on fresh routed artifacts rather than hard-coded summary numbers.

### 3.2 ECP5 - iCE Pi Zero

**Target:** `ecp5_icepi_zero`  
**Toolchain:** Yosys + nextpnr-ecp5 + ecppack  
**Programming:** `openFPGALoader`  
**Clocking:** direct 50 MHz board clock, no PLL currently required

The ECP5 target is no longer a proposed extension. It is a supported,
repository-integrated build.

Current workflow highlights:

- Build artifacts live in `rtl/fpga/build/ecp5_icepi_zero/`
- The wrapper runs directly from the 50 MHz board oscillator
- CI verifies that the target still synthesizes successfully
- Timing and resource analysis now live in stable docs under `docs/fpga/`

### 3.3 Artix-7 - Alchitry Au

**Target:** `artix7_alchitry_au`  
**Toolchain:** **Vivado batch/Tcl**  
**Programming:** `openFPGALoader`  
**Clocking:** `PLLE2_ADV` + `BUFG`, 100 MHz input -> 50 MHz system clock

The Artix-7 flow is important because it establishes the repository's existing
pattern for a **supported proprietary FPGA backend**.

Current workflow highlights:

- The Makefile invokes `artix7_alchitry_au/vivado_build.tcl`
- The Tcl flow reads all RTL/XDC inputs, then runs synthesis, placement,
  optimization, routing, and report generation
- The target writes at least the following artifacts to
  `rtl/fpga/build/artix7_alchitry_au/`:
  - `riscv_fpga.bit`
  - `riscv_fpga_timing.rpt`
  - `riscv_fpga_timing_summary.rpt`
  - `riscv_fpga_utilization.rpt`

This flow is the best template for future proprietary-tool targets.

---

## 4. Reporting, Timing, and CI in the Current Flow

### 4.1 Standardized Build Artifacts

The repository now has a normalized stats workflow for supported targets:

```bash
cd rtl/fpga
make TARGET=<target> stats STATS_FORMAT=json
```

This produces normalized timing/resource summaries in `build/<target>/`, such as:

- `riscv_fpga_stats.json`
- `riscv_fpga_stats.md`

For open-source targets, the flow also prefers routed timing reports when the
installed nextpnr build supports them, with fallback to `nextpnr.log`.

### 4.2 Prefer Artifact-Driven Reporting Over Hard-Coded Numbers

One lesson from the older version of this document is that static “expected Fmax”
and resource estimates age quickly. The repository now has a better pattern:

- stable timing/resource narratives live under `docs/fpga/`
- authoritative numbers come from fresh build artifacts under `rtl/fpga/build/`
- the stats tooling provides a concise machine-readable summary for comparison

That is the model future targets should follow.

### 4.3 Current CI Coverage

CI currently verifies:

- RTL lint,
- Rust formatting/lint/tests,
- **iCE40 synthesis**, and
- **ECP5 synthesis**.

The Alchitry Au flow is not part of standard CI because it depends on Vivado.
That is already an accepted repository pattern, and it is relevant when thinking
about future proprietary-tool targets such as the Analogue Pocket.

---

## 5. Implications for Future FPGA Targets

The repository's current shape suggests a clear rule for new FPGA targets:

1. **Keep `rtl/common/` vendor-neutral**
2. **Add a thin target wrapper under `rtl/fpga/<target>/`**
3. **Integrate the build in `rtl/fpga/Makefile` via `TARGET=...`**
4. **Emit normalized artifacts into `rtl/fpga/build/<target>/`**
5. **Document long-lived operational details in `rtl/fpga/README.md` and `docs/fpga/`**

That is the lens through which the Analogue Pocket should be evaluated.

---

## 6. Analogue Pocket (Cyclone V) - Primary Future-Work Focus

### 6.1 Strategic Framing

The Analogue Pocket should now be treated as the repository's **primary future
FPGA expansion target**. The right planning assumption is **not** “fully
open-source Cyclone V flow first.” Instead, the working assumption should be:

> A future Pocket target will most likely use a **proprietary-tool-based flow**
> analogous to the current `artix7_alchitry_au` Vivado flow, with project-owned
> scripts, standardized output artifacts, and target-specific packaging.

That framing matches the practical state of Pocket development more closely than
older Mistral-centered research.

### 6.2 Why the Pocket Is Interesting for This Repository

The Analogue Pocket is compelling for several reasons:

- it provides substantially more logic/memory headroom than the iCE40 target,
- it has an active openFPGA core ecosystem,
- it is a strong fit for interactive CPU-driven applications and retro-computing
  experiments,
- and it would broaden the repository from “FPGA board bring-up” into
  “platform-integrated deployable core” territory.

In other words, the Pocket is not only a bigger FPGA. It is a different product
surface: a deployed handheld platform with its own packaging, platform
interfaces, and user-facing loading model.

### 6.3 Recommended Workflow Model

The closest existing repository precedent is the Alchitry Au flow:

- repository-managed target name,
- proprietary vendor tool in batch mode,
- checked-in project/Tcl script,
- predictable output directory under `rtl/fpga/build/<target>/`,
- Makefile integration for build/report/program/package steps.

A Pocket implementation should therefore be designed around something like:

```bash
cd rtl/fpga
make TARGET=cyclonev_analogue_pocket
```

with the Makefile delegating to a Quartus-driven non-interactive script.

### 6.4 Recommended Tooling Assumption

**Recommended default assumption:**

- **Primary implementation path:** Quartus Prime Lite / Quartus Prime Standard
  batch flow (whichever best matches Pocket/openFPGA requirements)
- **Driver mechanism:** checked-in Tcl or project-generation script under a new
  target directory such as `rtl/fpga/cyclonev_analogue_pocket/`
- **Optional secondary research path:** Mistral / nextpnr-mistral retained only
  as background investigation, not as the planned production workflow

This mirrors the repository's current Artix-7 position:

- open-source historical research may remain useful as background,
- but the supported flow should optimize for practicality, reproducibility, and
  artifact quality.

### 6.5 Recommended Build Outputs

A future Pocket target should emit predictable build artifacts into
`rtl/fpga/build/cyclonev_analogue_pocket/`.

At minimum, the build should aim to produce:

- synthesis log
- fitter / place-and-route log
- timing report
- utilization / resource report
- Quartus project report summaries
- FPGA programming image (`.sof`, `.pof`, or device-appropriate intermediate)
- Pocket/openFPGA deployment package inputs and final packaged output

If the final deployable artifact is an openFPGA-compatible packaged core, then
that packaged core should be treated as the Pocket analogue of `riscv_fpga.bit`
for review and release purposes.

### 6.6 Platform Integration Is the Real Work

This is the most important difference between the Pocket and the currently
supported Cu / iCE Pi Zero / Au boards.

The existing supported targets are **board wrappers** around a shared UART-based
integration model. The Pocket is more likely to require a **platform wrapper**
that adapts repository logic to the Analogue openFPGA / APF environment.

That likely means a future Pocket target needs more than just:

- a Cyclone V synthesis backend,
- a PLL replacement,
- and a new constraints file.

It likely also needs one or more of the following:

- an APF-facing top-level wrapper,
- a bridge between platform-defined clocks/signals and internal repository
  interfaces,
- a transport adapter for host/debug/data exchange,
- packaging metadata required by the Pocket runtime environment.

### 6.7 Clock and Reset Strategy

Current wrappers in the repository follow a consistent rule: board-specific
clock/reset behavior belongs in the target wrapper.

For the Pocket, the design should begin by answering these concrete questions:

1. **Which clock(s) are provided by the Pocket platform?**
2. **Can the repository run directly from a platform clock, or is a PLL required?**
3. **How is reset delivered by the platform, and how is it converted into the
   repository's internal synchronous active-high `rst` convention?**
4. **Does any platform clocking requirement force a change to the current
   `CLK_FREQ_HZ` assumptions used by UART, timers, or reset timing?**

The likely answer will differ from current boards because the Pocket is not just
exposing raw FPGA pins. It provides a structured platform interface.

### 6.8 Host Transport and Debug Strategy

This is the biggest architectural question after packaging.

Today, `fpga_common_top` assumes:

- UART transport,
- a host bus carried over that UART,
- board-local serial pins.

The Pocket likely wants something else. Plausible options include:

1. **Preserve the existing logical host bus** and tunnel it over a Pocket
   platform bridge or command channel.
2. **Create a Pocket-specific adapter below the existing shared integration** so
   `top.sv` and most FPGA-side logic stay unchanged.
3. **Define a Pocket-specific shared top** that keeps the CPU/peripheral behavior
   but drops the UART assumption entirely.

The preferred option should minimize divergence from the current verification and
runtime model. In practice, that probably means preserving the logical host-bus
contract if at all possible, even if the physical transport changes.

### 6.9 Constraint and Project Model

The Pocket should not be modeled mentally as “just add a `.qsf` like we add a
`.pcf`, `.lpf`, or `.xdc`.” For this target, the build probably has three
separate configuration layers:

1. **Quartus device/project configuration**
2. **platform/openFPGA integration requirements**
3. **repository-local build/package orchestration**

This is another reason a proprietary scripted flow is the right baseline. The
Makefile can remain the public entry point while the target script owns the more
complex vendor/project details.

### 6.10 Packaging and Deployment Model

Current boards can be described with a relatively simple `make program` story.
The Pocket likely cannot.

A future Pocket section of the operational documentation should explicitly define:

- what the final user-consumable artifact is,
- how it is packaged,
- how it is loaded onto the device,
- and what the repository's equivalent of “program the FPGA” means for this
  platform.

Likely candidates include:

- an SD-card-deployed openFPGA core package,
- metadata files required by the Pocket ecosystem,
- optional JTAG/developer loading for local bring-up,
- and possibly separate “developer build” versus “distribution build” outputs.

### 6.11 Reporting and Stats Integration

The repository's current stats workflow is valuable enough that a future Pocket
flow should try to join it rather than remain a one-off build.

A good target-level completion bar would be:

- Quartus-generated timing/resource reports are placed in the standard build
  directory,
- the stats tooling is extended to parse the relevant report format,
- `make TARGET=cyclonev_analogue_pocket stats STATS_FORMAT=json` works,
- and the Pocket can therefore participate in the same artifact-review workflow
  as the existing targets.

### 6.12 CI Expectations

The repository already has precedent for excluding proprietary-tool flows from
standard CI while still keeping them well-structured and reviewable.

A future Pocket target should therefore assume:

- **not part of default open-source CI**,
- local reproducibility through documented tool installation and scripted builds,
- review through committed source changes plus generated local artifact summaries,
- and standardized output/report locations even when CI cannot run the vendor
  toolchain.

This is essentially the same policy shape as the Alchitry Au flow.

### 6.13 Proposed Minimal Implementation Plan

A realistic minimal Pocket bring-up plan would look like this:

1. **Add target directory**
   - `rtl/fpga/cyclonev_analogue_pocket/`
2. **Add proprietary-tool batch script**
   - Quartus Tcl/project-generation flow
3. **Add target wrapper**
   - platform-facing top-level module
4. **Resolve platform transport strategy**
   - UART replacement, bridge, or adapter layer
5. **Add Makefile integration**
   - `TARGET=cyclonev_analogue_pocket`
6. **Define artifact/report conventions**
   - timing, utilization, logs, deployable package
7. **Extend stats tooling if feasible**
   - normalize Quartus timing/resource output
8. **Document deployment flow**
   - developer bring-up and user packaging

That is the smallest useful vertical slice that would make the target feel
native to the existing repository architecture.

### 6.14 Risks and Open Questions

The remaining unknowns are mostly integration and packaging questions rather than
RTL portability questions.

Key open questions:

- What exact openFPGA/APF interface requirements must the top-level wrapper obey?
- Is a UART-like debug/data path acceptable, or is a different Pocket-native path
  required?
- Which Quartus edition/version is the most appropriate reproducible baseline?
- What is the exact deployable artifact shape for this repository's intended
  Pocket use case?
- Can the current `fpga_common_top` be preserved through adaptation, or is a new
  shared integration layer warranted?

Those questions should drive the next phase of research.

---

## 7. Recommendations

### 7.1 What Should Be Treated as Stable Today

These items are already implemented and should be documented/maintained as
current repository behavior rather than future research:

- iCE40 synthesis flow
- ECP5 synthesis flow
- Alchitry Au Vivado flow
- Makefile `TARGET=...` orchestration
- thin-wrapper plus shared-top architecture
- standardized timing/resource artifact workflow

### 7.2 What Should Remain Research-Focused

The main remaining research topic in this area is now:

- **Analogue Pocket / Cyclone V platform integration via a proprietary scripted flow**

Historical open-source Xilinx and Cyclone V notes can still be useful for
background, but they should not drive the main repository roadmap unless tool
support or project goals change.

### 7.3 Recommended Next Research Step

The next high-value research output should not be “how to run Mistral.” Instead,
it should answer this narrower and more actionable question:

> What is the smallest repository-native `TARGET=cyclonev_analogue_pocket`
> implementation that fits the current thin-wrapper / shared-top architecture and
> produces a deployable openFPGA-compatible package through a proprietary batch
> flow?

That question is aligned with the repository's actual architecture and current
priorities.

---

## 8. Bottom Line

The repository has already solved the general multi-target build problem for the
currently supported FPGA families. The current state is:

- **Implemented open-source targets:** iCE40, ECP5
- **Implemented proprietary target:** Artix-7 Alchitry Au
- **Primary future target:** Analogue Pocket / Cyclone V

The right future-work framing for the Pocket is therefore:

- **not** “add another experimental open-source backend first,”
- but **“add a repository-native proprietary scripted target that matches the
  existing Artix-7 operational model, while solving the Pocket-specific platform
  integration and packaging problems.”**

That is the most accurate reflection of the repository's current state and the
most practical direction for future work.
