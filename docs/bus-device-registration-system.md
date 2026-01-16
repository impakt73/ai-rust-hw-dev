# Bus Device Registration System - Technical Implementation Plan

## Overview

This document provides a comprehensive technical plan for implementing a dynamic bus device registration system in the `cpu-sim` crate. The system uses a **handle-based architecture** that allows abstract devices to be registered onto the system bus with non-overlapping address ranges, enabling future extensibility (e.g., video devices, custom peripherals) while maintaining backward compatibility with existing code.

**Key Innovation**: The design uses 100% safe Rust with a `DeviceId` handle system that decouples address mapping from device storage, providing zero-cost access to internal devices while supporting dynamic registration of external devices.

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
- **DRAM**: Everything else (default, catch-all)
- **Tohost**: `0xFFFF_FFF0` (hardcoded in simulator logic)

### Problems with Current Architecture

1. **No Extensibility**: Adding new devices requires modifying `SystemBus` code
2. **Address Conflicts**: No validation to prevent overlapping device ranges
3. **Tight Coupling**: `SimulatorView` directly exposes FIFO and DRAM internals
4. **Hardcoded Routing**: Memory map is not configurable or documented
5. **Tohost Hardcoded**: Simulator directly hooks memory writes to detect tohost, should be a device

## Design Goals

1. **Extensibility**: Allow registration of custom devices without modifying core code
2. **Safety**: Validate that device address ranges do not overlap
3. **100% Safe Rust**: No unsafe code blocks, no raw pointers, borrow-checker compliant
4. **Backward Compatibility**: Preserve all existing functionality and tests
5. **Zero-Cost Abstraction**: Direct field access to internal devices via handle dispatch
6. **Relative Addressing**: Device implementations use offsets relative to their base address
7. **Clean API**: Simple interface for both internal and external device registration
8. **Error Handling**: Proper error types for invalid operations (read-only writes, write-only reads, invalid addresses)

## Proposed Architecture - Handle-Based Design

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
    /// Operation size not supported by this device at this address
    UnsupportedSize { offset: u32, size: u8 },
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
            BusDeviceError::UnsupportedSize { offset, size } => {
                write!(f, "Unsupported size {} at offset 0x{:08x}", size, offset)
            }
            BusDeviceError::Other(msg) => write!(f, "Device error: {}", msg),
        }
    }
}

impl std::error::Error for BusDeviceError {}

/// Trait for devices that can be registered on the system bus
///
/// All operations operate on offsets relative to the device's base address
/// in the system memory map. Devices can support byte (u8), halfword (u16),
/// and word (u32) operations. Devices that don't support a particular size
/// at a given address should return `BusDeviceError::UnsupportedSize`.
pub trait BusDevice {
    /// Read a 32-bit word from the device at the given offset
    ///
    /// # Arguments
    /// * `offset` - Byte offset relative to the device's base address (must be word-aligned)
    ///
    /// # Returns
    /// * `Ok(u32)` - The word value read from the device
    /// * `Err(BusDeviceError)` - If the read is invalid (e.g., write-only register)
    fn read_word(&mut self, offset: u32) -> Result<u32, BusDeviceError>;

    /// Write a 32-bit word to the device at the given offset
    ///
    /// # Arguments
    /// * `offset` - Byte offset relative to the device's base address (must be word-aligned)
    /// * `value` - The 32-bit value to write
    ///
    /// # Returns
    /// * `Ok(())` - Write successful
    /// * `Err(BusDeviceError)` - If the write is invalid (e.g., read-only register)
    fn write_word(&mut self, offset: u32, value: u32) -> Result<(), BusDeviceError>;

    /// Read a 16-bit halfword from the device at the given offset
    ///
    /// # Arguments
    /// * `offset` - Byte offset relative to the device's base address (must be halfword-aligned)
    ///
    /// # Returns
    /// * `Ok(u16)` - The halfword value read from the device
    /// * `Err(BusDeviceError)` - If the read is invalid or size not supported
    ///
    /// # Default Implementation
    /// Returns `UnsupportedSize` error. Devices that support halfword access should override this.
    fn read_halfword(&mut self, offset: u32) -> Result<u16, BusDeviceError> {
        Err(BusDeviceError::UnsupportedSize { offset, size: 2 })
    }

    /// Write a 16-bit halfword to the device at the given offset
    ///
    /// # Arguments
    /// * `offset` - Byte offset relative to the device's base address (must be halfword-aligned)
    /// * `value` - The 16-bit value to write
    ///
    /// # Returns
    /// * `Ok(())` - Write successful
    /// * `Err(BusDeviceError)` - If the write is invalid or size not supported
    ///
    /// # Default Implementation
    /// Returns `UnsupportedSize` error. Devices that support halfword access should override this.
    fn write_halfword(&mut self, offset: u32, value: u16) -> Result<(), BusDeviceError> {
        Err(BusDeviceError::UnsupportedSize { offset, size: 2 })
    }

