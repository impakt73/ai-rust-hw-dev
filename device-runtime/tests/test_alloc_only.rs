//! Allocation-only ELF integration test migrated from cpu-sim.

mod common;

use common::{create_test_runtime, run_elf_until_halt, LONG_TIMEOUT};

#[test]
fn test_alloc_only() {
    let mut runtime = create_test_runtime();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "test_alloc_only", LONG_TIMEOUT),
        Some(42)
    );
}
