# Tang Primer 25K Platform Target Support

**Research Document**  
**Context:** Work required to add the Tang Primer 25K as a new FPGA platform target  
**Date:** 2026-03-21

---

## Executive Summary

Adding Tang Primer 25K support would be a meaningful platform expansion, but it
is **not** a copy-and-paste extension of the repository's existing ECP5 path.

The most important current fact is that **Tang Primer 25K is a Gowin-based
board**, not a Lattice ECP5 board. The official Sipeed documentation describes
it as using a **GW5A-LV25MG121** FPGA with onboard **JTAG+UART** debugging over
USB-C, plus dock-board peripherals such as buttons, PMODs, and SDRAM support:

- Sipeed wiki / board overview:  
  <https://github.com/sipeed/sipeed_wiki/blob/main/docs/hardware/en/tang/tang-primer-25k/primer-25k.md>

That changes the integration plan in three ways:

1. **new FPGA family and new vendor/toolchain assumptions**
2. **new board collateral and constraints**
3. **a decision about whether the current UART-centric shared host model can be
   reused directly**

The good news is that the repository already has the right high-level
architecture for a new target:

- target-local wrapper under `rtl/fpga/<target>/`
- Makefile entry via `TARGET=...`
- shared system integration in `rtl/fpga/common/fpga_common_top.sv`
- normalized outputs under `rtl/fpga/build/<target>/`

The main work is therefore **target bring-up and flow integration**, not a
top-to-bottom redesign of the CPU RTL.

---

## 1. Why Tang Primer 25K Is Not a Drop-In Variant of Existing Targets

The current repository target families are:

- Lattice iCE40 (`ice40_alchitry_cu`)
- Lattice ECP5 (`ecp5_icepi_zero`)
- Xilinx Artix-7 (`artix7_alchitry_au`)
- Intel Cyclone V (`cyclonev_analogue_pocket`)

Tang Primer 25K would introduce a **fifth family: Gowin**.

That means the target would not naturally inherit:

- the current `synth_ecp5` / `nextpnr-ecp5` / `ecppack` flow,
- the current Vivado batch flow,
- or the current Quartus batch flow.

Instead, Tang support requires a new decision about the synthesis /
place-and-route / bitstream / programming toolchain.

---

## 2. Board Facts That Matter to This Repository

The following board-level facts are relevant before implementation:

- **FPGA device:** Gowin `GW5A-LV25MG121`
- **Onboard debugger:** JTAG + UART over USB-C
- **Dock-board user I/O:** buttons, PMOD connectors, and dock-level expansion
  connectivity
- **Flash present on platform:** documented by Sipeed, so persistent programming
  is likely possible

Those facts suggest Tang could plausibly fit the repository's existing FPGA user
experience better than Analogue Pocket in one important respect: it appears to
offer a practical onboard UART/debug path rather than a platform-managed
transport abstraction.

However, the exact implementation still depends on board-level verification of:

1. system clock source and frequency
2. reset button polarity / debounce needs
3. LED pin mapping
4. UART pin mapping between FPGA fabric and onboard debugger
5. programming interface and file format for SRAM vs flash workflows

The Sipeed board documentation and schematics are the authoritative source for
those final details and should be consulted before writing constraints.

---

## 3. Repository Areas That Would Need New Work

### 3.1 New target directory

The project should add a new target-local directory, likely named:

```text
rtl/fpga/gowin_tang_primer_25k/
```

That name is preferable to an `ecp5_*` name because it preserves the real FPGA
family and avoids implying that the board uses the existing ECP5 flow.

Expected contents:

- `gowin_tang_primer_25k_top.sv`
- target-specific constraint files
- target-specific batch/build script(s)
- target-local README
- any programming collateral or vendor project templates

### 3.2 Board wrapper module

The wrapper should remain thin, consistent with existing board targets:

1. accept raw board clocks / resets / LEDs / UART pins
2. perform board-local clock/reset conditioning
3. map board LEDs or buttons
4. instantiate `fpga_common_top`

If Tang can expose a usable UART pair to the FPGA fabric, the wrapper may be
able to reuse the current shared FPGA integration model almost unchanged.

### 3.3 Makefile integration

`rtl/fpga/Makefile` would need a new `ifeq ($(TARGET),...)` block with:

- `FPGA_DIR`
- `TOP_MODULE`
- `DEFAULT_PROGRAM_MODE`
- toolchain commands
- output extension
- programming command

The Makefile help text, unsupported-target error, tool checks, and any packaging
or stats conditionals would also need updates.

### 3.4 Stats support

`rtl/fpga/fpga_design_stats.py` currently supports:

- `ice40_alchitry_cu`
- `ecp5_icepi_zero`
- `artix7_alchitry_au`

It does not currently support either the Pocket target or any Gowin target.
Tang support would therefore need either:

- a new parser path for Gowin timing/utilization reports, or
- an explicit temporary docs note that stats are not yet normalized for Tang

### 3.5 Documentation

At minimum, the following docs would need updates:

- `rtl/fpga/README.md`
- `docs/research/fpga-multi-target-synthesis-workflow.md`
- target-local Tang documentation under `rtl/fpga/gowin_tang_primer_25k/`
- optionally new stable docs under `docs/fpga/` once timing/utilization analysis
  becomes repeatable

### 3.6 CI policy

A new Tang target would require an explicit CI choice:

