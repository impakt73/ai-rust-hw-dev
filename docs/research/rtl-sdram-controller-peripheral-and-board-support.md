# RTL SDRAM Controller Peripheral and Board Support Research Report

**Research Document**  
**Context:** Investigate adding a new RTL-based SDRAM controller peripheral and enabling it on every checked-in FPGA target that exposes SDRAM-class hardware  
**Date:** 2026-03-24

---

## Executive Summary

The repository already has a clean shared FPGA architecture for **RTL peripherals** on
the lower half of the address space and a separate **host-routed / Rust-owned** path
for DRAM and other upper-half devices. Today, main memory at
`0x8000_0000 - 0x8FFF_FFFF` is explicitly implemented as a **Rust** device rather than
an RTL peripheral (`docs/memory-map.md`, `riscv_shared/src/bus.rs`,
`bus-shared/src/bus.rs`, `bus-shared/src/dram.rs`).

A new RTL SDRAM controller therefore is not just “one more peripheral.” It is a
cross-cutting architecture change that touches:

1. the **CPU request split** in `rtl/common/io/host_bus_mux.sv`,
2. the **RTL slave map** in `rtl/common/top.sv`,
3. the **registered RTL peripheral bus** in `rtl/common/memory/registered_bus.sv`,
4. the **software-visible memory map** in `docs/memory-map.md` and
   `riscv_shared/src/bus.rs`, and
5. any **board-specific external-memory pin wrappers** that expose real SDRAM signals.

From the checked-in FPGA targets, **Analogue Pocket (`TARGET=cyclonev_analogue_pocket`) is the only board target with explicit external SDRAM hardware exposed in-repo**. Its
platform-facing files export `dram_*` signals and Quartus pin assignments, but the
current Pocket integration still ties those signals off instead of driving them from
repository RTL (`rtl/fpga/cyclonev_analogue_pocket/src/fpga/apf/apf_top.v`,
`rtl/fpga/cyclonev_analogue_pocket/src/fpga/core/core_top.v`,
`rtl/fpga/cyclonev_analogue_pocket/src/fpga/ap_core.qsf`). The other checked-in FPGA
targets continue to use the shared UART-backed host-memory model and do not expose
checked-in SDRAM/DDR/PSRAM interfaces in their target wrappers.

The practical conclusion is:

- the repository can support an RTL SDRAM controller **generically** at the shared SoC
  level,
- but **Pocket is the only immediately actionable board-specific hardware target** for
  native SDRAM enablement based on repository evidence,
- and preserving the current DRAM address range at `0x8000_0000` is the most
  software-compatible migration path even though it requires changes to the current
  top-bit routing rule.

---

## 1. Current Repository Architecture

### 1.1 Shared FPGA structure

The shared FPGA model is:

```text
board wrapper -> rtl/fpga/common/fpga_common_top.sv -> rtl/common/top.sv
```

This pattern is visible in the ordinary board wrappers:

- `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv`
- `rtl/fpga/ecp5_icepi_zero/ecp5_icepi_zero_top.sv`
- `rtl/fpga/artix7_alchitry_au/artix7_alchitry_au_top.sv`
- `rtl/fpga/gowin_tang_primer_25k/gowin_tang_primer_25k_top.sv`

Those wrappers are intentionally thin. They mainly handle board clock/reset details
and then instantiate `fpga_common_top`, which instantiates `rtl/common/top.sv` plus a
1,000,000-baud UART for the host-bus link (`rtl/fpga/common/fpga_common_top.sv`).

### 1.2 Shared CPU memory split

The existing SoC split is simple and very important:

- `rtl/common/io/host_bus_mux.sv` routes `addr[31] == 0` to the **RTL/system-bus** path.
- The same module routes `addr[31] == 1` to the **host bus / external memory** path.

That routing rule matches the documentation and Rust constants:

- `docs/memory-map.md` says RTL peripherals live below `0x8000_0000` and DRAM lives in
  the upper-half Rust peripheral space.
- `riscv_shared/src/bus.rs` defines `RTL_PERIPH_LIMIT = 0x8000_0000`,
  `RUST_PERIPH_BASE = 0x8000_0000`, `DRAM_BASE = 0x8000_0000`, and
  `DRAM_END = 0x8FFF_FFFF`.

### 1.3 Shared RTL peripheral bus

Within `rtl/common/top.sv`, lower-half accesses go through `registered_bus`.

The current four registered RTL slaves are hard-wired in `rtl/common/top.sv`:

- `0x2000_0000` system controller
- `0x5000_0000` LED peripheral
- `0x6000_0000` clock peripheral
- `0x7000_0000` SRAM peripheral

