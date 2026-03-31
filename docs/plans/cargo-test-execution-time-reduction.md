# Cargo Test Execution Time Reduction Plan

## 1. Goal

Reduce Rust workspace `cargo test` wall-clock time without reducing coverage or changing test semantics.

This plan focuses on the parts of the workspace that dominate test cost today:

- `testbench` integration tests
- `device-runtime` simulation-backed integration tests
- the nested `rust-test-program` build triggered through `sim-tests`

---

## 2. Non-Goals

- Do not delete tests or reduce functional coverage.
- Do not change RTL behavior to make tests faster.
- Do not depend on marlin workarounds meant only for the old thread-safety bug.
- Do not optimize CI-only steps unless they also improve the local `cargo test` experience.

---

## 3. Current State Snapshot (verified)

### 3.1 `testbench` is structurally expensive

`testbench` is an integration-test crate, and its manifest explicitly states that files under
`testbench/tests/` run as separate binaries:

- `testbench/Cargo.toml`

That means the suite pays repeated compile/link/process-startup overhead across many small test files.

### 3.2 Many test files repeatedly create fresh marlin/Verilator runtimes

Representative examples:

- `testbench/tests/fpu_test.rs` creates a new FPU runtime via `create_fpu_runtime()` in many tests.
- `testbench/tests/alu_test.rs` creates a new ALU runtime in separate tests.
- `testbench/tests/uart_test.rs` creates a new UART runtime repeatedly across the file.

This pattern amplifies runtime setup and model creation overhead inside already-separate integration binaries.

### 3.3 All marlin-backed runtimes currently share one artifact root

All `create_*_runtime()` helpers in `riscv_core` funnel through the shared helper in
`riscv_core/src/lib.rs`, which constructs the runtime under:

```rust
"target/verilator"
```

This shared root should now be treated as a cache/reuse opportunity, not something to avoid by default,
assuming the repository's pinned marlin dependency in `riscv_core/Cargo.toml` has been updated to a
revision that includes the upstream thread-safety fix discussed in `docs/research/marlin-parallel-test-instability.md`.

### 3.4 `device-runtime` already uses an aggressive Verilator optimization level

The simulator helper in `device-runtime/src/sim/sim_core.rs` uses Verilator optimization level `3`
for model creation. That likely helps long-running simulation speed, but it also means the most obvious
optimization-level wins are more likely in direct `testbench` runtime creation than in `device-runtime`.

### 3.5 `sim-tests` performs a nested `cargo build --release`

`sim-tests/build.rs` shells out to `cargo build --release` in `rust-test-program/`, then copies the
resulting binaries into `OUT_DIR`. The build script already has `rerun-if-changed` directives, but the
nested build is still a meaningful contributor whenever that build script is re-invoked.

---

## 4. Bottlenecks to Address First

Ranked by expected impact and implementation risk:

1. **Repeated runtime/model setup inside `testbench` files**
2. **Large number of separate integration-test binaries in `testbench/tests/`**
3. **Lack of explicit performance tuning for direct marlin-backed testbench runtimes**
4. **Nested `rust-test-program` build in `sim-tests/build.rs`**
5. **Untuned test-process parallelism after the marlin fix**

---

## 5. Implementation Strategy

### Phase 1 — Benchmark first and establish a repeatable measurement workflow

Before changing structure, capture clean and incremental timings for:

```bash
cargo clean
time cargo test
time cargo test -p testbench
time cargo test -p device-runtime
```

Then capture incremental timings for high-cost suites:

```bash
time cargo test -p testbench --test fpu_test
time cargo test -p testbench --test alu_test
time cargo test -p testbench --test uart_test
```

### Deliverables

- A small benchmark table checked into the implementation PR description or follow-up research notes
- A clear baseline for clean vs incremental runs

### Exit criteria

- We can identify whether clean-build time or repeated test execution time is dominating
- We can compare each later phase against a stable baseline

---

### Phase 2 — Remove repeated runtime creation in the highest-cost `testbench` files

Convert the worst repeated-runtime test files to create one shared runtime per test binary instead of
creating a fresh runtime inside every `#[test]`.

### Initial target files

- `testbench/tests/uart_test.rs`
- `testbench/tests/fpu_test.rs`
- `testbench/tests/alu_test.rs`
- `testbench/tests/fpu_submodule_test.rs`
- `testbench/tests/system_controller_test.rs`

### Approach

- Add a small shared-runtime helper pattern using `OnceLock` in a common test utility location.
- Keep model instances per test, but reuse the underlying `VerilatorRuntime` where safe.
- Require every converted test to begin from an explicit reset path so shared setup does not leak state.

### Why this phase comes first

It preserves the current test layout while directly attacking the most obvious duplicated work.

### Exit criteria

- The targeted files no longer create a fresh runtime per individual test case
- Targeted suite timings improve measurably versus the Phase 1 baseline
- No ordering-dependent failures are introduced

---

### Phase 3 — Reduce `testbench` binary fragmentation

After runtime reuse is in place, regroup the most closely related `testbench/tests/*.rs` files into a
smaller number of subsystem-oriented integration binaries.

### Proposed grouping direction

