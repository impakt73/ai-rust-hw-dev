//! Portable minimal ELF subset migrated from cpu-sim.

mod common;

use common::{create_test_runtime, run_elf_until_halt, LONG_TIMEOUT};

#[test]
fn test_minimal_postcard_byte_by_byte() {
    let mut runtime = create_test_runtime();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "minimal_postcard_test", LONG_TIMEOUT),
        Some(42)
    );
}

#[test]
fn test_minimal_postcard_word_packing() {
    let mut runtime = create_test_runtime();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "minimal_postcard_test2", LONG_TIMEOUT),
        Some(42)
    );
}
