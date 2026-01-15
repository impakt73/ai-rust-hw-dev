# Bus Device Registration System - Technical Implementation Plan

## Overview

This document provides a comprehensive technical plan for implementing a dynamic bus device registration system in the `cpu-sim` crate. The system will allow abstract devices to be registered onto the system bus with non-overlapping address ranges, enabling future extensibility (e.g., video devices, custom peripherals) while maintaining backward compatibility with existing code.

## Current Architecture

### Existing System Bus Implementation

The current `SystemBus` in `cpu-sim/src/bus.rs` uses a hardcoded approach:

```rust
pub struct SystemBus {
    pub dram: Dram,
    pub fifo: Fifo,
}

impl SystemBus {
    pub fn read_word(&mut self, addr: u32) -> u32 {
        match addr {
            // FIFO DATA register
            a if a == FIFO_BASE + FIFO_DATA_OFFSET => self.fifo.read_data(),
            // FIFO STATUS register
            a if a == FIFO_BASE + FIFO_STATUS_OFFSET => self.fifo.read_status(),
            // Default: DRAM
            _ => self.dram.read_word(addr),
        }
    }
    // ... similar for write_word, read_byte, etc.
}
```

**Hardcoded Memory Map:**
- **FIFO Base Address**: `0x4000_0000`
  - DATA offset: `0x00` (read/write)
  - STATUS offset: `0x04` (read-only)
- **DRAM**: Everything else (default)

### Problems with Current Architecture

1. **No Extensibility**: Adding new devices requires modifying `SystemBus` code
2. **Address Conflicts**: No validation to prevent overlapping device ranges
3. **Tight Coupling**: `SimulatorView` directly exposes FIFO and DRAM internals
4. **Hardcoded Routing**: Memory map is not configurable or documented

## Design Goals

1. **Extensibility**: Allow registration of custom devices without modifying core code
2. **Safety**: Validate that device address ranges do not overlap
3. **Backward Compatibility**: Preserve all existing functionality and tests
4. **Lifetime Safety**: Ensure devices live as long as the simulator
5. **Relative Addressing**: Device implementations use offsets relative to their base address
6. **Clean API**: Simple interface for both internal and external device registration
7. **Error Handling**: Proper error types for invalid operations (read-only writes, write-only reads, invalid addresses)

## Proposed Architecture

### 1. BusDevice Trait

Define a trait that all bus-connected devices must implement:

```rust
/// Error types for bus device operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BusDeviceError {
    /// Attempted to write to a read-only register
    WriteToReadOnly { offset: u32 },
    /// Attempted to read from a write-only register
    ReadFromWriteOnly { offset: u32 },
    /// Invalid address/offset within the device's address range
    InvalidAddress { offset: u32 },
    /// Other device-specific errors
    Other(String),
}

impl std::fmt::Display for BusDeviceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BusDeviceError::WriteToReadOnly { offset } => {
                write!(f, "Write to read-only register at offset 0x{:08x}", offset)
            }
            BusDeviceError::ReadFromWriteOnly { offset } => {
                write!(f, "Read from write-only register at offset 0x{:08x}", offset)
            }
            BusDeviceError::InvalidAddress { offset } => {
                write!(f, "Invalid address offset 0x{:08x}", offset)
            }
            BusDeviceError::Other(msg) => write!(f, "Device error: {}", msg),
        }
    }
}

impl std::error::Error for BusDeviceError {}

/// Trait for devices that can be registered on the system bus
///
/// All operations use word granularity (u32) and operate on offsets
/// relative to the device's base address in the system memory map.
pub trait BusDevice {
    /// Read a 32-bit word from the device at the given offset
    ///
    /// # Arguments
    /// * `offset` - Byte offset relative to the device's base address
    ///
    /// # Returns
    /// * `Ok(u32)` - The word value read from the device
    /// * `Err(BusDeviceError)` - If the read is invalid (e.g., write-only register)
    fn read_word(&mut self, offset: u32) -> Result<u32, BusDeviceError>;

    /// Write a 32-bit word to the device at the given offset
    ///
    /// # Arguments
    /// * `offset` - Byte offset relative to the device's base address
    /// * `value` - The 32-bit value to write
    ///
    /// # Returns
    /// * `Ok(())` - Write successful
    /// * `Err(BusDeviceError)` - If the write is invalid (e.g., read-only register)
    fn write_word(&mut self, offset: u32, value: u32) -> Result<(), BusDeviceError>;

    /// Get the size of the device's address space in bytes
    ///
    /// This is used during registration to validate address ranges.
    /// Must be a multiple of 4 (word-aligned).
    fn size(&self) -> u32;

    /// Optional: Get a human-readable name for this device (for debugging/logging)
    fn name(&self) -> &str {
        "Unknown Device"
    }
}
```

### 2. Device Registration System

The `SystemBus` will maintain a collection of registered devices with their address ranges:

