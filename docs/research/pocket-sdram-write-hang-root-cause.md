# Pocket SDRAM Write Hang Root Cause

**Research Document**  
**Context:** Root-cause analysis of the Pocket SDRAM hang that appeared after write traffic reached `sdram_peripheral`  
**Date:** 2026-04-11

## Executive Summary

The Pocket SDRAM hang was caused by a timing mismatch between the real Analogue Pocket `io_sdram` controller and the assumptions made by `sdram_peripheral`.

The real controller drives `burstwr_ready` as a **registered** FSM output. It is cleared to `0` at the top of every clocked block, then reasserted to `1` while the FSM is in `ST_BURSTWR_3` (`rtl/fpga/cyclonev_analogue_pocket/src/fpga/core/io_sdram.v:170-177,468-489`). Because those are non-blocking assignments, `burstwr_ready` remains high for one extra cycle after a write beat has been accepted and the controller has already moved on to `ST_BURSTWR_4`.

The original `sdram_peripheral` write FSM assumed that seeing `burstwr_ready=1` in its wait state meant it was safe to advance immediately to the next halfword request. That let it consume the stale high level as if it were a fresh acceptance window, so the next beat could be skipped from the controller's point of view. Once that happened, `io_sdram` still had a queued write request pending and re-entered `ST_BURSTWR_3`, but the peripheral had already moved on and never provided the missing strobe. The controller then waited forever, which matched the observed hardware symptom: reads worked until the first write, then later accesses timed out.

The local test wrapper originally modeled `burstwr_ready` too optimistically. It behaved like a combinational ready signal instead of a registered one, so the stale-high window never appeared in simulation. That hid the bug until the design ran on real hardware. The fix was twofold:

1. update `sdram_peripheral` to wait for `burstwr_ready` to drop before issuing the next halfword request (`rtl/common/peripherals/sdram_peripheral.sv:396-417`), and
2. update the test wrapper so its `burstwr_ready` timing matches the real controller (`rtl/common/wrappers/sdram_peripheral_test_wrapper.sv:70-72,107-168`).

## Scope

This writeup focuses on the following files:

- `rtl/fpga/cyclonev_analogue_pocket/src/fpga/core/io_sdram.v`
- `rtl/common/peripherals/sdram_peripheral.sv`
- `rtl/common/wrappers/sdram_peripheral_test_wrapper.sv`
- `testbench/tests/sdram_peripheral_test.rs`

The goal is to document:

- what the real controller does,
- why the original bridge logic failed against it,
- why the old stub masked the issue,
- and what should be considered if `io_sdram` is revised in the future.

## Observed Hardware Behavior

The field symptom was consistent:

1. SDRAM word reads through the host/bus path worked.
2. The first SDRAM word write returned an acknowledge.
3. After that write, later SDRAM reads no longer completed and eventually timed out.

That symptom strongly suggested that the failure was not basic address decoding or read-path corruption. Instead, the write side was leaving the controller in a state where it no longer returned to idle. The confirmed root cause matches that pattern exactly: the controller became stuck waiting for a write beat that the peripheral believed it had already completed.

## Relevant Real `io_sdram` Behavior

### `burstwr_ready` is registered, not combinational

At the top of the main controller `always @(posedge controller_clk)` block, `io_sdram` clears `burstwr_ready` every cycle:

```verilog
burstwr_ready <= 0;
```

(`rtl/fpga/cyclonev_analogue_pocket/src/fpga/core/io_sdram.v:170-177`)

Later, in `ST_BURSTWR_3`, it reasserts `burstwr_ready`:

```verilog
ST_BURSTWR_3: begin
    burstwr_ready <= 1;
    ...
    if (burstwr_strobe | burstwr_done) begin
        state <= ST_BURSTWR_4;
    end
end
```

(`rtl/fpga/cyclonev_analogue_pocket/src/fpga/core/io_sdram.v:468-489`)

Because both assignments are registered, the signal is effectively:

- high while the FSM sits in `ST_BURSTWR_3`, and
- still high for one extra cycle immediately after the strobe/done edge that causes the transition out of `ST_BURSTWR_3`.

That extra cycle is the critical stale-ready window.

### Write requests are queued even while the FSM is busy

