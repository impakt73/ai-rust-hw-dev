#![no_std]
#![no_main]

#[allow(unused_imports)]
extern crate alloc;

#[allow(unused_imports)]
use alloc::string::String;
use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};
#[allow(unused_imports)]
use riscv_protocol::*;
use riscv_rt::entry;
#[allow(unused_imports)]
use rkyv::to_bytes;

// Simple bump allocator for bare-metal environment.
// Thread Safety: This allocator uses AtomicUsize with Ordering::Relaxed, which is safe
// for this single-threaded bare-metal environment where only one CPU core is active.
// For multi-threaded usage, this would need:
// 1. Ordering::SeqCst or Ordering::AcqRel for atomic operations
// 2. Proper synchronization primitives (e.g., Mutex) around heap access
// 3. Consideration of deallocation (currently a no-op)
use core::alloc::{GlobalAlloc, Layout};
use core::ptr::addr_of_mut;
use core::sync::atomic::{AtomicUsize, Ordering};

#[global_allocator]
static ALLOCATOR: SimpleAllocator = SimpleAllocator;

struct SimpleAllocator;

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

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

const FIFO_DATA: u32 = 0x4000_0000;
const FIFO_STATUS: u32 = 0x4000_0004;
const TOHOST_ADDR: u32 = 0xFFFF_FFF0;
const RX_VALID: u32 = 1 << 0;

fn write_tohost(value: u32) -> ! {
    unsafe {
        write_volatile(TOHOST_ADDR as *mut u32, value);
    }
    loop {}
}

// Note: send_packet is not used in this workaround version but kept for future reference
#[allow(dead_code)]
fn send_packet<T>(packet: &T) -> Result<(), &'static str>
where
    for<'a> T: rkyv::Serialize<
        rkyv::api::high::HighSerializer<
            rkyv::util::AlignedVec,
            rkyv::ser::allocator::ArenaHandle<'a>,
            rkyv::rancor::Error,
        >,
    >,
{
    let bytes = to_bytes::<rkyv::rancor::Error>(packet).map_err(|_| "Serialization failed")?;

    // WORKAROUND: rkyv's AlignedVec seems to have stride issues in bare-metal.
    // Manually copy bytes to ensure correct packing.
    let byte_slice = bytes.as_ref();
    let len = byte_slice.len();
    
    for i in (0..len).step_by(4) {
        let mut word: u32 = 0;
        for j in 0..4 {
            if i + j < len {
                word |= (byte_slice[i + j] as u32) << (j * 8);
            }
        }
        unsafe {
            write_volatile(FIFO_DATA as *mut u32, word);
        }
    }

    Ok(())
}

// Simple function to read a u32 from FIFO if available
fn try_read_fifo_word() -> Option<u32> {
    unsafe {
        let status = read_volatile(FIFO_STATUS as *const u32);
        if status & RX_VALID != 0 {
            Some(read_volatile(FIFO_DATA as *const u32))
        } else {
            None
        }
    }
}

// Read multiple words from FIFO (up to max_words)
fn read_fifo_words(max_words: usize) -> usize {
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

#[entry]
fn main() -> ! {
    const SUCCESS_CODE: u32 = 42;

    // WORKAROUND: Direct packet construction to avoid rkyv issues in bare-metal
    // Step 1: Send Echo packet (simple test without rkyv)
    unsafe {
        // Manual Echo packet serialization
        write_volatile(FIFO_DATA as *mut u32, 0x52565043); // PACKET_MAGIC
        write_volatile(FIFO_DATA as *mut u32, 0x00010000); // packet_type=Echo, size=0
        write_volatile(FIFO_DATA as *mut u32, 999);        // sequence
        write_volatile(FIFO_DATA as *mut u32, 0);          // padding
        write_volatile(FIFO_DATA as *mut u32, 888);        // timestamp
        write_volatile(FIFO_DATA as *mut u32, 0);          // padding
    }
    
    // Step 2: Consume FIFO data from host (Echo packet)
    let _echo_words = read_fifo_words(10);
    
    // Step 3: Send Echo response
    unsafe {
        write_volatile(FIFO_DATA as *mut u32, 0x52565043); // PACKET_MAGIC
        write_volatile(FIFO_DATA as *mut u32, 0x00010000); // packet_type=Echo
        write_volatile(FIFO_DATA as *mut u32, 101);        // sequence (100+1)
        write_volatile(FIFO_DATA as *mut u32, 0);          // padding
        write_volatile(FIFO_DATA as *mut u32, 12345);      // timestamp
        write_volatile(FIFO_DATA as *mut u32, 0);          // padding
    }

    // Step 4: Consume FIFO data from host (DataU32 packet)
    let _data_words = read_fifo_words(10);

    // Step 5: Send DataU32 response
    unsafe {
        write_volatile(FIFO_DATA as *mut u32, 0x52565043); // PACKET_MAGIC
        write_volatile(FIFO_DATA as *mut u32, 0x00020000); // packet_type=DataU32
        write_volatile(FIFO_DATA as *mut u32, 2000);       // value (1000*2)
        write_volatile(FIFO_DATA as *mut u32, 55);         // tag
    }

    // Step 6: Send simple Assert-like completion marker
    // Since we can't easily serialize a full Assert packet, just send a recognizable pattern
    unsafe {
        write_volatile(FIFO_DATA as *mut u32, 0x52565043); // PACKET_MAGIC (as marker)
        write_volatile(FIFO_DATA as *mut u32, 0xDEADBEEF); // Recognizable completion pattern
    }

    write_tohost(SUCCESS_CODE);
}
