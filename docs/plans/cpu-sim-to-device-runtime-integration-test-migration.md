# CPU-Sim → Device-Runtime Integration Test Migration Plan

## 1. Goal

Migrate the largest practical subset of integration tests currently living in `cpu-sim/tests` into `device-runtime/tests`, so the same tests can execute against:

1. `DeviceRuntimeType::Sim` (fast local/CI simulation), and
2. `DeviceRuntimeType::Fpga` (real hardware validation).

The success target is to move enough tests that **the majority of integration coverage no longer depends on cpu-sim-only APIs** and can be run on FPGA.

---

## 2. Current State Snapshot (verified against repository)

### 2.1 Test inventory (today)

- `cpu-sim/tests`: **94** integration tests
- `device-runtime/tests`: **51** integration tests

Largest concentration inside `cpu-sim/tests`:

| File | Tests |
|---|---:|
| `test_rtl_verification.rs` | 29 |
| `tests.rs` | 20 |
| `test_memory_bounds.rs` | 12 |
| `test_interactive_simulator.rs` | 10 |
| `test_minimal.rs` | 7 |
| `test_memory_latency.rs` | 4 |
| `test_device_lifecycle.rs` | 3 |

### 2.2 Why full migration is still blocked

Phase 0 has already landed: `device-runtime/tests/common/mod.rs` exists and is used by
`test_load_program.rs`, `test_reset.rs`, and migrated suites.

The remaining migration blockers are now concentrated in cpu-sim-only patterns:

- heavy use of `run_elf(...)` in `cpu-sim/tests` with `sim-tests::test_program_path(...)`
- setup/teardown callbacks with `SimulatorView`
- direct simulation introspection (`led_out()`, FIFO read/write helpers, trace/vcd/hung detector checks)
- simulator-only device registration (`register_device(...)`)
- simulator memory debug access patterns

`device-runtime` already exposes backend-agnostic primitives (`load_program`, `load_elf`,
`boot_cpu`, `send_host_request`, `poll`, `reset`) and a reusable test harness layer.
What is still missing is an ELF-focused helper workflow in `device-runtime/tests/common`.

---

## 3. Migration Strategy

### 3.1 Core principle

Move tests to a **backend-agnostic runtime harness** in `device-runtime/tests/common` and avoid any dependency on `SimulatorView` internals.

Each migrated test must run unchanged for both backends:

- default: `Sim`
- optional FPGA mode selected by env vars (`FPGA_DEVICE_PATH`, `FPGA_BAUD_RATE`)

### 3.2 Categorize cpu-sim tests by portability

#### Tier A — High-value, high-portability (implemented in recent PRs)

These are mostly instruction/peripheral behaviors that can be expressed via load/boot/poll/host-bus operations:

- ✅ `test_led_peripheral.rs` (7) migrated to `device-runtime/tests`
- ✅ `test_host_initiated_requests.rs` (6) migrated to `device-runtime/tests`
- ✅ `test_rv32c_basic.rs` (9) migrated to `device-runtime/tests`
- ✅ `test_fp_integration.rs` (15) migrated to `device-runtime/tests`
- ⚠️ `test_rtl_verification.rs` partial migration complete in `device-runtime/tests` (10 tests)

**Implemented Tier A subtotal in `device-runtime/tests`: 47 tests**

#### Tier B — Portable with harness extensions (next)

- `test_memory_bounds.rs` (12)
- `test_programmatic_memory.rs` (2)
- selected cases from `tests.rs` that only require tohost + host-bus interaction

Requires additional helper APIs for host-side memory assertions and migration of portable subsets.

#### Tier B-ELF — Portable ELF-based tests (new requirement)

- `test_byte_enable.rs` (2)
- `test_simple_byte_store.rs` (1)
- portable subset of `test_minimal.rs` (currently 7)
- `test_alloc_only.rs` (1)

These are strong candidates for migration once ELF helpers are added to
`device-runtime/tests/common` (see Phase 2 below).

