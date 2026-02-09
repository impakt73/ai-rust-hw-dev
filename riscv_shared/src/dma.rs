//! DMA device register offsets, constants, and helper functions
//!
//! This module defines the DMA (Direct Memory Access) device interface for
//! hardware-accelerated memory-to-memory transfers.

use core::ptr::{read_volatile, write_volatile};

/// DMA device base address
/// Note: This uses the same base as Video device (0x4000_1000)
/// Programs using DMA should ensure it doesn't conflict with Video device usage
pub const DMA_BASE: u32 = 0x4000_1000;

/// DMA source address register offset
pub const DMA_SRC_ADDR: u32 = DMA_BASE;

/// DMA destination address register offset
pub const DMA_DST_ADDR: u32 = DMA_BASE + 0x04;

/// DMA transfer size register offset (in bytes)
pub const DMA_SIZE: u32 = DMA_BASE + 0x08;

/// DMA status register offset
pub const DMA_STATUS: u32 = DMA_BASE + 0x0C;

/// DMA dispatch/trigger register offset
pub const DMA_DISPATCH: u32 = DMA_BASE + 0x10;

/// DMA status bit flags
pub const DMA_STATUS_BUSY: u32 = 1 << 0;

/// Check if DMA controller is busy
///
/// Returns `true` if a DMA transfer is currently in progress.
///
/// # Safety
/// This function performs volatile MMIO reads and should only be called in a
/// bare-metal environment where the DMA_STATUS register is properly mapped.
pub fn is_dma_busy() -> bool {
    unsafe { (read_volatile(DMA_STATUS as *const u32) & DMA_STATUS_BUSY) != 0 }
}

/// Configure and initiate a DMA transfer
///
/// This function configures the DMA controller with source address, destination
/// address, and transfer size, then triggers the transfer.
///
/// # Arguments
/// * `src_addr` - Source memory address (must be in valid memory range)
/// * `dst_addr` - Destination memory address (must be in valid memory range)
/// * `size` - Number of bytes to transfer
///
/// # Safety
/// This function performs volatile MMIO writes and should only be called in a
/// bare-metal environment where DMA registers are properly mapped.
/// The caller must ensure:
/// - `src_addr` points to valid, readable memory
/// - `dst_addr` points to valid, writable memory
/// - Both address ranges don't overflow and are within valid memory regions
/// - The DMA controller is not already busy with another transfer
pub fn start_transfer(src_addr: u32, dst_addr: u32, size: u32) {
    unsafe {
        write_volatile(DMA_SRC_ADDR as *mut u32, src_addr);
        write_volatile(DMA_DST_ADDR as *mut u32, dst_addr);
        write_volatile(DMA_SIZE as *mut u32, size);
        write_volatile(DMA_DISPATCH as *mut u32, 1);
    }
}

/// Wait for the current DMA transfer to complete
///
/// This function polls the DMA_STATUS register until the BUSY bit is cleared,
/// indicating that the transfer has completed.
///
/// # Safety
/// This function performs volatile MMIO reads and should only be called in a
/// bare-metal environment where the DMA_STATUS register is properly mapped.
pub fn wait_for_completion() {
    while is_dma_busy() {
        // Spin wait for DMA to complete
    }
}
