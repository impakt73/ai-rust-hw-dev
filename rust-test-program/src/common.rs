// Common utilities for bare-metal test programs

#![allow(dead_code)]

use core::alloc::{GlobalAlloc, Layout};
use core::panic::PanicInfo;
use core::ptr::{addr_of_mut, read_volatile, write_volatile};
use core::sync::atomic::{AtomicUsize, Ordering};

/// Simple bump allocator for bare-metal environment.
///
/// This allocator uses a static 8KB heap placed in the .uninit section to avoid
/// startup zero-initialization and AtomicUsize with Ordering::Relaxed, which is
/// safe for this single-threaded bare-metal environment where only one CPU core
/// is active.
///
/// Using the .uninit section eliminates the costly zero-initialization loop in
/// the riscv-rt startup code, significantly reducing cycle count for programs
/// using heap allocation.
///
/// For multi-threaded usage, this would need:
/// 1. Ordering::SeqCst or Ordering::AcqRel for atomic operations
/// 2. Proper synchronization primitives (e.g., Mutex) around heap access
/// 3. Consideration of deallocation (currently a no-op)
pub struct SimpleAllocator;

unsafe impl GlobalAlloc for SimpleAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: Using .uninit section to avoid zero-initialization on startup.
        // The .uninit section is explicitly NOT zeroed by riscv-rt's startup code,
        // unlike .bss which is always zeroed. This significantly reduces startup cycles.
        // The allocated memory is uninitialized, which is fine because:
        // 1. Callers of alloc() must initialize the memory before use
        // 2. This is standard behavior for allocators (malloc doesn't zero either)
        #[link_section = ".uninit"]
        static mut HEAP: [u8; 8192] = [0; 8192];
        static OFFSET: AtomicUsize = AtomicUsize::new(0);

        let size = layout.size();
        let align = layout.align();
        let current_offset = OFFSET.load(Ordering::Relaxed);
        let aligned_offset = (current_offset + align - 1) & !(align - 1);

        if aligned_offset + size > 8192 {
            core::ptr::null_mut()
        } else {
            // SAFETY: We're computing a pointer within the static HEAP allocation.
            // The pointer arithmetic is valid as long as aligned_offset + size <= 8192,
            // which we've already checked above.
            let ptr = addr_of_mut!(HEAP).cast::<u8>().add(aligned_offset);
            OFFSET.store(aligned_offset + size, Ordering::Relaxed);
            ptr
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
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

/// TOHOST address for signaling halt to the simulator
///
/// This register is provided by the SimControl device and is used to signal
/// program termination to the simulator. Writing any value to this address
/// will cause the simulator to halt and capture the written value.
///
/// Note: The tohost register is write-only. Attempting to read from it will
/// result in a bus error.
pub const TOHOST_ADDR: u32 = 0x1000_0000;

/// Standard success code for tests (expected by cpu-sim)
pub const SUCCESS_CODE: u32 = 42;

/// Standard failure code for tests (indicates test logic failure, not panic)
pub const FAILURE_CODE: u32 = 1;

/// Standard panic/failure code (different from success to aid debugging)
pub const PANIC_CODE: u32 = 0xDEAD;

/// Write to tohost to signal halt with the given value
#[inline(never)]
pub fn write_tohost(value: u32) -> ! {
    unsafe {
        write_volatile(TOHOST_ADDR as *mut u32, value);
    }
    loop {}
}

/// FIFO memory-mapped I/O addresses and constants
pub const FIFO_BASE: u32 = 0x4000_0000;
pub const FIFO_DATA: u32 = FIFO_BASE + 0x0;
pub const FIFO_STATUS: u32 = FIFO_BASE + 0x4;
pub const RX_VALID: u32 = 1 << 0;
pub const TX_READY: u32 = 1 << 1;

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

/// Audio test utilities for generating consistent test patterns

/// Generate a sine wave sample at a given index
/// Uses a lookup table approach for consistency between test and test program
pub fn generate_sine_sample(index: u32, frequency_div: u32) -> i16 {
    // Simple sine wave using lookup table approximation
    // We'll use a 32-entry lookup table for a quarter wave
    const QUARTER_WAVE_LEN: u32 = 32;
    const FULL_WAVE_LEN: u32 = QUARTER_WAVE_LEN * 4;
    
    // Normalize index to position in full wave
    let phase = (index / frequency_div) % FULL_WAVE_LEN;
    
    // Quarter wave lookup table (0 to pi/2, scaled to 0-32767)
    const SINE_TABLE: [i16; 32] = [
        0, 1608, 3212, 4808, 6393, 7962, 9512, 11039,
        12539, 14010, 15446, 16846, 18204, 19519, 20787, 22005,
        23170, 24279, 25329, 26319, 27245, 28105, 28898, 29621,
        30273, 30852, 31356, 31785, 32137, 32412, 32609, 32728,
    ];
    
    // Determine which quarter of the wave we're in and compute the value
    if phase < QUARTER_WAVE_LEN {
        // First quarter (0 to π/2): rising, positive
        SINE_TABLE[phase as usize]
    } else if phase < QUARTER_WAVE_LEN * 2 {
        // Second quarter (π/2 to π): falling, positive
        SINE_TABLE[(QUARTER_WAVE_LEN * 2 - 1 - phase) as usize]
    } else if phase < QUARTER_WAVE_LEN * 3 {
        // Third quarter (π to 3π/2): falling, negative
        -SINE_TABLE[(phase - QUARTER_WAVE_LEN * 2) as usize]
    } else {
        // Fourth quarter (3π/2 to 2π): rising, negative
        -SINE_TABLE[(FULL_WAVE_LEN - 1 - phase) as usize]
    }
}