```rust
/// Represents a registered device with its address range
struct RegisteredDevice {
    base_addr: u32,
    size: u32,
    device: Box<dyn BusDevice>,
}

/// Error types for device registration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistrationError {
    /// Device address range overlaps with an existing device
    AddressOverlap {
        new_base: u32,
        new_end: u32,
        existing_base: u32,
        existing_end: u32,
        existing_name: String,
    },
    /// Device size is not word-aligned (must be multiple of 4)
    InvalidAlignment { size: u32 },
    /// Device size is zero
    ZeroSize,
}

pub struct SystemBus {
    devices: Vec<RegisteredDevice>,
    // Internal devices (backward compatibility)
    dram: Dram,
    fifo: Fifo,
}

impl SystemBus {
    /// Create a new system bus with default internal devices
    ///
    /// Automatically registers DRAM and FIFO at their standard addresses:
    /// - FIFO: 0x4000_0000 - 0x4000_0007 (8 bytes)
    /// - DRAM: 0x0000_0000 - 0xFFFF_FFFF (entire address space, lowest priority)
    pub fn new() -> Self {
        // Implementation details in next section
    }

    /// Register a custom device at the specified base address
    ///
    /// # Arguments
    /// * `base_addr` - Base address for the device in the system memory map
    /// * `device` - The device to register (must implement BusDevice trait)
    ///
    /// # Returns
    /// * `Ok(())` - Device registered successfully
    /// * `Err(RegistrationError)` - Address range conflicts with existing device
    ///
    /// # Errors
    /// Returns error if:
    /// - Device address range overlaps with existing device
    /// - Device size is not word-aligned (multiple of 4)
    /// - Device size is zero
    pub fn register_device(
        &mut self,
        base_addr: u32,
        device: Box<dyn BusDevice>,
    ) -> Result<(), RegistrationError> {
        // Validation and registration logic
    }

    /// Read a 32-bit word from the bus
    ///
    /// Routes the request to the appropriate device based on address.
    /// If no device matches, logs a warning and returns 0.
    pub fn read_word(&mut self, addr: u32) -> u32 {
        // Route to appropriate device
    }

    /// Write a 32-bit word to the bus
    ///
    /// Routes the request to the appropriate device based on address.
    /// If no device matches, logs a warning and discards the write.
    pub fn write_word(&mut self, addr: u32, value: u32) {
        // Route to appropriate device
    }
}
```

### 3. DRAM as BusDevice

Convert the existing DRAM to implement `BusDevice`:

```rust
impl BusDevice for Dram {
    fn read_word(&mut self, offset: u32) -> Result<u32, BusDeviceError> {
        // Use existing read_word logic
        Ok(self.read_word_internal(offset))
    }

    fn write_word(&mut self, offset: u32, value: u32) -> Result<(), BusDeviceError> {
        // Use existing write_word logic
        self.write_word_internal(offset, value);
        Ok(())
    }

    fn size(&self) -> u32 {
        // DRAM covers the entire address space
        0xFFFF_FFFF
    }

    fn name(&self) -> &str {
        "DRAM"
    }
}
```

**Note**: DRAM will be registered with **lowest priority** so it acts as a catch-all for unmapped addresses. The device routing algorithm searches registered devices in reverse order, so DRAM (registered first) is checked last.

### 4. FIFO as BusDevice

Convert the existing FIFO to implement `BusDevice`:

```rust
impl BusDevice for Fifo {
    fn read_word(&mut self, offset: u32) -> Result<u32, BusDeviceError> {
        match offset {
            0x00 => Ok(self.read_data()),
            0x04 => Ok(self.read_status()),
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }

    fn write_word(&mut self, offset: u32, value: u32) -> Result<(), BusDeviceError> {
        match offset {
            0x00 => {
                self.write_data(value);
                Ok(())
            }
            0x04 => {
                // STATUS register is read-only
                Err(BusDeviceError::WriteToReadOnly { offset })
            }
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }

    fn size(&self) -> u32 {
        // FIFO has 2 registers: DATA (0x00) and STATUS (0x04)
        // Total size: 8 bytes (2 words)
        8
    }

    fn name(&self) -> &str {
        "FIFO"
    }
}
```

### 5. System Bus Implementation

The `SystemBus` will own DRAM and FIFO internally and automatically register them during initialization:

