//! FIFO device register offsets and constants

use crate::bus::FIFO_BASE;
use core::convert::Infallible;
use core::ptr::{read_volatile, write_volatile};

/// FIFO data register offset
pub const FIFO_DATA: u32 = FIFO_BASE;

/// FIFO status register offset
pub const FIFO_STATUS: u32 = FIFO_BASE + 0x4;

/// RX_VALID status bit - indicates data is available to read
pub const RX_VALID: u32 = 1 << 0;

/// TX_READY status bit - indicates FIFO is ready to accept data
pub const TX_READY: u32 = 1 << 1;

/// Byte-oriented writer that sends UTF-8 text to FIFO DATA for logging output.
pub struct FifoUwrite;

impl FifoUwrite {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    fn write_byte(&mut self, byte: u8) {
        unsafe {
            while read_volatile(FIFO_STATUS as *const u32) & TX_READY == 0 {}
            write_volatile(FIFO_DATA as *mut u8, byte);
        }
    }
}

impl Default for FifoUwrite {
    fn default() -> Self {
        Self::new()
    }
}

impl ufmt::uWrite for FifoUwrite {
    type Error = Infallible;

    fn write_str(&mut self, s: &str) -> Result<(), Self::Error> {
        for byte in s.bytes() {
            self.write_byte(byte);
        }
        Ok(())
    }
}
