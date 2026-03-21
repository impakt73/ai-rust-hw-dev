# Marlin Parallel Test Instability Investigation

**Research Document**  
**Context:** Intermittent crashes when marlin-based Verilator tests run concurrently across multiple threads and processes  
**Date:** 2026-03-21

## Executive Summary

The current repository funnels every marlin runtime through the same `target/verilator` artifact root (`/home/runner/work/ai-rust-hw-dev/ai-rust-hw-dev/riscv_core/src/lib.rs:261-301`). That guarantees heavy sharing of generated Verilator build directories whenever many tests start at once.

Marlin 0.10.0 does attempt to serialize rebuilds for a given artifact directory, but the implementation has an internal race in its thread-lock registry: it uses `contains_key` followed by `insert` on a global `DashMap`, which can create different mutexes for the same artifact directory when two threads miss simultaneously (`/home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/marlin-verilator-0.10.0/src/lib.rs:319-324,649-659`). That matters because marlin also documents that `build_library()` is **not thread-safe** and must be externally guarded (`/home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/marlin-verilator-0.10.0/src/build_library.rs:360-372`), while the file lock it uses is explicitly described as insufficient for synchronizing threads inside one process (`/home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/marlin-verilator-0.10.0/src/lib.rs:319-321`).

That combination produces the most credible root cause for the intermittent crashes:

1. multiple tests start in parallel,
2. separate runtimes target the same marlin artifact directory under `target/verilator`,
3. marlin's per-directory thread lock can race during initialization,
4. two threads in one process can then enter a build path that marlin itself says is not thread-safe.

There is a second limitation on top of that: marlin only serializes **same-directory** builds. Distinct models or configs can still launch many independent Verilator builds at once, and marlin invokes Verilator with `-j 0`, which asks Verilator to use all available cores for each build (`/home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/marlin-verilator-0.10.0/src/build_library.rs:442-447`). That makes cold-start parallel test runs prone to CPU and memory oversubscription even when they avoid the same-directory race.

## Scope and Versions

This repository currently uses:

- `marlin 0.10.0`
- `marlin-verilator 0.10.0`
- `marlin-verilog 0.10.0`
- `marlin-verilog-macro 0.10.0`
- `marlin-verilog-macro-builder 0.10.0`

as recorded in `/home/runner/work/ai-rust-hw-dev/ai-rust-hw-dev/Cargo.lock:1332-1391`.

## How This Repository Uses Marlin

### `riscv_core` is the only marlin integration layer

`riscv_core` depends on marlin as both a normal dependency and a build dependency (`/home/runner/work/ai-rust-hw-dev/ai-rust-hw-dev/riscv_core/Cargo.toml:6-10`) and re-exports marlin runtime/model types from `src/lib.rs`.

The critical helper is:

- `create_runtime(files: &[&str]) -> Result<VerilatorRuntime, ...>`
- `/home/runner/work/ai-rust-hw-dev/ai-rust-hw-dev/riscv_core/src/lib.rs:261-301`

That helper always constructs the runtime with the same artifact root:

```rust
VerilatorRuntime::new(
    "target/verilator".into(),
    ...
)
```

So every CPU, FPU, regfile, and wrapper test shares one top-level marlin artifact tree.

### `testbench` magnifies the contention pattern

`testbench` is an integration-test crate, and its manifest explicitly notes that tests in `tests/` run as separate binaries (`/home/runner/work/ai-rust-hw-dev/ai-rust-hw-dev/testbench/Cargo.toml:6-12`).

Within those binaries, tests repeatedly create fresh runtimes and fresh models. For example, the FPU tests create a new runtime and then a new model per test case:

- runtime helper: `/home/runner/work/ai-rust-hw-dev/ai-rust-hw-dev/testbench/tests/fpu_test.rs:41-43`
- first test instantiation: `/home/runner/work/ai-rust-hw-dev/ai-rust-hw-dev/testbench/tests/fpu_test.rs:106-109`

That means concurrent test execution naturally produces many independent `VerilatorRuntime` instances, all pointed at the same shared `target/verilator` root.

## Marlin Source Analysis

