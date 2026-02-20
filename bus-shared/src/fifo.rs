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

    /// Clear all pending host→CPU bytes.
    fn clear(&mut self) {
        self.host_to_cpu.clear();
    }

    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.host_to_cpu.is_empty()
    }
}

/// Shared host→CPU FIFO data source.
pub type SharedFifoDataSource = Arc<Mutex<FifoDataSource>>;

/// FIFO peripheral for UART-style communication
/// Provides host→CPU data via shared source and CPU→host notification via callback.
///
/// The FIFO operates on individual bytes only.
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
    /// Bit 1 (TX_READY): Always 1 (CPU writes are consumed immediately)
    fn read_status(&self) -> u32 {
        let source = self
            .host_to_cpu
            .lock()
            .expect("Fifo host_to_cpu lock poisoned in read_status");
        let queue_len = source.host_to_cpu.len();
        let rx_valid = u32::from(queue_len > 0);
        let tx_ready = 1;
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
            0x00 => Err(BusDeviceError::UnsupportedSize { offset, size: 4 }),
            0x04 => Ok(self.read_status()),
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }

    fn write_word(
        &mut self,
        _ctx: &mut SystemContext,
        offset: u32,
        _value: u32,
    ) -> Result<(), BusDeviceError> {
        match offset {
            0x00 => Err(BusDeviceError::UnsupportedSize { offset, size: 4 }),
            0x04 => Err(BusDeviceError::WriteToReadOnly { offset }),
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }

    fn read_halfword(
        &mut self,
        _ctx: &mut SystemContext,
        offset: u32,
    ) -> Result<u16, BusDeviceError> {
        Err(BusDeviceError::UnsupportedSize { offset, size: 2 })
    }

    fn write_halfword(
        &mut self,
        _ctx: &mut SystemContext,
        offset: u32,
        _value: u16,
    ) -> Result<(), BusDeviceError> {
        Err(BusDeviceError::UnsupportedSize { offset, size: 2 })
    }

    fn read_byte(&mut self, _ctx: &mut SystemContext, offset: u32) -> Result<u8, BusDeviceError> {
        match offset {
            0x00 => Ok(self.read_data_byte()),
            0x04 => Err(BusDeviceError::UnsupportedSize { offset, size: 1 }),
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
            0x04 => Err(BusDeviceError::UnsupportedSize { offset, size: 1 }),
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }

    fn size(&self) -> u32 {
        // FIFO has 2 word-aligned registers within its address window:
        //   - DATA   at offset 0x00 (byte read/write only)
        //   - STATUS at offset 0x04 (word read-only)
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

    #[test]
    fn test_access_size_rules() {
        let source = Arc::new(Mutex::new(FifoDataSource::new()));
        let mut fifo = Fifo::new_with_callback(source, Box::new(|_| {}));
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        assert_eq!(fifo.read_word(&mut ctx, 0x04).unwrap(), 0b10);

        assert!(matches!(
            fifo.read_word(&mut ctx, 0x00),
            Err(BusDeviceError::UnsupportedSize { size: 4, .. })
        ));
        assert!(matches!(
            fifo.write_word(&mut ctx, 0x00, 0x1234_5678),
            Err(BusDeviceError::UnsupportedSize { size: 4, .. })
        ));
        assert!(matches!(
            fifo.write_word(&mut ctx, 0x04, 0x1234_5678),
            Err(BusDeviceError::WriteToReadOnly { .. })
        ));
        assert!(matches!(
            fifo.read_halfword(&mut ctx, 0x00),
            Err(BusDeviceError::UnsupportedSize { size: 2, .. })
        ));
        assert!(matches!(
            fifo.read_halfword(&mut ctx, 0x04),
            Err(BusDeviceError::UnsupportedSize { size: 2, .. })
        ));
        assert!(matches!(
            fifo.write_halfword(&mut ctx, 0x00, 0x1234),
            Err(BusDeviceError::UnsupportedSize { size: 2, .. })
        ));
        assert!(matches!(
            fifo.read_byte(&mut ctx, 0x04),
            Err(BusDeviceError::UnsupportedSize { size: 1, .. })
        ));
        assert!(matches!(
            fifo.write_byte(&mut ctx, 0x04, 0x12),
            Err(BusDeviceError::UnsupportedSize { size: 1, .. })
        ));
    }
}
