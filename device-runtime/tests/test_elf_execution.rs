//! ELF Program Execution Integration Tests
//!
//! This module contains integration tests that execute pre-compiled ELF binaries
//! on the simulated RISC-V CPU. These tests verify the complete execution path
//! from ELF loading through program termination, including:
//!
//! - Memory allocation and heap management
//! - Byte-enable and byte-level memory operations
//! - Postcard serialization/deserialization
//! - Bare-metal Rust execution
//! - Floating-point math operations
//! - Panic handlers
//! - Atomic operations
//! - Memory pattern verification
//! - Image data handling
//!
//! Unlike programmatic instruction tests, these tests use actual compiled binaries
//! from the `rust-test-program` crate, providing end-to-end verification of the
//! toolchain, runtime, and CPU implementation.

mod common;

use common::{
    create_test_runtime_with_registrations, read_word_with_timeout, run_elf_until_halt,
    LONG_TIMEOUT, SHORT_TIMEOUT,
};
use device_runtime::{BusDeviceRegistration, DeviceRuntime};
use riscv_shared::FIFO_BASE;

fn create_test_runtime_with_fifo() -> Box<dyn DeviceRuntime> {
    create_test_runtime_with_registrations(Some(vec![BusDeviceRegistration {
        base_addr: FIFO_BASE,
        device: Box::new(bus_shared::Fifo::new_with_callback(
            std::sync::Arc::new(std::sync::Mutex::new(bus_shared::FifoDataSource::new())),
            Box::new(|_| {}),
        )),
    }]))
}

// ============================================================================
// Allocation and Heap Tests
// ============================================================================

#[test]
fn test_alloc_only() {
    let mut runtime = create_test_runtime_with_fifo();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "test_alloc_only", LONG_TIMEOUT),
        Some(42)
    );
}

#[test]
fn test_minimal_debug_test() {
    let mut runtime = create_test_runtime_with_fifo();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "minimal_debug_test", LONG_TIMEOUT),
        Some(42)
    );
}

#[test]
fn test_allocator() {
    let mut runtime = create_test_runtime_with_fifo();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "test_allocator", LONG_TIMEOUT),
        Some(42)
    );
}

#[test]
fn test_static_heap() {
    let mut runtime = create_test_runtime_with_fifo();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "test_static_heap", LONG_TIMEOUT),
        Some(42)
    );
}

// ============================================================================
// Byte-Enable and Memory Access Tests
// ============================================================================

#[test]
fn test_byte_enable_heap_directly() {
    let mut runtime = create_test_runtime_with_fifo();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "test_heap_directly", LONG_TIMEOUT),
        Some(42)
    );
}

#[test]
fn test_byte_enable_stack_memory() {
    let mut runtime = create_test_runtime_with_fifo();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "test_stack_memory", LONG_TIMEOUT),
        Some(42)
    );
}

#[test]
fn test_simple_byte_store() {
    let mut runtime = create_test_runtime_with_fifo();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "test_byte_store_simple", LONG_TIMEOUT),
        Some(42)
    );
}

// ============================================================================
// Postcard Serialization Tests
// ============================================================================

#[test]
fn test_minimal_postcard_byte_by_byte() {
    let mut runtime = create_test_runtime_with_fifo();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "minimal_postcard_test", LONG_TIMEOUT),
        Some(42)
    );
}

#[test]
fn test_minimal_postcard_word_packing() {
    let mut runtime = create_test_runtime_with_fifo();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "minimal_postcard_test2", LONG_TIMEOUT),
        Some(42)
    );
}

// ============================================================================
// Bare-Metal Rust Execution Tests
// ============================================================================

#[test]
fn test_rust_bare_metal_elf() {
    let mut runtime = create_test_runtime_with_fifo();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "rust_test", LONG_TIMEOUT),
        Some(0x2a)
    );
}

#[test]
fn test_simple_test_elf() {
    let mut runtime = create_test_runtime_with_fifo();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "simple_test", LONG_TIMEOUT),
        Some(0x2a)
    );
}

#[test]
fn test_fp_math_elf() {
    let mut runtime = create_test_runtime_with_fifo();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "test_fp_math", LONG_TIMEOUT),
        Some(0x2a)
    );
}

#[test]
fn test_panic_handler() {
    let mut runtime = create_test_runtime_with_fifo();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "test_panic", LONG_TIMEOUT),
        Some(0xDEAD)
    );
}

// ============================================================================
// Atomic Operations Tests
// ============================================================================

#[test]
fn test_atomic_simple() {
    let mut runtime = create_test_runtime_with_fifo();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "test_atomic_simple", LONG_TIMEOUT),
        Some(0x2a)
    );
}

#[test]
fn test_atomic_operations() {
    let mut runtime = create_test_runtime_with_fifo();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "test_atomic", LONG_TIMEOUT),
        Some(0x2a)
    );
}

// ============================================================================
// Memory Pattern and Data Verification Tests
// ============================================================================

#[test]
fn test_memory_pattern_dump() {
    let mut runtime = create_test_runtime_with_fifo();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "test_memory_pattern", LONG_TIMEOUT),
        Some(0x2a)
    );

    const TEST_MEMORY_BASE: u32 = 0x8000_1000;
    const TEST_PATTERN_SIZE: usize = 256;

    let mut memory_data = vec![0u8; TEST_PATTERN_SIZE];
    for (word_idx, chunk) in memory_data.chunks_exact_mut(4).enumerate() {
        let addr = TEST_MEMORY_BASE + (word_idx as u32) * 4;
        let word = read_word_with_timeout(runtime.as_mut(), addr, SHORT_TIMEOUT);
        chunk.copy_from_slice(&word.to_le_bytes());
    }

    assert_eq!(memory_data[0], 0xDE);
    assert_eq!(memory_data[1], 0xAD);
    assert_eq!(memory_data[2], 0xBE);
    assert_eq!(memory_data[3], 0xEF);

    for (idx, byte) in memory_data.iter().enumerate().skip(4).take(16) {
        assert_eq!(
            *byte, idx as u8,
            "Byte at offset {} should match pattern",
            idx
        );
    }
}

#[test]
fn test_image_data_dump() {
    let mut runtime = create_test_runtime_with_fifo();
    assert_eq!(
        run_elf_until_halt(runtime.as_mut(), "test_image_data", LONG_TIMEOUT),
        Some(0x2a)
    );

    const TEST_IMAGE_BASE: u32 = 0x8000_2000;
    const IMAGE_WIDTH: u32 = 4;

    let read_pixel = |runtime: &mut dyn DeviceRuntime, x: u32, y: u32| -> [u8; 4] {
        let offset = (y * IMAGE_WIDTH + x) * 4;
        read_word_with_timeout(runtime, TEST_IMAGE_BASE + offset, SHORT_TIMEOUT).to_le_bytes()
    };

    assert_eq!(read_pixel(runtime.as_mut(), 0, 0), [255, 0, 0, 255]);
    assert_eq!(read_pixel(runtime.as_mut(), 0, 1), [0, 255, 0, 255]);
    assert_eq!(read_pixel(runtime.as_mut(), 0, 2), [0, 0, 255, 255]);
    assert_eq!(read_pixel(runtime.as_mut(), 0, 3), [255, 255, 255, 255]);
}
