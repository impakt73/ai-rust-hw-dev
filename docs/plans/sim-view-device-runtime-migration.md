# sim-view Runtime Migration Plan (cpu-sim thread → device-runtime)

## 1. Goal

Enable `sim-view` to run against either:

1. software simulation (`DeviceRuntimeType::Sim`), or
2. real FPGA hardware (`DeviceRuntimeType::Fpga`)

by replacing `sim-view`'s direct `cpu_sim::InteractiveSimulator` execution thread with a runtime layer driven by `device-runtime`.

The end state should make backend selection a runtime concern while preserving existing viewer behavior (state transitions, frame/audio capture, stepping semantics, and headless testability).

---

## 2. Verified Current State (repository snapshot)

### 2.1 sim-view coupling and execution model

- `sim-view` currently depends directly on `cpu-sim` (`sim-view/Cargo.toml`).
- `SimViewer::new` creates `InteractiveSimulator`, registers `Video` and `Audio` devices, then spawns `SimulationThread` (`sim-view/src/viewer.rs`).
- `SimulationThread` owns `InteractiveSimulator` and processes internal request/response enums (`SimRequest` / `SimResponse`) (`sim-view/src/simulation_thread.rs`).
- Execution model is command-driven (`LoadELF`, `Run`, `Step`, `Pause`, `Resume`, `Terminate`) with periodic `Progress` updates.

### 2.2 device-runtime capability baseline

- `device-runtime` provides backend selection via `create_device_runtime(DeviceRuntimeType, Option<Vec<BusDeviceRegistration>>) -> Box<dyn DeviceRuntime>`.
- `DeviceRuntime` already exposes backend-agnostic primitives needed by viewer orchestration:
  - `load_elf`
  - `boot_cpu`
  - `poll`
  - `send_host_request`
  - `reset`
- Sim backend is already threaded internally (`SimDeviceRuntime`) and supports custom bus device registration before startup reset.

### 2.3 binary location caveat

The workspace currently defines `sim-view` binary in `sim-view/Cargo.toml`, not in `cpu-sim`.

Interpretation for this migration:
- preserve intent (detach viewer runtime from cpu-sim internals), and
- relocate executable ownership to `device-runtime` without introducing dependency cycles.

---

## 3. Architectural Constraints and Non-Negotiables

1. **No dependency cycle:** if `sim-view` depends directly on `device-runtime`, then `device-runtime` cannot also depend on `sim-view`.
2. **Behavior parity:** preserve existing viewer semantics:
   - idle/running/paused/halted states,
   - frame-step behavior,
   - max-cycle halting,
   - audio config propagation in both `step()` and `run()` loops.
3. **Backend-agnostic UI logic:** GUI/headless backends should remain independent of Sim vs FPGA transport.
4. **Device registration fidelity:** existing video/audio bus device registration must remain in place for simulation backend and be explicitly documented for FPGA behavior.
5. **Error continuity:** maintain user-facing error quality (timeouts, disconnects, protocol failures).

---

## 4. Target Architecture

## 4.1 Runtime ownership split

Introduce a runtime-control boundary in `sim-view`:

- `sim-view` owns **viewer logic + UI/audio/video backends + shared buffers**.
- runtime execution is behind an internal adapter trait (e.g., `ViewerRuntime`), not hardcoded to `InteractiveSimulator`.
- one adapter implementation drives a `Box<dyn DeviceRuntime>` worker loop.

This allows sim-view library code to remain transport-agnostic.

## 4.2 Worker-loop semantics (device-runtime-backed)

Replace simulator-thread internals with runtime polling logic:

- `LoadELF(path)`:
  1. runtime `reset` (mode chosen by policy)
  2. `load_elf(path)`
  3. `boot_cpu(entry)`
- `Step`:
  - run bounded polling/advancement window and return cycle delta + optional halt/termination signal.
- `Run`:
  - continuous polling loop, periodic progress notifications, stop on halt/termination/max cycles.
- `Pause` / `Resume`:
  - control worker loop state only (not device reset).

## 4.3 Event translation contract

Define explicit mapping from `BusEvent` to viewer-visible signals:

- `TohostTermination` → halt condition (`RunCompleted`/`StepCompleted` with `tohost_value`).
- host response/read/write events → internal bookkeeping or ignored unless needed.
- `poll()` fatal errors/timeouts → `SimResponse::Error(...)` equivalent.

## 4.4 Binary ownership

Final binary should be owned by `device-runtime` crate (new `[[bin]]` target), while `sim-view` becomes library-only UI/runtime orchestration surface.

To avoid cycles, choose one of:

- **Preferred:** move minimal viewer CLI wiring into a new crate if needed (clean graph), or
- keep CLI in `sim-view` temporarily and defer binary relocation to final phase after dependency split.

If strict relocation to `device-runtime` is required immediately, first make `sim-view` independent of `device-runtime` concrete types via trait-only boundary and inject runtime factory from the binary layer.

---

## 5. Detailed Implementation Phases

## Phase 0 — Safety rails and inventory

1. Snapshot behavior with targeted existing tests:
   - `cargo test -p sim-view --tests`
   - `cargo test -p sim-view`
2. Document current command/state semantics from:
   - `sim-view/src/viewer.rs`
   - `sim-view/src/simulation_thread.rs`
3. Capture current CLI flags and defaults from `sim-view/src/main.rs`.

## Phase 1 — Introduce runtime abstraction in sim-view

