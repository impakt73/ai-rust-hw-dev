# Marlin Parallel Test Instability Investigation

**Research Document**

**Context:** Intermittent crashes when marlin-based Verilator tests run concurrently across multiple threads

**Originally written:** 2026-03-21

**Revalidated:** 2026-11-22

## Status

The original conclusion in this document was based on **marlin 0.10.0**. The
repository now pins **marlin 0.11.1** (`48731fc5bc0a54de01681f6c2736a1cb8f7736ad`),
and the specific thread-lock registry race described below has been fixed
upstream. It is **not** the cause of the current failures.

Revalidation under marlin 0.11.1 and Verilator 5.032 shows a different failure:
the generated Verilator model shared library is unloaded while Verilator's static
runtime state is still referenced. The original marlin 0.10.0 analysis is
retained at the end of this document as historical context, but the current root
cause and recommendations below supersede it.

## Executive Summary

Current reproduction:

- `testbench` FPU tests (`fpu_family_test`) pass consistently with
  `--test-threads=1`, `2`, or `4`.
- At `--test-threads=8`, `12`, and `16`, the process fails intermittently.
  Typical outcomes are:
  - all 51 tests print `test result: ok`, followed by a process SIGSEGV during
    teardown, or
  - a test aborts while creating a model with:

    ```text
    %Error: Attempt to create model using a bad/deleted VerilatedContext pointer
    Aborting...
    ```

- A signal backtrace from the failing process lands in the generated model
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

The evidence therefore points to **library/context lifetime management during
unload**, not to concurrent Verilator build-directory access. The repository’s
shared `target/verilator` root remains a scalability concern, but it is not
sufficient to explain the current crash.

## Scope and Versions (Current)

- `marlin 0.11.1`
- `marlin-verilator 0.11.1`
- `marlin-verilog 0.11.1`
- `marlin-verilog-macro 0.11.1`
- `marlin-verilog-macro-builder 0.11.1`
- Verilator 5.032 (Ubuntu distro package used for the local reproduction)

The pin is recorded in `Cargo.lock` under the `marlin` package entry.

## How This Repository Uses Marlin

### `riscv_core` wraps runtime creation

`riscv_core` depends on marlin as a normal dependency and re-exports its runtime
and model types. The critical helper is:

- `create_runtime(files: &[&str]) -> Result<VerilatorRuntime, ...>` in
  `riscv_core/src/lib.rs`

It still hardcodes the shared artifact root:

```rust
VerilatorRuntime::new(
    "target/verilator".into(),
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

The exact defect boundary (marlin library lifetime management, Verilator's
static context handling, or the platform loader interaction) has not been fully
isolated. What is established is that the current failures require library
unload to occur; avoiding unload avoids the crashes in this environment.

### 4. Build concurrency is still a separate concern

Marlin still invokes Verilator with:

```text
verilator --cc -sv -j 0 --build ...
```

and locks builds per artifact directory. Distinct models can still build
concurrently, and each build may use all available cores. This can oversubscribe
a machine during cold runs, but it does not match the observed high-thread-count
crash signature after artifacts are already built.

## Current Limitations

1. **Model libraries are unloaded with the runtime**
   - runtimes own `libloading::Library` handles
   - dropping the runtime can unload the library
   - Verilator static state lives in that library

2. **No “keep library loaded” option is exposed by the repository**
   - each test worker can load and unload its own runtime/model libraries
   - high thread counts create overlapping load/unload cycles

3. **Shared artifact root in this repository**
   - `riscv_core` still hardcodes `target/verilator`
   - this increases build contention, but is not the current crash cause

4. **No global build concurrency limit**
   - different models can build concurrently
   - each build asks Verilator to parallelize aggressively

5. **Models/runtimes are not thread-shareable**
   - one runtime/model must stay private to one owner

6. **Tracing uses global Verilator state**
   - traced large-scale concurrency is harder than non-traced runs

## Recommendations

### Immediate mitigation

Avoid unloading marlin model libraries while the process can still create or
destroy Verilated state. Practical options, in rough order of preference:

1. **Keep runtimes alive for the process lifetime** in test binaries, rather
   than letting short-lived worker threads drop them.
2. **Expose/use a no-unload loader option** (`RTLD_NODELETE` or equivalent) for
   marlin-generated libraries.
3. **Use process isolation**: run heavy test binaries with a single test thread
   per process, or split tests so each process has a simpler library lifetime.
4. **Upgrade the marlin/Verilator combination** after verifying upstream fixes
   for the context/library lifetime behavior.

Serializing tests with `--test-threads=1` avoids the immediate crash, but it is
a workaround for the observed lifecycle pattern rather than a root-cause fix.

### Long-term

- Add a marlin option to keep generated libraries loaded for the process
  lifetime, or fix the library lifetime/context ownership in marlin/Verilator.
- Keep the per-directory build lock atomic (already true in marlin 0.11.1).
- Add a global build throttle so cold runs do not launch many `-j 0` Verilator
  builds at once.
- Consider worker-unique artifact roots if build-directory contention returns,
  but do not treat them as a fix for the current unload-related crashes.

## Reproduction Commands

```bash
# Fails intermittently in this environment (Verilator 5.032, marlin 0.11.1)
cargo test -p testbench --test fpu_family_test -- --test-threads=16

# Control: passes consistently
cargo test -p testbench --test fpu_family_test -- --test-threads=1

# Diagnostic: disabling dlclose avoided the failures in 5/5 runs
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

The current high-concurrency failures are best explained by unloading generated
marlin/Verilator shared libraries while Verilator static context and hierarchy
state are still in use. The marlin 0.10.0 build-lock race described in the
original document has been fixed and is not the present cause.

For the current repository, the priority is controlling model-library lifetime
(keep libraries loaded, isolate heavy tests by process, or upgrade to a fixed
marlin/Verilator combination). Worker-unique artifact roots and build throttles
remain useful for cold-build scalability, but they do not address the observed
crash.
