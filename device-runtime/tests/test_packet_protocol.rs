mod common;

use bus_shared::{Fifo, FifoDataSource};
use common::{
    create_test_runtime_with_registrations, load_and_boot_elf, resolve_test_elf_path, LONG_TIMEOUT,
};
use device_runtime::BusDeviceRegistration;
use riscv_shared::FIFO_BASE;
use std::sync::{Arc, Mutex};

#[test]
fn test_println_macro() {
    let elf_path = resolve_test_elf_path("println_test");

    // Collect FIFO TX bytes from CPU
    let fifo_data: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let fifo_data_clone = fifo_data.clone();
    let fifo_source = Arc::new(Mutex::new(FifoDataSource::new()));
    let fifo_callback = move |byte: u8| {
        fifo_data_clone
            .lock()
            .expect("fifo_data lock poisoned in callback")
            .push(byte);
    };

    let mut runtime = create_test_runtime_with_registrations(Some(vec![BusDeviceRegistration {
        base_addr: FIFO_BASE,
        device: Box::new(Fifo::new_with_callback(
            fifo_source,
            Box::new(fifo_callback),
        )),
    }]));

    load_and_boot_elf(runtime.as_mut(), &elf_path);
    let tohost_value = common::wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT);

    assert_eq!(
        tohost_value,
        Some(42),
        "Program should complete with success code 42"
    );

    let fifo_bytes = fifo_data.lock().expect("fifo_data lock poisoned");
    let output = core::str::from_utf8(fifo_bytes.as_slice()).expect("FIFO should contain UTF-8");
    assert!(output.contains("Hello from RISC-V CPU!\n"));
    assert!(output.contains("The answer is 42\n"));
    assert!(output.contains("Testing println macro\n"));
}
