use crate::bus_device::{BusDevice, BusDeviceError};
use std::collections::HashMap;

/// DRAM model for the RISC-V CPU simulator
/// Uses a byte-addressable HashMap for sparse memory representation
/// Supports LR/SC reservation tracking for RV32A atomic extension
pub struct Dram {
    data: HashMap<u32, u8>,
    /// LR/SC reservation tracking (RV32A atomic extension)
    reservation_valid: bool,
    reservation_addr: u32,
}

impl Dram {
    /// Create a new, empty DRAM
    pub fn new() -> Self {
        Dram {
            data: HashMap::new(),
            reservation_valid: false,
            reservation_addr: 0,
        }
    }

    /// Read a 32-bit word from DRAM (little-endian)
    /// No alignment is performed - reads from the exact address specified
    pub fn read_word(&self, addr: u32) -> u32 {
        let b0 = *self.data.get(&addr).unwrap_or(&0) as u32;
        let b1 = *self.data.get(&addr.wrapping_add(1)).unwrap_or(&0) as u32;
        let b2 = *self.data.get(&addr.wrapping_add(2)).unwrap_or(&0) as u32;
        let b3 = *self.data.get(&addr.wrapping_add(3)).unwrap_or(&0) as u32;
        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }

    /// Read a single byte from DRAM
    pub fn read_byte(&self, addr: u32) -> u8 {
        *self.data.get(&addr).unwrap_or(&0)
    }

    /// Read a 16-bit halfword from DRAM (little-endian)
    pub fn read_halfword(&self, addr: u32) -> u16 {
        let b0 = *self.data.get(&addr).unwrap_or(&0) as u16;
        let b1 = *self.data.get(&addr.wrapping_add(1)).unwrap_or(&0) as u16;
        b0 | (b1 << 8)
    }

    /// Write a 32-bit word to DRAM (little-endian)
    /// Clears LR/SC reservation if writing to reserved address
    pub fn write_word(&mut self, addr: u32, data: u32) {
        self.data.insert(addr, (data & 0xFF) as u8);
        self.data
            .insert(addr.wrapping_add(1), ((data >> 8) & 0xFF) as u8);
        self.data
            .insert(addr.wrapping_add(2), ((data >> 16) & 0xFF) as u8);
        self.data
            .insert(addr.wrapping_add(3), ((data >> 24) & 0xFF) as u8);

        // Clear reservation if writing to reserved address (word-aligned check)
        if self.reservation_valid && (addr & !0x3) == (self.reservation_addr & !0x3) {
            self.reservation_valid = false;
        }
    }

    /// Write a single byte to DRAM
    /// Clears LR/SC reservation if writing to reserved address
    pub fn write_byte(&mut self, addr: u32, data: u8) {
        self.data.insert(addr, data);

        // Clear reservation if writing to reserved address (word-aligned check)
        if self.reservation_valid && (addr & !0x3) == (self.reservation_addr & !0x3) {
            self.reservation_valid = false;
        }
    }

    /// Write a 16-bit halfword to DRAM (little-endian)
    /// Clears LR/SC reservation if writing to reserved address
    pub fn write_halfword(&mut self, addr: u32, data: u16) {
        self.data.insert(addr, (data & 0xFF) as u8);
        self.data
            .insert(addr.wrapping_add(1), ((data >> 8) & 0xFF) as u8);

        // Clear reservation if writing to reserved address (word-aligned check)
        if self.reservation_valid && (addr & !0x3) == (self.reservation_addr & !0x3) {
            self.reservation_valid = false;
        }
    }

    /// Set LR/SC reservation (RV32A atomic extension)
    /// Called when LR.W instruction completes
    #[allow(dead_code)]
    pub fn set_reservation(&mut self, addr: u32) {
        self.reservation_valid = true;
        self.reservation_addr = addr & !0x3; // Word-align the address
    }

    /// Clear LR/SC reservation (RV32A atomic extension)
    /// Called when SC.W instruction executes (regardless of success)
    #[allow(dead_code)]
    pub fn clear_reservation(&mut self) {
        self.reservation_valid = false;
    }

    /// Check if reservation is valid for the given address (RV32A atomic extension)
    /// Used by SC.W to determine success/failure
    #[allow(dead_code)]
    pub fn check_reservation(&self, addr: u32) -> bool {
        self.reservation_valid && (addr & !0x3) == (self.reservation_addr & !0x3)
    }
}

impl Default for Dram {
    fn default() -> Self {
        Self::new()
    }
}

impl BusDevice for Dram {
    fn read_word(&mut self, offset: u32) -> Result<u32, BusDeviceError> {
        // DRAM receives offset relative to its base address (0x8000_0000)
        // Use offset directly - internal HashMap stores relative addresses
        Ok(Dram::read_word(self, offset))
    }

    fn write_word(&mut self, offset: u32, value: u32) -> Result<(), BusDeviceError> {
        // Use offset directly - internal HashMap stores relative addresses
        Dram::write_word(self, offset, value);
        Ok(())
    }

    fn read_halfword(&mut self, offset: u32) -> Result<u16, BusDeviceError> {
        // Use offset directly - internal HashMap stores relative addresses
        Ok(Dram::read_halfword(self, offset))
    }

    fn write_halfword(&mut self, offset: u32, value: u16) -> Result<(), BusDeviceError> {
        // Use offset directly - internal HashMap stores relative addresses
        Dram::write_halfword(self, offset, value);
        Ok(())
    }

    fn read_byte(&mut self, offset: u32) -> Result<u8, BusDeviceError> {
        // Use offset directly - internal HashMap stores relative addresses
        Ok(Dram::read_byte(self, offset))
    }

    fn write_byte(&mut self, offset: u32, value: u8) -> Result<(), BusDeviceError> {
        // Use offset directly - internal HashMap stores relative addresses
        Dram::write_byte(self, offset, value);
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
