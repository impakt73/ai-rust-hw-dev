# Marlin Parallel Test Instability Investigation

**Research Document**

**Context:** Intermittent crashes when marlin-based Verilator tests run concurrently across multiple threads

**Originally written:** 2026-03-21

**Revalidated:** 2026-11-22

## Status

The original conclusion in this document was based on **marlin 0.10.0**. The
specific thread-lock registry race described below was fixed upstream and is
**not** the cause of the current failures.

Revalidation under marlin 0.11.1 and Verilator 5.032 showed a different failure:
the generated Verilator model shared library was unloaded while Verilator's
static default context was still referenced. That root cause is now fixed by
marlin's explicit `VerilatedContext` ownership, pinned at fork revision
`bc388f18bbf792219ad3810f92ff72e2e1388f21`.

The interim repository-level lifecycle lock has been removed. Parallel test
execution now relies on marlin owning one context per loaded model library.

## Executive Summary

Original reproduction under the old pin:

- `testbench` FPU tests (`fpu_family_test`) passed consistently with
  `--test-threads=1`, `2`, or `4`.
- At `--test-threads=8`, `12`, and `16`, the process failed intermittently.
  Typical outcomes were:
  - all 51 tests print `test result: ok`, followed by a process SIGSEGV during
    teardown, or
  - a test aborts while creating a model with:

    ```text
    %Error: Attempt to create model using a bad/deleted VerilatedContext pointer
    Aborting...
    ```

- A signal backtrace from the failing process landed in the generated model
  library:

  ```text
  libmarlin_Vfpu_int_to_float.so(VerilatedImp::hierarchyRemove(...))
  ```

- Temporarily giving each test thread its own marlin artifact root
  (`target/verilator-worker-<id>`) did **not** prevent the crashes.
- Preventing `dlclose` from unloading the marlin model libraries eliminated the
  crashes in five consecutive 16-thread runs: `5/5` passed, versus `2/5` passed
  in the control runs (`3` failures, all with the bad/deleted
  `VerilatedContext` message).

The evidence therefore pointed to **library/context lifetime management during
unload**, not to concurrent Verilator build-directory access. The repository’s
shared `target/verilator` root remains a scalability concern, but it is not
sufficient to explain the crash.

## Scope and Versions (Current)

- `marlin 0.11.1`
- `marlin-verilator 0.11.1`
- `marlin-verilog 0.11.1`
- `marlin-verilog-macro 0.11.1`
- `marlin-verilog-macro-builder 0.11.1`
- Verilator 5.032 (Ubuntu distro package used for the local reproduction)
- marlin fork revision `bc388f18bbf792219ad3810f92ff72e2e1388f21`
  (`fix/explicit-verilated-context`)

The pin is recorded in `Cargo.lock` under the `marlin` package entry.

## How This Repository Uses Marlin

### `riscv_core` re-exports the marlin runtime

`riscv_core` depends on marlin as a normal dependency and re-exports its runtime
and model types directly. The critical helper is:

- `create_runtime(files: &[&str]) -> Result<VerilatorRuntime, ...>` in
  `riscv_core/src/lib.rs`

It still hardcodes the shared artifact root:

```rust
VerilatorRuntime::new(
    Utf8Path::new("target/verilator"),
    ...
)
```

So every CPU, FPU, regfile, and wrapper test in the same package shares one
top-level marlin artifact tree.

### `testbench` caches runtimes per thread

`testbench/src/lib.rs` uses `with_cached_runtime` and a thread-local `OnceCell`
to keep one runtime per DUT family per test-worker thread:

```rust
cache.with(|runtime| {
    let runtime = runtime.get_or_init(|| create_runtime()...);
    f(runtime)
})
```

This avoids recreating a runtime for every model within a worker, but libtest
still creates short-lived worker threads for test execution. Each thread loads
the generated marlin `.so`, builds a model, runs the test, drops the model and
runtime, and exits. At high test-thread counts, many of these load/unload cycles
overlap.

## Current Findings

### 1. The marlin 0.10.0 lock-registry race is fixed in 0.11.1

