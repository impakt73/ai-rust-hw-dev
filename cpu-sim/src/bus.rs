use crate::dram::Dram;
use crate::fifo::Fifo;

/// Memory map constants
const FIFO_BASE: u32 = 0x4000_0000;
const FIFO_DATA_OFFSET: u32 = 0x00;
const FIFO_STATUS_OFFSET: u32 = 0x04;

/// System bus that routes memory accesses to the correct device
pub struct SystemBus {
    pub dram: Dram,
    pub fifo: Fifo,
}

impl SystemBus {
    /// Create a new system bus with initialized DRAM and FIFO
    pub fn new(dram: Dram) -> Self {
        SystemBus {
            dram,
            fifo: Fifo::new(),
        }
    }

    /// Read a 32-bit word from the bus
    /// Routes to FIFO or DRAM based on address
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

    /// Read a single byte from the bus
    /// Routes to DRAM only (FIFO is word-based)
    pub fn read_byte(&mut self, addr: u32) -> u8 {
        self.dram.read_byte(addr)
    }

    /// Read a 16-bit halfword from the bus
    /// Routes to DRAM only (FIFO is word-based)
    pub fn read_halfword(&mut self, addr: u32) -> u16 {
        self.dram.read_halfword(addr)
    }

    /// Write a 32-bit word to the bus
    /// Routes to FIFO or DRAM based on address
    pub fn write_word(&mut self, addr: u32, data: u32) {
        match addr {
            // FIFO DATA register
            a if a == FIFO_BASE + FIFO_DATA_OFFSET => self.fifo.write_data(data),
            // FIFO STATUS register (read-only, ignore writes)
            a if a == FIFO_BASE + FIFO_STATUS_OFFSET => {
                // Status is read-only, ignore write
            }
            // Default: DRAM
            _ => self.dram.write_word(addr, data),
        }
    }

    /// Write a single byte to the bus
    /// Routes to DRAM only (FIFO is word-based)
    pub fn write_byte(&mut self, addr: u32, data: u8) {
        self.dram.write_byte(addr, data);
    }

    /// Write a 16-bit halfword to the bus
    /// Routes to DRAM only (FIFO is word-based)
    pub fn write_halfword(&mut self, addr: u32, data: u16) {
        self.dram.write_halfword(addr, data);
    }

    /// Write to the bus with byte enables
    /// Only writes the bytes indicated by the byte_enable mask
    pub fn write_word_with_be(&mut self, addr: u32, data: u32, byte_enable: u8) {
        match addr {
            // FIFO DATA register - always writes full word
            a if a == FIFO_BASE + FIFO_DATA_OFFSET => self.fifo.write_data(data),
            // FIFO STATUS register (read-only, ignore writes)
            a if a == FIFO_BASE + FIFO_STATUS_OFFSET => {
                // Status is read-only, ignore write
            }
            // Default: DRAM with byte enables
            _ => self.dram.write_word_with_be(addr, data, byte_enable),
        }
    }
}
