mod common;

use bus_shared::{BusDevice, BusDeviceError, SystemContext};
use common::{instructions_to_bytes, load_and_boot, wait_for_cpu_halt, LONG_TIMEOUT, TEST_BOOT_PC};
use device_runtime::{create_device_runtime, BusDeviceRegistration, DeviceRuntimeType};
use riscv_core::instruction::{addi, ebreak, lui, lw, sw};
use riscv_shared::bus::SIM_CONTROL_BASE;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

const DUMMY_DEVICE_BASE: u32 = 0x4000_5000;

struct DummyBusDevice {
    value: u32,
    write_count: Arc<AtomicU32>,
    read_count: Arc<AtomicU32>,
}

impl BusDevice for DummyBusDevice {
    fn read_word(&mut self, _ctx: &mut SystemContext, offset: u32) -> Result<u32, BusDeviceError> {
        if offset != 0 {
            return Err(BusDeviceError::InvalidAddress { offset });
        }
        self.read_count.fetch_add(1, Ordering::Relaxed);
        Ok(self.value)
    }

    fn write_word(
        &mut self,
        _ctx: &mut SystemContext,
        offset: u32,
        value: u32,
    ) -> Result<(), BusDeviceError> {
        if offset != 0 {
            return Err(BusDeviceError::InvalidAddress { offset });
        }
        self.value = value;
        self.write_count.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn size(&self) -> u32 {
        4
    }

    fn name(&self) -> &str {
        "dummy-device"
    }

    fn reset(&mut self, _ctx: &mut SystemContext) {
        self.value = 0;
    }
}

#[test]
fn test_custom_bus_device_registration_and_access() {
    let write_count = Arc::new(AtomicU32::new(0));
    let read_count = Arc::new(AtomicU32::new(0));
    let bus_device = DummyBusDevice {
        value: 0,
        write_count: Arc::clone(&write_count),
        read_count: Arc::clone(&read_count),
    };

    let mut runtime = create_device_runtime(
        DeviceRuntimeType::Sim,
        vec![BusDeviceRegistration {
            base_addr: DUMMY_DEVICE_BASE,
            device: Box::new(bus_device),
        }],
    )
    .expect("Failed to create simulator runtime");

    let instructions = vec![
        lui(15, DUMMY_DEVICE_BASE),
        addi(14, 0, 42),
        sw(15, 14, 0),
        lw(13, 15, 0),
        lui(12, SIM_CONTROL_BASE),
        sw(12, 13, 0),
        ebreak(),
    ];
    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program_bytes);

    assert_eq!(wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT), Some(42));
    assert!(write_count.load(Ordering::Relaxed) > 0);
    assert!(read_count.load(Ordering::Relaxed) > 0);
}