Current marlin initializes the per-artifact-directory mutex atomically:

```rust
let thread_mutex = THREAD_LOCKS_PER_BUILD_DIR
    .entry(local_artifacts_directory.clone())
    .or_default();
```

Source: `marlin/verilator/src/lib.rs` in the pinned marlin checkout
(`THREAD_LOCKS_PER_BUILD_DIR` and the `.entry(...).or_default()` call).

This is the exact `entry().or_insert*()`-style fix that the original document
recommended. The old `contains_key` + `insert` TOCTOU sequence is no longer
present. A concurrent same-directory build race may have explained crashes on
marlin 0.10.0, but it does not explain the current behavior.

### 2. Worker-unique artifact roots do not fix the crashes

To test whether artifact-directory sharing was still the cause,
`create_runtime` was temporarily changed to use a thread-local
`target/verilator-worker-<id>` directory. Under high thread counts the failures
continued, including SIGSEGV and the bad/deleted `VerilatedContext` abort. This
rules out shared build directories as the sole or primary cause of the current
crashes.

### 3. The failures are tied to unloading the generated shared library

The generated marlin model libraries are loaded with `libloading::Library::new`
and unloaded when the runtime is dropped. `LD_DEBUG=files` shows repeated
`dlopen` / `dlclose` activity for `libmarlin_V*.so` during a high-thread-count
run.

Two failure signatures were captured:

- `VerilatedImp::hierarchyRemove` crashing inside the model library while a
  model is being destroyed.
- `VerilatedContext::checkMagic` failing with:

  ```text
  Attempt to create model using a bad/deleted VerilatedContext pointer
  ```

Both point at Verilator static state that belongs to the loaded model library,
not at the model's own instance data or at a build directory.

As a diagnostic, a preload library that made `dlclose` a no-op was used. With
unloading disabled, the same 16-thread test passed five consecutive times
(`5/5`), while the unmodified control failed three of five runs. That is strong
evidence that unloading the shared library while other Verilated state is still
in use is the failure trigger.

The exact defect boundary was Verilator's static default context inside each
generated model library. Marlin constructed models with the generated
no-argument constructor, which falls back to that library-local
`Verilated::defaultContextp()`. When one runtime unloaded its copy of the library
while another thread still held Verilator state tied to that context, the
remaining thread could observe a deleted context or corrupt hierarchy state.

### 4. Explicit context ownership fixes the unload lifetime

The fork now gives every loaded model library an explicitly owned
`VerilatedContext`:

- generated libraries export `ffi_new_context`, `ffi_delete_context`, and
  `ffi_set_thread_context`,
- models are constructed with `new V{top}{context}` and
  `AsVerilatedModel::init_from` receives that context,
- the context lives in the same arena entry as the loaded `Library`,
- runtime teardown deallocates models first, then deletes the context, then
  unloads the library,
- tracing uses `VerilatedContext::traceEverOn` rather than the global
  `Verilated::traceEverOn`,
- the per-library thread context is set before model construction and one-time
  library setup.

This removes the dependency on the library's static default context, so lazy
loading and normal runtime teardown remain safe without serializing tests. The
fork commit notes the breaking `AsVerilatedModel::init_from` API change.

### 5. Build concurrency is still a separate concern

Marlin still invokes Verilator with:

```text
verilator --cc -sv -j 0 --build ...
```

and locks builds per artifact directory. Distinct models can still build
concurrently, and each build may use all available cores. This can oversubscribe
a machine during cold runs, but it does not match the observed high-thread-count
crash signature after artifacts are already built.

## Current Limitations

1. **Model libraries are still unloaded with the runtime**
   - runtimes own `libloading::Library` handles
   - the explicit context makes unload safe, but unloading still costs time

2. **One context per loaded library**
   - models from the same runtime share a context, which matches the expected
     single-threaded use of a runtime
   - sharing a runtime and its models across threads is still unsupported

3. **Shared artifact root in this repository**
   - `riscv_core` still hardcodes `target/verilator`
   - this increases build contention, but is not a crash cause