    /// Read a single byte from the device at the given offset
    ///
    /// # Arguments
    /// * `offset` - Byte offset relative to the device's base address
    ///
    /// # Returns
    /// * `Ok(u8)` - The byte value read from the device
    /// * `Err(BusDeviceError)` - If the read is invalid or size not supported
    ///
    /// # Default Implementation
    /// Returns `UnsupportedSize` error. Devices that support byte access should override this.
    fn read_byte(&mut self, offset: u32) -> Result<u8, BusDeviceError> {
        Err(BusDeviceError::UnsupportedSize { offset, size: 1 })
    }

    /// Write a single byte to the device at the given offset
    ///
    /// # Arguments
    /// * `offset` - Byte offset relative to the device's base address
    /// * `value` - The 8-bit value to write
    ///
    /// # Returns
    /// * `Ok(())` - Write successful
    /// * `Err(BusDeviceError)` - If the write is invalid or size not supported
    ///
    /// # Default Implementation
    /// Returns `UnsupportedSize` error. Devices that support byte access should override this.
    fn write_byte(&mut self, offset: u32, value: u8) -> Result<(), BusDeviceError> {
        Err(BusDeviceError::UnsupportedSize { offset, size: 1 })
    }

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

### 2. Device Registration System - Handle-Based Architecture

The `SystemBus` uses a decoupled handle-based architecture that separates address mapping from device storage. This approach provides 100% safe Rust with zero-cost access to internal devices.

**Key Innovation**: Instead of storing device pointers in the address map, we use lightweight `DeviceId` handles that identify which concrete field or external device Vec index to access.

```rust
/// Lightweight handle identifying which device owns an address range
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeviceId {
    /// Internal DRAM device (SystemBus.dram field)
    Dram,
    /// Internal FIFO device (SystemBus.fifo field)
    Fifo,
    /// Internal SimControl device (SystemBus.sim_control field)
    SimControl,
    /// External device (index into SystemBus.external_devices Vec)
    External(usize),
}

/// Memory map entry separating "where" from "what"
/// Maps an address range to a device handle
struct MemoryMapEntry {
    base: u32,
    end: u32,  // Exclusive end address (base + size)
    id: DeviceId,
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
    /// Device base address is not word-aligned
    InvalidBaseAlignment { base_addr: u32 },
    /// Device size is zero
    ZeroSize,
}

pub struct SystemBus {
    // Internal devices as concrete public fields (for SimulatorView access)
    pub dram: Dram,
    pub fifo: Fifo,
    pub sim_control: SimControl,
    
    // External devices (owned by the bus)
    external_devices: Vec<Box<dyn BusDevice>>,
    
    // Address map (lightweight handles, not device references)
    memory_map: Vec<MemoryMapEntry>,
}

impl SystemBus {
    /// Create a new system bus with internal devices initialized
    ///
    /// The bus owns DRAM, FIFO, and SimControl as concrete fields,
    /// pre-registered in the memory map. External devices can be added later.
    pub fn new() -> Self {
        let dram = Dram::new();
        let fifo = Fifo::new();
        let sim_control = SimControl::new();
        
        // Pre-populate memory map with internal devices
        let memory_map = vec![
            MemoryMapEntry {
                base: 0x1000_0000,
                end: 0x1000_0004,  // SimControl: 4 bytes
                id: DeviceId::SimControl,
            },
            MemoryMapEntry {
                base: 0x4000_0000,
                end: 0x4000_0008,  // FIFO: 8 bytes
                id: DeviceId::Fifo,
            },
            MemoryMapEntry {
                base: 0x8000_0000,
                end: 0xFFFF_FFFF + 1,  // DRAM: 2 GiB (wraps to 0)
                id: DeviceId::Dram,
            },
        ];
        
        SystemBus {
            dram,
            fifo,
            sim_control,
            external_devices: Vec::new(),
            memory_map,
        }
    }

    /// Register an external device at the specified base address
    ///
    /// Takes ownership of the device and adds it to the memory map.
    ///
    /// # Arguments
    /// * `base_addr` - Base address for the device (must be word-aligned)
    /// * `device` - The device to register (must implement BusDevice trait)
    ///
    /// # Returns
    /// * `Ok(())` - Device registered successfully
    /// * `Err(RegistrationError)` - Address range conflicts or invalid alignment
    pub fn register_device(
        &mut self,
        base_addr: u32,
        device: Box<dyn BusDevice>,
    ) -> Result<(), RegistrationError> {
        let size = device.size();

        // Validate size and alignment
        if size == 0 {
            return Err(RegistrationError::ZeroSize);
        }
        if size % 4 != 0 {
            return Err(RegistrationError::InvalidAlignment { size });
        }
        if base_addr % 4 != 0 {
            return Err(RegistrationError::InvalidBaseAlignment { base_addr });
        }

        let end = base_addr.saturating_add(size);

        // Check for overlaps with existing devices
        for entry in &self.memory_map {
            if ranges_overlap(base_addr, end, entry.base, entry.end) {
                let device_name = match entry.id {
                    DeviceId::Dram => "DRAM",
                    DeviceId::Fifo => "FIFO",
                    DeviceId::SimControl => "SimControl",
                    DeviceId::External(idx) => self.external_devices[idx].name(),
                };
                return Err(RegistrationError::AddressOverlap {
                    new_base: base_addr,
                    new_end: end,
                    existing_base: entry.base,
                    existing_end: entry.end,
                    existing_name: device_name.to_string(),
                });
            }
        }

        // Add device to storage and create memory map entry
        let device_idx = self.external_devices.len();
        let device_name = device.name().to_string();
        self.external_devices.push(device);
        
        self.memory_map.push(MemoryMapEntry {
            base: base_addr,
            end,
            id: DeviceId::External(device_idx),
        });

        log::info!(
            "Registered device '{}' at 0x{:08x} - 0x{:08x} (size: {} bytes)",
            device_name,
            base_addr,
            end - 1,
            size
        );

        Ok(())
    }

    /// Get immutable access to registered devices (for introspection/debugging)
    ///
    /// Returns an iterator over (base_addr, size, name) tuples for all registered devices.
    pub fn registered_devices(&self) -> impl Iterator<Item = (u32, u32, &str)> + '_ {
        self.memory_map.iter().map(|entry| {
            let size = entry.end.wrapping_sub(entry.base);
            let name = match entry.id {
                DeviceId::Dram => self.dram.name(),
                DeviceId::Fifo => self.fifo.name(),
                DeviceId::SimControl => self.sim_control.name(),
                DeviceId::External(idx) => self.external_devices[idx].name(),
            };
            (entry.base, size, name)
        })
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

    /// Read a 16-bit halfword from the bus
    ///
    /// Routes the request to the appropriate device based on address.
    /// If no device matches or device doesn't support halfword access,
    /// logs a warning and returns 0.
    pub fn read_halfword(&mut self, addr: u32) -> u16 {
        // Route to appropriate device
    }

    /// Write a 16-bit halfword to the bus
    ///
    /// Routes the request to the appropriate device based on address.
    /// If no device matches or device doesn't support halfword access,
    /// logs a warning and discards the write.
    pub fn write_halfword(&mut self, addr: u32, value: u16) {
        // Route to appropriate device
    }

    /// Read a single byte from the bus
    ///
    /// Routes the request to the appropriate device based on address.
    /// If no device matches or device doesn't support byte access,
    /// logs a warning and returns 0.
    pub fn read_byte(&mut self, addr: u32) -> u8 {
        // Route to appropriate device
    }

    /// Write a single byte to the bus
    ///
    /// Routes the request to the appropriate device based on address.
    /// If no device matches or device doesn't support byte access,
    /// logs a warning and discards the write.
    pub fn write_byte(&mut self, addr: u32, value: u8) {
        // Route to appropriate device
    }
}
```

### 3. DRAM as BusDevice

Convert the existing DRAM to implement `BusDevice`. DRAM will be registered at `0x8000_0000` to match the linker script configuration used by Rust test programs.

```rust
impl BusDevice for Dram {
    fn read_word(&mut self, offset: u32) -> Result<u32, BusDeviceError> {
        // Offset is relative to device base (0x8000_0000)
        // DRAM internally uses absolute addresses, so we don't add the base here
        // as DRAM's internal storage is already keyed by absolute addresses
        Ok(self.read_word_internal(offset))
    }

    fn write_word(&mut self, offset: u32, value: u32) -> Result<(), BusDeviceError> {
        // Offset is relative to device base (0x8000_0000)
        self.write_word_internal(offset, value);
        Ok(())
    }

    fn read_halfword(&mut self, offset: u32) -> Result<u16, BusDeviceError> {
        Ok(self.read_halfword_internal(offset))
    }

    fn write_halfword(&mut self, offset: u32, value: u16) -> Result<(), BusDeviceError> {
        self.write_halfword_internal(offset, value);
        Ok(())
    }

    fn read_byte(&mut self, offset: u32) -> Result<u8, BusDeviceError> {
        Ok(self.read_byte_internal(offset))
    }

    fn write_byte(&mut self, offset: u32, value: u8) -> Result<(), BusDeviceError> {
        self.write_byte_internal(offset, value);
        Ok(())
    }

    fn size(&self) -> u32 {
        // DRAM size: 2 GiB mapped from 0x8000_0000 to 0xFFFF_FFFF
        // Size = 0xFFFF_FFFF - 0x8000_0000 + 1 = 0x8000_0000 bytes
        0x8000_0000
    }

    fn name(&self) -> &str {
        "DRAM"
    }
}
```

**Memory Map**: DRAM will be registered at base address `0x8000_0000` with size `0x8000_0000` (2 GiB), covering the range `0x8000_0000` to `0xFFFF_FFFF`.


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
        // FIFO has 2 word-aligned registers within its address window:
        //   - DATA   at offset 0x00 (read/write)
        //   - STATUS at offset 0x04 (read-only)
        //
        // The device reserves a contiguous 8-byte region [0x00..=0x07] on the bus
        // to allow for potential future expansion. Currently, only word-aligned
        // offsets 0x00 and 0x04 are valid for word access operations.
        // All other offsets will result in BusDeviceError::InvalidAddress.
        8
    }

    fn name(&self) -> &str {
        "FIFO"
    }
}
```

### 5. SimControl as BusDevice

Create a new `SimControl` device to handle simulator control operations, including the tohost register for program termination. This device will be registered at `0x1000_0000`.

```rust
/// Simulator control device
///
/// Provides memory-mapped registers for controlling the simulator,
/// including the tohost register for signaling program completion.
pub struct SimControl {
    tohost_value: Option<u32>,
}

