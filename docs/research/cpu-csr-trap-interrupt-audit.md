# CPU CSR, Trap, and Interrupt RTL Audit

**Research Document**  
**Context:** Audit of CPU RTL behavior related to CSRs, traps, and interrupts in the multi-cycle RV32IMACF core  
**Date:** 2026-03-24

## Executive Summary

The current CPU RTL has a solid foundation for basic CSR access, but trap and interrupt support is only partially realized. The CSR datapath itself already gets several important details right: CSR addresses are decoded centrally, read-before-write semantics are preserved for normal CSR instructions, writable machine CSRs are stored in BRAM, and the writeback path returns the old CSR value rather than the newly written value (`rtl/common/cpu/csr_file.sv:89-205`, `rtl/common/cpu/cpu.sv:302,392-406,803-812,1089-1104,1196-1201`, `rtl/common/cpu/writeback_mux.sv:47-53`).

However, the CPU does **not** currently implement machine-mode trap entry or interrupt handling. The trap-related CSRs (`mstatus`, `mtvec`, `mepc`, `mcause`, `mtval`, `mie`) exist as software-visible storage, but the core never consumes them to redirect control flow and never updates them on exceptions or interrupts. Instead, exception-like events such as `ECALL`, `EBREAK`, and invalid instructions fall into `S_HALT`, which is useful for debug bring-up but not architecturally correct machine-mode behavior (`rtl/common/cpu/cpu.sv:617-667`).

There are also two concrete CSR/system-instruction bugs:

1. `SYSTEM` decode only inspects `imm_i_dec[0]`, so `MRET` is misdecoded as `ECALL` and `WFI` is misdecoded as `EBREAK` (`rtl/common/cpu/decoder.sv:361-375`).
2. FCSR sub-CSR writes in `cpu.sv` bypass the main CSR operation logic and always assign raw `rs1` data, which breaks `CSRRS`, `CSRRC`, and all immediate forms for `fflags`, `frm`, and `fcsr` (`rtl/common/cpu/cpu.sv:1152-1168`, `rtl/common/cpu/csr_file.sv:164-191`).

The overall conclusion is that the design currently implements **CSR access** but not a complete **privileged architecture**. Trap-related CSRs are present, but machine-mode exception/interrupt semantics are largely absent.

## Scope and Method

This audit focused on the CPU RTL files that directly participate in CSR and privileged control flow:

- `rtl/common/cpu/cpu.sv`
- `rtl/common/cpu/csr_file.sv`
- `rtl/common/cpu/decoder.sv`
- `rtl/common/cpu/writeback_mux.sv`
- `rtl/common/memory/sync_dpram.sv`

The goal was to identify:

- incorrect behavior,
- missing functionality,
- and improvement opportunities

related specifically to CSR operations, exception/trap handling, and interrupt support.

## Current Architecture Overview

### CSR storage model

`csr_file.sv` implements a hybrid CSR model:

- FCSR/FRM/FFLAGS are read from the top-level `fcsr` register (`rtl/common/cpu/csr_file.sv:127-130`).
- Writable machine CSRs are stored in a sparse BRAM-backed array via `sync_dpram` (`rtl/common/cpu/csr_file.sv:66-114,189-205`).
- read-only CSRs such as `misa`, machine ID CSRs, and counters are produced combinationally (`rtl/common/cpu/csr_file.sv:125-157`).

The writable BRAM-backed CSR set is:

- `mstatus`
- `medeleg`
- `mideleg`
- `mie`
- `mtvec`
- `mscratch`
- `mepc`
- `mcause`
- `mtval`

(`rtl/common/cpu/csr_file.sv:70-78,104-112`)

### CSR timing model

CSR reads depend on the synchronous two-cycle read latency of `sync_dpram`, which registers the BRAM output twice before exposing `rdata` (`rtl/common/memory/sync_dpram.sv:67-76`).

The CPU FSM aligns with that latency:

- `S_DECODE` captures the CSR address via `imm_i_reg`
- `S_DECODE_WAIT` lets the decoded address settle
- `S_REG_READ` advances the BRAM read pipeline
- `S_REG_READ_WAIT` captures `csr_rdata` into `csr_rdata_reg`
- `S_CSR` performs the write side effect if needed
- `S_WRITEBACK` returns the old CSR value to `rd`

