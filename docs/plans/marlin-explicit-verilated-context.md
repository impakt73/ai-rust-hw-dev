# Marlin Explicit VerilatedContext Handoff Plan

## 1. Status

- **State:** Implemented
- **Created:** 2026-11-22
- **Completed:** 2026-11-22
- **Fork revision:** `bc388f18bbf792219ad3810f92ff72e2e1388f21`
  (`impakt73/marlin` branch `fix/explicit-verilated-context`)
- **Scope:** marlin fork + this repository's pin and runtime wrapper
- **Primary goal:** give each marlin runtime/model library an explicitly owned `VerilatedContext`, so unloading the generated shared library is safe in multithreaded test processes.

This plan replaces the interim workaround currently committed as `def2966`
("Serialize Verilator runtime lifecycle to fix parallel test crashes").

## 2. Related Material

- Crash investigation: `docs/research/marlin-parallel-test-instability.md`
- Local CI runner: `scripts/run-ci-local.sh`
- Current marlin pin: `riscv_core/Cargo.toml`
- Current runtime wrapper/lock: `riscv_core/src/lib.rs`
- Current test helper: `testbench/src/lib.rs`
- Marlin fork: `https://github.com/impakt73/marlin`
- Current pin revision: `48731fc5bc0a54de01681f6c2736a1cb8f7736ad`

## 3. Goal

Fix the parallel-test crash at its source in marlin.

Today, marlin-generated model libraries are loaded lazily and unloaded when a
`VerilatorRuntime` drops. The generated model constructor currently uses the
Verilator default constructor:

```cpp
new V{top}{}
```

That constructor calls `Verilated::threadContextp()` and falls back to the
library's static `Verilated::defaultContextp()`. Because that static context lives
inside the generated shared library, unload/lifecycle interactions can leave other
test threads with a deleted `VerilatedContext` or corrupt Verilator hierarchy
state.

The proposed fix is:

1. allocate a `VerilatedContext` explicitly per loaded model library,
2. construct every model from that library with the explicit context,
3. keep the context alive for the same lifetime as the loaded library,
4. destroy models first, then the context, then unload the library, and
5. set Verilator's per-thread context explicitly where required.

This removes the dependency on the library's static default context and makes
unloading safe without serializing test execution.

## 4. Non-Goals

- Do not add another global test lock.
- Do not keep all model libraries loaded until process exit as the final design.
- Do not change RTL behavior.
- Do not change test coverage or remove tests.
- Do not require `--test-threads=1`.

## 5. Background

### 5.1 Observed failure

The crash is reproducible with:

```bash
cargo test -p testbench --test fpu_family_test -- --test-threads=16
cargo test -p testbench --test host_bus_test -- --test-threads=16
```

Typical outcomes:

```text
%Error: Attempt to create model using a bad/deleted VerilatedContext pointer
Aborting...
```

or a SIGSEGV with a backtrace in the generated library:

```text
libmarlin_Vfpu_int_to_float.so(VerilatedImp::hierarchyRemove(...))
```

Single-threaded runs pass consistently. The failure rate rises sharply around
8+ test threads.

### 5.2 Evidence gathered

- Worker-unique marlin artifact roots did **not** prevent the crashes, so the
  failure is not primarily a shared build-directory problem.
- Preventing `dlclose` with a diagnostic `LD_PRELOAD` made 16-thread runs pass
  5/5, while unmodified control runs failed 3/5.
- The current marlin 0.11.1 pin already uses atomic
  `entry(...).or_default()` initialization for its per-directory build lock, so
  the historical marlin 0.10.0 `contains_key` + `insert` race is not the cause.
- Verilator's headers document that a user-provided `VerilatedContext` should be
  used when the user wrapper creates threads; otherwise Verilator uses the global
  default context.

### 5.3 Why the explicit context is the proper fix

Keeping the library loaded until process exit avoids the symptom, but preserves
the underlying lifetime ambiguity. An explicit context makes ownership clear:

- each loaded model library owns one context,
- every model from that library is constructed with that context,
- model teardown happens before context teardown,
- context teardown happens before library unload.

This is the correct model if the project wants to retain lazy loading, parallel
test execution, and normal runtime teardown.

## 6. Current Code Shape

### 6.1 Marlin fork

Relevant files in the marlin checkout:

- `verilator/src/lib.rs`
  - `VerilatorRuntime`
  - `library_arena: BoxcarVec<Library>`
  - `build_or_retrieve_library`
  - `create_model`
  - `create_dyn_model`
  - `one_time_library_setup`
  - `Drop for VerilatorRuntime`
- `verilator/src/build_library.rs`
  - `build_ffi`
  - `build_ffi_for_tracing`
  - generated `ffi_new_V{top}` / `ffi_delete_V{top}`
- `verilator/src/ffi_names.rs`
  - FFI symbol naming helpers
- `verilator/src/dynamic.rs`
  - `DynamicVerilatedModel`
- `language-support/verilog-macro-builder/src/lib.rs`
  - generated `AsVerilatedModel::init_from`
  - generated `new_model()` call
- `verilator/src/vcd.rs`
  - VCD handle ownership

