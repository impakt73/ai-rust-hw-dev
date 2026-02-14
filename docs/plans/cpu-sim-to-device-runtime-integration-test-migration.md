# CPU-Sim → Device-Runtime Integration Test Migration Plan

## 1. Goal

Migrate the largest practical subset of integration tests currently living in `cpu-sim/tests` into `device-runtime/tests`, so the same tests can execute against:

1. `DeviceRuntimeType::Sim` (fast local/CI simulation), and
2. `DeviceRuntimeType::Fpga` (real hardware validation).

The success target is to move enough tests that **the majority of integration coverage no longer depends on cpu-sim-only APIs** and can be run on FPGA.

---

## 2. Current State Snapshot

### 2.1 Test inventory (today)

- `cpu-sim/tests`: **149** integration tests
- `device-runtime/tests`: **4** integration tests (`test_load_program.rs`, `test_reset.rs`)

Largest concentration inside `cpu-sim/tests`:

| File | Tests |
|---|---:|
| `test_rtl_verification.rs` | 39 |
| `tests.rs` | 20 |
| `test_fp_integration.rs` | 15 |
| `test_memory_bounds.rs` | 12 |
| `test_interactive_simulator.rs` | 10 |
| `test_rv32c_basic.rs` | 9 |
| `test_uart_peripheral.rs` | 8 |
| `test_led_peripheral.rs` | 7 |
| `test_minimal.rs` | 7 |
| `test_host_initiated_requests.rs` | 6 |

### 2.2 Why migration is blocked today

Most cpu-sim tests use cpu-sim-specific hooks that are not part of `DeviceRuntime`:

- `run_program(...)`, `run_elf(...)`
- setup/teardown callbacks with `SimulatorView`
- direct simulation introspection (`led_out()`, FIFO read/write helpers, trace/vcd/hung detector checks)
- simulator-only device registration (`register_device(...)`)
- simulator memory debug access patterns

`device-runtime` currently exposes transport-oriented primitives (`load_program`, `load_elf`, `boot_cpu`, `send_host_request`, `poll`, `reset`) that are backend-agnostic, but lacks a reusable test harness layer.

---

## 3. Migration Strategy

### 3.1 Core principle

Move tests to a **backend-agnostic runtime harness** in `device-runtime/tests/common` and avoid any dependency on `SimulatorView` internals.

Each migrated test must run unchanged for both backends:

- default: `Sim`
- optional FPGA mode selected by env vars (`FPGA_DEVICE_PATH`, `FPGA_BAUD_RATE`)

### 3.2 Categorize cpu-sim tests by portability

#### Tier A — High-value, high-portability (migrate first)

These are mostly instruction/peripheral behaviors that can be expressed via load/boot/poll/host-bus operations:

- `test_rtl_verification.rs` (39)
- `test_fp_integration.rs` (15)
- `test_rv32c_basic.rs` (9)
- `test_led_peripheral.rs` (7)
- `test_uart_peripheral.rs` (8)
- `test_host_initiated_requests.rs` (6)
- `test_byte_enable.rs` (2)
- `test_simple_byte_store.rs` (1)

**Potential migrated subtotal: 87 tests**

#### Tier B — Portable with harness extensions (migrate second)

- `test_memory_bounds.rs` (12)
- `test_programmatic_memory.rs` (2)
- selected cases from `tests.rs` that only require tohost + host-bus interaction

Requires robust helper APIs for host-side memory reads/writes and standard polling/timeouts.

#### Tier C — Keep in cpu-sim (simulation-internal by design)

- `test_interactive_simulator.rs` (interactive API contract)
- `test_device_lifecycle.rs` (custom `BusDevice` lifecycle hooks)
- trace/vcd/hung-detector validation from `tests.rs`
- tests that depend on Rust-only peripherals (`test_audio.rs`, `test_video.rs`, `test_dma.rs`, parts of `test_minimal.rs`)

These validate simulator internals, not hardware/runtime portability.

---

## 4. Target Architecture for Migrated Tests

### 4.1 New shared test harness (`device-runtime/tests/common/mod.rs`)

Create reusable helpers:

1. `create_test_runtime()`
   - chooses FPGA or Sim backend via env