(`rtl/common/cpu/cpu.sv:302,617-667,803-812,1089-1104,1196-1201`)

This is an important positive result: normal CSR read-before-write behavior is intentionally preserved.

## What Is Already Correct

### 1. Normal CSR instruction semantics are mostly implemented correctly

The central CSR write datapath computes the correct value for all six Zicsr instructions:

- `CSRRW`
- `CSRRS`
- `CSRRC`
- `CSRRWI`
- `CSRRSI`
- `CSRRCI`

using `funct3` and either `rs1_data` or the 5-bit immediate (`rtl/common/cpu/csr_file.sv:164-177`).

The design also correctly suppresses writes for `CSRRS`, `CSRRC`, `CSRRSI`, and `CSRRCI` when `rs1/zimm == 0`, so pure read forms do not produce side effects (`rtl/common/cpu/csr_file.sv:179-189`).

### 2. Old CSR value is returned to `rd`

The old CSR value is captured into `csr_rdata_reg` in `S_REG_READ_WAIT` before any write is performed, and the writeback mux explicitly uses that registered old value for CSR instructions (`rtl/common/cpu/cpu.sv:803-812,1196-1201`, `rtl/common/cpu/writeback_mux.sv:47-53`).

That matches the architecturally expected read-before-write behavior for CSR instructions.

### 3. The CSR address map itself is reasonably complete for machine mode

`csr_file.sv` declares:

- machine status/control CSRs (`mstatus`, `mie`, `mtvec`, `mepc`, `mcause`, `mtval`, `mip`)
- machine information CSRs
- the standard counters
- FCSR-related CSRs

(`rtl/common/cpu/csr_file.sv:35-63`)

The `misa` constant also correctly reflects enabled `I`, `A`, `C`, `M`, and `F` extensions (`rtl/common/cpu/csr_file.sv:79-81`).

### 4. Counter CSRs are internally consistent

`cycle` increments every cycle and `instret` increments on `instr_complete`, while the upper-half counter CSRs are intentionally hardwired to zero (`rtl/common/cpu/csr_file.sv:116-157,207-220`).

This is a simplification, but it is internally consistent and clearly encoded in the RTL.

## Incorrect Behavior

### 1. `MRET` and `WFI` are decoded incorrectly

The `SYSTEM` decoder distinguishes only between:

- `funct3 == 000` and `imm_i_dec[0] == 0` → `ECALL`
- `funct3 == 000` and `imm_i_dec[0] == 1` → `EBREAK`

(`rtl/common/cpu/decoder.sv:361-375`)

That is too coarse. It means:

- `ECALL` (`funct12 = 12'h000`) is decoded correctly
- `EBREAK` (`funct12 = 12'h001`) is decoded correctly
- `MRET` (`funct12 = 12'h302`) is incorrectly decoded as `ECALL`
- `WFI` (`funct12 = 12'h105`) is incorrectly decoded as `EBREAK`

Because the CPU transitions `ECALL`/`EBREAK` to `S_HALT`, `MRET` and `WFI` currently halt the machine instead of performing their architectural behavior (`rtl/common/cpu/cpu.sv:656-660`).

This is the most severe privileged-architecture bug in the current RTL, because it makes a real machine-mode trap return impossible.

### 2. FCSR sub-CSR writes ignore the CSR operation type

`csr_file.sv` already computes the correct Zicsr write value in `csr_wdata` (`rtl/common/cpu/csr_file.sv:164-191`).

But FCSR writes do not use that logic. Instead, `cpu.sv` directly updates:

- `fflags` from `a_reg[4:0]`
- `frm` from `a_reg[2:0]`
- `fcsr` from `a_reg`

whenever the core is in `S_CSR` and `is_csr_reg` is true (`rtl/common/cpu/cpu.sv:1160-1168`).

That is only correct for `CSRRW`. It is incorrect for:

- `CSRRS`
- `CSRRC`
- `CSRRSI`
- `CSRRCI`
- `CSRRWI`

