use crate::memory::Memory;

/// DRAM memory range: DRAM_BASE to DRAM_END (inclusive)
use crate::bus::{is_valid_dram_range, DRAM_BASE, DRAM_END};

/// SystemContext provides BusDevice implementations with access to system memory
///
/// This structure contains a mutable reference to the system's Memory and allows
/// BusDevice implementations to read and write memory. This enables devices to
/// perform DMA-like operations or use memory for bulk data transfers.
///
/// **Validation:** All memory operations validate that addresses fall within the valid
/// DRAM range (0x8000_0000 - 0xFFFF_FFFF). Out-of-bounds accesses are logged as
/// warnings and return zero for reads or silently fail for writes.
pub struct SystemContext<'a> {
    memory: &'a mut Memory,
}

impl<'a> SystemContext<'a> {
    /// Create a new SystemContext with access to system memory
    pub fn new(memory: &'a mut Memory) -> Self {
        SystemContext { memory }
    }

    /// Read a 32-bit word from memory at the given address
    /// Addresses are absolute (not offset-relative)
    ///
    /// **Validation:** Address must be within DRAM range (0x8000_0000 - 0xFFFF_FFFF).
    /// Out-of-bounds reads are logged as warnings and return 0.
    pub fn read_word(&self, addr: u32) -> u32 {
        if !is_valid_dram_range(addr, 4) {
            log::warn!(
                "SystemContext::read_word: Address 0x{:08x} is outside valid DRAM range (0x{:08x} - 0x{:08x}), returning 0",
                addr,
                DRAM_BASE,
                DRAM_END
            );
            return 0;
        }
        self.memory.read_word(addr)
    }

    /// Read a 16-bit halfword from memory at the given address
    /// Addresses are absolute (not offset-relative)
    ///
    /// **Validation:** Address must be within DRAM range (0x8000_0000 - 0xFFFF_FFFF).
    /// Out-of-bounds reads are logged as warnings and return 0.
    pub fn read_halfword(&self, addr: u32) -> u16 {
        if !is_valid_dram_range(addr, 2) {
            log::warn!(
                "SystemContext::read_halfword: Address 0x{:08x} is outside valid DRAM range (0x{:08x} - 0x{:08x}), returning 0",
                addr,
                DRAM_BASE,
                DRAM_END
            );
            return 0;
        }
        self.memory.read_halfword(addr)
    }

    /// Read a single byte from memory at the given address
    /// Addresses are absolute (not offset-relative)
    ///
    /// **Validation:** Address must be within DRAM range (0x8000_0000 - 0xFFFF_FFFF).
    /// Out-of-bounds reads are logged as warnings and return 0.
    pub fn read_byte(&self, addr: u32) -> u8 {
        if !is_valid_dram_range(addr, 1) {
            log::warn!(
                "SystemContext::read_byte: Address 0x{:08x} is outside valid DRAM range (0x{:08x} - 0x{:08x}), returning 0",
                addr,
                DRAM_BASE,
                DRAM_END
            );
            return 0;
        }
        self.memory.read_byte(addr)
    }

    /// Write a 32-bit word to memory at the given address
    /// Addresses are absolute (not offset-relative)
    ///
    /// **Validation:** Address must be within DRAM range (0x8000_0000 - 0xFFFF_FFFF).
    /// Out-of-bounds writes are logged as warnings and silently discarded.
    pub fn write_word(&mut self, addr: u32, data: u32) {
        if !is_valid_dram_range(addr, 4) {
            log::warn!(
                "SystemContext::write_word: Address 0x{:08x} is outside valid DRAM range (0x{:08x} - 0x{:08x}), discarding write (value=0x{:08x})",
                addr,
                DRAM_BASE,
                DRAM_END,
                data
            );
            return;
        }
        self.memory.write_word(addr, data);
    }

    /// Write a 16-bit halfword to memory at the given address
    /// Addresses are absolute (not offset-relative)
    ///
    /// **Validation:** Address must be within DRAM range (0x8000_0000 - 0xFFFF_FFFF).
    /// Out-of-bounds writes are logged as warnings and silently discarded.
    pub fn write_halfword(&mut self, addr: u32, data: u16) {
        if !is_valid_dram_range(addr, 2) {
            log::warn!(
                "SystemContext::write_halfword: Address 0x{:08x} is outside valid DRAM range (0x{:08x} - 0x{:08x}), discarding write (value=0x{:04x})",
                addr,
                DRAM_BASE,
                DRAM_END,
                data
            );
            return;
        }
        self.memory.write_halfword(addr, data);
    }

