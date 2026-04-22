# External Device Interrupt Support Plan

## 1. Objective

Add real machine-mode external device interrupt support to the RV32IMACF CPU and platform, while closing the remaining adjacent gaps that are still required for correct interrupt behavior.

This plan is intentionally broader than “add a `meip` wire.” The current repo already has synchronous trap entry and `MRET`, but external interrupts still require CPU, CSR, top-level platform, peripheral, host-integration, software-visible register, and verification work before they are usable end to end.

## 2. Current State

### What already exists

- Synchronous trap entry is present for illegal instructions, `ECALL`, and `EBREAK`.
- `MRET` now redirects `pc` from `mepc`.
- Trap-critical CSRs are discrete registers in `csr_file.sv`, with direct outputs for `mtvec`, `mepc`, `mstatus`, `mie`, and `mip`.
- There is directed RTL and integration coverage for synchronous traps and `MRET`.

### What is still missing for interrupts

- `cpu.sv` has no interrupt input ports, so the core cannot observe any pending interrupt source.
- `top.sv` does not route any interrupt signals into the CPU.
- `csr_file.sv` still hardwires `mip` to zero instead of reflecting pending sources.
- `cpu.sv` instantiates `csr_file` but discards `csr_mstatus_out`, `csr_mie_out`, and `csr_mip_out`, so the FSM cannot evaluate interrupt eligibility.
- There is no interrupt polling/arbitration point in the CPU FSM between instructions.
- `WFI` is still a placeholder that behaves like a normal sequential advance rather than an interrupt wait state.
- No RTL peripheral exports an interrupt request line today.
- The platform has no interrupt controller / aggregator / claim-complete block for device-originated external interrupts.
- The host-bus path is request/response only, so host-side Rust devices cannot currently assert an interrupt into FPGA RTL.
- The software-visible memory map has no interrupt-controller register block.
- Verification only covers synchronous traps, not asynchronous interrupt delivery.

## 3. Best Architectural Direction

### Recommended principle

Do **not** wire individual devices directly into ad hoc CPU-specific logic.

Instead:

1. extend the CPU with standard machine interrupt pending inputs,
2. make `mip` reflect those inputs,
3. implement precise interrupt acceptance in the CPU,
4. add a small platform interrupt controller/aggregator for external device sources,
5. then connect individual peripherals and host-routed devices to that controller.

This keeps the CPU architectural, keeps platform policy outside the core, and avoids repainting the interrupt architecture every time a new device is added.

### Recommended minimum interrupt model

Implement all three machine-mode pending classes in the CPU interface even if external interrupts are the immediate goal:

- machine software interrupt pending (`msip`)
- machine timer interrupt pending (`mtip`)
- machine external interrupt pending (`meip`)

External device interrupts should then arrive through the **machine external interrupt** path. Even if `msip` and `mtip` are not fully plumbed to devices in the first milestone, reserving the standard CPU-side hooks now avoids another control-path refactor later.

### Recommended platform block

Add a **minimal PLIC-like external interrupt controller** as a distinct RTL peripheral rather than overloading the system controller.

That controller should:

- accept multiple device interrupt request inputs,
- latch pending status,
- provide per-source enable bits,
- reduce enabled pending sources into a single `meip` output to the CPU,
- expose source identification to software,
- support claim/complete or an equivalent acknowledge flow so software can service and clear a source deterministically.

For this machine-mode-only design, the controller can start much smaller than full privileged-spec PLIC complexity, but it should still provide a clean software contract instead of a raw “some interrupt happened” bit.

## 4. Required Workstreams

## 4.1 CPU interrupt entry support

### Scope

- `rtl/common/cpu/cpu.sv`
- `rtl/common/cpu/csr_file.sv`

### Work items

1. Add pending interrupt input ports to `cpu.sv` for `msip`, `mtip`, and `meip`.
2. Connect those pending inputs into `csr_file.sv` so `mip` reflects machine pending bits instead of returning zero.
3. Stop dropping `csr_mstatus_out`, `csr_mie_out`, and `csr_mip_out` in `cpu.sv`; use them in the FSM.
4. Add a precise interrupt-accept check at an instruction boundary, not mid-instruction.
5. Compute interrupt eligibility from:
   - `mstatus.MIE`
   - `mie`
   - `mip`
