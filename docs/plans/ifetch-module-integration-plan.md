# `ifetch` RTL Module Integration Plan

## 1. Overview

This plan introduces a new `rtl/common/cpu/ifetch.sv` module that absorbs the CPU's instruction-fetch assembly path while deliberately reusing the existing `fetch_buffer.sv` logic for compressed-instruction alignment and decompression.

The goal is to remove the direct `fetch_buffer` integration from `cpu.sv` and replace it with a small multi-cycle fetch front-end that:

1. waits for a D-channel instruction response,
2. captures the returned word into an internal register,
3. feeds that registered word through `fetch_buffer`,
4. registers the final instruction outputs, and
5. presents a stable external fetch result back to the CPU.

This keeps the compressed-instruction behavior centralized in `fetch_buffer` while moving timing-sensitive fetch/decompress work behind a dedicated state machine.

## 2. Goals

### 2.1 Functional Goals

- Create a new `ifetch` RTL module under `rtl/common/cpu/`.
- Feed the module from the unified memory D channel used by `cpu.sv`.
- Expose registered outputs for:
  - the current fetched instruction,
  - a valid bit for that instruction,
  - a flag indicating whether the PC should advance by 2 bytes (`pc_inc_2=1`) or 4 bytes (`pc_inc_2=0`).
- Reuse `fetch_buffer.sv` internally rather than duplicating compressed-instruction assembly/decompression logic.
- Replace the existing direct `fetch_buffer` instantiation and related fetch glue in `cpu.sv`.

### 2.2 Timing / Structural Goals

- Break the current fetch path into explicit multi-cycle stages instead of consuming the D-channel response and `fetch_buffer` outputs in the same CPU cycle.
- Register the raw D-channel word before passing it into `fetch_buffer`.
- Register the final outputs that leave `ifetch` so the CPU sees a clean, stable fetch result.

### 2.3 Non-Goals

- No change to the architectural PC, branch, jump, or exception behavior.
- No change to the unified A/D memory interface protocol.
- No new prefetching, buffering of multiple instructions, or support for multiple outstanding memory requests.
- No rewrite of the compressed decompressor.

## 3. Current-State Summary

Today `cpu.sv` directly instantiates `fetch_buffer` and couples it tightly to the `S_FETCH` state:

- `S_FETCH` waits for the unified D-channel handshake (`mem_d_valid && mem_d_ready`).
- That same handshake immediately drives `ir_write`.
- `ir_write` simultaneously:
  - advances `fetch_buffer`,
  - latches `fetched_instruction` into `ir_reg`,
  - latches fetch validity into `is_instruction_valid_reg`,
  - lets the decoder capture the same instruction,
  - and updates compressed-width tracking used later for `pc_increment`.

Relevant existing behavior:

- `cpu.sv:489-505` instantiates `fetch_buffer` directly.
- `cpu.sv:559-566` leaves `S_FETCH` as soon as the D-channel response handshakes.
- `cpu.sv:747-750` asserts `ir_write` directly on that handshake.
- `cpu.sv:314-315` computes `instr_pc_next_reg` from the registered width information one state later.
- `fetch_buffer.sv:119-149` only advances or invalidates internal halfword state when `ir_write` or `invalidate_buffer` is asserted.

The new module must preserve those architectural behaviors while decoupling the timing.

## 4. Proposed Target Architecture

### 4.1 Module Placement

Add:

- `rtl/common/cpu/ifetch.sv`

Keep and reuse:

- `rtl/common/cpu/fetch_buffer.sv`
- `rtl/common/cpu/decompress.sv`

Update:

- `rtl/common/cpu/cpu.sv`
- targeted Rust/Verilator tests for fetch behavior

### 4.2 High-Level Structure

```text
cpu.sv
  └── ifetch.sv
        └── fetch_buffer.sv
              └── decompress.sv
```

`cpu.sv` remains the owner of:

- architectural `pc`,
- unified A-channel request generation,
- D-channel `mem_d_ready`,
- branch/jump redirect decisions,
- instruction register / decode pipeline.

`ifetch.sv` becomes the owner of:

