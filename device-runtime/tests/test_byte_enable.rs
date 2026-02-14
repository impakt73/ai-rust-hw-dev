//! Byte-enable ELF integration tests migrated from cpu-sim.

mod common;

use common::{create_test_runtime, run_elf_until_halt, LONG_TIMEOUT};

#[test]
fn test_byte_enable_heap_directly() {
    let mut runtime = create_test_runtime();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "test_heap_directly", LONG_TIMEOUT),
        Some(42)
    );
}

#[test]
fn test_byte_enable_stack_memory() {
    let mut runtime = create_test_runtime();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "test_stack_memory", LONG_TIMEOUT),
        Some(42)
    );
}