`rtl/common/top.sv` instantiates `registered_bus` with `NUM_SLAVES(4)` and wires those
four windows into the shared RTL system.

`rtl/common/memory/registered_bus.sv` decodes slave selection by comparing the top
nibble (`addr[31:28]`) against each slave base address when the slave entry is enabled.
This means the current shared RTL fabric is already designed for adding another memory-
mapped slave, but it is currently limited to the lower-half address space because the
CPU-side mux diverts all upper-half traffic away before `registered_bus` ever sees it.

### 1.4 Current DRAM ownership

The current DRAM implementation is host-owned / Rust-owned:

- `docs/memory-map.md` documents DRAM as a **Rust** device.
- `bus-shared/src/bus.rs` pre-registers a `Dram` device at `DRAM_BASE` inside
  `SystemBus`.
- `bus-shared/src/dram.rs` implements that device by translating device-relative
  offsets into accesses against host-managed memory.

On FPGA targets, CPU accesses to that region are serialized by
`rtl/common/io/host_bus_interface.sv` and serviced on the host side rather than by
native board memory logic (`rtl/common/top.sv`, `rtl/fpga/README.md`).

---

## 2. Existing Board Inventory: Which Targets Have SDRAM-Class Hardware?

The authoritative target list is in `rtl/fpga/Makefile` and mirrored in
`rtl/fpga/README.md`. The checked-in targets are:

- `ice40_alchitry_cu`
- `ecp5_icepi_zero`
- `artix7_alchitry_au`
- `cyclonev_analogue_pocket`
- `gowin_tang_primer_25k`

### 2.1 Targets without checked-in SDRAM evidence

The following wrappers only show the shared UART-backed integration path and do not
contain checked-in SDRAM/DDR/PSRAM interfaces in the repository target wrapper files:

- `rtl/fpga/ice40_alchitry_cu/ice40_alchitry_cu_top.sv`
- `rtl/fpga/ecp5_icepi_zero/ecp5_icepi_zero_top.sv`
- `rtl/fpga/artix7_alchitry_au/artix7_alchitry_au_top.sv`
- `rtl/fpga/gowin_tang_primer_25k/gowin_tang_primer_25k_top.sv`

The Cu wrapper is especially explicit: its header comment states that the **host
computer handles external memory (DRAM) accesses**.

This does **not** prove those physical boards can never support external memory in some
other future variant, but it does mean the checked-in repository has no concrete
board-specific SDRAM enablement seam for them today.

### 2.2 Target with explicit SDRAM evidence: Analogue Pocket

Analogue Pocket is different.

The platform-facing Pocket top exposes multiple external-memory classes:

- cellular PSRAM (`cram0_*`, `cram1_*`)
- SDRAM (`dram_*`)
- SRAM (`sram_*`)

This is visible directly in:

- `rtl/fpga/cyclonev_analogue_pocket/src/fpga/apf/apf_top.v`
- `rtl/fpga/cyclonev_analogue_pocket/src/fpga/core/core_top.v`

The corresponding pin assignments exist in:

- `rtl/fpga/cyclonev_analogue_pocket/src/fpga/ap_core.qsf`

So, from repository evidence, **Pocket is the only currently supported FPGA target with
checked-in native SDRAM hardware exposure**.

### 2.3 Important current limitation on Pocket

Although Pocket exposes the hardware, it is not used yet.

`rtl/fpga/cyclonev_analogue_pocket/src/fpga/core/core_top.v` explicitly ties off:

- `cram0_*`
- `cram1_*`
- `dram_*`
- `sram_*`

The checked-in Pocket README also states that the current target reuses the shared
UART-backed host path rather than a native memory controller
(`rtl/fpga/cyclonev_analogue_pocket/README.md`).

So Pocket is the right first board for native SDRAM support, but it still requires real
integration work at the board seam.

---

## 3. What an RTL SDRAM Controller Must Change

### 3.1 Shared SoC integration points

A repository-native SDRAM controller should integrate at the same architectural level as
other RTL peripherals, which makes the following files the primary shared seam:

- `rtl/common/top.sv`
- `rtl/common/memory/registered_bus.sv`
- `rtl/common/io/host_bus_mux.sv`
- `docs/memory-map.md`
- `riscv_shared/src/bus.rs`

The best existing design reference for a memory-like RTL slave is:

- `rtl/common/peripherals/sram_peripheral.sv`

That module already demonstrates the project’s A-channel / D-channel handshake style,
registered request handling, and memory-backed slave behavior.

### 3.2 Why the current top-bit split is the key blocker

