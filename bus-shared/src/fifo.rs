use crate::bus_device::{BusDevice, BusDeviceError, SystemContext};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

const FIFO_MAX_BUFFER_SIZE: usize = 4096;

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
        if self.host_to_cpu.len() >= FIFO_MAX_BUFFER_SIZE {
            log::warn!(
                "FIFO RX queue full ({} bytes), dropping byte 0x{:02x}",
                FIFO_MAX_BUFFER_SIZE,
                byte
            );
            return;
        }
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
    /// Bit 1 (TX_READY): 1 while queue is below capacity
    fn read_status(&self) -> u32 {
        let queue_len = self
            .host_to_cpu
            .lock()
            .expect("Fifo host_to_cpu lock poisoned in read_status")
            .host_to_cpu
            .len();
        let rx_valid = if queue_len == 0 { 0 } else { 1 };
        let tx_ready = if queue_len < FIFO_MAX_BUFFER_SIZE {
            1
        } else {
            0
        };
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
        Err(BusDeviceError::UnsupportedSize { offset, size: 4 })
    }

    fn write_word(
        &mut self,
        _ctx: &mut SystemContext,
        offset: u32,
        _value: u32,
    ) -> Result<(), BusDeviceError> {
        Err(BusDeviceError::UnsupportedSize { offset, size: 4 })
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
            0x04 => Ok(self.read_status() as u8),
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
        //   - DATA   at offset 0x00 (byte read/write only)
        //   - STATUS at offset 0x04 (byte read-only)
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

        assert!(source
            .lock()
            .expect("test source lock poisoned")
            .host_to_cpu
            .is_empty());
    }

    #[test]
    fn test_data_source_is_bounded() {
        let mut source = FifoDataSource::new();
        for _ in 0..(FIFO_MAX_BUFFER_SIZE + 16) {
            source.write_byte(0xAA);
        }
        assert_eq!(source.host_to_cpu.len(), FIFO_MAX_BUFFER_SIZE);
    }

    #[test]
    fn test_word_and_halfword_access_are_unsupported() {
        let source = Arc::new(Mutex::new(FifoDataSource::new()));
        let mut fifo = Fifo::new_with_callback(source, Box::new(|_| {}));
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        assert!(matches!(
            fifo.read_word(&mut ctx, 0x00),
            Err(BusDeviceError::UnsupportedSize { size: 4, .. })
        ));
        assert!(matches!(
            fifo.write_word(&mut ctx, 0x00, 0x1234_5678),
            Err(BusDeviceError::UnsupportedSize { size: 4, .. })
        ));
        assert!(matches!(
            fifo.read_halfword(&mut ctx, 0x00),
            Err(BusDeviceError::UnsupportedSize { size: 2, .. })
        ));
        assert!(matches!(
            fifo.write_halfword(&mut ctx, 0x00, 0x1234),
            Err(BusDeviceError::UnsupportedSize { size: 2, .. })
        ));
    }
}