`io_sdram` also unconditionally captures incoming `burstwr` pulses into `burstwr_queue` whenever its FSM is busy:

```verilog
if (burstwr) begin
    burstwr_queue <= 1;
end
```

(`rtl/fpga/cyclonev_analogue_pocket/src/fpga/core/io_sdram.v:536-548`)

That queueing behavior is important. It means a premature next request is not discarded outright. Instead, the controller can remember it and later re-enter the burst-write path expecting a corresponding data strobe.

## Original `sdram_peripheral` Assumption

The peripheral translates 32-bit CPU-visible writes into 16-bit SDRAM halfword writes. Its write data ordering is intentionally derived from `io_sdram`'s read-side packing (`rtl/common/peripherals/sdram_peripheral.sv:208-233`), and the actual write-side request/accept loop lives in the clocked FSM (`rtl/common/peripherals/sdram_peripheral.sv:303-430`).

The pre-fix bug was in the transition between write beats. Once `S_WRITE_WAIT_READY` observed `burstwr_ready`, the FSM could immediately advance toward the next request. That worked if `burstwr_ready` behaved like a pulse that disappeared before the next beat was considered. It did **not** work with the real registered behavior, because the next loop iteration could re-enter the wait state while the stale-ready cycle was still present.

The current fixed code makes the intended protection explicit:

```systemverilog
S_WRITE_WAIT_READY: begin
    if (burstwr_ready) begin
        ...
        write_halfword_index_reg <= write_halfword_index_reg + 3'd1;
        state <= S_WRITE_WAIT_READY_LOW;
    end
end

S_WRITE_WAIT_READY_LOW: begin
    if (!burstwr_ready) begin
        state <= S_WRITE_REQ;
    end
end
```

(`rtl/common/peripherals/sdram_peripheral.sv:396-417`)

That extra state was not present in the buggy version.

## Failure Sequence

For an aligned 32-bit write, `sdram_peripheral` emits two 16-bit SDRAM beats.

### Beat 0 works

1. `sdram_peripheral` enters `S_WRITE_REQ` and asserts `burstwr` with the halfword address (`rtl/common/peripherals/sdram_peripheral.sv:390-394`).
2. `io_sdram` eventually reaches `ST_BURSTWR_3` and asserts `burstwr_ready` (`rtl/fpga/cyclonev_analogue_pocket/src/fpga/core/io_sdram.v:468-489`).
3. The peripheral sees ready, drives `burstwr_strobe`, `burstwr_done`, and `burstwr_data`, and beat 0 is accepted (`rtl/common/peripherals/sdram_peripheral.sv:396-409`).

### Beat 1 is mis-timed

Because `burstwr_ready` is registered, it remains high during the next cycle even though `io_sdram` has already moved on to `ST_BURSTWR_4`. In the buggy implementation, the peripheral could treat that stale high level as permission for the next beat:

1. the peripheral advanced its write-halfword index,
2. reissued `burstwr` for the next halfword,
3. and then saw `burstwr_ready=1` again before the controller had returned to a real accept state.

At that point, `io_sdram` was no longer in `ST_BURSTWR_3`, so the next `burstwr_strobe` was not accepted as a real beat. However, the `burstwr` request itself had already been captured by `burstwr_queue` (`rtl/fpga/cyclonev_analogue_pocket/src/fpga/core/io_sdram.v:536-548`).

### The controller wedges

Later, after the cleanup states complete, `io_sdram` services the queued request:

```verilog
if (burstwr_queue) begin
    burstwr_queue <= 0;
    addr <= burstwr_addr;
    state <= ST_BURSTWR_0;
end
```

(`rtl/fpga/cyclonev_analogue_pocket/src/fpga/core/io_sdram.v:321-330`)

That takes the controller back to `ST_BURSTWR_3`, where it asserts `burstwr_ready` and waits for `burstwr_strobe | burstwr_done` (`rtl/fpga/cyclonev_analogue_pocket/src/fpga/core/io_sdram.v:468-489`). But the peripheral already thinks that second beat completed earlier, so it never sends the missing handshake. The controller remains stuck in the write FSM and later reads time out because the SDRAM path never returns to idle.