- **Option A:** do not add Tang synthesis to default CI because it depends on
  vendor tools
- **Option B:** add CI only if an acceptable open-source or containerized flow
  becomes reliable enough

Given the current repository pattern, Option A is the safer initial assumption.

---

## 4. Most Important Technical Question: Can Tang Reuse the UART Host Model?

The current shared FPGA runtime model assumes:

- a board-local UART interface
- `fpga_common_top`
- the host bus serialized over that UART
- `rtl/common/top.sv` forwarding external-memory transactions through the host
  byte stream

This is the biggest architectural question for Tang support.

### Best-case outcome

If the Tang Primer 25K exposes a straightforward UART connection between the
FPGA fabric and the onboard debugger/USB bridge, then Tang can likely follow the
same model as:

- `ice40_alchitry_cu`
- `ecp5_icepi_zero`
- `artix7_alchitry_au`

In that case, Tang is mostly a **new board wrapper + new build flow** task.

### Worse-case outcome

If the onboard debugger/UART path is not practically usable from the fabric, or
if it requires a non-standard bridge, then Tang support becomes more like a
platform integration task:

- transport adaptation below `fpga_common_top`, or
- a Tang-specific top-level integration wrapper, or
- a bring-up-only target until host transport is solved

The official docs strongly suggest JTAG+UART support is available, which makes
the best-case outcome plausible, but that still needs to be verified against the
board schematic and example projects before implementation.

---

## 5. Toolchain Options

### 5.1 Recommended baseline: Gowin vendor flow

The safest initial assumption is that Tang support should be built around a
scripted **Gowin vendor-tool flow**, analogous in repository structure to:

- Vivado for `artix7_alchitry_au`
- Quartus for `cyclonev_analogue_pocket`

That would likely mean:

- target-local project collateral under `rtl/fpga/gowin_tang_primer_25k/`
- non-interactive batch build command(s)
- standardized outputs copied into `rtl/fpga/build/gowin_tang_primer_25k/`
- documented local tool prerequisites rather than standard CI coverage

### 5.2 Secondary research path: open-source Gowin flow

An open-source Gowin path may be worth evaluating separately, but it should be
treated as background research rather than the default plan unless it can meet
all of the following:

- reproducible synthesis and place-and-route
- usable timing/resource reports
- practical bitstream generation
- practical programmer support
- enough maturity for repository CI expectations

Until that is demonstrated, the repo should assume a vendor-tool baseline.

---

## 6. Concrete Work Breakdown

### Phase 1 - Board bring-up research

Before writing RTL or build files, confirm:

1. exact system clock input and required internal operating clock
2. reset input polarity and whether a debouncer is needed
3. LED and button pin assignments
4. UART fabric connectivity and voltage domain expectations
5. programming flow for SRAM and flash
6. which vendor tools and command-line interfaces are stable enough to script

### Phase 2 - Minimal target integration

Add the smallest repo-native vertical slice:

1. target directory
2. thin board wrapper
3. scripted batch build flow
4. Makefile target entry
5. documented output artifact locations
6. local build instructions in target README

This phase should aim for a successful bitstream build and basic LED / reset /
UART bring-up.

### Phase 3 - Runtime parity validation

Once the target builds:

1. verify UART host communication
2. verify host-bus transactions
3. verify CPU boot / reset behavior
4. verify SRAM/peripheral access
5. verify host-backed external-memory behavior

This is the phase that determines whether Tang becomes a full board target or
remains a limited bring-up target.

### Phase 4 - Repository polish

After runtime parity is established:

1. add stats parsing if practical
2. update main FPGA docs
3. decide whether any CI or artifact cache integration is warranted
4. add stable timing/resource summaries under `docs/fpga/` if the board becomes
   a maintained target

---

## 7. Suggested Completion Bar for a “Supported” Tang Target

Tang Primer 25K should not be considered fully supported until all of the
following are true:

- `make TARGET=gowin_tang_primer_25k` works from `rtl/fpga/`
- build outputs land under `rtl/fpga/build/gowin_tang_primer_25k/`
- the wrapper cleanly converts board-local clock/reset/UART signals into the
  repo's internal conventions
- the target can load the CPU design and exercise basic peripherals
- the host/UART transport is functional enough to preserve the normal repo FPGA
  workflow
- programming/deployment steps are documented

Anything short of that should be described more narrowly as **bring-up**.

---

## 8. Recommended Initial Plan

The smallest practical plan for Tang support is:

1. **Confirm board electrical facts from Sipeed schematics and examples**
2. **Decide the toolchain baseline**
   - recommended default: scripted Gowin vendor flow
3. **Prototype a thin Tang board wrapper**
4. **Verify that UART can connect cleanly into `fpga_common_top`**
5. **Add a Makefile target and local build instructions**
6. **Treat stats and CI as follow-on work unless the toolchain story is already
   solid**

That plan aligns with how this repository already absorbs new FPGA targets while
keeping the CPU and shared SoC RTL vendor-neutral.

---

## 9. Bottom Line

Tang Primer 25K support looks feasible, but the project should approach it as:

- a **new Gowin-family target**,
- likely a **vendor-tool-first integration**,
- and a **board support project whose success depends heavily on UART/runtime
  reuse**.

If the board's onboard UART/debug path is easy to wire into the existing shared
host model, Tang could become a strong repo-native board target. If not, it
should first be scoped as a bring-up target until the transport story is solved.
