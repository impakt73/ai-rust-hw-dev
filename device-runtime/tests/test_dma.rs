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

    let tohost_value = common::run_elf_until_halt(runtime.as_mut(), "test_dma_copy", LONG_TIMEOUT);

    assert_eq!(
        tohost_value,
        Some(42),
        "DMA test should exit with success code 42"
    );
}