### 6.2 This repository

The interim fix wraps marlin's runtime:

```rust
pub struct VerilatorRuntime {
    inner: Option<MarlinVerilatorRuntime>,
}
```

and serializes `create_model*` and `drop` with a process-global mutex. That wrapper
should be removed once marlin owns contexts correctly.

`testbench/src/lib.rs` currently uses a per-thread cached runtime and calls
`runtime.create_model(...)`. Restoring the raw marlin runtime is expected after
the fix.

## 7. Proposed Marlin Changes

### 7.1 Add context FFI symbols

In `verilator/src/ffi_names.rs`, add names such as:

```rust
pub const NEW_CONTEXT: &str = "ffi_new_context";
pub const DELETE_CONTEXT: &str = "ffi_delete_context";
pub const SET_THREAD_CONTEXT: &str = "ffi_set_thread_context";
```

### 7.2 Generate context creation/destruction

In `verilator/src/build_library.rs::build_ffi`, emit:

```cpp
extern "C" {
    void* ffi_new_context() {
        return new VerilatedContext{};
    }

    void ffi_delete_context(void* contextp) {
        delete static_cast<VerilatedContext*>(contextp);
    }

    void ffi_set_thread_context(void* contextp) {
        Verilated::threadContextp(static_cast<VerilatedContext*>(contextp));
    }

    void* ffi_new_V{top}(void* contextp) {
        return new V{top}{static_cast<VerilatedContext*>(contextp)};
    }

    void ffi_delete_V{top}(V{top}* top) {
        delete top;
    }
}
```

Update the `AsVerilatedModel` trait and macro builder so `init_from` receives the
context pointer and calls:

```rust
let new_model: extern "C" fn(*mut std::ffi::c_void) -> *mut std::ffi::c_void =
    *unsafe { library.get(concat!("ffi_new_V", #top_name).as_bytes()) }
        .expect("failed to get symbol");
let model = (new_model)(context);
```

The explicit `VerilatedContext` constructor is already supported by Verilator's
generated model headers:

```cpp
explicit V{top}(VerilatedContext* contextp, const char* name = "TOP");
```

### 7.3 Add an owned library/context entry

Change the runtime arena from raw libraries to library entries:

```rust
struct VerilatedLibrary {
    library: Library,
    context: *mut ffi::c_void,
    delete_context: extern "C" fn(*mut ffi::c_void),
}
```

`VerilatorRuntime::library_arena` becomes:

```rust
library_arena: BoxcarVec<VerilatedLibrary>,
```

`build_or_retrieve_library` should:

1. load the `Library`,
2. resolve `ffi_new_context` and `ffi_delete_context`,
3. create the context,
4. store the entry in the arena,
5. return `&VerilatedLibrary`.

`create_model` should pass the entry's context into `M::init_from` and set the
thread context:

```rust
let entry = self.build_or_retrieve_library(...)?;
let _ = (entry.set_thread_context)(entry.context);
let model = M::init_from(&entry.library, entry.context, config.enable_tracing);
```

`create_dyn_model` should likewise call the dynamic constructor with the context.

### 7.4 Drop order

`Drop for VerilatorRuntime` already deallocates models via
`model_deallocators`. Preserve that order, then let the arena drop entries.

Each `VerilatedLibrary` must delete its context before its `Library` field unloads:

```rust
impl Drop for VerilatedLibrary {
    fn drop(&mut self) {
        if !self.context.is_null() {
            unsafe { (self.delete_context)(self.context) };
            self.context = std::ptr::null_mut();
        }
    }
}
```

Because Rust drops struct fields after the custom `Drop` body, the context is
deleted first and the `Library` unloads afterward.

### 7.5 Tracing

Today `build_ffi_for_tracing` generates:

```cpp
void ffi_Verilated_traceEverOn(bool everOn) {
    Verilated::traceEverOn(everOn);
}
```

Change this to take the context explicitly, for example:

```cpp
void ffi_Verilated_traceEverOn(void* contextp, bool everOn) {
    static_cast<VerilatedContext*>(contextp)->traceEverOn(everOn);
}
```

Update the Rust call site in `one_time_library_setup` accordingly. Model tracing
itself uses `top->trace(...)`, which already goes through the model's context.

### 7.6 DPI

DPI callbacks are installed per library. Audit `one_time_library_setup` and the
DPI initialization path to ensure no global/default context is required. If DPI
callbacks use Verilator thread context, set the library context before invoking
the callback.

### 7.7 Thread context handling

Verilator documents that user-created threads should set the context explicitly.
Use the generated `ffi_set_thread_context(context)` before model construction and
before any operation that may fall back to `Verilated::threadContextp()`.

Model `eval()` uses the model's own context, so normal evaluation should remain
per-runtime. Tracing and helper APIs are the main paths to audit.

Do not set the thread context to a dangling pointer on context deletion. The
runtime should be dropped only on a thread that will not use that runtime's
library again. Document this assumption in marlin.

## 8. Repository Changes After the Marlin Fix

### 8.1 Update the pin

Update `riscv_core/Cargo.toml` to the new marlin fork revision and refresh
`Cargo.lock`.

