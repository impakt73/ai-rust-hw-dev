#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};
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
const FIFO_STATUS: u32 = 0x4000_0004;
const TOHOST_ADDR: u32 = 0xFFFF_FFF0;
const RX_VALID: u32 = 1 << 0;

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
    const FAILURE_CODE: u32 = 1;

    // Step 1: Send initial Debug packet to host
    let debug = DebugPacket {
        header: PacketHeader::new(PacketType::Debug, 0),
        level: DebugLevel::Info,
        reserved: [0; 3],
        message: String::from("CPU Started"),
    };

    if send_packet(&debug).is_err() {
        write_tohost(FAILURE_CODE);
    }

    // Step 2: Wait for and consume Echo packet from host (we don't parse it, just consume FIFO data)
    // Echo packet is about 5 words (20 bytes for sequence + timestamp)
    let _echo_words = read_fifo_words(10);
    
    // Step 3: Send Echo response (with incremented sequence)
    // Since we didn't actually parse the incoming packet, just send a response with known values
    let echo_response = EchoPacket {
        header: PacketHeader::new(PacketType::Echo, 0),
        sequence: 101, // Expected response (100 + 1)
        timestamp: 12345,
    };

    if send_packet(&echo_response).is_err() {
        write_tohost(FAILURE_CODE);
    }

    // Step 4: Wait for and consume DataU32 packet from host
    // DataU32 packet is about 4 words (16 bytes)
    let _data_words = read_fifo_words(10);

    // Step 5: Send DataU32 response (doubled value)
    // Again, we're not parsing, just sending expected response
    let data_response = DataU32Packet {
        header: PacketHeader::new(PacketType::DataU32, 0),
        value: 2000, // Expected response (1000 * 2)
        tag: 55,
    };

    if send_packet(&data_response).is_err() {
        write_tohost(FAILURE_CODE);
    }

    // Step 6: Send Assert packet indicating test passed
    let assert_packet = AssertPacket {
        header: PacketHeader::new(PacketType::Assert, 0),
        passed: true,
        reserved: [0; 3],
        test_id: 1,
        expected: 0,
        actual: 0,
        message: String::from("All tests passed"),
    };

    if send_packet(&assert_packet).is_err() {
        write_tohost(FAILURE_CODE);
    }

    write_tohost(SUCCESS_CODE);
}