#### Tier C — Keep in cpu-sim (simulation-internal by design)

- `test_interactive_simulator.rs` (interactive API contract)
- `test_device_lifecycle.rs` (custom `BusDevice` lifecycle hooks)
- trace/vcd/hung-detector validation from `tests.rs`
- tests that depend on Rust-only peripherals (`test_audio.rs`, `test_video.rs`, `test_dma.rs`, parts of `test_minimal.rs`)

These validate simulator internals, not hardware/runtime portability.

---

## 4. Target Architecture for Migrated Tests

### 4.1 New shared test harness (`device-runtime/tests/common/mod.rs`)

Implemented reusable helpers:

1. `create_test_runtime()`
   - chooses FPGA or Sim backend via env
2. `load_and_boot(runtime, boot_pc, program_bytes)`
3. `wait_for_tohost(runtime, timeout) -> u32`
4. `wait_for_cpu_halt(runtime, timeout) -> Option<u32>`
5. `wait_for_host_read_response(...)` / `wait_for_host_write_response(...)`
6. `read_word_with_timeout(...)`, `write_word_with_timeout(...)`
7. `drain_events_until_idle(...)` for deterministic sequencing
8. standardized timeout constants (short/medium/long)

This removes duplicated polling loops currently repeated across tests.

### 4.2 Program builders

Implemented reusable instruction-to-bytes helpers in device-runtime test common:

- `instructions_to_bytes(...)`
- `tohost_termination(...)`
- `append_tohost_termination(...)`

Reuse conventions already present in `cpu-sim/tests/common/mod.rs`.

### 4.3 ELF program support (required for next migration stage)

Add test helpers that make ELF-based tests backend-agnostic:

1. `load_and_boot_elf(runtime, elf_path) -> u32` (returns ELF entry point)
2. `run_elf_until_halt(runtime, elf_path, timeout) -> Option<u32>`
3. optional helper for resolving test ELF paths via `sim-tests`

### 4.4 Test style contract for portability

Migrated tests must:

- assert via tohost code and host-bus reads (no simulator internals)
- avoid direct cycle-precise assumptions unless architecturally required
- use bounded polling loops with explicit timeout diagnostics
- pass on both Sim and FPGA backends (with backend-specific timeout multipliers if needed)

---

## 5. Phased Implementation Plan (updated)

### Phase 0: Foundation (completed)

1. ✅ `device-runtime/tests/common/mod.rs` harness added.
2. ✅ `test_load_program.rs` and `test_reset.rs` refactored to use common helpers.
3. ✅ Shared timeout/runtime/host polling helpers established.

### Phase 1: Tier A ISA/peripheral migration (mostly completed)

1. ✅ Migrated: `test_led_peripheral.rs`, `test_host_initiated_requests.rs`, `test_rv32c_basic.rs`, `test_fp_integration.rs`.
2. ⚠️ Partial: `test_rtl_verification.rs` migrated subset (10 tests); remaining tests still in `cpu-sim/tests/test_rtl_verification.rs`.
3. ✅ CPU-sim files fully migrated in Phase 1 were removed from `cpu-sim/tests`.

### Phase 2: ELF migration enablement (new)

1. Add ELF-focused helpers in `device-runtime/tests/common` (`load_and_boot_elf`, `run_elf_until_halt`, and path-resolution helper as needed).
2. Add `sim-tests` as a `device-runtime` dev-dependency if needed for ELF path discovery.
3. Migrate portable ELF suites first:
   - `test_byte_enable.rs`
   - `test_simple_byte_store.rs`
   - portable cases from `test_minimal.rs`
   - `test_alloc_only.rs`
4. Keep simulator-internal ELF tests in `cpu-sim/tests` (audio/video/dma/interactive internals).

### Phase 3: Migrate Tier B non-ELF with targeted harness growth

1. Add host-memory helper coverage needed for `test_memory_bounds.rs` and `test_programmatic_memory.rs`.
2. Port only tests that remain backend-agnostic on Sim + FPGA.
3. Selectively migrate portable `tests.rs` cases; keep simulator-contract checks in cpu-sim.