because those instructions require either read-modify-write behavior or a 5-bit immediate source, neither of which is respected by the hard-coded `a_reg` assignment path.

Practical effect:

- software trying to clear sticky FP exception flags with `CSRRCI` will not get the expected result,
- immediate-form writes to `fflags`, `frm`, or `fcsr` use register data rather than the encoded immediate,
- and sub-CSR behavior diverges from the main CSR implementation.

### 3. Illegal or unimplemented CSR accesses do not trap

Unimplemented CSRs read as zero by default (`rtl/common/cpu/csr_file.sv:155-156`), and writes only occur when `is_writable_csr_addr(csr_addr)` is true (`rtl/common/cpu/csr_file.sv:104-112,189-191`).

That means:

- accesses to unknown CSRs silently read zero,
- writes to read-only or unsupported CSRs are silently ignored.

For a full privileged implementation, many such accesses should raise an illegal-instruction exception rather than behaving as a benign no-op.

This is especially important once the core claims meaningful machine-mode trap support, because firmware expects illegal CSR accesses to trap rather than disappear.

## Missing Trap Functionality

### 1. `ECALL`, `EBREAK`, and illegal instructions do not enter a trap handler

In `S_REG_READ_WAIT`, invalid instructions go straight to `S_HALT` (`rtl/common/cpu/cpu.sv:620-625`).

For `SYSTEM` instructions, `ECALL` and `EBREAK` also go straight to `S_HALT` (`rtl/common/cpu/cpu.sv:656-660`).

What is missing for a real machine-mode trap:

- save faulting PC into `mepc`
- write cause code into `mcause`
- write relevant trap value into `mtval`
- update trap-enable state in `mstatus`
- redirect `pc` to `mtvec`

None of those actions exist anywhere in `cpu.sv`. A repository-wide search finds the trap CSRs defined only in `csr_file.sv`, with no hardware trap-entry logic consuming or updating them elsewhere (`rtl/common/cpu/csr_file.sv:47-57,89-112,132-137`).

As a result, the trap-related CSRs are currently software-readable storage only, not active architectural state.

### 2. `MRET` is not implemented

There is no dedicated `is_mret` decode signal, no `MRET` state/flow in the FSM, and no RTL that:

- loads `pc` from `mepc`,
- restores interrupt-enable state from `mstatus`,
- or exits trap context.

Since `MRET` is currently misdecoded as `ECALL`, machine-mode software cannot return from an exception or interrupt handler at all (`rtl/common/cpu/decoder.sv:361-375`, `rtl/common/cpu/cpu.sv:656-660`).

### 3. Trap CSRs exist, but most are not consumed by hardware

The following CSRs are stored in hardware:

- `mstatus`
- `mie`
- `mtvec`
- `mepc`
- `mcause`
- `mtval`

(`rtl/common/cpu/csr_file.sv:70-78,132-137`)

But outside `csr_file.sv`, there is no RTL using those fields to control execution flow. In practice:

- `mtvec` never determines the next PC,
- `mepc` is never loaded into the PC,
- `mcause` and `mtval` are never written by hardware,
- `mie` does not gate any interrupt source,
- and `mstatus` trap-enable bits are not interpreted anywhere.

That makes the trap CSR block structurally present but functionally dormant.

## Missing Interrupt Functionality

### 1. The CPU has no interrupt input ports

The CPU module interface contains no hardware interrupt inputs at all (`rtl/common/cpu/cpu.sv:17-62`).

There are no ports corresponding to machine-level interrupt sources such as:

- external interrupt pending,
- timer interrupt pending,
- software interrupt pending.

Without those signals, the core cannot observe asynchronous interrupts no matter what software writes into `mie` or `mstatus`.

### 2. `mip` is hardwired to zero

`csr_file.sv` returns `32'h0` for `mip` unconditionally (`rtl/common/cpu/csr_file.sv:137`).

That is a valid placeholder for “interrupts not yet implemented,” but it also means:

- software can never observe a pending interrupt,
- and the interrupt CSR set is incomplete from a functional perspective.