6. Choose and document the interrupt priority order. For a machine-mode-only core, use the standard machine-level ordering consistently.
7. On accepted interrupt:
   - write `mepc` with the resume PC,
   - write `mcause` with the interrupt bit set and the proper machine interrupt code,
   - write `mtval` with zero unless a different payload is intentionally defined,
   - update `mstatus.MPIE/MIE`,
   - redirect `pc` to `mtvec`,
   - flush/invalidate fetch state so no stale prefetch survives the redirect,
   - ensure the interrupted instruction stream remains precise.

### Key design choice

Poll for interrupts **between retired instructions**, at the same architectural boundary where the next sequential fetch would otherwise begin. That keeps the multi-cycle core precise and avoids partially executing the next instruction before taking the trap.

## 4.2 `WFI` completion

### Scope

- `rtl/common/cpu/cpu.sv`
- likely `rtl/common/cpu/decoder.sv`

### Work items

1. Replace the current placeholder `WFI` behavior with a real wait state.
2. Hold the CPU in an idle/wait condition until:
   - an interrupt becomes pending in a way that should wake the hart, or
   - debug/reset/halt control requires exit.
3. Reuse the same interrupt eligibility/pending logic used by normal interrupt entry so `WFI` does not become a divergent special case.
4. Keep the implementation timing-friendly by making `WFI` a control-state hold, not a combinational side path.

### Why this matters

External interrupt support is incomplete if software cannot sleep and wake on interrupts. Leaving `WFI` as a sequential no-op would make the interrupt model functionally awkward even if trap entry itself works.

## 4.3 External interrupt controller / aggregator

### Scope

- new RTL peripheral under `rtl/common/peripherals/`
- `rtl/common/top.sv`
- `riscv_shared/src/bus.rs`
- `docs/memory-map.md`

### Work items

1. Add a dedicated interrupt-controller peripheral and memory-map window.
2. Define a small, stable register interface, for example:
   - pending bits
   - enable bits
   - optional raw-status bits
   - claim register
   - complete register
3. Produce a single registered `meip` output from enabled pending external sources.
4. Decide whether pending bits are:
   - level-sensitive mirrors of live device IRQs,
   - edge-latched until software completion,
   - or mixed depending on source type.
5. Keep the controller machine-mode-only at first; one target context is enough initially.
6. Make the controller source-count-parameterized so adding devices later does not require interface redesign.

### Recommended first implementation

Start with a **PLIC-lite** design:

- one machine-mode target,
- fixed priority or simple source-number priority,
- one claim register returning the highest-priority enabled pending source,
- one completion register clearing the claimed source.

That is enough to make external device interrupts software-usable without pulling in the full complexity of a complete multi-context PLIC implementation.

## 4.4 Peripheral interrupt-source plumbing

### Scope

- `rtl/common/peripherals/*`
- `rtl/common/top.sv`

### Work items

1. Identify which existing peripherals should be interrupt-capable in the near term.
2. Add explicit IRQ outputs only where there is a real service event to report.
3. Register those outputs in their local clock domain before they enter the interrupt controller.
4. Add CDC/synchronization where a peripheral lives outside the CPU clock domain.

### Recommended sequencing

Do not convert every peripheral at once. Bring up the path with one or two clear sources first, then expand.

Good initial candidates are whichever devices already have obvious service events, such as:

- gamepad state-change interrupt,
- video frame/vblank interrupt,
- audio buffer-threshold / completion interrupt,
- or a software-injected test interrupt source inside the controller itself.

The controller should be added before device-specific interrupt enable/status policy is spread across unrelated peripherals.

## 4.5 Host-routed device interrupt injection

### Scope

- `rtl/common/io/host_bus_interface.sv`
- host packet/protocol crates
- host-side runtime/device code
- possibly the new interrupt controller peripheral

### Work items

1. Decide how Rust/host-side devices can raise an interrupt into FPGA/top-level RTL.
2. Add one supported path for that notification.

### Recommended options

#### Option A — MMIO interrupt-inject register in the new RTL controller

The host writes a controller register over the existing bus protocol to set or clear pending external interrupt bits.

This is the simplest first step because the current host-bus path already supports request/response MMIO transactions, while it has no interrupt-specific sideband mechanism today.

#### Option B — extend the host packet protocol with an explicit interrupt message

