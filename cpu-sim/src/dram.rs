use std::collections::HashMap;

/// DRAM model for the RISC-V CPU simulator
/// Uses a byte-addressable HashMap for sparse memory representation
pub struct Dram {
    data: HashMap<u32, u8>,
}

impl Dram {
    /// Create a new, empty DRAM
    pub fn new() -> Self {
        Dram {
            data: HashMap::new(),
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
    pub fn write_word(&mut self, addr: u32, data: u32) {
        self.data.insert(addr, (data & 0xFF) as u8);
        self.data
            .insert(addr.wrapping_add(1), ((data >> 8) & 0xFF) as u8);
        self.data
            .insert(addr.wrapping_add(2), ((data >> 16) & 0xFF) as u8);
        self.data
            .insert(addr.wrapping_add(3), ((data >> 24) & 0xFF) as u8);
    }

    /// Write a single byte to DRAM
    pub fn write_byte(&mut self, addr: u32, data: u8) {
        self.data.insert(addr, data);
    }

    /// Write a 16-bit halfword to DRAM (little-endian)
    pub fn write_halfword(&mut self, addr: u32, data: u16) {
        self.data.insert(addr, (data & 0xFF) as u8);
        self.data
            .insert(addr.wrapping_add(1), ((data >> 8) & 0xFF) as u8);
    }
}

impl Default for Dram {
    fn default() -> Self {
        Self::new()
    }
}