```rust
impl SystemBus {
    pub fn new() -> Self {
        let mut bus = SystemBus {
            devices: Vec::new(),
            dram: Dram::new(),
            fifo: Fifo::new(),
        };

        // Auto-register internal devices
        // Note: We cannot move dram/fifo into Box since we need them for SimulatorView
        // Instead, we'll use a different approach - see "Internal vs External Devices" section

        bus
    }

    fn find_device(&mut self, addr: u32) -> Option<(&mut dyn BusDevice, u32)> {
        // Search devices in reverse order (last registered = highest priority)
        // This allows DRAM (registered first) to act as catch-all
        for registered in self.devices.iter_mut().rev() {
            let base = registered.base_addr;
            let end = base.saturating_add(registered.size);
            
            if addr >= base && addr < end {
                let offset = addr - base;
                return Some((registered.device.as_mut(), offset));
            }
        }
        None
    }

    pub fn read_word(&mut self, addr: u32) -> u32 {
        if let Some((device, offset)) = self.find_device(addr) {
            match device.read_word(offset) {
                Ok(value) => value,
                Err(e) => {
                    log::warn!("Bus read error at 0x{:08x}: {}", addr, e);
                    0
                }
            }
        } else {
            log::warn!("Bus read from unmapped address 0x{:08x}, returning 0", addr);
            0
        }
    }

    pub fn write_word(&mut self, addr: u32, value: u32) {
        if let Some((device, offset)) = self.find_device(addr) {
            if let Err(e) = device.write_word(offset, value) {
                log::warn!("Bus write error at 0x{:08x}: {}", addr, e);
            }
        } else {
            log::warn!(
                "Bus write to unmapped address 0x{:08x} (value=0x{:08x}), discarding",
                addr, value
            );
        }
    }
}
```

### 6. Internal vs External Devices

**Challenge**: `SimulatorView` needs direct access to DRAM and FIFO for backward compatibility, but we also want them to be `BusDevice` implementations registered on the bus.

**Solution**: Use a hybrid approach:

1. **Internal Devices** (DRAM, FIFO): Owned directly by `SystemBus` as fields
2. **Device Wrappers**: Create wrapper structs that implement `BusDevice` and delegate to internal devices
3. **Registration**: Register the wrappers instead of moving the internal devices

```rust
/// Wrapper that provides BusDevice access to the internal DRAM
struct DramBusAdapter<'a> {
    dram: &'a mut Dram,
}

impl<'a> BusDevice for DramBusAdapter<'a> {
    fn read_word(&mut self, offset: u32) -> Result<u32, BusDeviceError> {
        Ok(self.dram.read_word(offset))
    }
    
    fn write_word(&mut self, offset: u32, value: u32) -> Result<(), BusDeviceError> {
        self.dram.write_word(offset, value);
        Ok(())
    }
    
    fn size(&self) -> u32 {
        0xFFFF_FFFF
    }
    
    fn name(&self) -> &str {
        "DRAM"
    }
}

// Similar wrapper for FIFO
```

**Alternative (Simpler) Approach**: Keep the current hardcoded routing for DRAM and FIFO in `SystemBus`, and only use the dynamic device registration for external devices. This maintains full backward compatibility while still enabling extensibility.

```rust
pub struct SystemBus {
    // Internal devices (not moved into the registry)
    pub(crate) dram: Dram,
    pub(crate) fifo: Fifo,
    
    // External devices (registered dynamically)
    devices: Vec<RegisteredDevice>,
}

impl SystemBus {
    pub fn read_word(&mut self, addr: u32) -> u32 {
        // First, check registered external devices
        if let Some((device, offset)) = self.find_device(addr) {
            return match device.read_word(offset) {
                Ok(value) => value,
                Err(e) => {
                    log::warn!("Bus read error at 0x{:08x}: {}", addr, e);
                    0
                }
            };
        }
        
        // Fall back to hardcoded FIFO/DRAM routing
        const FIFO_BASE: u32 = 0x4000_0000;
        match addr {
            a if a == FIFO_BASE => self.fifo.read_data(),
            a if a == FIFO_BASE + 4 => self.fifo.read_status(),
            _ => self.dram.read_word(addr),
        }
    }
}
```

**Recommendation**: Use the **simpler approach** for this initial implementation. It:
- Maintains 100% backward compatibility
- Keeps `SimulatorView` unchanged
- Allows external devices to be registered
- Reserves the FIFO address range (0x4000_0000 - 0x4000_0007) during validation
- Can be refactored later if needed

### 7. Address Range Validation

During device registration, validate that the new device does not overlap with existing devices or reserved ranges:

