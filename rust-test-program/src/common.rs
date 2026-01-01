// Common utilities for bare-metal test programs

use core::alloc::{GlobalAlloc, Layout};
use core::panic::PanicInfo;
use core::ptr::{addr_of_mut, write_volatile};
use core::sync::atomic::{AtomicUsize, Ordering};

/// Simple bump allocator for bare-metal environment.
///
/// This allocator uses a static 8KB heap and AtomicUsize with Ordering::Relaxed,
/// which is safe for this single-threaded bare-metal environment where only one
/// CPU core is active.
///
/// For multi-threaded usage, this would need:
/// 1. Ordering::SeqCst or Ordering::AcqRel for atomic operations
/// 2. Proper synchronization primitives (e.g., Mutex) around heap access
/// 3. Consideration of deallocation (currently a no-op)
pub struct SimpleAllocator;

unsafe impl GlobalAlloc for SimpleAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        static mut HEAP: [u8; 8192] = [0; 8192];
        static OFFSET: AtomicUsize = AtomicUsize::new(0);

        let size = layout.size();
        let align = layout.align();
        let current_offset = OFFSET.load(Ordering::Relaxed);
        let aligned_offset = (current_offset + align - 1) & !(align - 1);

        if aligned_offset + size > 8192 {
            core::ptr::null_mut()
        } else {
            let ptr = addr_of_mut!(HEAP).cast::<u8>().add(aligned_offset);
            OFFSET.store(aligned_offset + size, Ordering::Relaxed);
            ptr
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

/// Default panic handler for bare-metal programs - infinite loop on panic
#[inline(never)]
pub fn default_panic_handler(_info: &PanicInfo) -> ! {
    loop {}
}

/// TOHOST address for signaling halt to the simulator
pub const TOHOST_ADDR: u32 = 0xFFFF_FFF0;

/// Write to tohost to signal halt with the given value
#[inline(never)]
pub fn write_tohost(value: u32) -> ! {
    unsafe {
        write_volatile(TOHOST_ADDR as *mut u32, value);
    }
    loop {}
}