If the new SDRAM controller is intended to replace the current DRAM region at
`0x8000_0000 - 0x8FFF_FFFF`, then the present `host_bus_mux` rule cannot remain as-is.
Today, any address with bit 31 set is forced to the host path before the RTL fabric sees
it.

That leaves three architectural options:

#### Option A: Keep the mux unchanged and place SDRAM in lower-half RTL space

Example: give the SDRAM controller a new lower-half window such as `0x1000_0000` or
`0x3000_0000`.

**Pros**
- smallest RTL delta in `host_bus_mux`
- fits the current `registered_bus` assumptions naturally

**Cons**
- breaks current software/tooling assumptions that DRAM lives at `0x8000_0000`
- requires linker/runtime/test changes
- weak migration story because “main memory” moves

#### Option B: Preserve DRAM at `0x8000_0000` and add an exception route

Update `host_bus_mux` so the DRAM range is treated as **local RTL SDRAM** while the rest
of the upper-half address space continues to route to the host/Rust path.

**Pros**
- preserves the current software-visible DRAM base address
- minimizes disruption to tests and runtimes already using `DRAM_BASE`
- gives the cleanest long-term architectural story: DRAM becomes native while other
  host-side services stay host-side

**Cons**
- requires special-case routing in the mux
- slightly weakens the current very simple “top bit decides everything” rule

#### Option C: Replace the current top-bit split with explicit address-range decode

Generalize `host_bus_mux` to route based on explicit windows instead of only `addr[31]`.

**Pros**
- most scalable long-term architecture
- clean way to support mixed local and remote windows anywhere in the address map
- future-proofs the design for more native external-memory controllers

**Cons**
- larger architectural change than needed for a first SDRAM migration
- more regression surface than Option B

### 3.3 Recommended address-map strategy

**Recommendation:** use **Option B** first.

Preserve `DRAM_BASE = 0x8000_0000` while routing the DRAM window to a new RTL SDRAM
controller. That yields the best compatibility with the current simulator/runtime model
while keeping the shared change set bounded.

Once that works, the project can later decide whether to generalize the mux into a more
fully range-based router.

---

## 4. Recommended RTL Controller Architecture

This section focuses on the controller shape that best matches the repository’s existing
bus style and board abstractions.

### 4.1 Front-end contract

The controller should present the same bus contract used by current RTL peripherals:

- request A-channel: `mem_a_*`
- response D-channel: `mem_d_*`
- ready/valid decoupling
- byte/halfword/word access sizes

This makes it compatible with the existing `registered_bus` and keeps CPU integration
consistent with `sram_peripheral.sv`.

### 4.2 Internal controller blocks

A practical first controller should separate the following functions:

1. **Bus front-end / request capture**
   - translate A-channel requests into SDRAM operations
   - enforce alignment, burst policy, and subword mask rules
2. **Initialization FSM**
   - power-up wait
   - precharge-all
   - auto-refresh sequence
   - mode register load
3. **Refresh scheduler**
   - periodic refresh request generation
   - arbitration against CPU traffic
4. **Command sequencer**
   - ACTIVE / READ / WRITE / PRECHARGE / REFRESH issuance
   - timing rule enforcement between commands
5. **Row/bank tracking**
   - open-row bookkeeping per bank
   - row-hit vs row-miss handling
6. **Read datapath**
   - CAS-latency tracking
   - DQ capture
   - subword extraction and response timing
7. **Write datapath**
   - DQ drive control
   - DQM byte-mask generation
   - write-to-precharge sequencing
8. **Board I/O wrapper / PHY-lite layer**
   - connect controller-side commands/data to board pins
   - own bidirectional `dram_dq` handling and clock forwarding

### 4.3 Keep the shared peripheral generic; isolate board timing details

The shared controller logic should live under `rtl/common/` and remain vendor-neutral.
Board-specific signal adaptation should live in the FPGA target wrapper, especially for:

- SDRAM clock output behavior
- I/O cell/tristate mapping
- any platform-specific reset or clock conditioning

That matches the existing repo decomposition where `rtl/common/` contains vendor-neutral
logic and `rtl/fpga/<target>/` contains board adaptation.

### 4.4 Suggested file split

A good first decomposition would be:

- `rtl/common/peripherals/sdram_controller_peripheral.sv`
  - A/D bus slave front-end + command engine + timing state
- `rtl/common/peripherals/sdram_phy_if.sv` (optional)
  - vendor-neutral outward-facing SDRAM signal pack / DQ direction handling
- board wrapper wiring inside the relevant `rtl/fpga/<target>/...` file(s)

