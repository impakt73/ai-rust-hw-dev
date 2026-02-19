use crate::bus_device::{BusDevice, BusDeviceError, SystemContext};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Callback invoked whenever the CPU writes a word to FIFO DATA.
pub type FifoDataReceivedCallback = Box<dyn FnMut(u32) + Send>;

/// Shared host→CPU FIFO data source.
#[derive(Clone, Default)]
pub struct FifoDataSource {
    host_to_cpu: Arc<Mutex<VecDeque<u32>>>,
}

impl FifoDataSource {
    /// Create an empty host→CPU data source.
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a word that the CPU can later read from FIFO DATA.
    pub fn write_word(&self, word: u32) {
        self.host_to_cpu
            .lock()
            .expect("FifoDataSource lock poisoned in write_word")
            .push_back(word);
    }

    /// Pop the next word for CPU consumption.
    pub fn read_word(&self) -> Option<u32> {
        self.host_to_cpu
            .lock()
            .expect("FifoDataSource lock poisoned in read_word")
            .pop_front()
    }

    /// Returns whether host→CPU queue is empty.
    pub fn is_empty(&self) -> bool {
        self.host_to_cpu
            .lock()
            .expect("FifoDataSource lock poisoned in is_empty")
            .is_empty()
    }

    /// Clear all pending host→CPU words.
    pub fn clear(&self) {
        self.host_to_cpu
            .lock()
            .expect("FifoDataSource lock poisoned in clear")
            .clear();
    }
}

/// FIFO peripheral for UART-style communication
/// Provides host→CPU data via shared source and CPU→host notification via callback.
pub struct Fifo {
    /// Data sent FROM Host -> CPU (as u32 words)
    pub host_to_cpu: FifoDataSource,
    on_data_received: FifoDataReceivedCallback,
}

impl Fifo {
    /// Create a new FIFO backed by a shared host→CPU data source.
    pub fn new_with_callback(
        host_to_cpu: FifoDataSource,
        on_data_received: FifoDataReceivedCallback,
    ) -> Self {
        Fifo {
            host_to_cpu,
            on_data_received,
        }
    }

    /// Read the STATUS register
    /// Bit 0 (RX_VALID): 1 if RX has data, 0 if empty
    /// Bit 1 (TX_READY): Always 1 (simulated buffer is infinite)
    pub fn read_status(&self) -> u32 {
        let rx_valid = if self.host_to_cpu.is_empty() { 0 } else { 1 };
        let tx_ready = 1; // Always ready (infinite buffer)
        (tx_ready << 1) | rx_valid
    }

    /// Read the DATA register
    /// Pops a u32 word from the RX queue
    /// Returns 0 if RX is empty
    pub fn read_data(&mut self) -> u32 {
        match self.host_to_cpu.read_word() {
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
    /// Forwards a u32 word to host callback immediately.
    pub fn write_data(&mut self, val: u32) {
        (self.on_data_received)(val);
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
        self.host_to_cpu.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Memory;

    #[test]
    fn test_reset_clears_host_to_cpu_queue() {
        let source = FifoDataSource::new();
        let mut fifo = Fifo::new_with_callback(source.clone(), Box::new(|_| {}));
        fifo.write_data(0x1111_1111);
        source.write_word(0x2222_2222);

        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);
        fifo.reset(&mut ctx);

        assert!(source.is_empty());
    }
}