This is a better long-term abstraction if host-routed devices must signal interrupts with low software overhead or without CPU polling, but it is a larger protocol change and should follow only if the MMIO-driven mechanism proves inadequate.

### Recommendation

Implement **Option A first**. It reuses the current host request path and gets end-to-end external interrupt delivery working without redesigning the transport protocol immediately.

## 4.6 Software-visible architecture cleanup that should ship with interrupts

These items are adjacent to interrupts and should be completed as part of the same overall effort, even if some land in separate commits.

### `mip` semantics

- Make `mip` read-only from software for implemented machine pending bits unless there is a deliberate reason to emulate writable software-pending bits locally.
- Document exactly which bits are implemented and which remain zero.

### `mcause` interrupt coding

- Use the interrupt bit plus the correct machine interrupt cause numbers.
- Add named constants in RTL/tests for machine software, timer, and external interrupt causes.

### `mtvec` behavior

- Keep direct mode only initially if desired, but document that vectored mode is unsupported and sanitize writes accordingly.
- Do not silently imply vectored external interrupt support if the controller still targets a direct handler entry model.

### CSR legality / WARL polish

- Tighten unsupported CSR and read-only write behavior once interrupt functionality is live.
- At minimum, keep the implemented interrupt-related CSR bits architecturally consistent and documented.

These are not the first blockers for external interrupt bring-up, but interrupt support will be easier to debug and use if the CSR contract is explicit rather than permissive and underspecified.

## 4.7 Verification

### RTL-focused CPU tests

Extend `testbench/tests/cpu_control_flow_test.rs` with direct pending-signal control to cover:

1. pending-but-disabled interrupt does not trap,
2. enabled `meip` traps between instructions,
3. simultaneous pending sources follow the documented priority,
4. `mcause`, `mepc`, and `mstatus` update correctly on interrupt entry,
5. `MRET` resumes at the interrupted PC,
6. `WFI` sleeps until an interrupt arrives.

### Integration tests

Extend device-runtime / bare-metal style tests to cover:

1. programming `mtvec`, `mie`, and `mstatus`,
2. taking an external interrupt and logging architectural state to DRAM,
3. servicing the interrupt through the controller claim/complete flow,
4. returning with `MRET`,
5. optional host-injected external interrupt delivery if host-routed devices are in scope.

### Peripheral/controller tests

Add focused tests for the new interrupt controller covering:

1. source pending latching,
2. enable masking,
3. claim priority,
4. completion clearing,
5. `meip` deassertion when no enabled pending source remains.

## 5. Recommended Execution Order

1. **CPU/CSR plumbing for `msip`/`mtip`/`meip` and real `mip` reflection**
2. **Precise interrupt polling/arbitration in the CPU**
3. **Real `WFI` wait behavior**
4. **Add the external interrupt controller peripheral and memory-map contract**
5. **Hook the controller’s `meip` output into `top.sv` and CPU instantiation**
6. **Bring up one synthetic or simple device source end to end**
7. **Add host MMIO interrupt injection path if host-routed external devices are required**
8. **Expand to additional peripherals**
9. **Tighten interrupt-adjacent CSR legality/WARL behavior**
10. **Update docs, shared constants, and software examples**

## 6. Definition of Done

External device interrupt support should be considered complete only when all of the following are true:

- the CPU can accept an asynchronous machine external interrupt without halting,
- `mip`, `mie`, `mstatus`, `mepc`, and `mcause` behave coherently for interrupt entry/return,
- `WFI` can sleep and wake on interrupts,
- at least one platform/device interrupt source can trigger `meip` end to end,
- software can identify and acknowledge the source through a stable MMIO contract,
- the memory map and shared constants document that contract,
- and both RTL-level and device-runtime-level tests cover the full interrupt path.

## 7. Summary

The remaining interrupt work is not just “add external interrupt wiring.” The missing pieces span:

- CPU architectural interrupt acceptance,
- `mip` plumbing,
- `WFI`,
- top-level signal routing,
- a platform interrupt controller,
- source-specific device wiring,
- host-to-RTL interrupt injection for Rust-side devices,
- and end-to-end verification.

The best path is to keep the CPU focused on standard machine interrupt semantics and place device-specific aggregation/claim logic in a small platform interrupt controller. That gives the repo a clean foundation for both immediate external device interrupts and later timer/software interrupt expansion.
