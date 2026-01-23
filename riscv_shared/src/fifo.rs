//! FIFO device register offsets and constants

use crate::bus::FIFO_BASE;

/// FIFO data register offset
pub const FIFO_DATA: u32 = FIFO_BASE;

/// FIFO status register offset
pub const FIFO_STATUS: u32 = FIFO_BASE + 0x4;

/// RX_VALID status bit - indicates data is available to read
pub const RX_VALID: u32 = 1 << 0;

/// TX_READY status bit - indicates FIFO is ready to accept data
pub const TX_READY: u32 = 1 << 1;
