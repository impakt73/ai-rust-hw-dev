// Quick debug test for simple_test ELF loading
// Run with: cd /home/runner/work/ai-rust-hw-dev/ai-rust-hw-dev && cargo test --package device-runtime --test debug_simple_test -- --nocapture

mod common;

use common::{
    create_test_runtime_with_registrations, read_word_with_timeout, wait_for_cpu_halt,
    LONG_TIMEOUT, SHORT_TIMEOUT,
};
use device_runtime::{BusDeviceRegistration, DeviceRuntime};
use riscv_shared::FIFO_BASE;
use std::path::PathBuf;

fn create_test_runtime_with_fifo() -> Box<dyn DeviceRuntime> {
    create_test_runtime_with_registrations(Some(vec![BusDeviceRegistration {
        base_addr: FIFO_BASE,
        device: Box::new(bus_shared::Fifo::new_with_callback(
            std::sync::Arc::new(std::sync::Mutex::new(bus_shared::FifoDataSource::new())),
            Box::new(|_| {}),
        )),
    }]))
}

#[test]
fn debug_simple_test_elf() {
    let elf_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("rust-test-program/target/riscv32imafc-unknown-none-elf/release/simple_test");

    println!("ELF path: {}", elf_path.display());
    assert!(
        elf_path.exists(),
        "simple_test ELF not found at {}",
        elf_path.display()
    );

    let mut runtime = create_test_runtime_with_fifo();

    // Load ELF
    println!("Loading ELF...");
    let entry = runtime.load_elf(&elf_path).expect("Failed to load ELF");
    println!("ELF entry point: 0x{:08x}", entry);

    // Verify SRAM contents after loading
    println!("\n=== Verifying SRAM contents after ELF load ===");
    let sram_base = 0x5200_0000u32;
    for i in 0..8 {
        let addr = sram_base + i * 4;
        let word = read_word_with_timeout(runtime.as_mut(), addr, SHORT_TIMEOUT);
        println!("SRAM[0x{:08x}] = 0x{:08x}", addr, word);
    }

    // Boot CPU
    println!("\nBooting CPU at 0x{:08x}...", entry);
    runtime.boot_cpu(entry).expect("Failed to boot CPU");

    println!("Waiting for CPU halt...");
    let result = wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT);
    println!("Result: {:?}", result);

    assert_eq!(result, Some(0x2a), "Expected success code 42 (0x2a)");
}
