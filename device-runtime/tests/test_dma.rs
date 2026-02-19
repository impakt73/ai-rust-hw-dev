mod common;

use bus_shared::Dma;
use common::{create_test_runtime_with_registrations, LONG_TIMEOUT};
use device_runtime::BusDeviceRegistration;
use riscv_shared::dma::DMA_BASE;

#[test]
fn test_dma_copy() {
    let mut runtime = create_test_runtime_with_registrations(Some(vec![BusDeviceRegistration {
        base_addr: DMA_BASE,
        device: Box::new(Dma::new()),
    }]));

    let elf_path =
        sim_tests::test_program_path("test_dma_copy").expect("Failed to find test_dma_copy");

    let entry = runtime.load_elf(&elf_path).expect("Failed to load ELF");
    runtime.boot_cpu(entry).expect("Failed to boot CPU");

    let tohost_value = common::wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT);

    assert_eq!(
        tohost_value,
        Some(42),
        "DMA test should exit with success code 42"
    );
}