### Phase 4: CI and execution model changes

1. Make `device-runtime` integration tests the primary portability gate.
2. Keep cpu-sim integration tests for simulator-specific contracts.
3. Add optional FPGA workflow/job for migrated suites (likely nightly/manual trigger).
4. Publish test matrix summary in CI logs:
   - portable tests pass in Sim
   - portable tests pass on FPGA (when hardware runner available)

---

## 6. Recommended File-by-File Migration Matrix

| Source file (`cpu-sim/tests`) | Plan | Destination in `device-runtime/tests` |
|---|---|---|
| `test_rtl_verification.rs` | Partial migration complete; continue chunked migration | `test_rtl_verification_*.rs` or existing file |
| `test_fp_integration.rs` | ✅ Migrated (cpu-sim source removed) | `test_fp_integration.rs` |
| `test_rv32c_basic.rs` | ✅ Migrated (cpu-sim source removed) | `test_rv32c_basic.rs` |
| `test_led_peripheral.rs` | ✅ Migrated (cpu-sim source removed) | `test_led_peripheral.rs` |
| `test_host_initiated_requests.rs` | ✅ Migrated (cpu-sim source removed) | `test_host_initiated_requests.rs` |
| `test_byte_enable.rs` | Migrate in ELF phase | `test_byte_enable.rs` |
| `test_simple_byte_store.rs` | Migrate in ELF phase | `test_simple_byte_store.rs` |
| `test_minimal.rs` (portable subset) | Migrate in ELF phase | split into focused files |
| `test_alloc_only.rs` | Migrate in ELF phase | `test_alloc_only.rs` |
| `test_memory_bounds.rs` | Migrate after read/write helper hardening | `test_memory_bounds.rs` |
| `test_programmatic_memory.rs` | Migrate after helper hardening | `test_programmatic_memory.rs` |
| `tests.rs` | Selective migration only (portable subset) | split into thematic files |
| `test_interactive_simulator.rs` | Keep in cpu-sim | N/A |
| `test_device_lifecycle.rs` | Keep in cpu-sim | N/A |
| `test_audio.rs`, `test_video.rs`, `test_dma.rs` | Keep in cpu-sim unless equivalent FPGA peripherals exist | N/A |

---

## 7. Verification and Quality Gates

For each migration PR:

1. Demonstrate equivalence:
   - old cpu-sim test behavior vs new device-runtime behavior
2. Run targeted tests in Sim backend:
   - `cargo test -p device-runtime --test <migrated_file>`
3. If FPGA runner is available, run same test binary with FPGA env vars.
4. Only delete/disable cpu-sim test after equivalent device-runtime test is stable.

Final migration exit criteria:

- Portable migrated test count > 50% of former cpu-sim integration suite
- Core ISA/peripheral confidence suite executes on real FPGA
- Remaining cpu-sim tests are explicitly documented as simulator-internal

---

## 8. Risks and Mitigations

1. **FPGA timing variance / flakiness**
   - Mitigation: timeout tiers, retry-once policy for known transient serial startup failures.

2. **Event ordering differences between Sim and FPGA backends**
   - Mitigation: helper APIs that match by address/type and ignore unrelated events.

3. **Duplicate maintenance during transition**
   - Mitigation: phase migration by file; promptly retire duplicated cpu-sim tests once parity is proven.

4. **Over-migrating simulator-contract tests**
   - Mitigation: strict Tier C boundary; keep simulator-internal tests in cpu-sim.

---

## 9. Definition of Done

This initiative is complete when:

1. `device-runtime/tests` contains the dominant share of portable integration tests.
2. Migrated instruction-byte and ELF-based portable suites run on both `Sim` and `Fpga` backends without test code forks.
3. Remaining `cpu-sim/tests` coverage is explicitly documented as simulator-internal or intentionally deferred.
4. Migration status in this document is kept current as files are moved.