### 1. Marlin's build step is explicitly not thread-safe

`marlin-verilator` documents this directly:

> `This function is not thread-safe; the artifact_directory must be guarded.`

Source:

- `/home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/marlin-verilator-0.10.0/src/build_library.rs:360-372`

This is the core invariant that the rest of marlin's locking scheme must satisfy.

### 2. Marlin tries to guard builds with a thread lock plus a file lock

The runtime stores a global map of per-build-directory mutexes:

- `/home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/marlin-verilator-0.10.0/src/lib.rs:319-324`

The comment on that map is important:

> `file_guard` handles locking across processes, but does not guarantee locking between threads in one process.

Source:

- `/home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/marlin-verilator-0.10.0/src/lib.rs:319-321`

Later, marlin acquires:

1. the thread-local mutex for the artifact directory, then
2. an exclusive file lock for the same directory.

Source:

- `/home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/marlin-verilator-0.10.0/src/lib.rs:649-706`

If the thread mutex were initialized atomically, that would largely protect same-directory rebuilds inside one process.

### 3. The thread-lock registry has a TOCTOU race

Marlin does **not** initialize the per-directory mutex atomically. It does this:

```rust
if !THREAD_LOCKS_PER_BUILD_DIR.contains_key(&local_artifacts_directory) {
    THREAD_LOCKS_PER_BUILD_DIR.insert(
        local_artifacts_directory.clone(),
        Default::default(),
    );
}
let thread_mutex = THREAD_LOCKS_PER_BUILD_DIR.get(&local_artifacts_directory)
    .expect("We just inserted if it didn't exist");
```

Source:

- `/home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/marlin-verilator-0.10.0/src/lib.rs:649-659`

This is a classic check-then-act race:

- Thread A checks `contains_key` and sees `false`
- Thread B checks `contains_key` and also sees `false`
- both threads `insert` a different mutex for the same key
- each thread can then retrieve and lock a different mutex instance

Because marlin's own comment says the file lock is **not** relied on for thread synchronization inside one process, this race undermines the exact guard that `build_library()` depends on.

### 4. Artifact reuse is guaranteed by the repo's shared root

Marlin derives the per-model subdirectory from:

- model name,
- `source_path`,
- a hash of the port list,
- a hash of `VerilatedModelConfig`.

Source:

- `/home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/marlin-verilator-0.10.0/src/lib.rs:609-631`

Since this repository always passes the same top-level artifact root (`target/verilator`), separate tests for the same model/config converge on the same marlin subdirectory beneath that root.

That sharing is good for incremental rebuild speed, but it also means all same-model cold starts must rely on marlin's locking to be correct.

### 5. Marlin does not serialize independent builds globally

Marlin's locks are per artifact directory. Different models or configs can still compile simultaneously. During a rebuild, marlin invokes Verilator as:

```text
verilator --cc -sv -j 0 --build ...
```

Source:

- `/home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/marlin-verilator-0.10.0/src/build_library.rs:442-447`

`-j 0` means "use as much parallelism as Verilator wants." So if several different models all cold-build at once, each build is itself fully parallel, which can easily saturate CPU and memory.

This does not explain the same-directory corruption race by itself, but it does explain why large parallel runs can still become unstable or noisy even after same-model locking is fixed.

### 6. Marlin models are intentionally not shareable across threads

The proc-macro-generated model struct includes:

```rust
_unsend_unsync: PhantomData<(Cell<()>, MutexGuard<'static, ()>)>
```

Source:

- `/home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/marlin-verilog-macro-builder-0.10.0/src/lib.rs:355-368`

That is a deliberate way to make the generated model type `!Send` and `!Sync`. So a marlin model is not meant to be shared across threads, and any robust strategy must keep each runtime/model instance private to the thread or process that owns it.

### 7. Tracing also touches global Verilator state

When tracing is enabled, marlin generates FFI that calls:

```cpp
Verilated::traceEverOn(everOn);
```

Source:

- `/home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/marlin-verilator-0.10.0/src/build_library.rs:28-62`