impl SimControl {
    pub fn new() -> Self {
        SimControl { tohost_value: None }
    }

    /// Check if a termination request has been made
    pub fn termination_requested(&self) -> Option<u32> {
        self.tohost_value
    }

    /// Clear the termination request (for reset)
    pub fn clear_termination(&mut self) {
        self.tohost_value = None;
    }
}

impl BusDevice for SimControl {
    fn read_word(&mut self, offset: u32) -> Result<u32, BusDeviceError> {
        match offset {
            0x00 => {
                // TOHOST register is write-only
                Err(BusDeviceError::ReadFromWriteOnly { offset })
            }
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }

    fn write_word(&mut self, offset: u32, value: u32) -> Result<(), BusDeviceError> {
        match offset {
            0x00 => {
                // TOHOST register - write triggers termination
                self.tohost_value = Some(value);
                log::info!("SimControl: tohost write detected, value={:#010x}", value);
                Ok(())
            }
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }

    fn size(&self) -> u32 {
        // Single 32-bit register: TOHOST
        4
    }

    fn name(&self) -> &str {
        "SimControl"
    }
}
```

**Memory Map**: SimControl will be registered at base address `0x1000_0000` with size `4` bytes, providing the TOHOST register at offset `0x00`.

**Integration**: The Simulator will need to query `SimControl::termination_requested()` after each instruction to check if the program has signaled completion.

### 6. System Bus Implementation - Handle-Based Dispatch

The `SystemBus` routing logic uses a two-step process:
1. Find the memory map entry containing the address (returns a `DeviceId` handle)
2. Dispatch to the appropriate concrete field or external device based on the handle

This approach is 100% safe Rust - no unsafe code, no raw pointers.

```rust
impl SystemBus {
    /// Find the device ID for the given address
    ///
    /// Returns the DeviceId handle and the offset relative to the device's base address.
    fn find_device_id(&self, addr: u32) -> Option<(DeviceId, u32)> {
        for entry in &self.memory_map {
            if addr >= entry.base && addr < entry.end {
                let offset = addr - entry.base;
                return Some((entry.id, offset));
            }
        }
        None
    }

    pub fn read_word(&mut self, addr: u32) -> u32 {
        if let Some((id, offset)) = self.find_device_id(addr) {
            let result = match id {
                DeviceId::Dram => self.dram.read_word(offset),
                DeviceId::Fifo => self.fifo.read_word(offset),
                DeviceId::SimControl => self.sim_control.read_word(offset),
                DeviceId::External(idx) => self.external_devices[idx].read_word(offset),
            };
            
            match result {
                Ok(value) => value,
                Err(e) => {
                    log::warn!("Bus read_word error at 0x{:08x}: {}", addr, e);
                    0
                }
            }
        } else {
            log::warn!("Bus read_word from unmapped address 0x{:08x}, returning 0", addr);
            0
        }
    }

    pub fn write_word(&mut self, addr: u32, value: u32) {
        if let Some((id, offset)) = self.find_device_id(addr) {
            let result = match id {
                DeviceId::Dram => self.dram.write_word(offset, value),
                DeviceId::Fifo => self.fifo.write_word(offset, value),
                DeviceId::SimControl => self.sim_control.write_word(offset, value),
                DeviceId::External(idx) => self.external_devices[idx].write_word(offset, value),
            };
            
            if let Err(e) = result {
                log::warn!("Bus write_word error at 0x{:08x}: {}", addr, e);
            }
        } else {
            log::warn!(
                "Bus write_word to unmapped address 0x{:08x} (value=0x{:08x}), discarding",
                addr, value
            );
        }
    }

    pub fn read_halfword(&mut self, addr: u32) -> u16 {
        if let Some((id, offset)) = self.find_device_id(addr) {
            let result = match id {
                DeviceId::Dram => self.dram.read_halfword(offset),
                DeviceId::Fifo => self.fifo.read_halfword(offset),
                DeviceId::SimControl => self.sim_control.read_halfword(offset),
                DeviceId::External(idx) => self.external_devices[idx].read_halfword(offset),
            };
            
            match result {
                Ok(value) => value,
                Err(e) => {
                    log::warn!("Bus read_halfword error at 0x{:08x}: {}", addr, e);
                    0
                }
            }
        } else {
            log::warn!("Bus read_halfword from unmapped address 0x{:08x}, returning 0", addr);
            0
        }
    }

    pub fn write_halfword(&mut self, addr: u32, value: u16) {
        if let Some((id, offset)) = self.find_device_id(addr) {
            let result = match id {
                DeviceId::Dram => self.dram.write_halfword(offset, value),
                DeviceId::Fifo => self.fifo.write_halfword(offset, value),
                DeviceId::SimControl => self.sim_control.write_halfword(offset, value),
                DeviceId::External(idx) => self.external_devices[idx].write_halfword(offset, value),
            };
            
            if let Err(e) = result {
                log::warn!("Bus write_halfword error at 0x{:08x}: {}", addr, e);
            }
        } else {
            log::warn!(
                "Bus write_halfword to unmapped address 0x{:08x} (value=0x{:04x}), discarding",
                addr, value
            );
        }
    }

    pub fn read_byte(&mut self, addr: u32) -> u8 {
        if let Some((id, offset)) = self.find_device_id(addr) {
            let result = match id {
                DeviceId::Dram => self.dram.read_byte(offset),
                DeviceId::Fifo => self.fifo.read_byte(offset),
                DeviceId::SimControl => self.sim_control.read_byte(offset),
                DeviceId::External(idx) => self.external_devices[idx].read_byte(offset),
            };
            
            match result {
                Ok(value) => value,
                Err(e) => {
                    log::warn!("Bus read_byte error at 0x{:08x}: {}", addr, e);
                    0
                }
            }
        } else {
            log::warn!("Bus read_byte from unmapped address 0x{:08x}, returning 0", addr);
            0
        }
    }

    pub fn write_byte(&mut self, addr: u32, value: u8) {
        if let Some((id, offset)) = self.find_device_id(addr) {
            let result = match id {
                DeviceId::Dram => self.dram.write_byte(offset, value),
                DeviceId::Fifo => self.fifo.write_byte(offset, value),
                DeviceId::SimControl => self.sim_control.write_byte(offset, value),
                DeviceId::External(idx) => self.external_devices[idx].write_byte(offset, value),
            };
            
            if let Err(e) = result {
                log::warn!("Bus write_byte error at 0x{:08x}: {}", addr, e);
            }
        } else {
            log::warn!(
                "Bus write_byte to unmapped address 0x{:08x} (value=0x{:02x}), discarding",
                addr, value
            );
        }
    }
}
```

**Key Points:**
- 100% Safe Rust - no unsafe code required
- Handle-based dispatch using DeviceId enum
- Zero-cost access to internal devices (direct field access)
- External devices accessed via Vec index
- All unmapped accesses log warnings and return 0 (reads) or discard (writes)
- Device receives offset relative to its base address for portability

### 7. Simulator Integration

The `Simulator` simply owns a `SystemBus`, which internally owns all devices. No special registration logic is needed - the bus initializes with DRAM, FIFO, and SimControl already registered.

```rust
// In sim.rs
pub struct Simulator {
    // ... existing fields
    bus: SystemBus,
    // No need to separately own devices - bus owns them
}

impl Simulator {
    pub fn new(...) -> Result<Self, String> {
        // Create bus with internal devices pre-initialized
        let bus = SystemBus::new();
        
        Ok(Simulator {
            bus,
            // ... other fields
        })
    }
    
    pub fn step(&mut self) -> Result<SimulationStepResult, HungStateError> {
        // ... execute instruction ...
        
        // Check for termination via SimControl device
        if let Some(tohost_value) = self.bus.sim_control.termination_requested() {
            return Ok(SimulationStepResult {
                tohost_value: Some(tohost_value),
                // ... other fields
            });
        }
        
        // ... rest of step logic
    }
}
```

**SimulatorView Access**: `SimulatorView` can access internal devices directly through public fields:

```rust
impl<'a> SimulatorView<'a> {
    pub(crate) fn new(
        bus: &'a mut SystemBus,
        hung_detector: &'a mut Option<HungDetector>,
    ) -> Self {
        SimulatorView {
            bus,
            hung_detector,
        }
    }
    
    // Direct access to FIFO via bus.fifo field
    pub fn fifo_read_tx(&mut self) -> Option<u32> {
        self.bus.fifo.tx.pop_front()
    }
    
    // Direct access to DRAM via bus.dram field
    pub fn write_memory_region(&mut self, start_addr: u32, data: &[u8], is_instructions: bool) {
        for (offset, &byte) in data.iter().enumerate() {
            let addr = start_addr.wrapping_add(offset as u32);
            self.bus.dram.write_byte(addr, byte);
        }
        // ... hung detector update
    }
    
    // External device registration routes to bus
    pub fn register_device(
        &mut self,
        base_addr: u32,
        device: Box<dyn BusDevice>,
    ) -> Result<(), String> {
        self.bus.register_device(base_addr, device)
            .map_err(|e| format!("{:?}", e))
    }
}
```

**Benefits of Handle-Based Architecture:**
1. **100% Safe Rust**: No unsafe blocks, no raw pointers, no lifetime issues
2. **Zero-cost Internal Access**: Direct field access to `bus.dram`, `bus.fifo`, `bus.sim_control`
3. **Simplified Ownership**: Bus owns everything, no complex lifetime management
4. **Borrow-Checker Friendly**: No mutable aliasing concerns
5. **Unified Routing**: All devices (internal and external) use the same dispatch logic

### 8. Address Range Validation

Address range validation is built into the `SystemBus::register_device()` method (shown in section 2). The validation logic checks for:
- Zero size
- Size not word-aligned (multiple of 4)
- Base address not word-aligned  
- Overlaps with existing memory map entries

```rust
fn ranges_overlap(a_start: u32, a_end: u32, b_start: u32, b_end: u32) -> bool {
    a_start < b_end && b_start < a_end
}
```

The internal devices are pre-registered in `SystemBus::new()` with their fixed addresses, so validation automatically prevents external devices from overlapping with them.

### 9. SimulatorView Updates

The `SimulatorView` provides the public API for accessing the simulator during callbacks:

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
/// | 0x1000_0000 - 0x1000_0003 | SimControl | TOHOST register (W/O)      |
/// | 0x4000_0000 - 0x4000_0003 | FIFO   | FIFO_DATA register (R/W)       |
/// | 0x4000_0004 - 0x4000_0007 | FIFO   | FIFO_STATUS register (R/O)     |
/// | 0x8000_0000 - 0xFFFF_FFFF | DRAM   | Main memory (code and data)    |
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
   - `BusDeviceError` enum (including `UnsupportedSize`)
   - `BusDevice` trait with word, halfword, and byte operations
   - Helper functions (`ranges_overlap`)

2. Create `cpu-sim/src/sim_control.rs` with:
   - `SimControl` struct
   - `BusDevice` implementation for `SimControl`

3. Update `cpu-sim/src/lib.rs` to export public types:
   ```rust
   mod bus_device;
   mod sim_control;
   pub use bus_device::{BusDevice, BusDeviceError};
   pub use sim_control::SimControl;
   ```

### Phase 2: BusDevice Implementations

1. Implement `BusDevice` for `Dram`:
   - All six methods: read/write word, halfword, byte
   - Registered at 0x8000_0000 with size 0x8000_0000
   - Add error handling (though DRAM never errors)

2. Implement `BusDevice` for `Fifo`:
   - Handle DATA and STATUS registers
   - Return `WriteToReadOnly` for STATUS writes
   - Return `InvalidAddress` for invalid offsets
   - Return `UnsupportedSize` for byte/halfword operations

3. `SimControl` implementation (from Phase 1):
   - TOHOST register at offset 0x00
   - Track termination value
   - Return `UnsupportedSize` for non-word operations

### Phase 3: SystemBus Refactoring

1. Update `SystemBus` to use handle-based architecture:
   ```rust
   pub struct SystemBus {
       pub dram: Dram,
       pub fifo: Fifo,
       pub sim_control: SimControl,
       external_devices: Vec<Box<dyn BusDevice>>,
       memory_map: Vec<MemoryMapEntry>,
   }
   ```

2. Implement `DeviceId` enum and `MemoryMapEntry` struct

3. Update `new()` to initialize internal devices and pre-populate memory map

4. Implement handle-based routing in all bus methods (read/write word/halfword/byte)

5. Remove all unsafe code - 100% safe Rust

### Phase 4: Simulator Updates

1. Simplify `Simulator` structure:
   ```rust
   pub struct Simulator {
       bus: SystemBus,
       // ... existing fields (no separate device fields)
   }
   ```

2. Update `Simulator::new()` - just create `SystemBus::new()`, no device registration needed

3. Update `Simulator::step()` to query `bus.sim_control.termination_requested()` instead of checking tohost writes

4. Remove hardcoded tohost address constant (now in SimControl)

### Phase 5: SimulatorView Updates

1. Update `SimulatorView` to reference bus:
   ```rust
   pub struct SimulatorView<'a> {
       bus: &'a mut SystemBus,
       hung_detector: &'a mut Option<HungDetector>,
   }
   ```

2. Update all methods to access devices via `self.bus.dram`, `self.bus.fifo`

3. Add `register_device()` method that forwards to `self.bus.register_device()`

4. Update all `SimulatorView::new()` call sites to pass only bus reference

### Phase 6: Update Rust Test Programs

1. Update `rust-test-program/src/common.rs`:
   - Change `TOHOST_ADDR` from `0xFFFF_FFF0` to `0x1000_0000`
   - Update documentation comments explaining the tohost mechanism to reflect:
     * New address (0x1000_0000)
     * That tohost is now handled via the SimControl device
     * The device is write-only (reads return error)

2. Recompile all ELF files in `test_programs/`:
   ```bash
   cd rust-test-program
   cargo build --release
   # Copy ELF files to test_programs/
   ```

3. Update non-ELF based tests that use tohost:
   - Search for `0xFFFF_FFF0` in cpu-sim tests
   - Replace with `0x1000_0000`

### Phase 7: Documentation

1. Add module-level documentation to `bus_device.rs`

2. Update `SystemBus` documentation with memory map

3. Create usage examples in doc comments

4. Document SimControl device

5. Update memory map references throughout codebase

### Phase 8: Testing

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
           if offset >= self.size() || offset % 4 != 0 {
               return Err(BusDeviceError::InvalidAddress { offset });
           }
           Ok(*self.registers.get(&offset).unwrap_or(&0))
       }
       
       fn write_word(&mut self, offset: u32, value: u32) -> Result<(), BusDeviceError> {
           if offset >= self.size() || offset % 4 != 0 {
               return Err(BusDeviceError::InvalidAddress { offset });
           }
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

### Challenge 2: Device Ownership in Callbacks

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
        if offset == Self::CONTROL_OFFSET {
            Ok(self.control_reg)
        } else if offset >= Self::FRAMEBUFFER_BASE 
            && offset < Self::FRAMEBUFFER_BASE + Self::FRAMEBUFFER_SIZE
            && offset % 4 == 0 {
            // Read from frame buffer (word-aligned access only)
            let fb_offset = (offset - Self::FRAMEBUFFER_BASE) as usize;
            let fb = self.frame_buffer.borrow();
            
            // Bounds check before indexing
            if fb_offset + 4 > fb.len() {
                return Err(BusDeviceError::InvalidAddress { offset });
            }
            
            let word = u32::from_le_bytes([
                fb[fb_offset],
                fb[fb_offset + 1],
                fb[fb_offset + 2],
                fb[fb_offset + 3],
            ]);
            Ok(word)
        } else {
            // Invalid offset (gap between CONTROL and FRAMEBUFFER, or misaligned)
            Err(BusDeviceError::InvalidAddress { offset })
        }
    }

    fn write_word(&mut self, offset: u32, value: u32) -> Result<(), BusDeviceError> {
        if offset == Self::CONTROL_OFFSET {
            self.control_reg = value;
            
            // Bit 0: Present frame (trigger display update)
            if value & 0x01 != 0 {
                log::info!("Video: Presenting frame");
                // In real implementation, trigger callback or external system
            }
            Ok(())
        } else if offset >= Self::FRAMEBUFFER_BASE 
            && offset < Self::FRAMEBUFFER_BASE + Self::FRAMEBUFFER_SIZE
            && offset % 4 == 0 {
            // Write to frame buffer (word-aligned access only)
            let fb_offset = (offset - Self::FRAMEBUFFER_BASE) as usize;
            let bytes = value.to_le_bytes();
            let mut fb = self.frame_buffer.borrow_mut();
            
            // Bounds check before indexing
            if fb_offset + 4 > fb.len() {
                return Err(BusDeviceError::InvalidAddress { offset });
            }
            
            fb[fb_offset..fb_offset + 4].copy_from_slice(&bytes);
            Ok(())
        } else {
            // Invalid offset (gap between CONTROL and FRAMEBUFFER, or misaligned)
            Err(BusDeviceError::InvalidAddress { offset })
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

## API Compatibility

### Breaking Changes

**None**. This is a backward-compatible change. All existing code continues to work without modification.

### New Public API

```rust
// New trait (cpu-sim/src/bus_device.rs)
pub trait BusDevice { ... }
pub enum BusDeviceError { ... }

// New device (cpu-sim/src/sim_control.rs)
pub struct SimControl { ... }

// New SystemBus methods (cpu-sim/src/bus.rs)
impl SystemBus {
    pub fn register_device(&mut self, base_addr: u32, device: Box<dyn BusDevice>) 
        -> Result<(), RegistrationError>;
    pub fn registered_devices(&self) -> impl Iterator<Item = (u32, u32, &str)>;
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

### Internal Device Ranges

| Start Address | End Address   | Size    | Device     | Description                         |
|---------------|---------------|---------|------------|-------------------------------------|
| 0x1000_0000   | 0x1000_0003   | 4 bytes | SimControl | TOHOST register (W/O - halt signal) |
| 0x4000_0000   | 0x4000_0003   | 4 bytes | FIFO       | FIFO_DATA register (R/W)            |
| 0x4000_0004   | 0x4000_0007   | 4 bytes | FIFO       | FIFO_STATUS register (R/O)          |
| 0x8000_0000   | 0xFFFF_FFFF   | 2 GiB   | DRAM       | Main memory (code and data)         |

### Recommended Ranges for Custom Devices

| Start Address | End Address   | Suggested Use                        |
|---------------|---------------|--------------------------------------|
| 0x2000_0000   | 0x2FFF_FFFF   | Video/Graphics devices               |
| 0x3000_0000   | 0x3FFF_FFFF   | Audio devices                        |
| 0x5000_0000   | 0x7FFF_FFFF   | Custom peripherals                   |

**Notes**:
- These are suggestions only. Custom devices can use any non-overlapping range not already occupied by internal devices.
- All device base addresses and sizes must be word-aligned (multiple of 4).
- Devices have exclusive address ranges - no overlapping is allowed.
- DRAM range (0x8000_0000 - 0xFFFF_FFFF) matches the linker script used by Rust test programs.

## Appendix: Complete Type Definitions

### BusDevice Trait

```rust
/// Error types for bus device operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BusDeviceError {
    WriteToReadOnly { offset: u32 },
    ReadFromWriteOnly { offset: u32 },
    InvalidAddress { offset: u32 },
    UnsupportedSize { offset: u32, size: u8 },
    Other(String),
}

/// Trait for devices that can be registered on the system bus
pub trait BusDevice {
    fn read_word(&mut self, offset: u32) -> Result<u32, BusDeviceError>;
    fn write_word(&mut self, offset: u32, value: u32) -> Result<(), BusDeviceError>;
    fn read_halfword(&mut self, offset: u32) -> Result<u16, BusDeviceError> { /* default impl */ }
    fn write_halfword(&mut self, offset: u32, value: u16) -> Result<(), BusDeviceError> { /* default impl */ }
    fn read_byte(&mut self, offset: u32) -> Result<u8, BusDeviceError> { /* default impl */ }
    fn write_byte(&mut self, offset: u32, value: u8) -> Result<(), BusDeviceError> { /* default impl */ }
    fn size(&self) -> u32;
    fn name(&self) -> &str { "Unknown Device" }
}
```

### Registration Types - Handle-Based Architecture

```rust
/// Lightweight handle identifying which device owns an address range
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeviceId {
    Dram,
    Fifo,
    SimControl,
    External(usize),  // Index into external_devices Vec
}

/// Memory map entry separating address mapping from device storage
struct MemoryMapEntry {
    base: u32,
    end: u32,  // Exclusive end address
    id: DeviceId,
}

pub struct SystemBus {
    // Internal devices as concrete public fields
    pub dram: Dram,
    pub fifo: Fifo,
    pub sim_control: SimControl,
    
    // External devices (owned by bus)
    external_devices: Vec<Box<dyn BusDevice>>,
    
    // Address map (lightweight handles)
    memory_map: Vec<MemoryMapEntry>,
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
    InvalidBaseAlignment { base_addr: u32 },
    ZeroSize,
}
```

## Summary

This plan provides a complete, production-ready design for adding dynamic bus device registration to `cpu-sim`. The system:

- ✅ Maintains 100% backward compatibility
- ✅ Enables extensibility for future devices (video, audio, etc.)
- ✅ Validates address ranges to prevent conflicts
- ✅ Uses exclusive non-overlapping ranges (no priority system)
- ✅ Supports byte, halfword, and word operations
- ✅ Includes SimControl device for tohost register
- ✅ DRAM at 0x8000_0000 matching linker script
- ✅ Device introspection for debugging
- ✅ Uses relative addressing for device portability
- ✅ Provides proper error handling
- ✅ **100% Safe Rust** - No unsafe code, no raw pointers
- ✅ **Zero-cost internal device access** - Direct field access via handle dispatch
- ✅ **Simplified ownership** - Bus owns all devices, eliminating lifetime complexity
- ✅ Includes comprehensive documentation and examples
- ✅ Has a clear testing strategy
- ✅ Follows Rust best practices for safety and lifetime management

The implementation can be done incrementally, with each phase independently testable, making it suitable for AI-assisted development.