The key idea is that the shared controller should look like a **memory peripheral** from
`top.sv`, while the target wrapper owns physical pin exposure.

---

## 5. Board Support Plan

### 5.1 Shared SoC work required for every build target

Even if only one board can immediately use native SDRAM pins, every target that builds
`rtl/common/top.sv` needs the shared architectural updates:

1. add the new SDRAM slave interface signals in `rtl/common/top.sv`
2. increase `registered_bus` slave count and add an SDRAM address window
3. update the host/local routing rule in `rtl/common/io/host_bus_mux.sv`
4. update the documented/canonical memory-map definitions
5. decide how simulation and non-SDRAM FPGA targets behave when the native SDRAM path is
   unavailable

### 5.2 Native board enablement: Analogue Pocket

For Pocket, the controller can be connected to real hardware through:

- `rtl/fpga/cyclonev_analogue_pocket/src/fpga/core/core_top.v`
- `rtl/fpga/cyclonev_analogue_pocket/src/fpga/apf/apf_top.v`
- `rtl/fpga/cyclonev_analogue_pocket/src/fpga/ap_core.qsf`

The implementation work is likely:

1. stop tying off `dram_*` in `core_top.v`
2. instantiate a repository-owned Pocket-side wrapper that bridges the shared SDRAM
   controller to those pins
3. preserve the current Pocket link-port / UART path for debug, boot, or fallback
4. verify that the chosen controller timing works at the Pocket clock domain used by the
   repository wrapper (`cyclonev_analogue_pocket_top.sv` currently drives the shared
   repo logic from 74.25 MHz)

### 5.3 Non-native targets without checked-in SDRAM hardware

For Cu, iCE Pi Zero, Au, and Tang Primer 25K, the repository currently has no checked-in
native SDRAM seam.

That suggests two realistic policies:

#### Policy 1: Compile the shared controller, but disable native SDRAM on those targets

- keep the controller present in shared RTL
- use parameters or build-time wiring so those boards still route DRAM through the
  existing host-bus path
- native SDRAM remains enabled only on Pocket initially

This is the lowest-risk rollout.

#### Policy 2: Make the shared RTL always expect native SDRAM

This would force every target to provide a board-specific external-memory adaptation
layer immediately.

That is **not** well aligned with current repository structure, because the non-Pocket
wrappers do not expose such interfaces today.

### 5.4 Recommended board rollout

**Recommendation:**

1. implement the shared SDRAM controller architecture once,
2. enable **native SDRAM only on Pocket** in the first hardware-backed phase,
3. keep existing host-routed DRAM behavior as the fallback on other targets until those
   targets gain real checked-in external-memory wrappers.

This satisfies “support on all boards with SDRAM hardware available” using the current
repository definition of “available”: only Pocket has checked-in SDRAM hardware exposure.

---

## 6. Simulator and Runtime Migration Impact

Even an RTL-native SDRAM controller still needs a coherent story for simulation and host
software.

### 6.1 Current runtime behavior

Today, FPGA and simulator runtimes both rely on host-owned `SystemBus` services:

- `bus-shared/src/bus.rs`
- `bus-shared/src/dram.rs`
- `device-runtime/src/fpga.rs`
- `device-runtime/src/sim/sim_core.rs`

The host path is therefore doing more than “just FPGA transport”; it is also the current
main-memory implementation for the simulator/runtime architecture.

### 6.2 Migration choices

There are two main ways to transition:

#### Choice A: Native SDRAM only on physical SDRAM boards, keep Rust DRAM in sim

- simulation continues using Rust DRAM
- Pocket hardware uses native RTL SDRAM
- address map stays the same, but backend ownership differs by target

**Benefit:** lowest disruption to existing simulation infrastructure.

**Cost:** sim and hardware diverge in memory implementation details.

#### Choice B: Build an RTL SDRAM model into simulation too

- simulation exercises the same controller RTL path as hardware
- requires a simulated memory model behind the SDRAM controller

**Benefit:** stronger architectural parity.

**Cost:** larger implementation and validation effort.

### 6.3 Recommended runtime migration

For a first implementation, **Choice A** is the better fit.

The project already tolerates backend differences where appropriate, and preserving the
existing simulator path keeps the initial SDRAM migration focused on shared address-map
integration plus Pocket hardware enablement.

---

## 7. Verification Strategy

A minimal-risk verification plan should include three layers.

### 7.1 Shared RTL unit/integration tests

Add focused tests for the controller itself using the existing standalone-wrapper and
Rust integration-test pattern already used elsewhere in the repository.
Relevant precedents include:

- `rtl/common/wrappers/`
- `testbench/tests/`
- `riscv_core/src/lib.rs`

