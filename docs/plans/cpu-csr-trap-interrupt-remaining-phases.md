# CPU CSR, Trap, and Interrupt Remaining Phases

This plan captures the work that remains after **phase 1** of the CSR/trap audit follow-up.

## Completed in phase 1

- Fixed `SYSTEM` decode so `MRET` and `WFI` are no longer misclassified as `ECALL` and `EBREAK`.
- Added temporary non-halting placeholder execution for decoded `MRET` and `WFI` so the core keeps making progress until real privileged behavior is implemented.
- Added focused verification that these instructions now decode distinctly from the halt-causing `ECALL` and `EBREAK` paths.

## Phase 2 — fix FCSR sub-CSR write semantics

### Objective

Make `fflags`, `frm`, and `fcsr` obey the same Zicsr operation semantics as the rest of the CSR datapath.

### Scope

- `rtl/common/cpu/cpu.sv`
- `rtl/common/cpu/csr_file.sv`
- likely Rust coverage in:
  - `device-runtime/tests/test_instructions.rs`

### Work items

1. Remove the ad hoc FCSR sub-CSR write path in `cpu.sv`.
2. Reuse the shared CSR write result for:
   - `CSRRW`
   - `CSRRS`
   - `CSRRC`
   - `CSRRWI`
   - `CSRRSI`
   - `CSRRCI`
3. Preserve read-only forms when the register/immediate source is zero.
4. Preserve FP exception flag accumulation behavior.

### Verification

- Add directed tests for `fflags`, `frm`, and `fcsr`.
- Specifically cover immediate forms and clear/set read-modify-write cases.

## Phase 3 — implement synchronous trap entry

### Objective

Replace halt-for-debug behavior with architectural machine-mode trap entry for:

- illegal instruction
- `ECALL`
- `EBREAK`

### Scope

- `rtl/common/cpu/cpu.sv`
- `rtl/common/cpu/csr_file.sv`
- possibly:
  - `rtl/common/cpu/decoder.sv`
  - `rtl/common/top.sv`

### Work items

1. Add a real trap-entry control path in the CPU FSM.
2. Save the faulting PC into `mepc`.
3. Write the correct synchronous exception code into `mcause`.
4. Write the chosen `mtval` payload.
5. Update `mstatus` trap-enable state.
6. Redirect `pc` to `mtvec`.
7. Invalidate or flush fetch/decompression state on trap redirect.
8. Ensure the trapping instruction does **not** retire.

### Key design decisions

- Whether to add a dedicated trap-entry state or fold trap actions into existing states.
- What value `mtval` should contain for illegal compressed or decompressed instructions.
- How to make trap-critical CSRs available without depending on the normal CSR read pipeline latency.

### Verification

- Add direct CPU tests that:
  - execute `ECALL`, `EBREAK`, and illegal instructions,
  - verify `pc` redirects to `mtvec`,
  - verify `mepc`, `mcause`, and `mtval`,
  - verify the faulting instruction does not retire.

## Phase 4 — implement `MRET`

### Objective

Replace the phase 1 placeholder with real machine-mode trap return behavior.

### Scope

- `rtl/common/cpu/cpu.sv`
- likely `rtl/common/cpu/csr_file.sv`

### Work items

1. Load `pc` from `mepc`.
2. Restore machine interrupt-enable state from `mstatus`.
3. Flush fetch state on the return redirect.
4. Preserve compressed-instruction alignment rules for `mepc`.

### Verification

- Add a minimal trap handler program that returns with `MRET`.
- Verify execution resumes at the saved return PC.
- Verify `mstatus.MIE`/`MPIE` restoration behavior.

## Phase 5 — add interrupt inputs and `mip` plumbing

### Objective

Expose machine interrupt sources to the CPU and make them software-visible.

### Scope

- `rtl/common/cpu/cpu.sv`
- `rtl/common/cpu/csr_file.sv`
- `rtl/common/top.sv`

### Work items

1. Add pending interrupt input ports for machine software, timer, and external interrupts.
2. Drive `mip` from those pending sources.
3. Decide whether `mip` is purely read-only or partially writable in this machine-mode-only design.

### Verification

- Direct CPU tests that assert and deassert each pending interrupt source.
- CSR reads that confirm `mip` reflects pending inputs correctly.

## Phase 6 — interrupt polling and arbitration

### Objective

Take interrupts between instructions when they are both pending and enabled.

### Scope

- `rtl/common/cpu/cpu.sv`
- `rtl/common/cpu/csr_file.sv`

### Work items

1. Add a precise interrupt poll point between retired instructions.
2. Evaluate:
   - global enable from `mstatus`,
   - per-source enables from `mie`,
   - pending bits from `mip`.
3. Define a priority scheme for simultaneous interrupts.
4. Redirect to `mtvec` on an accepted interrupt.
5. Write `mepc`, `mcause`, and `mstatus` for interrupt entry.

### Verification

- Tests for:
  - pending-but-disabled interrupts not trapping,
  - enabled pending interrupts trapping,
  - simultaneous pending sources following the documented priority,
  - return to interrupted code via `MRET`.

## Phase 7 — tighten CSR legality enforcement

### Objective

Stop silently accepting unsupported or illegal CSR accesses once trap entry exists.

### Scope

- `rtl/common/cpu/csr_file.sv`
- `rtl/common/cpu/cpu.sv`
- possibly `rtl/common/cpu/decoder.sv`

### Work items

1. Trap on unsupported CSR addresses.
2. Trap on writes to read-only CSRs.
3. Trap on unsupported `SYSTEM` encodings and other privileged misuse chosen for enforcement.

### Verification

- Direct CPU tests for illegal CSR reads/writes.
- Integration tests that verify software-visible illegal instruction behavior.

## Phase 8 — WARL and WPRI cleanup

### Objective

Mask or constrain unsupported bits so machine CSRs present cleaner architectural behavior.

### Scope

- `rtl/common/cpu/csr_file.sv`

### Work items

1. Document and constrain supported bits in `mstatus`.
2. Constrain `mtvec` mode and alignment behavior.
3. Decide whether `medeleg` and `mideleg` remain hardwired or writable-but-inert.
4. Apply final `mepc` bit-masking rules.

### Verification

- Directed CSR read/write tests for masked and constrained fields.

## Recommended execution order

1. Phase 2 — FCSR sub-CSR semantics
2. Phase 3 — synchronous trap entry
3. Phase 4 — real `MRET`
4. Phase 5 — interrupt inputs and `mip`
5. Phase 6 — interrupt polling and arbitration
6. Phase 7 — CSR legality enforcement
7. Phase 8 — WARL/WPRI cleanup