1. Add internal runtime trait and runtime response types in `sim-view` (minimal API mirroring current `SimRequest`/`SimResponse` behavior).
2. Refactor `SimViewer` to depend on trait object/factory instead of concrete `InteractiveSimulator` construction.
3. Keep existing simulation-thread implementation as temporary adapter to avoid large, risky jump.

Deliverable: no user-visible behavior change; compile + tests pass.

## Phase 2 — Build device-runtime adapter for sim-view

1. Implement new worker module that owns `Box<dyn DeviceRuntime>`.
2. Reimplement request handling (`LoadELF`, `Run`, `Step`, `Pause`, `Resume`, `Terminate`) on top of `DeviceRuntime` calls.
3. Preserve frame/audio callback pipelines by continuing to register `Video`/`Audio` devices through runtime creation (`BusDeviceRegistration`) for `Sim` backend.
4. Add explicit timeout handling and deterministic error mapping.

Deliverable: `sim-view` works with `DeviceRuntimeType::Sim` without `InteractiveSimulator` direct usage in viewer code.

## Phase 3 — Backend selection plumbing (Sim vs FPGA)

1. Add runtime configuration model in CLI/runtime factory:
   - default `Sim`
   - optional FPGA selection (device path, baud, startup reset).
2. Thread backend selection through viewer initialization without touching UI backends.
3. Validate that non-sim bus-event characteristics do not break viewer loop assumptions.

Deliverable: same viewer binary can run against sim or FPGA by configuration.

## Phase 4 — Binary relocation to device-runtime ownership

1. Remove `[[bin]]` from `sim-view/Cargo.toml` (library-only crate).
2. Add `device-runtime` binary target that wires:
   - CLI args,
   - backend selection,
   - sim-view GUI/headless backend construction,
   - runtime factory injection.
3. Ensure workspace command parity (document replacement for `cargo run -p sim-view ...`).

Deliverable: executable target is owned by `device-runtime`.

## Phase 5 — Test migration and parity hardening

1. Update `sim-view/tests/headless_integration.rs` setup to use runtime-injected constructor.
2. Add backend-parameterized integration coverage:
   - always run `Sim`
   - optionally gate FPGA runs with env vars.
3. Add targeted tests for subtle regressions:
   - pause/resume semantics,
   - step-frames stability,
   - audio config updates during both `step()` and `run()`,
   - tohost termination detection in both backends.

## Phase 6 — Cleanup and docs

1. Remove obsolete cpu-sim coupling code and imports.
2. Update `sim-view/README.md` execution instructions and backend selection docs.
3. Add troubleshooting notes for FPGA mode (serial config, expected latency differences).

---

## 6. File-by-File Change Map (planned)

- `sim-view/src/simulation_thread.rs`
  - replace `InteractiveSimulator` implementation with `DeviceRuntime`-driven worker or adapter layer.
- `sim-view/src/viewer.rs`
  - stop constructing `InteractiveSimulator` directly; consume runtime factory.
- `sim-view/src/main.rs`
  - either removed (if binary relocated immediately) or reduced to compatibility shim.
- `sim-view/Cargo.toml`
  - remove `cpu-sim` dependency once migration complete; adjust bin/lib targets.
- `device-runtime/Cargo.toml`
  - add binary target and any required CLI/log dependencies.
- `device-runtime/src/bin/sim-view.rs` (new)
  - binary entrypoint with backend selection and viewer bootstrap.
- `sim-view/tests/headless_integration.rs`
  - adapt constructors and add/adjust backend-aware assertions.
- `sim-view/README.md`
  - update usage/build commands and architecture notes.

---

## 7. Subtle Behavior Risks and Mitigations

1. **Cycle accounting drift** (runtime poll cadence differs from `step_instruction` loop)
   - Mitigation: define one canonical “step quantum” and verify against existing `max_cycles` tests.

2. **Termination semantics mismatch** (`TohostTermination` timing vs prior return path)
   - Mitigation: centralize termination detection in adapter and keep one response contract for viewer.

3. **Audio/video callback timing differences** (especially FPGA mode)
   - Mitigation: preserve buffer-based callbacks; keep UI loop decoupled from runtime transport latency.

4. **Pause/resume race conditions** due to asynchronous poll loop
   - Mitigation: command acknowledgments and clear worker-state transitions.

5. **Dependency graph breakage during binary move**
   - Mitigation: perform abstraction split before ownership relocation; use phased commits.

---

## 8. Validation Matrix

## 8.1 Required command set per phase

- `cargo fmt -- --check`
- `cargo clippy --fix --allow-dirty` (for Rust code-change phases)
- `cargo clippy -- -D warnings`
- `cargo test -p sim-view`
- `cargo test -p sim-view --test headless_integration`
- `cargo test -p device-runtime`

## 8.2 Manual validation (for agent execution checklist)

1. Run headless mode with known ELF and confirm frame/audio capture count is non-zero.
2. Verify pause/resume and step-frames behavior via injected test commands.
3. In FPGA mode, verify program load + boot + halt with expected tohost value.
4. Confirm progress logging and window title performance string continue updating.

---

## 9. Definition of Done

Migration is complete when all are true:

1. `sim-view` runtime path no longer directly depends on `cpu_sim::InteractiveSimulator`.
2. Runtime execution is driven through `device-runtime` abstractions.
3. `sim-view` functionality can be launched against both Sim and FPGA backends.
4. Binary ownership is moved to `device-runtime` (or an explicitly approved equivalent crate with no dependency cycle).
5. Existing headless integration behavior remains green, and backend-specific regressions are covered by tests.