- post-D-channel fetch staging,
- raw fetched-word storage,
- handoff into `fetch_buffer`,
- stable registered instruction/valid/width outputs,
- fetch-result-valid lifetime until the CPU consumes it,
- fetch-side flush of stale partial state on redirect.

## 5. Proposed `ifetch` Interface

The exact port names can be adjusted during implementation, but the module should expose a contract close to the following:

```systemverilog
module ifetch (
    input  wire logic        clk,
    input  wire logic        rst_n,

    // CPU control
    input  wire logic        fetch_start,
    input  wire logic        consume,
    input  wire logic        flush,
    input  wire logic [31:0] pc,

    // Unified memory D channel observation
    input  wire logic [31:0] mem_d_rdata,
    input  wire logic        mem_d_valid,
    input  wire logic        mem_d_ready,

    // Registered fetch result
    output logic [31:0] instruction,
    output logic        valid,
    output logic        pc_inc_2,

    // Status back to cpu.sv
    output logic        waiting_for_d,
    output logic        busy
);
```

### 5.1 Interface Notes

- `fetch_start` indicates that `cpu.sv` is actively fetching the instruction at `pc`.
- `consume` is asserted by `cpu.sv` when it latches the `ifetch` outputs into `ir_reg` / decode state.
- `flush` invalidates any partial fetch state after a control-flow redirect.
- `waiting_for_d` tells `cpu.sv` that it still needs to drive the instruction A-channel request for the current PC.
- `busy` covers all non-idle `ifetch` states, including post-response internal processing.

`ifetch` should not own `mem_d_ready`; the CPU already uses the unified memory response path for both instruction and data traffic.

## 6. Internal `ifetch` Datapath

### 6.1 Required Registers

The new module should contain at least these registers:

- `d_word_reg[31:0]`: raw D-channel instruction payload captured on response handshake.
- `d_word_valid`: marks `d_word_reg` as containing a response not yet consumed by the internal FSM.
- `stage_instruction_reg[31:0]`: staging register for the decompressed/aligned instruction coming out of `fetch_buffer`.
- `stage_valid_reg`: staging register for the fetch-buffer validity result.
- `instruction_reg[31:0]`: final registered output instruction.
- `valid_reg`: final registered output valid.
- `pc_inc_2_reg`: final registered width flag.
- `pc_reg[31:0]` or equivalent fetch-PC staging if the internal FSM needs to hold a stable PC copy while the CPU remains in `S_FETCH`.

The module should also instantiate `fetch_buffer` internally and provide it with:

- the staged raw fetch word,
- the fetch PC,
- an internal `ir_write` pulse,
- the flush/invalidate signal.

### 6.2 Why Two Output Register Layers Are Helpful

The existing `fetch_buffer` produces:

- combinational `instruction`,
- combinational `valid`,
- registered `pc_inc_2`.

Because `pc_inc_2` updates on `fetch_buffer`'s internal `ir_write` edge, `ifetch` should use a small two-step capture:

1. capture `instruction` / `valid` into staging registers while asserting the internal `fetch_buffer.ir_write`,
2. capture `fetch_buffer.pc_inc_2` into the final output register on the following cycle,
3. publish the full stable output bundle (`instruction`, `valid`, `pc_inc_2`) from `ifetch`'s output registers.

That avoids reimplementing compressed-width detection outside `fetch_buffer` while still giving the CPU fully registered outputs.

## 7. `ifetch` State Machine

The module should use a small synchronous FSM. A four-state design is sufficient.

### 7.1 State Definitions

#### `IF_IDLE`

- No outstanding internal fetch result.
- `valid=0`, `busy=0`.
- Transition to `IF_WAIT_D` when `fetch_start` is asserted.

#### `IF_WAIT_D`

- Wait for the instruction response handshake on the D channel.
- Detect response completion with `mem_d_valid && mem_d_ready`.
- On handshake:
  - capture `mem_d_rdata` into `d_word_reg`,
  - capture/stabilize the associated `pc`,
  - set `d_word_valid`,
  - transition to `IF_FEED_FETCH_BUFFER`.

#### `IF_FEED_FETCH_BUFFER`

