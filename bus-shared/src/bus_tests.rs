use super::*;
use crate::bus_device::{BusDevice, BusDeviceError, RegistrationError, SystemContext};
use std::collections::HashMap;

/// Mock device for testing device registration
struct MockDevice {
    size: u32,
    name: String,
    registers: HashMap<u32, u32>,
}

impl MockDevice {
    fn new(size: u32, name: &str) -> Self {
        MockDevice {
            size,
            name: name.to_string(),
            registers: HashMap::new(),
        }
    }
}

impl BusDevice for MockDevice {
    fn read_word(&mut self, _ctx: &mut SystemContext, offset: u32) -> Result<u32, BusDeviceError> {
        if offset >= self.size || !offset.is_multiple_of(4) {
            return Err(BusDeviceError::InvalidAddress { offset });
        }
        Ok(*self.registers.get(&offset).unwrap_or(&0))
    }

    fn write_word(
        &mut self,
        _ctx: &mut SystemContext,
        offset: u32,
        value: u32,
    ) -> Result<(), BusDeviceError> {
        if offset >= self.size || !offset.is_multiple_of(4) {
            return Err(BusDeviceError::InvalidAddress { offset });
        }
        self.registers.insert(offset, value);
        Ok(())
    }

    fn size(&self) -> u32 {
        self.size
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn reset(&mut self, _ctx: &mut SystemContext) {
        // MockDevice has no persistent state to reset
    }
}

/// Mock device with reset and clock_cycle tracking for testing lifecycle hooks
struct StatefulMockDevice {
    size: u32,
    name: String,
    reset_count: u32,
    clock_cycle_count: u32,
    registers: HashMap<u32, u32>,
}

impl StatefulMockDevice {
    fn new(size: u32, name: &str) -> Self {
        StatefulMockDevice {
            size,
            name: name.to_string(),
            reset_count: 0,
            clock_cycle_count: 0,
            registers: HashMap::new(),
        }
    }
}

impl BusDevice for StatefulMockDevice {
    fn read_word(&mut self, _ctx: &mut SystemContext, offset: u32) -> Result<u32, BusDeviceError> {
        if offset >= self.size || !offset.is_multiple_of(4) {
            return Err(BusDeviceError::InvalidAddress { offset });
        }
        // Special offsets to read internal state
        match offset {
            0x0 => Ok(self.reset_count),
            0x4 => Ok(self.clock_cycle_count),
            _ => Ok(*self.registers.get(&offset).unwrap_or(&0)),
        }
    }

    fn write_word(
        &mut self,
        _ctx: &mut SystemContext,
        offset: u32,
        value: u32,
    ) -> Result<(), BusDeviceError> {
        if offset >= self.size || !offset.is_multiple_of(4) {
            return Err(BusDeviceError::InvalidAddress { offset });
        }
        self.registers.insert(offset, value);
        Ok(())
    }

    fn reset(&mut self, _ctx: &mut SystemContext) {
        self.reset_count += 1;
        self.registers.clear();
    }

    fn clock_cycle(&mut self, _ctx: &mut SystemContext) {
        self.clock_cycle_count += 1;
    }