Suggested controller-focused checks:

1. power-up initialization completes correctly
2. refresh is issued at the required interval
3. word/halfword/byte writes generate correct DQM behavior
4. read data returns on the expected D-channel timing
5. row-hit vs row-miss behavior is correct
6. back-to-back reads/writes obey SDRAM timing guards

### 7.2 Shared top-level integration tests

Add tests that prove the CPU can:

1. fetch/load/store against the SDRAM window through the new routing rule
2. coexist with existing RTL peripherals
3. avoid regressions on unmapped host-routed upper-half devices such as FIFO, audio,
   video, DMA, and SimControl

### 7.3 Board-specific validation

For Pocket specifically:

1. synthesize the Pocket target with the new controller
2. inspect timing/resource impact in Quartus reports
3. verify pin-level integration still matches `ap_core.qsf`
4. run real memory smoke tests from software once the core is deployable

Because SDRAM controllers are timing-sensitive, this hardware bring-up phase is where
concrete signal observation and board-backed testing become essential.

---

## 8. Key Risks and Constraints

### 8.1 Address-map compatibility risk

Moving DRAM away from `0x8000_0000` would ripple into:

- software assumptions
- tests
- runtime code
- documentation
- any existing compiled program expectations

This is why preserving the current DRAM base address is preferable.

### 8.2 FPGA timing/resource risk

A true SDRAM controller is meaningfully more complex than the current SRAM peripheral.
It introduces:

- initialization sequencing
- refresh scheduling
- data-direction control
- tighter cycle timing
- larger control state

Pocket is the most realistic first target because its checked-in platform integration
already exposes the real memory pins. By contrast, forcing the same native design into
non-Pocket wrappers would add board adaptation work before the repository has the needed
pin-level seams.

### 8.3 Multi-target maintenance risk

If the shared top-level is changed without a clear fallback policy, non-SDRAM targets
could lose buildability or runtime behavior. That argues for a staged rollout where:

- the shared top-level learns about an SDRAM slave,
- Pocket actually uses it,
- and other targets retain the host-routed DRAM path until they have checked-in native
  memory wrappers.

---

## 9. Recommended Implementation Sequence

### Phase 1: Research-to-plan transition

Produce an implementation plan from this report that specifies:

1. chosen DRAM routing policy (recommended: preserve `0x8000_0000`)
2. controller module/file split
3. Pocket-specific wrapper changes
4. fallback behavior for non-SDRAM targets
5. simulation strategy

### Phase 2: Shared architecture enablement

1. add the new SDRAM controller slave to `rtl/common/top.sv`
2. update `host_bus_mux` routing so the DRAM range can remain local RTL
3. update the shared memory-map documentation/constants
4. preserve host-routed service windows that are not moving to RTL

### Phase 3: Controller bring-up in simulation

1. create a standalone test wrapper and targeted Rust integration tests
2. validate init, refresh, read/write timing, and mask behavior
3. validate CPU-visible SDRAM access through the shared top-level

### Phase 4: Analogue Pocket native enablement

1. replace the current Pocket `dram_*` tie-offs with real controller wiring
2. preserve UART/link-port debug transport where still useful
3. synthesize and inspect Quartus timing/resource reports
4. run software smoke tests using the DRAM region

### Phase 5: Follow-on board work

Only after additional targets gain checked-in external-memory seams should the native
SDRAM path be enabled there. Until then, those targets should continue to use the
existing host-routed DRAM model.

---

## 10. Final Recommendation

The repository should pursue an **RTL SDRAM controller peripheral** as a shared SoC
feature, but the implementation plan should explicitly separate:

1. **shared architectural migration** from host-routed DRAM to RTL-routed DRAM, and
2. **board-specific physical SDRAM enablement**.

Based on current checked-in repository evidence:

- **Pocket is the only board that should receive immediate native SDRAM support**,
  because it is the only target exposing actual SDRAM pins in the repository.
- The controller should be integrated through the existing `top.sv` / `registered_bus`
  architecture rather than as a one-off Pocket-only subsystem.
- The DRAM window should remain at **`0x8000_0000`** and `host_bus_mux` should be
  adjusted so that the SDRAM range can be handled locally in RTL while unrelated
  host-routed upper-half devices remain on the existing path.
- Non-Pocket targets should keep the current host-routed DRAM behavior until they gain
  checked-in native external-memory seams.

That path gives the repository the best mix of:

- software compatibility,
- minimal unnecessary churn,
- immediate usefulness on real SDRAM hardware,
- and a clean long-term architecture for future native external-memory support.
