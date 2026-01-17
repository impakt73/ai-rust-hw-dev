use crate::bus_device::{BusDevice, BusDeviceError, SystemContext};

/// Simple DMA controller for memory-to-memory transfers
///
/// This device copies data from a source address to a destination address
/// one byte per clock cycle, providing a realistic approximation of
/// hardware DMA behavior.
///
/// Register Map (all word-aligned):
/// - 0x00: SRC_ADDR    - Source address (read/write)
/// - 0x04: DST_ADDR    - Destination address (read/write)
/// - 0x08: SIZE        - Transfer size in bytes (read/write)
/// - 0x0C: STATUS      - Status register (read-only)
///   Bit 0: BUSY (1 = transfer in progress, 0 = idle)
/// - 0x10: DISPATCH    - Dispatch register (write-only)
///   Writing any value starts the transfer
pub struct Dma {
    /// Source address for DMA transfer
    src_addr: u32,
    /// Destination address for DMA transfer
    dst_addr: u32,
    /// Total size of transfer in bytes
    size: u32,
    /// Number of bytes remaining to transfer (0 = idle)
    bytes_remaining: u32,
    /// Current source address (incremented during transfer)
    current_src: u32,
    /// Current destination address (incremented during transfer)
    current_dst: u32,
}

impl Dma {
    /// Create a new DMA controller
    pub fn new() -> Self {
        Dma {
            src_addr: 0,
            dst_addr: 0,
            size: 0,
            bytes_remaining: 0,
            current_src: 0,
            current_dst: 0,
        }
    }

    /// Check if a transfer is currently in progress
    fn is_busy(&self) -> bool {
        self.bytes_remaining > 0
    }

    /// Start a DMA transfer using the configured registers
    fn start_transfer(&mut self) {
        if self.is_busy() {
            log::warn!("DMA: Dispatch attempted while transfer already in progress");
            return;
        }

        if self.size == 0 {
            log::warn!("DMA: Dispatch attempted with size = 0");
            return;
        }

        log::debug!(
            "DMA: Starting transfer from 0x{:08x} to 0x{:08x}, size = {} bytes",
            self.src_addr,
            self.dst_addr,
            self.size
        );

        self.bytes_remaining = self.size;
        self.current_src = self.src_addr;
        self.current_dst = self.dst_addr;
    }

    /// Transfer one byte (called each clock cycle)
    fn transfer_one_byte(&mut self, ctx: &mut SystemContext) {
        if !self.is_busy() {
            return;
        }

        // Read one byte from source
        let byte = ctx.read_byte(self.current_src);

        // Write one byte to destination
        ctx.write_byte(self.current_dst, byte);

        // Increment addresses
        self.current_src = self.current_src.wrapping_add(1);
        self.current_dst = self.current_dst.wrapping_add(1);

        // Decrement remaining bytes
        self.bytes_remaining -= 1;

        if self.bytes_remaining == 0 {
            log::debug!("DMA: Transfer complete");
        }
    }
}

impl Default for Dma {
    fn default() -> Self {
        Self::new()
    }
}