- Arithmetic / ALU / multiplier / divider wrappers
- FPU and FPU submodule wrappers
- UART / host-bus / FIFO communication wrappers
- Small primitive and utility wrappers

### Rules

- Keep files small enough to stay understandable
- Only merge tests that already share the same DUT family and helper patterns
- Avoid giant “everything” binaries that make failures hard to isolate

### Why this phase is second

Even with runtime reuse, each integration test file still incurs separate compile/link/startup cost.
Merging the most related files should reduce that overhead without changing test meaning.

### Exit criteria

- The number of `testbench` integration binaries is materially smaller
- The reorganized files remain readable and locally runnable
- Per-suite wall-clock time improves again versus the post-Phase-2 baseline

---

### Phase 4 — Tune direct testbench marlin/Verilator settings

Investigate whether `riscv_core::create_runtime()` should expose an explicit optimization choice for
direct `testbench` runtimes instead of always using `VerilatorRuntimeOptions::default()`.

### Specific work

- Check what `VerilatorRuntimeOptions::default()` maps to in the marlin revision currently pinned in
  `riscv_core/Cargo.toml`
- Add an internal path to benchmark at least two configurations for direct testbench use
- Prefer a configuration that improves total wall-clock test time, not just raw simulated-cycle speed

### Important constraint

`device-runtime` already uses optimization level 3, so this phase is primarily about the direct
testbench wrapper path in `riscv_core`, not the simulator backend.

### Exit criteria

- We have benchmarked at least two option sets for direct testbench runtimes
- The chosen setting reduces total suite time without introducing instability

---

### Phase 5 — Reduce unnecessary nested rebuild work in `sim-tests`

Review `sim-tests/build.rs` and reduce avoidable rebuild work around the nested
`cargo build --release` for `rust-test-program`.

### Specific work

- Confirm which changes actually trigger the build script today
- Avoid unnecessary repeated work when the expected `rust-test-program` artifacts are already current
- Keep correctness first: never reuse stale test-program binaries silently

### Exit criteria

- `device-runtime`-related test runs do not spend avoidable time rebuilding `rust-test-program`
- The guard logic is still easy to reason about and safe for CI

---

### Phase 6 — Tune parallel execution after the marlin fix

Now that the marlin thread-safety issue has been fixed upstream, re-evaluate test-process parallelism
instead of assuming conservative execution is best.

### Specific work

- Benchmark plain `cargo test` with different job/thread settings on representative hardware
- If helpful, evaluate `cargo nextest` as a follow-up execution model
- Favor moderate parallelism that improves throughput without oversubscribing Verilator-heavy runs

### Important constraint

Do not revert to per-worker artifact directories as a first-choice optimization. Shared
`target/verilator` reuse is now potentially beneficial.

### Exit criteria

- The project has a documented recommended local and CI test command
- Parallel settings are chosen based on measured performance, not guesswork

---

## 6. Validation Plan

After each phase:

1. Re-run the same commands captured in Phase 1.
2. Compare clean-build and incremental timings.
3. Run the affected targeted suites first.
4. Before landing the final implementation, run the full Rust workspace test command again.

Primary validation commands:

```bash
cargo test -p testbench
cargo test -p device-runtime
cargo test
```

If a phase changes Rust source, also run the normal Rust quality gates required by the repository.

---

## 7. Risks and Mitigations

### Risk 1 — Shared runtime helpers introduce cross-test state leakage

**Mitigation:** keep model instances test-local, require explicit reset/setup at the start of every
test, and convert only a few high-value files first.

### Risk 2 — Merging test files hurts debuggability

**Mitigation:** merge by subsystem only, and keep the resulting files scoped and readable.

### Risk 3 — Faster model compile settings slow down simulated execution, or vice versa

**Mitigation:** treat optimization changes as benchmark-driven configuration work rather than a
blind “higher is always better” change.

### Risk 4 — `sim-tests` rebuild suppression accidentally reuses stale artifacts

**Mitigation:** keep the trigger logic conservative and verify it against clean and incremental runs.

### Risk 5 — More parallelism oversubscribes Verilator-heavy workloads

**Mitigation:** measure on real hardware/CI runners and prefer a documented moderate setting.

---

## 8. Definition of Done

This initiative is complete when:

1. The project has a measured before/after comparison for Rust workspace `cargo test`.
2. The highest-cost repeated-runtime `testbench` files reuse runtimes safely.
3. `testbench` runs through fewer integration binaries than it does today.
4. Direct testbench marlin/Verilator settings have been benchmarked and intentionally chosen.
5. `sim-tests` no longer performs avoidable rebuild work during normal test iteration.
6. The repository documents a recommended test execution strategy for both local development and CI.

---

## 9. Recommended Execution Order

Implement in this order:

1. **Phase 1** — benchmark and baseline
2. **Phase 2** — shared runtime reuse in high-cost `testbench` files
3. **Phase 3** — reduce `testbench` binary count
4. **Phase 4** — tune direct testbench runtime options
5. **Phase 5** — optimize `sim-tests` nested build behavior
6. **Phase 6** — finalize parallel execution strategy

This order keeps the work concrete, measurable, and reversible while prioritizing the most likely
sources of real wall-clock improvement.