```rust
impl SystemBus {
    // Reserved address ranges for internal devices
    const FIFO_BASE: u32 = 0x4000_0000;
    const FIFO_SIZE: u32 = 8; // 2 words: DATA and STATUS

    fn check_address_overlap(
        &self,
        new_base: u32,
        new_size: u32,
    ) -> Result<(), RegistrationError> {
        let new_end = new_base.saturating_add(new_size);

        // Check against reserved FIFO range
        let fifo_end = Self::FIFO_BASE.saturating_add(Self::FIFO_SIZE);
        if ranges_overlap(new_base, new_end, Self::FIFO_BASE, fifo_end) {
            return Err(RegistrationError::AddressOverlap {
                new_base,
                new_end,
                existing_base: Self::FIFO_BASE,
                existing_end: fifo_end,
                existing_name: "FIFO (internal)".to_string(),
            });
        }

        // Check against other registered devices
        for registered in &self.devices {
            let existing_end = registered.base_addr.saturating_add(registered.size);
            if ranges_overlap(new_base, new_end, registered.base_addr, existing_end) {
                return Err(RegistrationError::AddressOverlap {
                    new_base,
                    new_end,
                    existing_base: registered.base_addr,
                    existing_end,
                    existing_name: registered.device.name().to_string(),
                });
            }
        }

        Ok(())
    }

    pub fn register_device(
        &mut self,
        base_addr: u32,
        device: Box<dyn BusDevice>,
    ) -> Result<(), RegistrationError> {
        let size = device.size();

        // Validate size
        if size == 0 {
            return Err(RegistrationError::ZeroSize);
        }
        if size % 4 != 0 {
            return Err(RegistrationError::InvalidAlignment { size });
        }

        // Check for overlaps
        self.check_address_overlap(base_addr, size)?;

        // Register device
        self.devices.push(RegisteredDevice {
            base_addr,
            size,
            device,
        });

        log::info!(
            "Registered device '{}' at 0x{:08x} - 0x{:08x} (size: {} bytes)",
            self.devices.last().unwrap().device.name(),
            base_addr,
            base_addr.saturating_add(size),
            size
        );

        Ok(())
    }
}

fn ranges_overlap(a_start: u32, a_end: u32, b_start: u32, b_end: u32) -> bool {
    a_start < b_end && b_start < a_end
}
```

### 8. SimulatorView Updates

The `SimulatorView` provides the public API for accessing the simulator during callbacks. We need to update it to support device registration:

```rust
impl<'a> SimulatorView<'a> {
    // Existing FIFO and DRAM methods remain unchanged
    // ... fifo_read_tx, fifo_write_rx, write_memory_region, etc.

    /// Register a custom device on the system bus
    ///
    /// This allows user code to register custom peripherals that will be
    /// accessible via the CPU's memory-mapped I/O.
    ///
    /// # Arguments
    /// * `base_addr` - Base address for the device in the system memory map
    /// * `device` - The device to register (must implement BusDevice trait)
    ///
    /// # Returns
    /// * `Ok(())` - Device registered successfully
    /// * `Err(String)` - Address range conflicts with existing device
    ///
    /// # Example
    /// ```no_run
    /// use cpu_sim::*;
    ///
    /// run_program(
    ///     1000,
    ///     false,
    ///     false,
    ///     None::<fn(&mut SimulatorView)>,
    ///     None::<fn(&InstructionTrace)>,
    ///     None,
    ///     0,
    ///     |sim| {
    ///         // Register custom video device
    ///         let video_device = Box::new(MyVideoDevice::new());
    ///         sim.register_device(0x5000_0000, video_device)
    ///             .map_err(|e| format!("Failed to register device: {:?}", e))?;
    ///         
    ///         // Load program
    ///         Ok(0x8000_0000)
    ///     },
    ///     None::<fn(&SimulatorView, &SimulationResult)>,
    /// )?;
    /// ```
    pub fn register_device(
        &mut self,
        base_addr: u32,
        device: Box<dyn BusDevice>,
    ) -> Result<(), String> {
        // Access to SystemBus needed - requires adding bus to SimulatorView
        // See "Implementation Challenges" section
    }
}
```

**Note**: This requires adding a reference to `SystemBus` (or specifically its device registry) to `SimulatorView`. See implementation details below.

### 9. Lifetime Management

**Key Requirement**: Devices registered on the bus must have a lifetime that matches or exceeds the `Simulator` lifetime.

**Solution**: Use `Box<dyn BusDevice>` for dynamic dispatch and ownership transfer. The `SystemBus` owns all registered devices.

```rust
// User code example
impl BusDevice for MyCustomDevice {
    // ... implementation
}

// In setup callback
let my_device = MyCustomDevice::new();
sim.register_device(0x5000_0000, Box::new(my_device))?;

// Device is now owned by SystemBus and lives until Simulator is dropped
```

For devices that need external state or callbacks:

```rust
use std::rc::Rc;
use std::cell::RefCell;

struct VideoDevice {
    frame_buffer: Rc<RefCell<Vec<u8>>>,
    present_callback: Rc<dyn Fn()>,
}

