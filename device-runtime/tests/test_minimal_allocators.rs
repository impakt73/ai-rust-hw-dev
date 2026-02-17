//! Additional minimal allocator/heap ELF tests migrated from cpu-sim.

mod common;

use common::{create_test_runtime, run_elf_until_halt, LONG_TIMEOUT};

#[test]
fn test_minimal_debug_test() {
    let mut runtime = create_test_runtime();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "minimal_debug_test", LONG_TIMEOUT),
        Some(42)
    );
}

#[test]
fn test_allocator() {
    let mut runtime = create_test_runtime();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "test_allocator", LONG_TIMEOUT),
        Some(42)
    );
}

#[test]
fn test_static_heap() {
    let mut runtime = create_test_runtime();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "test_static_heap", LONG_TIMEOUT),
        Some(42)
    );
}