4. **No global build concurrency limit**
   - different models can build concurrently
   - each build asks Verilator to parallelize aggressively

5. **Models/runtimes are not thread-shareable**
   - one runtime/model must stay private to one owner

## Recommendations

### Implemented fix

Use the marlin fork revision pinned in `Cargo.lock`, which owns one
`VerilatedContext` per loaded model library and deletes it before unloading the
library. The interim `riscv_core::VerilatorRuntime` wrapper and process-global
lifecycle lock have been removed; `riscv_core` re-exports marlin's runtime
directly.

The full parallel `cargo test` suite passes both warm and after clearing all
per-package `target/verilator` directories. The previously failing 16-thread
`fpu_family_test` and `host_bus_test` binaries pass repeatedly without the
lifecycle lock.

### Alternatives considered

- **Keep generated libraries loaded for the process lifetime** (`RTLD_NODELETE`
  or similar) also avoids the symptom, but it preserves ambiguous ownership and
  leaks library mappings. It is not the chosen design.
- **Serialize tests with `--test-threads=1`** avoids the crash but gives up
  parallel execution and does not address the lifetime problem.
- **Worker-unique artifact roots** remain a possible response to build-directory
  contention, but do not fix library/context lifetime.

### Follow-up

- Keep the per-directory build lock atomic (already true in marlin 0.11.1).
- Add a global build throttle so cold runs do not launch many `-j 0` Verilator
  builds at once.

## Reproduction Commands

```bash
# Fixed pin: pass repeatedly without a lifecycle lock
cargo test -p testbench --test fpu_family_test -- --test-threads=16
cargo test -p testbench --test host_bus_test -- --test-threads=16
cargo test

# Historical diagnostic under the old pin: disabling dlclose avoided the
# failures in 5/5 runs, versus 2/5 control runs
cat >/tmp/nodlclose.c <<'EOF'
int dlclose(void *handle) { (void)handle; return 0; }
EOF
gcc -shared -fPIC -o /tmp/libnodlclose.so /tmp/nodlclose.c
LD_PRELOAD=/tmp/libnodlclose.so \
  cargo test -p testbench --test fpu_family_test -- --test-threads=16
```

The `LD_PRELOAD` method is a debugging tool only; it leaks library mappings and
is not a production test configuration.

## Historical Appendix: Marlin 0.10.0 Analysis

The original investigation reached a different conclusion because it analyzed
marlin 0.10.0, whose per-directory thread-lock registry used a check-then-act
sequence:

```rust
if !THREAD_LOCKS_PER_BUILD_DIR.contains_key(&local_artifacts_directory) {
    THREAD_LOCKS_PER_BUILD_DIR.insert(
        local_artifacts_directory.clone(),
        Default::default(),
    );
}
let thread_mutex = THREAD_LOCKS_PER_BUILD_DIR
    .get(&local_artifacts_directory)
    .expect("We just inserted if it didn't exist");
```

Two threads could both see the key absent and install different mutexes for the
same directory, allowing both to enter `build_library()` even though marlin
documents that function as not thread-safe. Combined with the repository’s
shared `target/verilator` root, that was a credible explanation for intermittent
same-directory build corruption on marlin 0.10.0.

That historical analysis remains useful context for older checkouts, but it
should not be applied to the current marlin 0.11.1 pin. In the current version,
the lock initialization is atomic, and the crashes persist even with
worker-unique artifact roots.

## Conclusion

The current high-concurrency failures came from unloading a generated
marlin/Verilator shared library while another thread still referenced Verilator
state tied to that library's static default context. The marlin 0.10.0
build-lock race described in the original document had already been fixed and
was not the present cause.

The fix is explicit context ownership in marlin: each loaded model library owns
a `VerilatedContext`, models are constructed with it, and it is destroyed before
the library unloads. This keeps lazy loading, normal teardown, and parallel test
execution safe without a repository-level lifecycle lock.

The interim `riscv_core::VerilatorRuntime` wrapper and process-global lock have
been removed. Worker-unique artifact roots and build throttles remain useful for
cold-build scalability, but are unrelated to the fixed crash.