impl BusDevice for VideoDevice {
    fn write_word(&mut self, offset: u32, value: u32) -> Result<(), BusDeviceError> {
        match offset {
            0x00 => {
                // Write to frame buffer control register
                if value & 0x01 != 0 {
                    // Trigger present callback
                    (self.present_callback)();
                }
                Ok(())
            }
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }
    // ... other methods
}

// Usage
let frame_buffer = Rc::new(RefCell::new(vec![0u8; 1024 * 768 * 4]));
let fb_clone = frame_buffer.clone();

let present_callback = Rc::new(move || {
    println!("Presenting frame!");
    // Access frame_buffer via fb_clone if needed
});

let device = VideoDevice {
    frame_buffer,
    present_callback,
};

sim.register_device(0x5000_0000, Box::new(device))?;
```

### 10. Memory Map Documentation

Document the reserved memory ranges:

```rust
/// System Memory Map
///
/// The following address ranges are reserved by internal devices:
///
/// | Address Range              | Device | Description                    |
/// |----------------------------|--------|--------------------------------|
/// | 0x4000_0000 - 0x4000_0003 | FIFO   | FIFO_DATA register (R/W)       |
/// | 0x4000_0004 - 0x4000_0007 | FIFO   | FIFO_STATUS register (R/O)     |
/// | 0x0000_0000 - 0xFFFF_FFFF | DRAM   | Main memory (default/catch-all)|
///
/// User devices can be registered at any non-overlapping address range.
/// Common conventions:
/// - 0x5000_0000 - 0x5FFF_FFFF: Video/Graphics devices
/// - 0x6000_0000 - 0x6FFF_FFFF: Audio devices
/// - 0x7000_0000 - 0x7FFF_FFFF: Custom peripherals
/// - 0x8000_0000 - 0xFFFF_FFFF: DRAM (typical code/data region)
///
/// Note: The FIFO range (0x4000_0000 - 0x4000_0007) is reserved and cannot
/// be used for custom devices. Attempting to register a device in this range
/// will result in a `RegistrationError::AddressOverlap`.
```

## Implementation Plan

### Phase 1: Core Trait and Error Types

1. Create `cpu-sim/src/bus_device.rs` with:
   - `BusDeviceError` enum
   - `BusDevice` trait
   - Helper functions (`ranges_overlap`)

2. Update `cpu-sim/src/lib.rs` to export public types:
   ```rust
   mod bus_device;
   pub use bus_device::{BusDevice, BusDeviceError};
   ```

### Phase 2: BusDevice Implementations

1. Implement `BusDevice` for `Dram`:
   - Word-only interface (no byte/halfword in trait)
   - Keep existing methods for internal use
   - Add error handling (though DRAM never errors)

2. Implement `BusDevice` for `Fifo`:
   - Handle DATA and STATUS registers
   - Return `WriteToReadOnly` for STATUS writes
   - Return `InvalidAddress` for invalid offsets

### Phase 3: SystemBus Refactoring

1. Add device registry to `SystemBus`:
   ```rust
   pub struct SystemBus {
       pub(crate) dram: Dram,
       pub(crate) fifo: Fifo,
       devices: Vec<RegisteredDevice>,
   }
   ```

2. Implement `register_device()` with validation

3. Update `read_word()` and `write_word()` to check registered devices first

4. Keep existing byte/halfword methods routing to DRAM only

5. Add `RegistrationError` type

### Phase 4: SimulatorView Updates

1. Add bus access to `SimulatorView`:
   ```rust
   pub struct SimulatorView<'a> {
       fifo: &'a mut Fifo,
       dram: &'a mut Dram,
       bus: &'a mut SystemBus,  // NEW
       hung_detector: &'a mut Option<HungDetector>,
   }
   ```

2. Add `register_device()` method to `SimulatorView`

3. Update all `SimulatorView::new()` call sites to pass bus reference

### Phase 5: Documentation

1. Add module-level documentation to `bus_device.rs`

2. Update `SystemBus` documentation with memory map

3. Create usage examples in doc comments

4. Update this technical plan with any implementation learnings

### Phase 6: Testing

1. Unit tests for overlap detection:
   ```rust
   #[test]
   fn test_device_registration_overlap() {
       let mut bus = SystemBus::new();
       
       // Register device at 0x5000_0000
       let dev1 = Box::new(MockDevice::new(256));
       bus.register_device(0x5000_0000, dev1).unwrap();
       
       // Try to register overlapping device
       let dev2 = Box::new(MockDevice::new(256));
       let result = bus.register_device(0x5000_0080, dev2);
       
       assert!(matches!(result, Err(RegistrationError::AddressOverlap { .. })));
   }
   
   #[test]
   fn test_fifo_range_reserved() {
       let mut bus = SystemBus::new();
       
       // Try to register device in FIFO range
       let dev = Box::new(MockDevice::new(8));
       let result = bus.register_device(0x4000_0000, dev);
       
       assert!(matches!(result, Err(RegistrationError::AddressOverlap { .. })));
   }
   ```

2. Integration tests:
   - Run existing test suite to verify no behavioral changes
   - Add test for custom device registration in setup callback
   - Test device read/write routing
   - Test error handling (read-only writes, invalid addresses)

3. Mock device for testing:
   ```rust
   struct MockDevice {
       size: u32,
       registers: HashMap<u32, u32>,
   }
   
   impl BusDevice for MockDevice {
       fn read_word(&mut self, offset: u32) -> Result<u32, BusDeviceError> {
           Ok(*self.registers.get(&offset).unwrap_or(&0))
       }
       
       fn write_word(&mut self, offset: u32, value: u32) -> Result<(), BusDeviceError> {
           self.registers.insert(offset, value);
           Ok(())
       }
       
       fn size(&self) -> u32 {
           self.size
       }
   }
   ```

## Implementation Challenges and Solutions

### Challenge 1: SimulatorView Lifetime

**Problem**: `SimulatorView` needs mutable references to both individual components (FIFO, DRAM) AND the bus for device registration.

**Solution**: Instead of passing `bus` to `SimulatorView`, pass only the device registry:

```rust
pub struct SimulatorView<'a> {
    fifo: &'a mut Fifo,
    dram: &'a mut Dram,
    device_registry: &'a mut Vec<RegisteredDevice>,  // Just the registry
    hung_detector: &'a mut Option<HungDetector>,
}

