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
    pub fn new() -> Self {
        SystemBus {
            dram: Dram::new(),
            fifo: Fifo::new(),
        }
    }

    /// Read a 32-bit word from the bus
    /// Routes to FIFO or DRAM based on address
    pub fn read_word(&mut self, addr: u32) -> u32 {
        let value = match addr {
            // FIFO DATA register
            a if a == FIFO_BASE + FIFO_DATA_OFFSET => self.fifo.read_data(),
            // FIFO STATUS register
            a if a == FIFO_BASE + FIFO_STATUS_OFFSET => self.fifo.read_status(),
            // Default: DRAM
            _ => self.dram.read_word(addr),
        };
        // DEBUG: Log reads from data section
        if addr >= 0x80001a00 && addr < 0x80002000 {
            eprintln!("[MEM_DEBUG] READ_WORD: addr=0x{:08x} -> data=0x{:08x}", addr, value);
        }
        value
    }

    /// Read a single byte from the bus
    /// Routes to DRAM only (FIFO is word-based)
    pub fn read_byte(&mut self, addr: u32) -> u8 {
        let value = self.dram.read_byte(addr);
        // DEBUG: Log reads from data section  
        if addr >= 0x80001a00 && addr < 0x80002000 {
            eprintln!("[MEM_DEBUG] READ_BYTE: addr=0x{:08x} -> data=0x{:02x} ('{}')", 
                     addr, value, if value.is_ascii_graphic() { value as char } else { '.' });
        }
        value
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
            a if a == FIFO_BASE + FIFO_DATA_OFFSET => {
                // DEBUG: Log FIFO writes
                eprintln!("[FIFO_DEBUG] FIFO_WRITE: addr=0x{:08x} data=0x{:08x} ({})", addr, data, 
                         std::str::from_utf8(&data.to_le_bytes()).unwrap_or("???"));
                self.fifo.write_data(data);
            }
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
        // DEBUG: Check if byte write to FIFO (shouldn't happen but let's verify)
        if addr >= FIFO_BASE && addr < FIFO_BASE + 0x100 {
            eprintln!("[FIFO_DEBUG] FIFO_BYTE_WRITE: addr=0x{:08x} data=0x{:02x} ('{}')", 
                     addr, data, if data.is_ascii_graphic() { data as char } else { '.' });
        }
        self.dram.write_byte(addr, data);
    }

    /// Write a 16-bit halfword to the bus
    /// Routes to DRAM only (FIFO is word-based)
    pub fn write_halfword(&mut self, addr: u32, data: u16) {
        // DEBUG: Check if halfword write to FIFO (shouldn't happen but let's verify)
        if addr >= FIFO_BASE && addr < FIFO_BASE + 0x100 {
            eprintln!("[FIFO_DEBUG] FIFO_HALF_WRITE: addr=0x{:08x} data=0x{:04x}", addr, data);
        }
        self.dram.write_halfword(addr, data);
    }
}

impl Default for SystemBus {
    fn default() -> Self {
        Self::new()
    }
}
