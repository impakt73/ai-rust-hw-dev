use std::collections::HashMap;

/// Memory model for the RISC-V CPU simulator
/// Uses a byte-addressable HashMap for sparse memory representation
///
/// This structure stores memory data and is owned by SystemBus.
/// It can be accessed by BusDevice implementations through SystemContext.
pub struct Memory {
    data: HashMap<u32, u8>,
}

impl Memory {
    /// Create a new, empty Memory
    pub fn new() -> Self {
        Memory {
            data: HashMap::new(),
        }
    }

    /// Read a 32-bit word from memory (little-endian)
    /// No alignment is performed - reads from the exact address specified
    pub fn read_word(&self, addr: u32) -> u32 {
        let b0 = *self.data.get(&addr).unwrap_or(&0) as u32;
        let b1 = *self.data.get(&addr.wrapping_add(1)).unwrap_or(&0) as u32;
        let b2 = *self.data.get(&addr.wrapping_add(2)).unwrap_or(&0) as u32;
        let b3 = *self.data.get(&addr.wrapping_add(3)).unwrap_or(&0) as u32;
        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }

    /// Read a single byte from memory
    pub fn read_byte(&self, addr: u32) -> u8 {
        *self.data.get(&addr).unwrap_or(&0)
    }

    /// Read a 16-bit halfword from memory (little-endian)
    pub fn read_halfword(&self, addr: u32) -> u16 {
        let b0 = *self.data.get(&addr).unwrap_or(&0) as u16;
        let b1 = *self.data.get(&addr.wrapping_add(1)).unwrap_or(&0) as u16;
        b0 | (b1 << 8)
    }

    /// Write a 32-bit word to memory (little-endian)
    pub fn write_word(&mut self, addr: u32, data: u32) {
        self.data.insert(addr, (data & 0xFF) as u8);
        self.data
            .insert(addr.wrapping_add(1), ((data >> 8) & 0xFF) as u8);
        self.data
            .insert(addr.wrapping_add(2), ((data >> 16) & 0xFF) as u8);
        self.data
            .insert(addr.wrapping_add(3), ((data >> 24) & 0xFF) as u8);
    }

    /// Write a single byte to memory
    pub fn write_byte(&mut self, addr: u32, data: u8) {
        self.data.insert(addr, data);
    }

    /// Write a 16-bit halfword to memory (little-endian)
    pub fn write_halfword(&mut self, addr: u32, data: u16) {
        self.data.insert(addr, (data & 0xFF) as u8);
        self.data
            .insert(addr.wrapping_add(1), ((data >> 8) & 0xFF) as u8);
    }

    /// Clear all memory contents
    pub fn reset(&mut self) {
        self.data.clear();
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}
