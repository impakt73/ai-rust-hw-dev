//! Sparse memory model for FPGA host
//!
//! This module provides a byte-addressable sparse memory implementation
//! using a HashMap for efficient storage of only written bytes.

use std::collections::HashMap;

/// Sparse memory model using a byte-addressable HashMap
///
/// Similar to cpu-sim/src/memory.rs but simplified for fpga-host use case
pub struct SparseMemory {
    data: HashMap<u32, u8>,
}

impl SparseMemory {
    /// Create a new empty sparse memory
    pub fn new() -> Self {
        SparseMemory {
            data: HashMap::new(),
        }
    }

    /// Read a single byte from memory
    pub fn read_byte(&self, addr: u32) -> u8 {
        *self.data.get(&addr).unwrap_or(&0)
    }

    /// Read a 16-bit halfword from memory (little-endian)
    pub fn read_halfword(&self, addr: u32) -> u16 {
        let b0 = self.read_byte(addr) as u16;
        let b1 = self.read_byte(addr.wrapping_add(1)) as u16;
        b0 | (b1 << 8)
    }

    /// Read a 32-bit word from memory (little-endian)
    pub fn read_word(&self, addr: u32) -> u32 {
        let b0 = self.read_byte(addr) as u32;
        let b1 = self.read_byte(addr.wrapping_add(1)) as u32;
        let b2 = self.read_byte(addr.wrapping_add(2)) as u32;
        let b3 = self.read_byte(addr.wrapping_add(3)) as u32;
        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
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

    /// Clear all memory contents
    pub fn clear(&mut self) {
        self.data.clear();
    }
}

impl Default for SparseMemory {
    fn default() -> Self {
        Self::new()
    }
}