- Drive `d_word_reg` and the staged `pc` into the internal `fetch_buffer`.
- Assert an internal one-cycle `fb_ir_write`.
- Capture the internal `fetch_buffer.instruction` and `fetch_buffer.valid` into staging registers.
- Transition to `IF_LATCH_OUTPUT`.

#### `IF_LATCH_OUTPUT`

- Capture `fetch_buffer.pc_inc_2` into `pc_inc_2_reg`.
- Copy the staged instruction/valid registers into the final output registers.
- Assert/preserve `valid_reg`.
- Clear temporary staging-valid flags as needed.
- Transition back to `IF_IDLE` after `consume`, or to a short output-hold behavior if `consume` can be delayed by one or more cycles.

### 7.2 Optional Output-Hold Variant

If the CPU does not guarantee immediate consumption on the first cycle that `ifetch.valid` is asserted, split `IF_LATCH_OUTPUT` into:

- `IF_PUBLISH`: output bundle is valid and held stable,
- transition back to `IF_IDLE` only after `consume`.

That variant is safer and likely clearer in waveform/debug traces.

## 8. CPU Integration Plan

### 8.1 Replace Direct `fetch_buffer` Usage

In `cpu.sv`:

- remove the direct `fetch_buffer u_fetch_buffer` instantiation,
- instantiate `ifetch u_ifetch`,
- rename the CPU-local signals as needed to make it clear they now come from `ifetch`, not directly from `fetch_buffer`.

Example signal migration:

- `fetched_instruction` stays as the CPU-local instruction source from `ifetch`
- `fetched_valid` stays as the CPU-local fetch validity source from `ifetch`
- `pc_inc_2` stays as the CPU-local width flag from `ifetch`
- `invalidate_fetch_buffer` becomes `ifetch_flush` or is wired directly into `ifetch.flush`

### 8.2 Change the Meaning of “fetch ready”

Today `S_FETCH` completes on the D-channel handshake itself.

After this refactor, `S_FETCH` should complete when the `ifetch` output bundle is valid, not merely when the memory response arrives.

Required CPU updates:

1. Keep `mem_d_ready` behavior unchanged for the unified memory protocol.
2. Change the `S_FETCH` next-state transition from “D-channel handshake happened” to “`ifetch.valid` is high”.
3. Change `ir_write` generation to pulse when `ifetch.valid` is consumed, not when the raw D-channel response arrives.

This is the most important behavioral change in `cpu.sv`.

### 8.3 Avoid Duplicate A-Channel Requests

This integration must prevent the CPU from issuing the same instruction request again while `ifetch` is still working through its internal post-response states.

The safest implementation is:

- assert `imem_req_internal` only while `ifetch.waiting_for_d` is high,
- deassert it once the D-channel response has been captured,
- keep `S_FETCH` active until `ifetch.valid` becomes high.

Without this change, `cpu.sv` could re-drive a duplicate instruction fetch for the same PC after `mem_req_inflight` drops but before the registered `ifetch` output is ready.

### 8.4 Preserve Existing PC / Link / Redirect Behavior

The refactor must preserve the existing CPU behavior for:

- `instr_pc_reg <= pc` when the fetched instruction is finally consumed,
- `instr_pc_next_reg <= instr_pc_reg + pc_increment`,
- compressed control-flow link addresses using `PC+2`,
- branch/jump redirect invalidation of buffered halfwords.

The current redirect flush condition in `cpu.sv` is still valid conceptually:

- taken branch in `S_BRANCH`,
- jump in `S_WRITEBACK`.

However, `ifetch.flush` must clear more than just the old `fetch_buffer.buffer_valid` if `ifetch` also has:

- a captured raw D-channel word,
- staged instruction/valid registers,
- a published-but-not-yet-consumed output.

## 9. Expected RTL Changes

### 9.1 New File

Add:

- `rtl/common/cpu/ifetch.sv`

Required conventions:

- begin with `` `default_nettype none ``
- end with `` `default_nettype wire ``
- use synchronous reset style (`always_ff @(posedge clk)` with `if (!rst_n)` inside)

### 9.2 `cpu.sv`

Update:

- fetch-signal declarations,
- `S_FETCH` next-state condition,
- `ir_write` generation,
- instruction-fetch request gating,
- flush wiring,
- module instantiation block.