    fn size(&self) -> u32 {
        self.size
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[test]
fn test_device_registration_success() {
    let mut bus = SystemBus::new();

    // Register device at 0x6000_0000
    let dev = Box::new(MockDevice::new(256, "TestDevice"));
    let result = bus.register_device(0x6000_0000, dev);
    assert!(result.is_ok());

    // Verify device is registered
    let devices: Vec<_> = bus.registered_devices().collect();
    assert_eq!(devices.len(), 3); // DRAM, SimControl, TestDevice

    // Find our test device
    let test_device = devices.iter().find(|(_, _, name)| *name == "TestDevice");
    assert!(test_device.is_some());
    let (base, size, _) = test_device.unwrap();
    assert_eq!(*base, 0x6000_0000);
    assert_eq!(*size, 256);
}

#[test]
fn test_device_registration_overlap_error() {
    let mut bus = SystemBus::new();

    // Register first device at 0x6000_0000, size 256 bytes (0x6000_0000 - 0x6000_0100)
    let dev1 = Box::new(MockDevice::new(256, "Device1"));
    bus.register_device(0x6000_0000, dev1).unwrap();

    // Try to register overlapping device at 0x6000_0080 (overlaps with Device1)
    let dev2 = Box::new(MockDevice::new(256, "Device2"));
    let result = bus.register_device(0x6000_0080, dev2);

    assert!(matches!(
        result,
        Err(RegistrationError::AddressOverlap { .. })
    ));

    if let Err(RegistrationError::AddressOverlap {
        new_base,
        new_end,
        existing_base,
        existing_end,
        existing_name,
    }) = result
    {
        assert_eq!(new_base, 0x6000_0080);
        assert_eq!(new_end, 0x6000_0180);
        assert_eq!(existing_base, 0x6000_0000);
        assert_eq!(existing_end, 0x6000_0100);
        assert_eq!(existing_name, "Device1");
    }
}

#[test]
fn test_device_registration_dram_range_protected() {
    let mut bus = SystemBus::new();

    // Try to register device in DRAM range (0x8000_0000 to 0x8FFF_FFFF)
    let dev = Box::new(MockDevice::new(256, "TestDevice"));
    let result = bus.register_device(0x8000_0000, dev);

    assert!(matches!(
        result,
        Err(RegistrationError::AddressOverlap { .. })
    ));

    if let Err(RegistrationError::AddressOverlap { existing_name, .. }) = result {
        assert_eq!(existing_name, "DRAM");
    }
}

#[test]
fn test_device_registration_sim_control_range_protected() {
    let mut bus = SystemBus::new();

    // Try to register device in SimControl range (SIM_CONTROL_BASE - SIM_CONTROL_BASE+4)
    let dev = Box::new(MockDevice::new(4, "TestDevice"));
    let result = bus.register_device(SIM_CONTROL_BASE, dev);

    assert!(matches!(
        result,
        Err(RegistrationError::AddressOverlap { .. })
    ));

    if let Err(RegistrationError::AddressOverlap { existing_name, .. }) = result {
        assert_eq!(existing_name, "SimControl");
    }
}

#[test]
fn test_device_registration_lower_half_range_allowed() {
    let mut bus = SystemBus::new();

    // Device can be registered in a non-overlapping lower-half range.
    let dev = Box::new(MockDevice::new(256, "TestDevice"));
    let result = bus.register_device(0x6000_0000, dev);
    assert!(result.is_ok());
}

#[test]
fn test_device_registration_adjacent_devices_ok() {
    let mut bus = SystemBus::new();

    // Register first device at 0x6000_0000, size 256 bytes
    let dev1 = Box::new(MockDevice::new(256, "Device1"));
    bus.register_device(0x6000_0000, dev1).unwrap();

    // Register adjacent device at 0x6000_0100 (immediately after Device1)
    let dev2 = Box::new(MockDevice::new(256, "Device2"));
    let result = bus.register_device(0x6000_0100, dev2);
    assert!(result.is_ok());
}

#[test]
fn test_device_registration_invalid_alignment() {
    let mut bus = SystemBus::new();

    // Try to register device with non-word-aligned size
    let dev = Box::new(MockDevice::new(5, "TestDevice"));
    let result = bus.register_device(0x6000_0000, dev);

    assert!(matches!(
        result,
        Err(RegistrationError::InvalidAlignment { size: 5 })
    ));
}

#[test]
fn test_device_registration_invalid_base_alignment() {
    let mut bus = SystemBus::new();

    // Try to register device with non-word-aligned base address
    let dev = Box::new(MockDevice::new(256, "TestDevice"));
    let result = bus.register_device(0x6000_0001, dev);

    assert!(matches!(
        result,
        Err(RegistrationError::InvalidBaseAlignment {
            base_addr: 0x6000_0001
        })
    ));
}

#[test]
fn test_device_registration_zero_size() {
    let mut bus = SystemBus::new();

    // Try to register device with zero size
    let dev = Box::new(MockDevice::new(0, "TestDevice"));
    let result = bus.register_device(0x6000_0000, dev);

    assert!(matches!(result, Err(RegistrationError::ZeroSize)));
}

#[test]
fn test_device_read_write_routing() {
    let mut bus = SystemBus::new();

    // Register device at 0x6000_0000
    let dev = Box::new(MockDevice::new(256, "TestDevice"));
    bus.register_device(0x6000_0000, dev).unwrap();

    // Write a value to the device
    bus.write_word(0x6000_0000, 0x12345678);

    // Read it back
    let value = bus.read_word(0x6000_0000);
    assert_eq!(value, 0x12345678);

    // Write to different offset
    bus.write_word(0x6000_0004, 0xABCDEF00);
    let value = bus.read_word(0x6000_0004);
    assert_eq!(value, 0xABCDEF00);

    // Verify first value is still there
    let value = bus.read_word(0x6000_0000);
    assert_eq!(value, 0x12345678);
}

#[test]
#[should_panic(expected = "RTL peripheral read should be handled by Verilator")]
fn test_device_unmapped_low_address_read_panics() {
    let mut bus = SystemBus::new();

    // Lower-half addresses are RTL-routed and should not be handled by Rust SystemBus.
    let _ = bus.read_word(0x1000_0000);
}

#[test]
#[should_panic(expected = "RTL peripheral write should be handled by Verilator")]
fn test_device_unmapped_low_address_write_panics() {
    let mut bus = SystemBus::new();

    // Lower-half addresses are RTL-routed and should not be handled by Rust SystemBus.
    bus.write_word(0x1000_0000, 0x12345678);
}

#[test]
fn test_multiple_devices_independent() {
    let mut bus = SystemBus::new();

    // Register two devices at different addresses
    let dev1 = Box::new(MockDevice::new(256, "Device1"));
    bus.register_device(0x6000_0000, dev1).unwrap();

    let dev2 = Box::new(MockDevice::new(256, "Device2"));
    bus.register_device(0x6000_0100, dev2).unwrap();

    // Write different values to each device
    bus.write_word(0x6000_0000, 0x11111111);
    bus.write_word(0x6000_0100, 0x22222222);

    // Verify each device has its own value
    assert_eq!(bus.read_word(0x6000_0000), 0x11111111);
    assert_eq!(bus.read_word(0x6000_0100), 0x22222222);
}

#[test]
fn test_reset_all_devices_called() {
    let mut bus = SystemBus::new();

    // Register a stateful device
    let dev = Box::new(StatefulMockDevice::new(256, "StatefulDevice"));
    bus.register_device(0x6000_0000, dev).unwrap();

    // Initially reset_count should be 0
    assert_eq!(bus.read_word(0x6000_0000), 0);

    // Call reset_all_devices
    bus.reset_all_devices();

    // reset_count should now be 1
    assert_eq!(bus.read_word(0x6000_0000), 1);

    // Call reset again
    bus.reset_all_devices();

    // reset_count should now be 2
    assert_eq!(bus.read_word(0x6000_0000), 2);
}

#[test]
fn test_clock_cycle_all_devices_called() {
    let mut bus = SystemBus::new();

    // Register a stateful device
    let dev = Box::new(StatefulMockDevice::new(256, "StatefulDevice"));
    bus.register_device(0x6000_0000, dev).unwrap();

    // Initially clock_cycle_count should be 0
    assert_eq!(bus.read_word(0x6000_0004), 0);

    // Call clock_cycle_all_devices
    bus.clock_cycle_all_devices();

    // clock_cycle_count should now be 1
    assert_eq!(bus.read_word(0x6000_0004), 1);

    // Call clock_cycle multiple times
    for _ in 0..10 {
        bus.clock_cycle_all_devices();
    }

    // clock_cycle_count should now be 11
    assert_eq!(bus.read_word(0x6000_0004), 11);
}

#[test]
fn test_reset_clears_device_state() {
    let mut bus = SystemBus::new();

    // Register a stateful device
    let dev = Box::new(StatefulMockDevice::new(256, "StatefulDevice"));
    bus.register_device(0x6000_0000, dev).unwrap();

    // Write some values to the device
    bus.write_word(0x6000_0008, 0x12345678);
    bus.write_word(0x6000_000C, 0xABCDEF00);

    // Verify values are written
    assert_eq!(bus.read_word(0x6000_0008), 0x12345678);
    assert_eq!(bus.read_word(0x6000_000C), 0xABCDEF00);

    // Call reset (which should clear registers in StatefulMockDevice)
    bus.reset_all_devices();

    // Verify registers are cleared
    assert_eq!(bus.read_word(0x6000_0008), 0);
    assert_eq!(bus.read_word(0x6000_000C), 0);

    // But reset_count should be incremented
    assert_eq!(bus.read_word(0x6000_0000), 1);
}

#[test]
fn test_multiple_stateful_devices_independent() {
    let mut bus = SystemBus::new();

    // Register two stateful devices
    let dev1 = Box::new(StatefulMockDevice::new(256, "Device1"));
    bus.register_device(0x6000_0000, dev1).unwrap();

    let dev2 = Box::new(StatefulMockDevice::new(256, "Device2"));
    bus.register_device(0x6000_0100, dev2).unwrap();

    // Call clock_cycle multiple times
    for _ in 0..5 {
        bus.clock_cycle_all_devices();
    }

    // Both devices should have same clock_cycle_count
    assert_eq!(bus.read_word(0x6000_0004), 5);
    assert_eq!(bus.read_word(0x6000_0104), 5);

    // Call reset
    bus.reset_all_devices();

    // Both devices should have reset_count of 1
    assert_eq!(bus.read_word(0x6000_0000), 1);
    assert_eq!(bus.read_word(0x6000_0100), 1);

    // Clock cycle counts should still be 5 (reset doesn't clear them in our mock)
    assert_eq!(bus.read_word(0x6000_0004), 5);
    assert_eq!(bus.read_word(0x6000_0104), 5);
}

#[test]
fn test_device_with_default_reset_and_clock_cycle() {
    let mut bus = SystemBus::new();

    // Register a regular MockDevice (no-op reset/default clock_cycle implementations)
    let dev = Box::new(MockDevice::new(256, "RegularDevice"));
    bus.register_device(0x6000_0000, dev).unwrap();

    // Write a value
    bus.write_word(0x6000_0000, 0x12345678);

    // Call reset (MockDevice reset is a no-op)
    bus.reset_all_devices();

    // Value should still be there (MockDevice reset does nothing)
    assert_eq!(bus.read_word(0x6000_0000), 0x12345678);

    // Call clock_cycle (should do nothing for default implementation)
    bus.clock_cycle_all_devices();

    // Value should still be there
    assert_eq!(bus.read_word(0x6000_0000), 0x12345678);
}
