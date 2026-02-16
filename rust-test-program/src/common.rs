// Common utilities for bare-metal test programs

#![allow(dead_code)]

use core::panic::PanicInfo;
use core::ptr::{addr_of, addr_of_mut, read_volatile, write_volatile};
pub use embedded_alloc::LlffHeap as Heap;

// Re-export constants from riscv_shared
// Note: Some re-exports may be unused in this module but are used by test programs that import from common
#[allow(unused_imports)]
pub use riscv_shared::{
    FAILURE_CODE, FIFO_DATA, FIFO_STATUS, PANIC_CODE, RX_VALID, SUCCESS_CODE, TOHOST_ADDR, TX_READY,
};

// Re-export helper functions from riscv_shared
#[allow(unused_imports)]
pub use riscv_shared::{
    generate_sine_sample, is_dma_ready, is_sample_buffer_ready, trigger_dma, trigger_present,
    wait_for_frame_ready, wait_for_present_ready, write_mono_sample, write_pixel, write_pixel_r8,
    write_pixel_rgb565, write_pixel_rgb8, write_pixel_rgba8, write_stereo_sample, DMA_READY,
    FRAME_READY, PRESENT_READY, SAMPLE_BUFFER_READY,
};

// Re-export video and audio format types for backward compatibility
#[allow(unused_imports)]
pub use riscv_shared::VideoFormat;
#[allow(unused_imports)]
pub use riscv_shared::{AudioChannels, AudioConfig, AudioSampleRate};

/// Initialize a provided global heap from linker-provided riscv-rt heap symbols.
pub fn init_heap(heap: &'static Heap) {
    unsafe extern "C" {
        static mut __sheap: u8;
        static _heap_size: u8;
    }

    let heap_start = addr_of_mut!(__sheap) as usize;
    let heap_size = addr_of!(_heap_size) as usize;

    unsafe {
        heap.init(heap_start, heap_size);
    }
}

/// Default panic handler for bare-metal programs - write to tohost to signal panic
///
/// When a panic occurs, we write a special value (PANIC_CODE = 0xDEAD) to tohost to signal
/// that the program panicked. This allows the simulator to detect panics and
/// report them properly instead of timing out in an infinite loop.
#[inline(never)]
pub fn default_panic_handler(_info: &PanicInfo) -> ! {
    // Write a special panic value to tohost (0xDEAD = 57005)
    // This is different from the success value (0x2a = 42) so the simulator
    // can distinguish between normal completion and panic
    write_tohost(PANIC_CODE)
}

/// Write to tohost to signal halt with the given value
#[inline(never)]
pub fn write_tohost(value: u32) -> ! {
    unsafe {
        write_volatile(TOHOST_ADDR as *mut u32, value);
        core::arch::asm!("ebreak");
    }
    #[allow(clippy::empty_loop)]
    loop {}
}

/// FIFO read error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FifoReadError {
    /// Attempted to read from an empty FIFO
    Empty,
}

/// FIFO write error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FifoWriteError {
    /// Attempted to write to a full FIFO
    Full,
}

/// Write a word to the FIFO, checking TX_READY status first
///
/// # Errors
///
/// Returns `FifoWriteError::Full` if the TX FIFO is not ready to accept data
#[inline(never)]
pub fn fifo_write_word(word: u32) -> Result<(), FifoWriteError> {
    unsafe {
        let status = read_volatile(FIFO_STATUS as *const u32);
        if status & TX_READY != 0 {
            write_volatile(FIFO_DATA as *mut u32, word);
            Ok(())
        } else {
            Err(FifoWriteError::Full)
        }
    }
}

/// Read a word from the FIFO, checking RX_VALID status first
///
/// # Errors
///
/// Returns `FifoReadError::Empty` if the RX FIFO has no data available
#[inline(never)]
pub fn fifo_read_word() -> Result<u32, FifoReadError> {
    unsafe {
        let status = read_volatile(FIFO_STATUS as *const u32);
        if status & RX_VALID != 0 {
            Ok(read_volatile(FIFO_DATA as *const u32))
        } else {
            Err(FifoReadError::Empty)
        }
    }
}

/// Read multiple words from FIFO (up to max_words)
/// Returns the number of words successfully read
pub fn read_fifo_words(max_words: usize) -> usize {
    let mut count = 0;
    while count < max_words {
        if fifo_read_word().is_ok() {
            count += 1;
        } else {
            break;
        }
    }
    count
}
