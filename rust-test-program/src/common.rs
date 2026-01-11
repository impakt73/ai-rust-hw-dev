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
pub const TOHOST_ADDR: u32 = 0xFFFF_FFF0;

/// Standard success code for tests (expected by cpu-sim)
pub const SUCCESS_CODE: u32 = 42;

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

/// Write a word to the FIFO
#[inline(never)]
pub fn fifo_write_word(word: u32) {
    unsafe {
        // TX is always ready in simulation, so just write
        write_volatile(FIFO_DATA as *mut u32, word);
    }
}

/// Read a word from the FIFO (without status check - just read and return)
#[inline(never)]
pub fn fifo_read_word_unchecked() -> u32 {
    unsafe { read_volatile(FIFO_DATA as *const u32) }
}

/// Simple function to read a u32 from FIFO if available
pub fn try_read_fifo_word() -> Option<u32> {
    unsafe {
        let status = read_volatile(FIFO_STATUS as *const u32);
        if status & RX_VALID != 0 {
            Some(read_volatile(FIFO_DATA as *const u32))
        } else {
            None
        }
    }
}

/// Read multiple words from FIFO (up to max_words)
pub fn read_fifo_words(max_words: usize) -> usize {
    let mut count = 0;
    while count < max_words {
        if try_read_fifo_word().is_some() {
            count += 1;
        } else {
            break;
        }
    }
    count
}
