//! Integration tests for BusDevice reset() and clock_cycle() lifecycle hooks.

mod common;

use bus_shared::{BusDevice, BusDeviceError, SystemContext, DRAM_BASE};
use common::{
    append_tohost_termination, create_test_runtime_with_registrations, instructions_to_bytes,
    load_and_boot, wait_for_cpu_halt,
};
use common::{LONG_TIMEOUT, TEST_BOOT_PC};
use device_runtime::BusDeviceRegistration;
use riscv_core::instruction::{addi, lui};
use riscv_shared::sim_control::SUCCESS_CODE;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

struct LifecycleTestDevice {
    size: u32,
    reset_count: Arc<AtomicU32>,
    clock_cycle_count: Arc<AtomicU32>,
}

impl LifecycleTestDevice {
    fn new(size: u32, reset_count: Arc<AtomicU32>, clock_cycle_count: Arc<AtomicU32>) -> Self {
        Self {
            size,
            reset_count,
            clock_cycle_count,
        }
    }
}

impl BusDevice for LifecycleTestDevice {
    fn read_word(&mut self, _ctx: &mut SystemContext, offset: u32) -> Result<u32, BusDeviceError> {
        match offset {
            0x0 => Ok(self.reset_count.load(Ordering::SeqCst)),
            0x4 => Ok(self.clock_cycle_count.load(Ordering::SeqCst)),
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }

    fn write_word(
        &mut self,
        _ctx: &mut SystemContext,
        _offset: u32,
        _value: u32,
    ) -> Result<(), BusDeviceError> {
        Ok(())
    }

    fn reset(&mut self, _ctx: &mut SystemContext) {
        self.reset_count.fetch_add(1, Ordering::SeqCst);
    }

    fn clock_cycle(&mut self, _ctx: &mut SystemContext) {
        self.clock_cycle_count.fetch_add(1, Ordering::SeqCst);
    }

    fn size(&self) -> u32 {
        self.size
    }

    fn name(&self) -> &str {
        "LifecycleTestDevice"
    }
}

fn create_runtime_with_devices(
    registrations: Vec<BusDeviceRegistration>,
) -> Box<dyn device_runtime::DeviceRuntime> {
    create_test_runtime_with_registrations(Some(registrations))
}

#[test]
fn test_device_reset_called_during_simulation() {
    let reset_count = Arc::new(AtomicU32::new(0));
    let clock_cycle_count = Arc::new(AtomicU32::new(0));

    let device = Box::new(LifecycleTestDevice::new(
        256,
        Arc::clone(&reset_count),
        Arc::clone(&clock_cycle_count),
    ));
    let mut runtime = create_runtime_with_devices(vec![BusDeviceRegistration {
        base_addr: 0x7000_0000,
        device,
    }]);

    let mut instructions = vec![lui(15, DRAM_BASE), addi(10, 0, SUCCESS_CODE as i32)];
    append_tohost_termination(&mut instructions, 11, 10, SUCCESS_CODE);
    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program_bytes);
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );
    assert!(
        reset_count.load(Ordering::SeqCst) >= 1,
        "reset() should be called at least once"
    );
    assert!(
        clock_cycle_count.load(Ordering::SeqCst) > 0,
        "clock_cycle() should be called at least once"
    );
}

#[test]
fn test_device_clock_cycle_called_every_cycle() {
    let reset_count = Arc::new(AtomicU32::new(0));
    let clock_cycle_count = Arc::new(AtomicU32::new(0));

    let device = Box::new(LifecycleTestDevice::new(
        256,
        Arc::clone(&reset_count),
        Arc::clone(&clock_cycle_count),
    ));
    let mut runtime = create_runtime_with_devices(vec![BusDeviceRegistration {
        base_addr: 0x7000_0000,
        device,
    }]);

    let mut instructions = vec![
        lui(15, DRAM_BASE),
        addi(0, 0, 0),
        addi(0, 0, 0),
        addi(0, 0, 0),
        addi(10, 0, 42),
    ];
    append_tohost_termination(&mut instructions, 11, 10, 42);
    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program_bytes);
    assert_eq!(wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT), Some(42));
    assert_eq!(
        reset_count.load(Ordering::SeqCst),
        1,
        "reset() should be called once during initialization"
    );
    assert!(
        clock_cycle_count.load(Ordering::SeqCst) > 0,
        "clock_cycle() should be called every cycle"
    );
}

#[test]
fn test_multiple_devices_receive_lifecycle_calls() {
    let reset_count_1 = Arc::new(AtomicU32::new(0));
    let clock_cycle_count_1 = Arc::new(AtomicU32::new(0));
    let reset_count_2 = Arc::new(AtomicU32::new(0));
    let clock_cycle_count_2 = Arc::new(AtomicU32::new(0));

    let device1 = Box::new(LifecycleTestDevice::new(
        256,
        Arc::clone(&reset_count_1),
        Arc::clone(&clock_cycle_count_1),
    ));
    let device2 = Box::new(LifecycleTestDevice::new(
        256,
        Arc::clone(&reset_count_2),
        Arc::clone(&clock_cycle_count_2),
    ));
    let mut runtime = create_runtime_with_devices(vec![
        BusDeviceRegistration {
            base_addr: 0x7000_0000,
            device: device1,
        },
        BusDeviceRegistration {
            base_addr: 0x6000_0000,
            device: device2,
        },
    ]);

    let mut instructions = vec![lui(15, DRAM_BASE), addi(10, 0, SUCCESS_CODE as i32)];
    append_tohost_termination(&mut instructions, 11, 10, SUCCESS_CODE);
    let program_bytes = instructions_to_bytes(&instructions);

    load_and_boot(runtime.as_mut(), TEST_BOOT_PC, &program_bytes);
    assert_eq!(
        wait_for_cpu_halt(runtime.as_mut(), LONG_TIMEOUT),
        Some(SUCCESS_CODE)
    );

    assert_eq!(
        reset_count_1.load(Ordering::SeqCst),
        reset_count_2.load(Ordering::SeqCst),
        "Both devices should receive same reset count"
    );
    assert_eq!(
        clock_cycle_count_1.load(Ordering::SeqCst),
        clock_cycle_count_2.load(Ordering::SeqCst),
        "Both devices should receive same clock_cycle count"
    );
}
