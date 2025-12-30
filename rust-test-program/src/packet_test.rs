#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use core::panic::PanicInfo;
use core::ptr::write_volatile;
use riscv_protocol::*;
use riscv_rt::entry;
use rkyv::to_bytes;

// Simple bump allocator
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
const TOHOST_ADDR: u32 = 0xFFFF_FFF0;

fn write_tohost(value: u32) -> ! {
    unsafe {
        write_volatile(TOHOST_ADDR as *mut u32, value);
    }
    loop {}
}

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

    for chunk in bytes.as_ref().chunks(4) {
        let mut word: u32 = 0;
        for (i, &byte) in chunk.iter().enumerate() {
            word |= (byte as u32) << (i * 8);
        }
        unsafe {
            write_volatile(FIFO_DATA as *mut u32, word);
        }
    }

    Ok(())
}

#[entry]
fn main() -> ! {
    const SUCCESS_CODE: u32 = 42;
    const FAILURE_CODE: u32 = 1;
    const DEBUG_VALUE: u32 = 0xDEADBEEF;

    // Send Debug packet
    let debug = DebugPacket {
        header: PacketHeader::new(PacketType::Debug, 0),
        level: DebugLevel::Info,
        reserved: [0; 3],
        message: String::from("Test message"),
    };

    if send_packet(&debug).is_err() {
        write_tohost(FAILURE_CODE);
    }

    // Send DataU32 packet
    let data = DataU32Packet {
        header: PacketHeader::new(PacketType::DataU32, 0),
        value: DEBUG_VALUE,
        tag: 0,
    };

    if send_packet(&data).is_err() {
        write_tohost(FAILURE_CODE);
    }

    write_tohost(SUCCESS_CODE);
}
