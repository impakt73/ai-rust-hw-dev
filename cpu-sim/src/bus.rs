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
}
