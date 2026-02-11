use crate::bus_device::{BusDevice, BusDeviceError, SystemContext};

/// Latched state for an active DMA transfer.
/// This is a snapshot of the configuration registers when dispatch is triggered,
/// ensuring that modifications to config registers during transfer don't corrupt
/// the active transfer.
#[derive(Debug, Clone, Copy)]
struct ActiveTransfer {
    /// Number of bytes remaining to transfer
    bytes_remaining: u32,
    /// Current source address (incremented during transfer)
    current_src: u32,
    /// Current destination address (incremented during transfer)
    current_dst: u32,
}

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
///
/// The DMA controller latches the configuration registers (SRC_ADDR, DST_ADDR, SIZE)
/// when a transfer is dispatched. This means that modifying these registers during
/// an active transfer will not affect the current transfer, but will take effect
/// on the next dispatch. Only one transfer can be active at a time.
pub struct Dma {
    /// Source address configuration register (read/write)
    src_addr: u32,
    /// Destination address configuration register (read/write)
    dst_addr: u32,
    /// Transfer size configuration register in bytes (read/write)
    size: u32,
    /// Active transfer state (None = idle, Some = transfer in progress)
    /// This is latched from the configuration registers on dispatch
    active_transfer: Option<ActiveTransfer>,
}

impl Dma {
    /// Create a new DMA controller
    pub fn new() -> Self {
        Dma {
            src_addr: 0,
            dst_addr: 0,
            size: 0,
            active_transfer: None,
        }
    }

    /// Check if a transfer is currently in progress
    fn is_busy(&self) -> bool {
        self.active_transfer.is_some()
    }

    /// Start a DMA transfer using the configured registers
    /// This latches the current configuration into active_transfer state
    fn start_transfer(&mut self) {
        if self.is_busy() {
            log::warn!("DMA: Dispatch attempted while transfer already in progress - ignoring");
            return;
        }

        if self.size == 0 {
            log::warn!("DMA: Dispatch attempted with size = 0 - ignoring");
            return;
        }

        log::debug!(
            "DMA: Starting transfer from 0x{:08x} to 0x{:08x}, size = {} bytes",
            self.src_addr,
            self.dst_addr,
            self.size
        );

        // Latch configuration registers into active transfer state
        // This ensures modifications to config registers during transfer don't corrupt it
        self.active_transfer = Some(ActiveTransfer {
            bytes_remaining: self.size,
            current_src: self.src_addr,
            current_dst: self.dst_addr,
        });
    }

    /// Transfer one byte (called each clock cycle)
    /// Only uses the latched active_transfer state, not the config registers
    fn transfer_one_byte(&mut self, ctx: &mut SystemContext) {
        let transfer = match self.active_transfer.as_mut() {
            Some(t) => t,
            None => return, // No active transfer
        };

        // Read one byte from source
        let byte = ctx.read_byte(transfer.current_src);

        // Write one byte to destination
        ctx.write_byte(transfer.current_dst, byte);

        // Increment addresses
        transfer.current_src = transfer.current_src.wrapping_add(1);
        transfer.current_dst = transfer.current_dst.wrapping_add(1);

        // Decrement remaining bytes
        transfer.bytes_remaining -= 1;

        if transfer.bytes_remaining == 0 {
            log::debug!("DMA: Transfer complete");
            self.active_transfer = None;
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
        self.active_transfer = None;
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

    #[test]
    fn test_dma_register_modification_during_transfer() {
        let mut dma = Dma::new();
        let mut memory = Memory::new();
        let mut ctx = SystemContext::new(&mut memory);

        // Set up source data in memory
        let src_addr = 0x8000_1000;
        let dst_addr = 0x8000_2000;
        let test_data = [0xAAu8, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x11, 0x22];

        for (i, &byte) in test_data.iter().enumerate() {
            ctx.write_byte(src_addr + i as u32, byte);
        }

        // Configure DMA for initial transfer
        dma.write_word(&mut ctx, 0x00, src_addr).unwrap();
        dma.write_word(&mut ctx, 0x04, dst_addr).unwrap();
        dma.write_word(&mut ctx, 0x08, test_data.len() as u32)
            .unwrap();

        // Dispatch transfer
        dma.write_word(&mut ctx, 0x10, 1).unwrap();

        // Check status is busy
        assert_eq!(dma.read_word(&mut ctx, 0x0C).unwrap(), 1);

        // Transfer 2 bytes
        for _ in 0..2 {
            dma.clock_cycle(&mut ctx);
        }

        // Modify configuration registers during transfer
        // This should NOT affect the active transfer
        let new_src = 0x8000_3000;
        let new_dst = 0x8000_4000;
        let new_size = 4;
        dma.write_word(&mut ctx, 0x00, new_src).unwrap();
        dma.write_word(&mut ctx, 0x04, new_dst).unwrap();
        dma.write_word(&mut ctx, 0x08, new_size).unwrap();

        // Verify config registers were modified
        assert_eq!(dma.read_word(&mut ctx, 0x00).unwrap(), new_src);
        assert_eq!(dma.read_word(&mut ctx, 0x04).unwrap(), new_dst);
        assert_eq!(dma.read_word(&mut ctx, 0x08).unwrap(), new_size);

        // Still busy - transfer should continue
        assert_eq!(dma.read_word(&mut ctx, 0x0C).unwrap(), 1);

        // Complete the original transfer (6 more bytes)
        for _ in 0..6 {
            dma.clock_cycle(&mut ctx);
        }

        // Check status is now idle
        assert_eq!(dma.read_word(&mut ctx, 0x0C).unwrap(), 0);

        // Verify destination data matches the ORIGINAL transfer parameters
        // despite the config registers being modified mid-transfer
        for (i, &expected) in test_data.iter().enumerate() {
            let actual = ctx.read_byte(dst_addr + i as u32);
            assert_eq!(
                actual, expected,
                "Mismatch at byte {} - transfer should have used original src/dst despite config modification",
                i
            );
        }

        // Verify that the NEW config is still set and can be used for next transfer
        assert_eq!(dma.read_word(&mut ctx, 0x00).unwrap(), new_src);
        assert_eq!(dma.read_word(&mut ctx, 0x04).unwrap(), new_dst);
        assert_eq!(dma.read_word(&mut ctx, 0x08).unwrap(), new_size);
    }

    #[test]
    fn test_dma_multiple_dispatch_rejected() {
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

        // Dispatch first transfer
        dma.write_word(&mut ctx, 0x10, 1).unwrap();
        assert_eq!(dma.read_word(&mut ctx, 0x0C).unwrap(), 1); // busy

        // Attempt to dispatch second transfer (should be rejected)
        dma.write_word(&mut ctx, 0x00, 0x8000_5000).unwrap();
        dma.write_word(&mut ctx, 0x04, 0x8000_6000).unwrap();
        dma.write_word(&mut ctx, 0x10, 1).unwrap(); // This should be ignored

        // Should still be busy with first transfer
        assert_eq!(dma.read_word(&mut ctx, 0x0C).unwrap(), 1);

        // Complete the first transfer
        for _ in 0..test_data.len() {
            dma.clock_cycle(&mut ctx);
        }

        // Should be idle now
        assert_eq!(dma.read_word(&mut ctx, 0x0C).unwrap(), 0);

        // Verify the first transfer completed correctly
        for (i, &expected) in test_data.iter().enumerate() {
            let actual = ctx.read_byte(dst_addr + i as u32);
            assert_eq!(actual, expected);
        }
    }
}