impl<'a> SimulatorView<'a> {
    pub fn register_device(
        &mut self,
        base_addr: u32,
        device: Box<dyn BusDevice>,
    ) -> Result<(), String> {
        // Validation logic here (check overlaps with FIFO and existing devices)
        // Then push to device_registry
    }
}
```

This avoids the borrowing conflict while still allowing device registration.

### Challenge 2: Byte/Halfword Operations

**Problem**: `BusDevice` trait only supports word operations, but the bus needs byte/halfword support for DRAM.

**Solution**: Keep byte/halfword operations as separate methods on `SystemBus` that directly route to DRAM:

```rust
impl SystemBus {
    pub fn read_byte(&mut self, addr: u32) -> u8 {
        // Always route to DRAM
        self.dram.read_byte(addr)
    }
    
    pub fn read_halfword(&mut self, addr: u32) -> u16 {
        // Always route to DRAM
        self.dram.read_halfword(addr)
    }
    
    // Similar for write_byte, write_halfword
}
```

This matches current behavior where byte operations only work on DRAM (FIFO is word-only).

### Challenge 3: Device Ownership in Callbacks

**Problem**: User callbacks receive `&mut SimulatorView`, but devices may need to be created in that scope and registered.

**Solution**: This is already handled by `Box<dyn BusDevice>` - ownership transfers to the bus:

```rust
run_program(
    1000,
    // ... other params
    |sim| {
        // Create device in this scope
        let my_device = MyDevice::new();
        
        // Transfer ownership to bus via Box
        sim.register_device(0x5000_0000, Box::new(my_device))?;
        
        // Device now lives in SystemBus, not on the stack
        Ok(0x8000_0000)
    },
    None
)?;
```

## Usage Examples

### Example 1: Simple Read-Only Device

```rust
use cpu_sim::{BusDevice, BusDeviceError};

/// A simple device with read-only configuration registers
struct ConfigDevice {
    version: u32,
    features: u32,
}

impl ConfigDevice {
    fn new() -> Self {
        ConfigDevice {
            version: 0x0001_0000,  // Version 1.0
            features: 0x0000_00AB, // Feature flags
        }
    }
}

impl BusDevice for ConfigDevice {
    fn read_word(&mut self, offset: u32) -> Result<u32, BusDeviceError> {
        match offset {
            0x00 => Ok(self.version),
            0x04 => Ok(self.features),
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }

    fn write_word(&mut self, offset: u32, _value: u32) -> Result<(), BusDeviceError> {
        // All registers are read-only
        Err(BusDeviceError::WriteToReadOnly { offset })
    }

    fn size(&self) -> u32 {
        8  // 2 registers × 4 bytes
    }

    fn name(&self) -> &str {
        "ConfigDevice"
    }
}

// Usage in setup callback
run_program(
    1000,
    false,
    false,
    None::<fn(&mut SimulatorView)>,
    None::<fn(&InstructionTrace)>,
    None,
    0,
    |sim| {
        // Register at 0x5000_0000
        sim.register_device(0x5000_0000, Box::new(ConfigDevice::new()))
            .map_err(|e| format!("Failed to register device: {:?}", e))?;
        
        // Load program
        let instructions = vec![
            // lw x1, 0(x0)  @ 0x5000_0000  (read version)
            // ... more instructions
        ];
        Ok(0x8000_0000)
    },
    None::<fn(&SimulatorView, &SimulationResult)>,
)?;
```

### Example 2: Video Frame Buffer Device

```rust
use std::rc::Rc;
use std::cell::RefCell;

/// Video device with frame buffer and present trigger
struct VideoDevice {
    frame_buffer: Rc<RefCell<Vec<u8>>>,
    control_reg: u32,
}

impl VideoDevice {
    const CONTROL_OFFSET: u32 = 0x00;
    const FRAMEBUFFER_BASE: u32 = 0x1000;
    const FRAMEBUFFER_SIZE: u32 = 1024 * 768 * 4;  // 1024x768 RGBA8
    