    /// Write a single byte to memory at the given address
    /// Addresses are absolute (not offset-relative)
    ///
    /// **Validation:** Address must be within DRAM range (0x8000_0000 - 0xFFFF_FFFF).
    /// Out-of-bounds writes are logged as warnings and silently discarded.
    pub fn write_byte(&mut self, addr: u32, data: u8) {
        if !is_valid_dram_range(addr, 1) {
            log::warn!(
                "SystemContext::write_byte: Address 0x{:08x} is outside valid DRAM range (0x{:08x} - 0x{:08x}), discarding write (value=0x{:02x})",
                addr,
                DRAM_BASE,
                DRAM_END,
                data
            );
            return;
        }
        self.memory.write_byte(addr, data);
    }
}

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
                write!(
                    f,
                    "Read from write-only register at offset 0x{:08x}",
                    offset
                )
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
///
/// All methods receive a SystemContext parameter that provides access to
/// system memory. This allows devices to perform DMA-like operations.
pub trait BusDevice {
    /// Read a 32-bit word from the device at the given offset
    ///
    /// # Arguments
    /// * `ctx` - System context providing access to system memory
    /// * `offset` - Byte offset relative to the device's base address (must be word-aligned)
    ///
    /// # Returns
    /// * `Ok(u32)` - The word value read from the device
    /// * `Err(BusDeviceError)` - If the read is invalid (e.g., write-only register)
    fn read_word(&mut self, ctx: &mut SystemContext, offset: u32) -> Result<u32, BusDeviceError>;

    /// Write a 32-bit word to the device at the given offset
    ///
    /// # Arguments
    /// * `ctx` - System context providing access to system memory
    /// * `offset` - Byte offset relative to the device's base address (must be word-aligned)
    /// * `value` - The 32-bit value to write
    ///
    /// # Returns
    /// * `Ok(())` - Write successful
    /// * `Err(BusDeviceError)` - If the write is invalid (e.g., read-only register)
    fn write_word(
        &mut self,
        ctx: &mut SystemContext,
        offset: u32,
        value: u32,
    ) -> Result<(), BusDeviceError>;

    /// Read a 16-bit halfword from the device at the given offset
    ///
    /// # Arguments
    /// * `ctx` - System context providing access to system memory
    /// * `offset` - Byte offset relative to the device's base address (must be halfword-aligned)
    ///
    /// # Returns
    /// * `Ok(u16)` - The halfword value read from the device
    /// * `Err(BusDeviceError)` - If the read is invalid or size not supported
    ///
    /// # Default Implementation
    /// Returns `UnsupportedSize` error. Devices that support halfword access should override this.
    fn read_halfword(
        &mut self,
        _ctx: &mut SystemContext,
        offset: u32,
    ) -> Result<u16, BusDeviceError> {
        Err(BusDeviceError::UnsupportedSize { offset, size: 2 })
    }

    /// Write a 16-bit halfword to the device at the given offset
    ///
    /// # Arguments
    /// * `ctx` - System context providing access to system memory
    /// * `offset` - Byte offset relative to the device's base address (must be halfword-aligned)
    /// * `_value` - The 16-bit value to write
    ///
    /// # Returns
    /// * `Ok(())` - Write successful
    /// * `Err(BusDeviceError)` - If the write is invalid or size not supported
    ///
    /// # Default Implementation
    /// Returns `UnsupportedSize` error. Devices that support halfword access should override this.
    fn write_halfword(
        &mut self,
        _ctx: &mut SystemContext,
        offset: u32,
        _value: u16,
    ) -> Result<(), BusDeviceError> {
        Err(BusDeviceError::UnsupportedSize { offset, size: 2 })
    }

    /// Read a single byte from the device at the given offset
    ///
    /// # Arguments
    /// * `ctx` - System context providing access to system memory
    /// * `offset` - Byte offset relative to the device's base address
    ///
    /// # Returns
    /// * `Ok(u8)` - The byte value read from the device
    /// * `Err(BusDeviceError)` - If the read is invalid or size not supported
    ///
    /// # Default Implementation
    /// Returns `UnsupportedSize` error. Devices that support byte access should override this.
    fn read_byte(&mut self, _ctx: &mut SystemContext, offset: u32) -> Result<u8, BusDeviceError> {
        Err(BusDeviceError::UnsupportedSize { offset, size: 1 })
    }