No other CPU execution-state behavior should change.

### 9.3 `fetch_buffer.sv`

Preferred initial approach:

- do **not** rewrite `fetch_buffer`,
- reuse it as-is inside `ifetch`.

If implementation proves that the existing registered `pc_inc_2` timing is too awkward, a very small enhancement to `fetch_buffer` may be considered, but only if the same effect cannot be achieved with `ifetch` staging registers.

The default implementation path should assume no functional rewrite of `fetch_buffer`.

## 10. Verification Plan

### 10.1 Unit-Level Tests

Add focused tests for the new `ifetch` module covering:

1. 32-bit instruction response path:
   - D-channel response captured,
   - instruction published,
   - `pc_inc_2=0`.
2. 16-bit compressed instruction response path:
   - instruction decompressed correctly,
   - `pc_inc_2=1`.
3. buffered-halfword behavior across compressed instruction boundaries.
4. flush while waiting on a redirect:
   - stale buffered halfword is discarded,
   - stale staged output is discarded.
5. delayed `consume`:
   - output bundle remains stable until consumed.

Existing `fetch_buffer_test.rs` should remain as the lower-level regression suite for the reused helper module.

### 10.2 CPU-Level Regression Coverage

Re-run and preserve at least the existing control-flow regressions that depend on compressed-width correctness:

- `test_cpu_c_jal_writes_halfword_link_address`
- `test_cpu_jalr_masks_target_and_uses_fallthrough_link_address`
- branch fall-through / redirect tests already present in `cpu_control_flow_test.rs`

Those tests already validate the user-visible effect of `PC+2` vs `PC+4`.

### 10.3 RTL Validation Commands

When the RTL implementation is eventually written, validate with:

```bash
find rtl/common -name '*.sv' -exec verilator --lint-only --Wno-MULTITOP {} +
(cd rtl/fpga && make)
cargo clean
cargo test --verbose
```

## 11. Implementation Sequence

1. Add `ifetch.sv` with the internal FSM and embedded `fetch_buffer`.
2. Add `ifetch`-specific unit tests in Rust/Verilator.
3. Replace the direct `fetch_buffer` usage in `cpu.sv`.
4. Update `S_FETCH` to wait on `ifetch.valid`.
5. Gate `imem_req_internal` using `ifetch.waiting_for_d` (or an equivalent status signal).
6. Re-run fetch-buffer, `ifetch`, and CPU control-flow regressions.
7. Run full RTL lint/synthesis/test validation.

## 12. Risks and Open Questions

### 12.1 Highest-Risk Areas

1. **Duplicate fetch requests**
   - Most likely if `cpu.sv` keeps issuing A-channel requests after the D response has already been captured by `ifetch`.
2. **Incorrect `PC+2` / `PC+4` timing**
   - Most likely if the output width flag no longer matches the same instruction instance that was decompressed.
3. **Flush incompleteness**
   - Most likely if redirect flush clears only the internal `fetch_buffer` state but leaves staged `ifetch` output live.
4. **Off-by-one fetch/consume timing**
   - Most likely if `S_FETCH` transitions before `ifetch` has published a fully registered output bundle.

### 12.2 Open Question: Output Handshake Shape

The implementation should decide explicitly whether `ifetch.valid` is:

- a one-cycle pulse consumed immediately by `cpu.sv`, or
- a held-valid level that remains high until `consume`.

The held-valid contract is preferred because it:

- is easier to reason about,
- matches ready/valid-style conventions better,
- simplifies debugging,
- and decouples `ifetch` timing from the CPU control FSM by one more cycle if needed.

## 13. Definition of Done

The work described by this plan is complete when:

- `ifetch.sv` replaces the old direct `fetch_buffer` integration in `cpu.sv`,
- the CPU no longer depends on the raw D-channel handshake to finish `S_FETCH`,
- the registered `ifetch` outputs drive instruction, validity, and PC increment selection,
- compressed instruction behavior remains unchanged,
- redirect flushes discard stale partial fetch state,
- and the targeted fetch/control-flow regressions pass without widening the architectural behavior of the CPU.