## Why the Old Stub Missed It

The test wrapper now includes an explicit note about the issue:

```systemverilog
// Mirror io_sdram's registered ready behavior so ready can remain high for one
// cycle after the strobe that ends the current write window.
```

(`rtl/common/wrappers/sdram_peripheral_test_wrapper.sv:70-72`)

The current wrapper drives `burstwr_ready` from a clocked process:

```systemverilog
burstwr_ready <= burstwr_pending && (burstwr_wait_cycles_remaining == 2'd0);
```

(`rtl/common/wrappers/sdram_peripheral_test_wrapper.sv:107-124`)

That registered assignment matches the key property of the real controller: ready can stay high for one cycle after the strobe that ends the current window.

The old wrapper behavior differed in a critical way. It did not preserve that registered stale-high cycle, so as soon as a beat completed, ready disappeared immediately from the DUT's point of view. That meant the peripheral never encountered the hazardous sequence that existed on real hardware. Simulation therefore validated an easier protocol than the one the Pocket target actually implemented.

## Why the Fix Works

The fix adds `S_WRITE_WAIT_READY_LOW` between accepted beats (`rtl/common/peripherals/sdram_peripheral.sv:36-45,396-417`).

That changes the contract from:

- "if ready is high, the next beat may be requested immediately"

to:

- "after a beat is accepted, wait until ready has gone low before requesting the next beat"

This is exactly what the real `io_sdram` protocol requires. The extra state drains the stale-ready cycle before the peripheral re-enters `S_WRITE_REQ`, so the next request cannot be mistaken for an acceptance in the previous window.

In effect:

1. beat N is accepted,
2. the peripheral waits for the old ready level to disappear,
3. then it requests beat N+1,
4. and only a fresh `ST_BURSTWR_3` assertion can accept that next beat.

That guarantees each halfword is accepted exactly once and prevents `io_sdram` from being left behind waiting for a beat that the bridge has already retired internally.

## Regression Coverage

The Rust regression already includes targeted SDRAM tests in `testbench/tests/sdram_peripheral_test.rs`. The write-side backpressure case is especially relevant because it verifies that a word write still completes when write readiness is delayed and that the written data can be read back successfully afterward (`testbench/tests/sdram_peripheral_test.rs:292-320`).

With the wrapper now modeling registered ready timing, this regression is much closer to the real hardware interface and should catch similar stale-ready bugs in the future.

## Future `io_sdram` Considerations

If the project later decides to adjust `io_sdram`, this investigation highlights the key interface question:

### Option 1: keep the current controller behavior

If `io_sdram` continues to expose registered `burstwr_ready` semantics, then clients must continue to treat `burstwr_ready` as a level that may remain high for one cycle after acceptance. The current `sdram_peripheral` fix is correct for that contract.

### Option 2: tighten `io_sdram` to a cleaner pulse-style interface

If a future revision of `io_sdram` wants a less error-prone client interface, it could be changed so that `burstwr_ready` deasserts immediately once a beat is accepted or so that acceptance is encoded differently. Any such change would need a careful audit of:

- `sdram_peripheral`,
- the test wrapper,
- and any other future `burstwr_*` client

to ensure they all agree on whether `burstwr_ready` is:

- a level,
- a pulse,
- or a ready/valid-style handshake with stricter same-cycle semantics.

### Option 3: document the current interface explicitly

Even if `io_sdram` itself is not changed, the project should consider treating the current behavior as a documented protocol rule:

> `burstwr_ready` is registered and may remain high for one cycle after the strobe/done edge that closes the current write window.

That single sentence would make the requirement obvious to any future client or test model and would have made this failure much easier to avoid.

## Bottom Line

The bug was not a random hardware instability. It was a precise protocol mismatch.

- The real Pocket `io_sdram` controller exposes a registered `burstwr_ready` signal with a stale-high cycle.
- The original `sdram_peripheral` assumed the next beat could be issued without first waiting for ready to fall.
- The original test wrapper failed to reproduce that stale-high behavior, so the bug stayed hidden in simulation.

The implemented fix resolves the immediate hardware problem, and this writeup should serve as the reference point if the project later chooses to simplify or formally document the `io_sdram` write-side contract.