### 8.2 Remove the interim lifecycle lock

After the marlin fix is verified:

- remove the `VerilatorRuntime` wrapper and `RUNTIME_LIFECYCLE_LOCK` from
  `riscv_core/src/lib.rs`,
- restore the direct marlin runtime re-export,
- restore `testbench/src/lib.rs` error mapping if needed,
- remove stale comments/documentation claiming a global lifecycle lock is the
  final mitigation.

The desired final state is parallel test execution with no repository-level
serialization.

### 8.3 Update documentation

Update `docs/research/marlin-parallel-test-instability.md` to:

- distinguish the historical marlin 0.10.0 build-lock race,
- document the explicit-context root cause and fix,
- remove the interim global-lock mitigation as the long-term recommendation,
- note that `keep_loaded` remains an alternative but is not the chosen design.

## 9. Implementation Phases

### Phase 1 — Build the marlin fork change

1. Create a branch in `impakt73/marlin`.
2. Add context FFI symbols and generated context constructors/destructors.
3. Change `VerilatedLibrary` ownership and drop order.
4. Change `AsVerilatedModel::init_from` and the macro builder to take a context.
5. Change `create_model` and `create_dyn_model` to use the library context.
6. Update tracing `traceEverOn` to be context-aware.
7. Build marlin and run its own tests/examples.

### Phase 2 — Validate against this repository before removing the lock

Point `riscv_core` at the new fork revision while keeping the interim wrapper in
place. This allows the new marlin behavior to be tested incrementally.

Run:

```bash
cargo clean
cargo test
cargo test -p testbench --test fpu_family_test -- --test-threads=16
cargo test -p testbench --test host_bus_test -- --test-threads=16
```

Also run repeated 16-thread iterations to catch rare failures.

### Phase 3 — Remove the repository workaround

Once the new pin passes cold and warm runs, remove the wrapper and lifecycle
lock, then repeat the verification.

### Phase 4 — Documentation and handoff

Update the research document, remove the intermediate lock description, and
record the new pin and test evidence.

## 10. Verification Matrix

Required before considering the plan complete:

- `cargo fmt -- --check`
- `cargo clippy -- -D warnings`
- `(cd rust-test-program && cargo fmt -- --check)`
- `(cd rust-test-program && cargo clippy --release -- -D warnings)`
- `cargo build --verbose`
- `cargo test --verbose` (parallel, cold and warm)
- repeated `fpu_family_test --test-threads=16`
- repeated `host_bus_test --test-threads=16`
- `./scripts/run-ci-local.sh`
- FPGA synthesis/timing path via the local CI script
- VCD/tracing tests, at minimum `testbench --test test_trace` and any
  `device-runtime` VCD tests
- DPI/example tests in the marlin fork if available

Acceptance criteria:

- full parallel `cargo test` passes repeatedly with no lifecycle lock,
- no regression in single-threaded tests,
- no regression in VCD/tracing or DPI behavior,
- no stale `VerilatedContext` failures,
- no new warnings or clippy failures,
- documentation reflects the final architecture.

## 11. Risks and Open Questions

- **Context-per-library vs context-per-model:** This plan uses one context per
  loaded library (and therefore per runtime/library entry). Verify that multiple
  models from the same runtime can share that context safely in the expected
  single-threaded usage. If not, move to one context per model.
- **Thread context lifetime:** Deleting a context while `Verilated::threadContextp`
  still points to it in a live thread is unsafe. Confirm the drop assumptions or
  add a safe clear/rebind path.
- **Tracing:** `VerilatedContext::traceEverOn` must be used instead of the global
  `Verilated::traceEverOn`; verify VCD output is unchanged.
- **DPI:** DPI callbacks may depend on thread/default context; audit and test.
- **Long-lived runtimes:** Verify that context deletion and library unload are
  correct for `device-runtime`, where a simulator thread owns a runtime for its
  lifetime.
- **Library refcounting:** Explicit contexts should make per-runtime unload safe,
  but confirm with simultaneous same-library runtimes in separate threads.
- **ABI/API break:** `AsVerilatedModel::init_from` is a public trait method; this
  is a breaking marlin API change. That is acceptable for the fork, but note it
  in the fork commit message.

## 12. Rollback

If the explicit-context approach is blocked:

- keep the current `def2966` lifecycle-lock wrapper as an interim fix, or
- switch to the simpler `keep_loaded`/`RTLD_NODELETE` option in marlin.

Do not ship a partial context change that deletes contexts while library static
state or thread-local context pointers may still be active.

## 13. Handoff Checklist

- [x] New marlin fork branch exists and is pushed
- [x] New fork revision recorded
- [x] `riscv_core/Cargo.toml` and `Cargo.lock` updated
- [x] interim runtime wrapper and lifecycle lock removed
- [x] cold and warm full `cargo test` pass
- [x] repeated 16-thread fpu and host-bus tests pass
- [x] `./scripts/run-ci-local.sh` passes
- [x] VCD/tracing and DPI paths tested
- [x] research document updated
- [ ] final commit(s) prepared for review
