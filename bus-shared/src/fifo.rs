use crate::bus_device::{BusDevice, BusDeviceError, SystemContext};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Callback invoked whenever the CPU writes a byte to FIFO DATA.
pub type FifoDataReceivedCallback = Box<dyn FnMut(u8) + Send>;

/// Host→CPU FIFO data source.
#[derive(Default)]
pub struct FifoDataSource {
    host_to_cpu: VecDeque<u8>,
}

impl FifoDataSource {
    /// Create an empty host→CPU data source.
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a byte that the CPU can later read from FIFO DATA.
    pub fn write_byte(&mut self, byte: u8) {
        self.host_to_cpu.push_back(byte);
    }

    /// Pop the next byte for CPU consumption.
    fn read_byte(&mut self) -> Option<u8> {
        self.host_to_cpu.pop_front()
    }

    /// Returns whether host→CPU queue is empty.
    fn is_empty(&self) -> bool {
        self.host_to_cpu.is_empty()
    }

    /// Clear all pending host→CPU bytes.
    fn clear(&mut self) {
        self.host_to_cpu.clear();
    }
}

/// Shared host→CPU FIFO data source.
pub type SharedFifoDataSource = Arc<Mutex<FifoDataSource>>;

/// FIFO peripheral for UART-style communication
/// Provides host→CPU data via shared source and CPU→host notification via callback.
///
/// The FIFO operates on individual bytes, allowing both byte-granular and word-granular
/// access patterns. Word reads/writes are decomposed into individual byte operations
/// in little-endian order.
pub struct Fifo {
    /// Data sent FROM Host -> CPU (as u8 bytes)
    host_to_cpu: SharedFifoDataSource,
    on_data_received: FifoDataReceivedCallback,
}

impl Fifo {
    /// Create a new FIFO backed by a shared host→CPU data source.
    pub fn new_with_callback(
        host_to_cpu: SharedFifoDataSource,
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
    fn read_status(&self) -> u32 {
        let rx_valid = if self
            .host_to_cpu
            .lock()
            .expect("Fifo host_to_cpu lock poisoned in read_status")
            .is_empty()
        {
            0
        } else {
            1
        };
        let tx_ready = 1; // Always ready (infinite buffer)
        (tx_ready << 1) | rx_valid
    }

    /// Read the DATA register as a byte
    /// Pops a u8 byte from the RX queue
    /// Returns 0 if RX is empty
    fn read_data_byte(&mut self) -> u8 {
        match self
            .host_to_cpu
            .lock()
            .expect("Fifo host_to_cpu lock poisoned in read_data_byte")
            .read_byte()
        {
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

    /// Write to the DATA register as a byte
    /// Forwards a u8 byte to host callback immediately.
    fn write_data_byte(&mut self, val: u8) {
        (self.on_data_received)(val);
    }
}

impl BusDevice for Fifo {
    fn read_word(&mut self, _ctx: &mut SystemContext, offset: u32) -> Result<u32, BusDeviceError> {
        match offset {
            0x00 => {
                // Read 4 bytes in little-endian order
                let b0 = self.read_data_byte() as u32;
                let b1 = self.read_data_byte() as u32;
                let b2 = self.read_data_byte() as u32;
                let b3 = self.read_data_byte() as u32;
                Ok(b0 | (b1 << 8) | (b2 << 16) | (b3 << 24))
            }
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
                // Write 4 bytes in little-endian order
                self.write_data_byte((value & 0xFF) as u8);
                self.write_data_byte(((value >> 8) & 0xFF) as u8);
                self.write_data_byte(((value >> 16) & 0xFF) as u8);
                self.write_data_byte(((value >> 24) & 0xFF) as u8);
                Ok(())
            }
            0x04 => {
                // STATUS register is read-only
                Err(BusDeviceError::WriteToReadOnly { offset })
            }
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }

    fn read_halfword(
        &mut self,
        _ctx: &mut SystemContext,
        offset: u32,
    ) -> Result<u16, BusDeviceError> {
        match offset {
            0x00 => {
                // Read 2 bytes in little-endian order
                let b0 = self.read_data_byte() as u16;
                let b1 = self.read_data_byte() as u16;
                Ok(b0 | (b1 << 8))
            }
            0x04 => {
                // Only lower 16 bits of status are meaningful
                Ok(self.read_status() as u16)
            }
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }

    fn write_halfword(
        &mut self,
        _ctx: &mut SystemContext,
        offset: u32,
        value: u16,
    ) -> Result<(), BusDeviceError> {
        match offset {
            0x00 => {
                // Write 2 bytes in little-endian order
                self.write_data_byte((value & 0xFF) as u8);
                self.write_data_byte(((value >> 8) & 0xFF) as u8);
                Ok(())
            }
            0x04 => {
                // STATUS register is read-only
                Err(BusDeviceError::WriteToReadOnly { offset })
            }
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }

    fn read_byte(&mut self, _ctx: &mut SystemContext, offset: u32) -> Result<u8, BusDeviceError> {
        match offset {
            0x00 => Ok(self.read_data_byte()),
            0x04 => {
                // Only lower 8 bits of status are meaningful (only 2 bits used)
                Ok(self.read_status() as u8)
            }
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }

    fn write_byte(
        &mut self,
        _ctx: &mut SystemContext,
        offset: u32,
        value: u8,
    ) -> Result<(), BusDeviceError> {
        match offset {
            0x00 => {
                self.write_data_byte(value);
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
        //   - DATA   at offset 0x00 (read/write, byte-granular)
        //   - STATUS at offset 0x04 (read-only)
        //
        // The device reserves a contiguous 8-byte region [0x00..=0x07] on the bus
        // to allow for potential future expansion. Word, halfword, and byte access
        // are supported at offset 0x00 and status can be read at offset 0x04.
        8
    }

    fn name(&self) -> &str {
        "FIFO"
    }

    fn reset(&mut self, _ctx: &mut SystemContext) {
        self.host_to_cpu
            .lock()
            .expect("Fifo host_to_cpu lock poisoned in reset")
            .clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Memory;

    #[test]
    fn test_reset_clears_host_to_cpu_queue() {
        let source = Arc::new(Mutex::new(FifoDataSource::new()));
        let mut fifo = Fifo::new_with_callback(source.clone(), Box::new(|_| {}));
        fifo.write_data_byte(0x11);
        source
            .lock()
            .expect("test source lock poisoned")
            .write_byte(0x22);

        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);
        fifo.reset(&mut ctx);

        assert!(source.lock().expect("test source lock poisoned").is_empty());
    }
}