### 3. The FSM never checks for pending interrupts

There is no logic anywhere in the CPU FSM that checks whether an interrupt should be taken between instructions. In particular, there is no point where the core evaluates something equivalent to:

- global machine interrupt enable,
- local enable bits in `mie`,
- pending bits in `mip`,
- and priority/arbitration rules

before redirecting to a trap vector.

So even if interrupt inputs were added later, the current FSM has no interrupt entry path to consume them.

## Areas That Could Be Improved

### 1. `SYSTEM` decode should use full `funct12`, not `imm_i_dec[0]`

The decoder currently collapses all `funct3 == 000` system instructions into a 1-bit choice between `ECALL` and `EBREAK` (`rtl/common/cpu/decoder.sv:361-375`).

A more robust approach is to decode the full 12-bit immediate field and explicitly recognize at least:

- `ECALL`
- `EBREAK`
- `MRET`
- `WFI`

This would remove the current misdecode bug and make future privileged-instruction expansion straightforward.

### 2. FCSR updates should share the same CSR operation result as the main CSR path

The FCSR-specific update path should not duplicate partial CSR semantics in `cpu.sv`. Instead, it should use the same decoded write value as the main CSR datapath, or the architecture should be refactored so that FCSR sub-CSR writes flow through a single shared CSR result path (`rtl/common/cpu/csr_file.sv:164-191`, `rtl/common/cpu/cpu.sv:1160-1168`).

That would eliminate the current semantic mismatch and reduce the chance of future divergence between normal CSRs and floating-point CSRs.

### 3. Trap entry should be implemented as a first-class architectural flow, not as halt-for-debug

Using `S_HALT` for `ECALL`, `EBREAK`, and illegal instructions is fine for early bring-up, but it prevents firmware from exercising the machine-mode trap architecture. A cleaner design would add an explicit trap-entry path that:

- captures the appropriate trap metadata,
- updates machine state,
- and redirects to `mtvec`.

`S_HALT` can remain for explicit debug halts or external stop requests, but it should not be the only outcome for synchronous exceptions.

### 4. Privileged CSR legality should eventually be enforced

Once trap entry exists, the design should move beyond “unknown CSR reads as zero / illegal write is ignored” and instead detect:

- unsupported CSR addresses,
- writes to read-only CSRs,
- and privileged instruction misuse

as illegal-instruction traps.

That will matter for compliance-oriented software and for making debugging behavior match software expectations.

### 5. Some stored machine CSRs may deserve tighter WARL/WPRI handling

Today, writable trap-related CSRs are stored as unrestricted 32-bit values in BRAM (`rtl/common/cpu/csr_file.sv:132-137,189-205`).

Potential improvements include:

- masking unsupported bits in `mstatus`,
- constraining `mtvec` mode/alignment to legal encodings,
- hardwiring delegation registers to zero if the core remains machine-mode only,
- and documenting which bits are architecturally meaningful versus currently inert.

These are secondary to trap-entry correctness, but they would make software-visible behavior cleaner and more predictable.

## Recommended Fix Order

If this research is converted into an implementation plan, the highest-value sequence is:

1. **Fix `SYSTEM` decode** so `MRET` and `WFI` are not misclassified.
2. **Fix FCSR sub-CSR write semantics** to honor all Zicsr operations.
3. **Implement synchronous trap entry** for illegal instruction, `ECALL`, and `EBREAK`.
4. **Implement `MRET`** so trap handlers can return.
5. **Add interrupt inputs, `mip` plumbing, and interrupt polling/arbitration**.
6. **Tighten CSR legality/WARL behavior** for reserved or unsupported cases.

## Bottom Line

The CSR block is not purely stubbed out: ordinary CSR reads/writes are meaningfully implemented, and the pipeline carefully preserves old-value writeback semantics. But the privileged architecture stops short at the point where those CSRs should control execution.

Today, the design behaves like:

- a core with working CSR read/write instructions,
- some useful performance and FP-control CSRs,
- and debug-oriented halting behavior for exception-like events.

It does **not** yet behave like a machine-mode RISC-V core with real trap entry, trap return, or interrupts.
