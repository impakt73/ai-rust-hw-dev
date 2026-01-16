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
}

#[test]
fn test_device_registration_success() {
    let mut bus = SystemBus::new();

    // Register device at 0x5000_0000
    let dev = Box::new(MockDevice::new(256, "TestDevice"));
    let result = bus.register_device(0x5000_0000, dev);
    assert!(result.is_ok());

    // Verify device is registered
    let devices: Vec<_> = bus.registered_devices().collect();
    assert_eq!(devices.len(), 4); // DRAM, FIFO, SimControl, TestDevice

    // Find our test device
    let test_device = devices.iter().find(|(_, _, name)| *name == "TestDevice");
    assert!(test_device.is_some());
    let (base, size, _) = test_device.unwrap();
    assert_eq!(*base, 0x5000_0000);
    assert_eq!(*size, 256);
}

#[test]
fn test_device_registration_overlap_error() {
    let mut bus = SystemBus::new();

    // Register first device at 0x5000_0000, size 256 bytes (0x5000_0000 - 0x5000_0100)
    let dev1 = Box::new(MockDevice::new(256, "Device1"));
    bus.register_device(0x5000_0000, dev1).unwrap();

    // Try to register overlapping device at 0x5000_0080 (overlaps with Device1)
    let dev2 = Box::new(MockDevice::new(256, "Device2"));
    let result = bus.register_device(0x5000_0080, dev2);

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
        assert_eq!(new_base, 0x5000_0080);
        assert_eq!(new_end, 0x5000_0180);
        assert_eq!(existing_base, 0x5000_0000);
        assert_eq!(existing_end, 0x5000_0100);
        assert_eq!(existing_name, "Device1");
    }
}

#[test]
fn test_device_registration_fifo_range_protected() {
    let mut bus = SystemBus::new();

    // Try to register device in FIFO range (0x4000_0000 - 0x4000_0008)
    let dev = Box::new(MockDevice::new(8, "TestDevice"));
    let result = bus.register_device(0x4000_0000, dev);

    assert!(matches!(
        result,
        Err(RegistrationError::AddressOverlap { .. })
    ));

    if let Err(RegistrationError::AddressOverlap { existing_name, .. }) = result {
        assert_eq!(existing_name, "FIFO");
    }
}

#[test]
fn test_device_registration_sim_control_range_protected() {
    let mut bus = SystemBus::new();

    // Try to register device in SimControl range (0x1000_0000 - 0x1000_0004)
    let dev = Box::new(MockDevice::new(4, "TestDevice"));
    let result = bus.register_device(0x1000_0000, dev);

    assert!(matches!(
        result,
        Err(RegistrationError::AddressOverlap { .. })
    ));

    if let Err(RegistrationError::AddressOverlap { existing_name, .. }) = result {
        assert_eq!(existing_name, "SimControl");
    }
}

#[test]
fn test_device_registration_dram_range_protected() {
    let mut bus = SystemBus::new();

    // Try to register device in DRAM range (0x8000_0000 and above)
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
fn test_device_registration_adjacent_devices_ok() {
    let mut bus = SystemBus::new();

    // Register first device at 0x5000_0000, size 256 bytes
    let dev1 = Box::new(MockDevice::new(256, "Device1"));
    bus.register_device(0x5000_0000, dev1).unwrap();

    // Register adjacent device at 0x5000_0100 (immediately after Device1)
    let dev2 = Box::new(MockDevice::new(256, "Device2"));
    let result = bus.register_device(0x5000_0100, dev2);
    assert!(result.is_ok());
}

#[test]
fn test_device_registration_invalid_alignment() {
    let mut bus = SystemBus::new();

    // Try to register device with non-word-aligned size
    let dev = Box::new(MockDevice::new(5, "TestDevice"));
    let result = bus.register_device(0x5000_0000, dev);

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
    let result = bus.register_device(0x5000_0001, dev);

    assert!(matches!(
        result,
        Err(RegistrationError::InvalidBaseAlignment {
            base_addr: 0x5000_0001
        })
    ));
}

#[test]
fn test_device_registration_zero_size() {
    let mut bus = SystemBus::new();

    // Try to register device with zero size
    let dev = Box::new(MockDevice::new(0, "TestDevice"));
    let result = bus.register_device(0x5000_0000, dev);

    assert!(matches!(result, Err(RegistrationError::ZeroSize)));
}

#[test]
fn test_device_read_write_routing() {
    let mut bus = SystemBus::new();

    // Register device at 0x5000_0000
    let dev = Box::new(MockDevice::new(256, "TestDevice"));
    bus.register_device(0x5000_0000, dev).unwrap();

    // Write a value to the device
    bus.write_word(0x5000_0000, 0x12345678);

    // Read it back
    let value = bus.read_word(0x5000_0000);
    assert_eq!(value, 0x12345678);

    // Write to different offset
    bus.write_word(0x5000_0004, 0xABCDEF00);
    let value = bus.read_word(0x5000_0004);
    assert_eq!(value, 0xABCDEF00);

    // Verify first value is still there
    let value = bus.read_word(0x5000_0000);
    assert_eq!(value, 0x12345678);
}

#[test]
fn test_device_unmapped_address_read_returns_zero() {
    let mut bus = SystemBus::new();

    // Read from unmapped address
    let value = bus.read_word(0x6000_0000);
    assert_eq!(value, 0);
}

#[test]
fn test_device_unmapped_address_write_discarded() {
    let mut bus = SystemBus::new();

    // Write to unmapped address (should not panic, just log warning)
    bus.write_word(0x6000_0000, 0x12345678);

    // Verify write was discarded (read returns 0)
    let value = bus.read_word(0x6000_0000);
    assert_eq!(value, 0);
}

#[test]
fn test_multiple_devices_independent() {
    let mut bus = SystemBus::new();

    // Register two devices at different addresses
    let dev1 = Box::new(MockDevice::new(256, "Device1"));
    bus.register_device(0x5000_0000, dev1).unwrap();

    let dev2 = Box::new(MockDevice::new(256, "Device2"));
    bus.register_device(0x6000_0000, dev2).unwrap();

    // Write different values to each device
    bus.write_word(0x5000_0000, 0x11111111);
    bus.write_word(0x6000_0000, 0x22222222);

    // Verify each device has its own value
    assert_eq!(bus.read_word(0x5000_0000), 0x11111111);
    assert_eq!(bus.read_word(0x6000_0000), 0x22222222);
}