2. `load_and_boot(runtime, boot_pc, program_bytes)`
3. `wait_for_tohost(runtime, timeout) -> u32`
4. `wait_for_host_read_response(...)` / `wait_for_host_write_response(...)`
5. `read_word_with_timeout(...)`, `write_word_with_timeout(...)`
6. `drain_events_until_idle(...)` for deterministic sequencing
7. standardized timeout constants (short/medium/long)

This removes duplicated polling loops currently repeated across tests.

### 4.2 Program builders

Move reusable instruction-to-bytes helpers into device-runtime test common:

- `instructions_to_bytes(...)`
- `tohost_termination(...)`
- `append_tohost_termination(...)`

Reuse conventions already present in `cpu-sim/tests/common/mod.rs`.

### 4.3 Test style contract for portability

Migrated tests must:

- assert via tohost code and host-bus reads (no simulator internals)
- avoid direct cycle-precise assumptions unless architecturally required
- use bounded polling loops with explicit timeout diagnostics
- pass on both Sim and FPGA backends (with backend-specific timeout multipliers if needed)

---

## 5. Phased Implementation Plan

### Phase 0: Foundation (prerequisite)

1. Add `device-runtime/tests/common/mod.rs` harness.
2. Refactor existing `test_load_program.rs` and `test_reset.rs` to use new common helpers.
3. Ensure no behavior change; tests still pass in Sim mode.

### Phase 1: Migrate Tier A core ISA/peripheral suites

1. Migrate `test_led_peripheral.rs`, `test_uart_peripheral.rs`, `test_host_initiated_requests.rs`.
2. Migrate `test_rv32c_basic.rs` and `test_fp_integration.rs`.
3. Migrate `test_rtl_verification.rs` in chunks (e.g., arithmetic/branch/memory/CSR/M/A groups).
4. Keep old cpu-sim versions temporarily behind `#[ignore]` or remove only when equivalent device-runtime coverage exists.

### Phase 2: Migrate Tier B with targeted harness growth

1. Add host-memory helper coverage needed for `test_memory_bounds.rs` and `test_programmatic_memory.rs`.
2. Port only tests that can remain backend-agnostic.
3. Leave simulator-internal cases in cpu-sim.

### Phase 3: CI and execution model changes

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
| `test_rtl_verification.rs` | Migrate most tests; split by instruction class for manageable files | `test_rtl_verification_*.rs` |
| `test_fp_integration.rs` | Migrate fully | `test_fp_integration.rs` |
| `test_rv32c_basic.rs` | Migrate fully | `test_rv32c_basic.rs` |
| `test_led_peripheral.rs` | Migrate fully; replace `sim.led_out()` checks with host register reads | `test_led_peripheral.rs` |
| `test_uart_peripheral.rs` | Migrate fully; keep larger timeout budget for loopback on FPGA | `test_uart_peripheral.rs` |
| `test_host_initiated_requests.rs` | Migrate fully; maps naturally to `DeviceRuntime` API | `test_host_initiated_requests.rs` |
| `test_byte_enable.rs` | Migrate fully | `test_byte_enable.rs` |
| `test_simple_byte_store.rs` | Migrate fully | `test_simple_byte_store.rs` |
| `test_memory_bounds.rs` | Migrate after read/write helper hardening | `test_memory_bounds.rs` |
| `test_programmatic_memory.rs` | Migrate after helper hardening | `test_programmatic_memory.rs` |
| `tests.rs` | Selective migration only (portable subset) | split into thematic files |
| `test_interactive_simulator.rs` | Keep in cpu-sim | N/A |
| `test_device_lifecycle.rs` | Keep in cpu-sim | N/A |
| `test_audio.rs`, `test_video.rs`, `test_dma.rs` | Keep in cpu-sim unless equivalent FPGA peripherals exist | N/A |
| `test_minimal.rs`, `test_alloc_only.rs` | Keep only portable portions; otherwise cpu-sim-specific | optional selective migration |

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

1. `device-runtime/tests` contains the dominant share of integration tests.
2. The migrated suite runs on both `Sim` and `Fpga` backends without test code forks.
3. The majority of previous `cpu-sim/tests` coverage is represented in device-runtime.
4. `cpu-sim/tests` is reduced to simulator-internal contract tests only.