impl BusDevice for Dma {
    fn read_word(&mut self, _ctx: &mut SystemContext, offset: u32) -> Result<u32, BusDeviceError> {
        match offset {
            0x00 => Ok(self.src_addr),
            0x04 => Ok(self.dst_addr),
            0x08 => Ok(self.size),
            0x0C => {
                // STATUS register: bit 0 = BUSY
                Ok(if self.is_busy() { 1 } else { 0 })
            }
            0x10 => {
                // DISPATCH register is write-only
                Err(BusDeviceError::ReadFromWriteOnly { offset })
            }
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
                self.src_addr = value;
                Ok(())
            }
            0x04 => {
                self.dst_addr = value;
                Ok(())
            }
            0x08 => {
                self.size = value;
                Ok(())
            }
            0x0C => {
                // STATUS register is read-only
                Err(BusDeviceError::WriteToReadOnly { offset })
            }
            0x10 => {
                // DISPATCH register - writing any value starts the transfer
                self.start_transfer();
                Ok(())
            }
            _ => Err(BusDeviceError::InvalidAddress { offset }),
        }
    }

    fn size(&self) -> u32 {
        // 5 registers × 4 bytes each = 20 bytes
        20
    }

    fn name(&self) -> &str {
        "DMA"
    }

    fn reset(&mut self, _ctx: &mut SystemContext) {
        self.src_addr = 0;
        self.dst_addr = 0;
        self.size = 0;
        self.bytes_remaining = 0;
        self.current_src = 0;
        self.current_dst = 0;
    }

    fn clock_cycle(&mut self, ctx: &mut SystemContext) {
        // Transfer one byte per clock cycle if a transfer is in progress
        self.transfer_one_byte(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::Memory;

    #[test]
    fn test_dma_register_access() {
        let mut dma = Dma::new();
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        // Write to registers
        dma.write_word(&mut ctx, 0x00, 0x8000_1000).unwrap();
        dma.write_word(&mut ctx, 0x04, 0x8000_2000).unwrap();
        dma.write_word(&mut ctx, 0x08, 256).unwrap();

        // Read back registers
        assert_eq!(dma.read_word(&mut ctx, 0x00).unwrap(), 0x8000_1000);
        assert_eq!(dma.read_word(&mut ctx, 0x04).unwrap(), 0x8000_2000);
        assert_eq!(dma.read_word(&mut ctx, 0x08).unwrap(), 256);

        // Status should be idle (0)
        assert_eq!(dma.read_word(&mut ctx, 0x0C).unwrap(), 0);
    }

    #[test]
    fn test_dma_status_register_read_only() {
        let mut dma = Dma::new();
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        let result = dma.write_word(&mut ctx, 0x0C, 1);
        assert!(matches!(
            result,
            Err(BusDeviceError::WriteToReadOnly { offset: 0x0C })
        ));
    }

    #[test]
    fn test_dma_dispatch_register_write_only() {
        let mut dma = Dma::new();
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        let result = dma.read_word(&mut ctx, 0x10);
        assert!(matches!(
            result,
            Err(BusDeviceError::ReadFromWriteOnly { offset: 0x10 })
        ));
    }

    #[test]
    fn test_dma_transfer() {
        let mut dma = Dma::new();
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        // Set up source data in memory
        let src_addr = 0x8000_1000;
        let dst_addr = 0x8000_2000;
        let test_data = [0x12u8, 0x34, 0x56, 0x78];

        for (i, &byte) in test_data.iter().enumerate() {
            ctx.write_byte(src_addr + i as u32, byte);
        }

        // Configure DMA
        dma.write_word(&mut ctx, 0x00, src_addr).unwrap();
        dma.write_word(&mut ctx, 0x04, dst_addr).unwrap();
        dma.write_word(&mut ctx, 0x08, test_data.len() as u32)
            .unwrap();

        // Dispatch transfer
        dma.write_word(&mut ctx, 0x10, 1).unwrap();

        // Check status is busy
        assert_eq!(dma.read_word(&mut ctx, 0x0C).unwrap(), 1);

        // Run clock cycles to transfer all bytes
        for _ in 0..test_data.len() {
            dma.clock_cycle(&mut ctx);
        }

        // Check status is idle
        assert_eq!(dma.read_word(&mut ctx, 0x0C).unwrap(), 0);

        // Verify destination data
        for (i, &expected) in test_data.iter().enumerate() {
            let actual = ctx.read_byte(dst_addr + i as u32);
            assert_eq!(actual, expected, "Mismatch at byte {}", i);
        }
    }

    #[test]
    fn test_dma_zero_size() {
        let mut dma = Dma::new();
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        // Configure DMA with zero size
        dma.write_word(&mut ctx, 0x00, 0x8000_1000).unwrap();
        dma.write_word(&mut ctx, 0x04, 0x8000_2000).unwrap();
        dma.write_word(&mut ctx, 0x08, 0).unwrap();

        // Dispatch transfer (should be no-op)
        dma.write_word(&mut ctx, 0x10, 1).unwrap();

        // Status should still be idle
        assert_eq!(dma.read_word(&mut ctx, 0x0C).unwrap(), 0);
    }

    #[test]
    fn test_dma_reset() {
        let mut dma = Dma::new();
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        // Configure DMA
        dma.write_word(&mut ctx, 0x00, 0x8000_1000).unwrap();
        dma.write_word(&mut ctx, 0x04, 0x8000_2000).unwrap();
        dma.write_word(&mut ctx, 0x08, 256).unwrap();

        // Reset
        dma.reset(&mut ctx);

        // All registers should be zero
        assert_eq!(dma.read_word(&mut ctx, 0x00).unwrap(), 0);
        assert_eq!(dma.read_word(&mut ctx, 0x04).unwrap(), 0);
        assert_eq!(dma.read_word(&mut ctx, 0x08).unwrap(), 0);
        assert_eq!(dma.read_word(&mut ctx, 0x0C).unwrap(), 0);
    }
}
