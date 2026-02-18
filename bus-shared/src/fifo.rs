use crate::bus_device::{BusDevice, BusDeviceError, SystemContext};
use std::collections::VecDeque;

/// Maximum capacity for TX FIFO buffer
/// While the software implementation can grow indefinitely, we define a logical
/// capacity to warn about potential hardware mismatches
const TX_FIFO_CAPACITY: usize = 1024;

/// Callback type for FIFO data-received events.
///
/// Called in `clock_cycle` when the TX queue (CPU → Host) contains data.
/// Receives mutable access to both the TX queue (drain it) and the RX queue
/// (optionally inject reply data for Host → CPU communication).
type FifoDataReceivedCallback = Box<dyn FnMut(&mut VecDeque<u32>, &mut VecDeque<u32>) + Send>;

/// FIFO peripheral for UART-style communication
/// Provides buffered I/O between the simulated CPU and host
pub struct Fifo {
    /// Data sent FROM CPU -> Host (as u32 words)
    pub tx: VecDeque<u32>,
    /// Data sent FROM Host -> CPU (as u32 words)
    pub rx: VecDeque<u32>,
    /// Optional callback invoked in `clock_cycle` when TX data is available.
    /// Receives mutable references to both TX (CPU→Host) and RX (Host→CPU) queues,
    /// allowing the host to drain TX data and optionally inject RX data.
    data_received_callback: Option<FifoDataReceivedCallback>,
}

impl Fifo {
    /// Create a new FIFO with empty TX and RX queues and no callback
    pub fn new() -> Self {
        Fifo {
            tx: VecDeque::new(),
            rx: VecDeque::new(),
            data_received_callback: None,
        }
    }

    /// Create a new FIFO with an optional data-received callback.
    ///
    /// The callback is invoked in `clock_cycle` whenever the TX queue (CPU → Host)
    /// contains data available for consumption. It receives mutable references to
    /// both the TX and RX queues so the host can drain received data and optionally
    /// inject reply data into the RX queue.
    pub fn new_with_callback(callback: FifoDataReceivedCallback) -> Self {
        Fifo {
            tx: VecDeque::new(),
            rx: VecDeque::new(),
            data_received_callback: Some(callback),
        }
    }

    /// Read the STATUS register
    /// Bit 0 (RX_VALID): 1 if RX has data, 0 if empty
    /// Bit 1 (TX_READY): Always 1 (simulated buffer is infinite)
    pub fn read_status(&self) -> u32 {
        let rx_valid = if self.rx.is_empty() { 0 } else { 1 };
        let tx_ready = 1; // Always ready (infinite buffer)
        (tx_ready << 1) | rx_valid
    }

    /// Read the DATA register
    /// Pops a u32 word from the RX queue
    /// Returns 0 if RX is empty
    pub fn read_data(&mut self) -> u32 {
        match self.rx.pop_front() {
            Some(val) => val,
            None => {
                log::warn!(
                    "FIFO RX queue read while empty. \
                     This may indicate the status register was not checked before reading."
                );
                0
            }
        }
    }

    /// Write to the DATA register
    /// Pushes a u32 word to the TX queue
    pub fn write_data(&mut self, val: u32) {
        if self.tx.len() >= TX_FIFO_CAPACITY {
            log::warn!(
                "FIFO TX queue write while at capacity ({}). \
                 This may indicate the status register was not checked before writing.",
                TX_FIFO_CAPACITY
            );
        }
        self.tx.push_back(val);
    }
}

impl Default for Fifo {
    fn default() -> Self {
        Self::new()
    }
}

impl BusDevice for Fifo {
    fn read_word(&mut self, _ctx: &mut SystemContext, offset: u32) -> Result<u32, BusDeviceError> {
        match offset {
            0x00 => Ok(self.read_data()),
            0x04 => Ok(self.read_status()),
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }

    fn write_word(
        &mut self,
        _ctx: &mut SystemContext,
        offset: u32,
        value: u32,
    ) -> Result<(), BusDeviceError> {
        match offset {
            0x00 => {
                self.write_data(value);
                Ok(())
            }
            0x04 => {
                // STATUS register is read-only
                Err(BusDeviceError::WriteToReadOnly { offset })
            }
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }

    fn size(&self) -> u32 {
        // FIFO has 2 word-aligned registers within its address window:
        //   - DATA   at offset 0x00 (read/write)
        //   - STATUS at offset 0x04 (read-only)
        //
        // The device reserves a contiguous 8-byte region [0x00..=0x07] on the bus
        // to allow for potential future expansion. Currently, only word-aligned
        // offsets 0x00 and 0x04 are valid for word access operations.
        // All other offsets will result in BusDeviceError::InvalidAddress.
        8
    }

    fn name(&self) -> &str {
        "FIFO"
    }

    fn reset(&mut self, _ctx: &mut SystemContext) {
        self.tx.clear();
        self.rx.clear();
    }

    fn clock_cycle(&mut self, _ctx: &mut SystemContext) {
        if !self.tx.is_empty() {
            if let Some(callback) = &mut self.data_received_callback {
                callback(&mut self.tx, &mut self.rx);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Memory;

    #[test]
    fn test_reset_clears_tx_and_rx_queues() {
        let mut fifo = Fifo::new();
        fifo.write_data(0x1111_1111);
        fifo.rx.push_back(0x2222_2222);

        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);
        fifo.reset(&mut ctx);

        assert!(fifo.tx.is_empty());
        assert!(fifo.rx.is_empty());
    }
}