The default model config has `enable_tracing: false` (`/home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/marlin-verilator-0.10.0/src/lib.rs:188-215`), and `create_model_simple()` uses that default (`/home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/marlin-verilator-0.10.0/src/lib.rs:411-419`), so tracing is probably not the primary cause of the current `testbench` crashes. Still, it is another limitation to keep in mind if large concurrent traced runs are ever needed.

## Root Cause

The intermittent crashes are best explained by a combination of repository-level sharing and a marlin internal race:

### Primary root cause

**Marlin's same-directory thread guard is racy, and this repository makes same-directory reuse extremely common.**

More concretely:

1. the repository hardcodes one shared artifact root: `target/verilator`
2. tests create many separate runtimes and models in parallel
3. marlin maps same model/config to the same artifact directory
4. marlin's per-directory mutex initialization is not atomic
5. two threads in one process can bypass the intended thread serialization
6. both threads can then enter `build_library()`, which marlin explicitly says is not thread-safe

That is the strongest source-backed explanation for "sometimes" crashing when multiple instances run simultaneously.

### Secondary limitation

Even when two builds do **not** collide on the same artifact directory, marlin still lets many different models compile in parallel, and each compile uses Verilator `-j 0`. So heavy parallel runs can oversubscribe the machine badly.

This is more of a scalability limitation than the direct root-cause bug, but it still prevents robust high-density parallel execution.

## Current Limitations

1. **Shared artifact root in this repository**
   - `riscv_core` always uses `target/verilator`
   - no per-worker or per-process isolation exists today

2. **Non-atomic thread-lock initialization in marlin**
   - same-directory protection can fail inside one process

3. **No global build concurrency limit**
   - different models can all build concurrently
   - each build asks Verilator to parallelize aggressively

4. **Models/runtimes are not thread-shareable**
   - one runtime/model must stay private to one owner

5. **Tracing uses global Verilator state**
   - traced large-scale concurrency is harder than non-traced runs

## Recommended Solution

### Best immediate solution for this repository

**Stop sharing marlin artifact directories between concurrently running workers.**

In practice, the repository should make the marlin artifact root configurable and default it to a worker-unique directory, for example:

- `target/verilator/$PID`
- `target/verilator/$PID-$TEST_WORKER`
- `target/verilator/<uuid>`

That change should live in `riscv_core::create_runtime()` so every caller benefits automatically.

Why this is the best immediate solution:

- it avoids the known same-directory marlin race entirely
- it avoids cross-process collisions in the same checkout
- it does not require sharing marlin runtimes across threads
- it is robust even before marlin itself is patched upstream

The tradeoff is more duplicated build work, but it is the simplest path to correctness.

### Best long-term solution

For scalable high-density parallelism, the best end state is:

1. **Patch marlin upstream**
   - replace the `contains_key` + `insert` sequence with an atomic `entry(...).or_insert_with(...)` style initialization
   - keep same-directory builds serialized correctly inside one process

2. **Add a global build throttle**
   - cap the number of concurrent Verilator builds across different artifact directories
   - avoid many simultaneous `-j 0` compiles fighting each other

3. **Use immutable shared artifacts instead of mutable shared build directories**
   - ideally marlin would build into a temporary directory and publish artifacts atomically
   - workers would then only read completed artifacts

Until marlin provides that stronger shared-cache model, **per-worker artifact roots are the safest repository-level answer**.

### Operational guidance

To run many marlin-based tests robustly:

- keep each runtime/model private to one thread or process
- avoid sharing `target/verilator` across concurrent workers
- prefer process isolation over thread sharing for heavy runs
- pre-warm artifacts in a controlled step if test startup time matters
- treat traced runs as a stricter mode that may need separate isolation

## Conclusion

The instability is not just "Verilator is flaky under load." The strongest evidence points to a real marlin concurrency bug in the same-directory thread lock path, amplified by this repository's choice to send every runtime through one shared `target/verilator` root.

If the goal is robust parallel execution across many threads and processes, the repository should move to **worker-unique marlin artifact roots immediately**, and marlin itself should eventually be patched so that same-directory build locking is initialized atomically and large build fan-out can be throttled safely.