    /// Write a single byte to the device at the given offset
    ///
    /// # Arguments
    /// * `ctx` - System context providing access to system memory
    /// * `offset` - Byte offset relative to the device's base address
    /// * `_value` - The 8-bit value to write
    ///
    /// # Returns
    /// * `Ok(())` - Write successful
    /// * `Err(BusDeviceError)` - If the write is invalid or size not supported
    ///
    /// # Default Implementation
    /// Returns `UnsupportedSize` error. Devices that support byte access should override this.
    fn write_byte(
        &mut self,
        _ctx: &mut SystemContext,
        offset: u32,
        _value: u8,
    ) -> Result<(), BusDeviceError> {
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

    /// Reset the device to its initial state
    ///
    /// This method is called when the simulator is reset. Device implementations
    /// should use this to clear any internal state that needs to be reset when
    /// the simulation starts.
    ///
    /// # Arguments
    /// * `_ctx` - System context providing access to system memory
    ///
    /// # Default Implementation
    /// Does nothing, which is appropriate for devices with no internal state.
    fn reset(&mut self, _ctx: &mut SystemContext) {}

    /// Execute device logic for one clock cycle
    ///
    /// This method is called once per simulated clock cycle, allowing devices to
    /// perform operations that span multiple cycles. This enables realistic
    /// simulation of hardware peripherals that cannot complete operations
    /// instantaneously (e.g., DMA controllers that transfer one word per cycle).
    ///
    /// # Arguments
    /// * `_ctx` - System context providing access to system memory
    ///
    /// # Default Implementation
    /// Does nothing, which is appropriate for devices that complete all operations
    /// synchronously within read/write handlers.
    fn clock_cycle(&mut self, _ctx: &mut SystemContext) {}
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

impl std::fmt::Display for RegistrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistrationError::AddressOverlap {
                new_base,
                new_end,
                existing_base,
                existing_end,
                existing_name,
            } => {
                write!(
                    f,
                    "Address range overlap: new device [0x{:08x}, 0x{:08x}) overlaps with '{}' [0x{:08x}, 0x{:08x})",
                    new_base, new_end, existing_name, existing_base, existing_end
                )
            }
            RegistrationError::InvalidAlignment { size } => {
                write!(
                    f,
                    "Device size {} is not word-aligned (must be multiple of 4)",
                    size
                )
            }
            RegistrationError::InvalidBaseAlignment { base_addr } => {
                write!(
                    f,
                    "Device base address 0x{:08x} is not word-aligned",
                    base_addr
                )
            }
            RegistrationError::ZeroSize => {
                write!(f, "Device size is zero")
            }
        }
    }
}

impl std::error::Error for RegistrationError {}

/// Helper function to check if two address ranges overlap
///
/// # Arguments
/// * `a_start` - Start address of first range (inclusive)
/// * `a_end` - End address of first range (exclusive)
/// * `b_start` - Start address of second range (inclusive)
/// * `b_end` - End address of second range (exclusive)
///
/// # Returns
/// `true` if the ranges overlap, `false` otherwise
pub(crate) fn ranges_overlap(a_start: u32, a_end: u32, b_start: u32, b_end: u32) -> bool {
    a_start < b_end && b_start < a_end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ranges_overlap() {
        // Overlapping ranges
        assert!(ranges_overlap(0x0, 0x100, 0x50, 0x150));
        assert!(ranges_overlap(0x50, 0x150, 0x0, 0x100));
        assert!(ranges_overlap(0x0, 0x100, 0x0, 0x100)); // Exact overlap
        assert!(ranges_overlap(0x0, 0x100, 0x20, 0x80)); // Contained

        // Non-overlapping ranges
        assert!(!ranges_overlap(0x0, 0x100, 0x100, 0x200)); // Adjacent
        assert!(!ranges_overlap(0x100, 0x200, 0x0, 0x100)); // Adjacent (reversed)
        assert!(!ranges_overlap(0x0, 0x100, 0x200, 0x300)); // Separate
    }
}
