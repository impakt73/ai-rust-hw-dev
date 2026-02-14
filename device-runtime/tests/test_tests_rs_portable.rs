mod common;

use common::{create_test_runtime, run_elf_until_halt, LONG_TIMEOUT};

#[test]
fn test_rust_bare_metal_elf() {
    let mut runtime = create_test_runtime();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "rust_test", LONG_TIMEOUT),
        Some(0x2a)
    );
}

#[test]
fn test_fp_math_elf() {
    let mut runtime = create_test_runtime();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "test_fp_math", LONG_TIMEOUT),
        Some(0x2a)
    );
}

#[test]
fn test_panic_handler() {
    let mut runtime = create_test_runtime();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "test_panic", LONG_TIMEOUT),
        Some(0xDEAD)
    );
}

#[test]
fn test_atomic_operations() {
    let mut runtime = create_test_runtime();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "test_atomic_simple", LONG_TIMEOUT),
        Some(0x2a)
    );
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "test_atomic", LONG_TIMEOUT),
        Some(0x2a)
    );
}