    fn new(frame_buffer: Rc<RefCell<Vec<u8>>>) -> Self {
        VideoDevice {
            frame_buffer,
            control_reg: 0,
        }
    }
}

impl BusDevice for VideoDevice {
    fn read_word(&mut self, offset: u32) -> Result<u32, BusDeviceError> {
        match offset {
            Self::CONTROL_OFFSET => Ok(self.control_reg),
            off if off >= Self::FRAMEBUFFER_BASE 
                && off < Self::FRAMEBUFFER_BASE + Self::FRAMEBUFFER_SIZE => {
                // Read from frame buffer
                let fb_offset = (off - Self::FRAMEBUFFER_BASE) as usize;
                let fb = self.frame_buffer.borrow();
                let word = u32::from_le_bytes([
                    fb[fb_offset],
                    fb[fb_offset + 1],
                    fb[fb_offset + 2],
                    fb[fb_offset + 3],
                ]);
                Ok(word)
            }
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }

    fn write_word(&mut self, offset: u32, value: u32) -> Result<(), BusDeviceError> {
        match offset {
            Self::CONTROL_OFFSET => {
                self.control_reg = value;
                
                // Bit 0: Present frame (trigger display update)
                if value & 0x01 != 0 {
                    log::info!("Video: Presenting frame");
                    // In real implementation, trigger callback or external system
                }
                Ok(())
            }
            off if off >= Self::FRAMEBUFFER_BASE 
                && off < Self::FRAMEBUFFER_BASE + Self::FRAMEBUFFER_SIZE => {
                // Write to frame buffer
                let fb_offset = (off - Self::FRAMEBUFFER_BASE) as usize;
                let bytes = value.to_le_bytes();
                let mut fb = self.frame_buffer.borrow_mut();
                fb[fb_offset..fb_offset + 4].copy_from_slice(&bytes);
                Ok(())
            }
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }

    fn size(&self) -> u32 {
        Self::FRAMEBUFFER_BASE + Self::FRAMEBUFFER_SIZE
    }

    fn name(&self) -> &str {
        "VideoDevice"
    }
}

// Usage
let frame_buffer = Rc::new(RefCell::new(vec![0u8; 1024 * 768 * 4]));
let fb_clone = frame_buffer.clone();

run_program(
    10000,
    false,
    false,
    None::<fn(&mut SimulatorView)>,
    None::<fn(&InstructionTrace)>,
    None,
    0,
    |sim| {
        // Register video device
        let video = VideoDevice::new(fb_clone.clone());
        sim.register_device(0x5000_0000, Box::new(video))
            .map_err(|e| format!("Failed to register video device: {:?}", e))?;
        
        // Load program that writes to frame buffer
        Ok(0x8000_0000)
    },
    Some(|_sim, _result| {
        // After simulation, access frame buffer
        let fb = fb_clone.borrow();
        println!("Frame buffer: {} bytes", fb.len());
    }),
)?;
```

### Example 3: Testing Device Registration

```rust
#[test]
fn test_custom_device_registration() {
    use cpu_sim::*;
    use std::sync::{Arc, Mutex};
    
    struct CounterDevice {
        count: Arc<Mutex<u32>>,
    }
    
    impl BusDevice for CounterDevice {
        fn read_word(&mut self, offset: u32) -> Result<u32, BusDeviceError> {
            if offset == 0 {
                Ok(*self.count.lock().unwrap())
            } else {
                Err(BusDeviceError::InvalidAddress { offset })
            }
        }
        
        fn write_word(&mut self, offset: u32, value: u32) -> Result<(), BusDeviceError> {
            if offset == 0 {
                *self.count.lock().unwrap() = value;
                Ok(())
            } else {
                Err(BusDeviceError::InvalidAddress { offset })
            }
        }
        
        fn size(&self) -> u32 { 4 }
        fn name(&self) -> &str { "CounterDevice" }
    }
    
    let count = Arc::new(Mutex::new(0));
    let count_clone = count.clone();
    
    let instructions = vec![
        // sw x10, 0(x5)  where x5 = 0x5000_0000
        // ... generate actual instructions
    ];
    
    let result = run_program(
        1000,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        None,
        0,
        |sim| {
            // Register counter device
            let device = CounterDevice { count: count_clone.clone() };
            sim.register_device(0x5000_0000, Box::new(device)).unwrap();
            
            // Load instructions
            // ... write instructions to memory
            Ok(0x8000_0000)
        },
        None::<fn(&SimulatorView, &SimulationResult)>,
    ).unwrap();
    
    // Verify counter was written
    assert_eq!(*count.lock().unwrap(), 42);
}
```

## Testing Strategy

### Unit Tests

1. **Overlap Detection** (`test_device_overlap.rs`):
   - Test overlapping device registration fails
   - Test FIFO range is protected
   - Test adjacent (non-overlapping) devices work
   - Test zero-size device rejection
   - Test non-aligned size rejection

2. **Device Routing** (`test_device_routing.rs`):
   - Test reads route to correct device
   - Test writes route to correct device
   - Test unmapped reads return 0
   - Test unmapped writes are discarded
   - Test error handling (read-only writes, invalid offsets)

3. **Backward Compatibility** (existing tests):
   - Run full existing test suite
   - Verify no behavioral changes
   - Verify FIFO and DRAM still work

### Integration Tests

1. **Custom Device in Callback**:
   - Register custom device in setup callback
   - Execute program that accesses the device
   - Verify device state after simulation

2. **Multiple Devices**:
   - Register multiple devices at different addresses
   - Verify each device receives correct requests
   - Verify no crosstalk between devices

3. **Error Cases**:
   - Attempt to register overlapping devices
   - Access unmapped addresses
   - Write to read-only registers
   - Read from write-only registers

## Future Enhancements

1. **Device Removal**: Add ability to unregister devices
2. **Device Introspection**: Query registered devices and their ranges
3. **Priority Levels**: Allow devices to specify priority for overlapping ranges
4. **Default Device**: Configurable default device instead of hardcoded DRAM
5. **Address Translation**: Support for MMU-style address translation
6. **DMA Support**: Direct memory access between devices
7. **Interrupt Support**: Devices can trigger CPU interrupts
8. **MMIO Callbacks**: Register callbacks for specific address ranges without full device implementation

## API Compatibility

### Breaking Changes

**None**. This is a backward-compatible change. All existing code continues to work without modification.

### New Public API

```rust
// New trait (cpu-sim/src/bus_device.rs)
pub trait BusDevice { ... }
pub enum BusDeviceError { ... }

// New SystemBus methods (cpu-sim/src/bus.rs)
impl SystemBus {
    pub fn register_device(&mut self, base_addr: u32, device: Box<dyn BusDevice>) 
        -> Result<(), RegistrationError>;
}

// New SimulatorView methods (cpu-sim/src/sim.rs)
impl<'a> SimulatorView<'a> {
    pub fn register_device(&mut self, base_addr: u32, device: Box<dyn BusDevice>) 
        -> Result<(), String>;
}

// New error type
pub enum RegistrationError { ... }
```

### Deprecations

**None**.

## Memory Map Reference

### Reserved Ranges (Internal Devices)

| Start Address | End Address   | Size    | Device | Description                          |
|---------------|---------------|---------|--------|--------------------------------------|
| 0x4000_0000   | 0x4000_0003   | 4 bytes | FIFO   | FIFO_DATA register (R/W)             |
| 0x4000_0004   | 0x4000_0007   | 4 bytes | FIFO   | FIFO_STATUS register (R/O)           |
| 0x0000_0000   | 0xFFFF_FFFF   | 4 GiB   | DRAM   | Main memory (catch-all, low priority)|

### Recommended Ranges for Custom Devices

| Start Address | End Address   | Suggested Use                        |
|---------------|---------------|--------------------------------------|
| 0x5000_0000   | 0x5FFF_FFFF   | Video/Graphics devices               |
| 0x6000_0000   | 0x6FFF_FFFF   | Audio devices                        |
| 0x7000_0000   | 0x7FFF_FFFF   | Custom peripherals                   |
| 0x8000_0000   | 0xBFFF_FFFF   | DRAM (typical code region)           |
| 0xC000_0000   | 0xFFFF_FFEF   | DRAM (typical data region)           |
| 0xFFFF_FFF0   | 0xFFFF_FFF3   | tohost (halt signal) - **Reserved**  |

**Note**: These are suggestions only. Custom devices can use any non-overlapping range except the FIFO range (0x4000_0000 - 0x4000_0007) and tohost (0xFFFF_FFF0).

## Appendix: Complete Type Definitions

### BusDevice Trait

```rust
/// Error types for bus device operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BusDeviceError {
    WriteToReadOnly { offset: u32 },
    ReadFromWriteOnly { offset: u32 },
    InvalidAddress { offset: u32 },
    Other(String),
}

/// Trait for devices that can be registered on the system bus
pub trait BusDevice {
    fn read_word(&mut self, offset: u32) -> Result<u32, BusDeviceError>;
    fn write_word(&mut self, offset: u32, value: u32) -> Result<(), BusDeviceError>;
    fn size(&self) -> u32;
    fn name(&self) -> &str { "Unknown Device" }
}
```

### Registration Types

```rust
struct RegisteredDevice {
    base_addr: u32,
    size: u32,
    device: Box<dyn BusDevice>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistrationError {
    AddressOverlap {
        new_base: u32,
        new_end: u32,
        existing_base: u32,
        existing_end: u32,
        existing_name: String,
    },
    InvalidAlignment { size: u32 },
    ZeroSize,
}
```

## Summary

This plan provides a complete, production-ready design for adding dynamic bus device registration to `cpu-sim`. The system:

- ✅ Maintains 100% backward compatibility
- ✅ Enables extensibility for future devices (video, audio, etc.)
- ✅ Validates address ranges to prevent conflicts
- ✅ Uses relative addressing for device portability
- ✅ Provides proper error handling
- ✅ Includes comprehensive documentation and examples
- ✅ Has a clear testing strategy
- ✅ Follows Rust best practices for safety and lifetime management

The implementation can be done incrementally, with each phase independently testable, making it suitable for AI-assisted development.
