#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::ToString;
use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};
use riscv_rt::entry;
use riscv_protocol::*;
use rkyv::{from_bytes, rancor::Error as RkyvError, to_bytes};

// Simple allocator for bare-metal
use alloc::vec::Vec;

#[global_allocator]
static ALLOCATOR: SimpleAllocator = SimpleAllocator;

struct SimpleAllocator;

use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicUsize, Ordering};
use core::ptr::addr_of_mut;

unsafe impl GlobalAlloc for SimpleAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Simple bump allocator using a static buffer
        static mut HEAP: [u8; 8192] = [0; 8192];
        static OFFSET: AtomicUsize = AtomicUsize::new(0);

        let size = layout.size();
        let align = layout.align();
        
        // Align the offset
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

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // No-op: simple bump allocator doesn't support deallocation
    }
}

/// Panic handler for bare metal - infinite loop on panic
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

/// Write to tohost address to signal completion
#[inline(never)]
fn write_tohost(value: u32) -> ! {
    const TOHOST_ADDR: u32 = 0xFFFF_FFF0;
    unsafe {
        write_volatile(TOHOST_ADDR as *mut u32, value);
    }
    loop {}
}

/// FIFO memory-mapped I/O addresses
const FIFO_BASE: u32 = 0x4000_0000;
const FIFO_DATA: u32 = FIFO_BASE + 0x0;
const FIFO_STATUS: u32 = FIFO_BASE + 0x4;

const RX_VALID: u32 = 1 << 0;
const TX_READY: u32 = 1 << 1;

/// Read status register
#[inline(never)]
fn fifo_read_status() -> u32 {
    unsafe { read_volatile(FIFO_STATUS as *const u32) }
}

/// Read a word from the FIFO (blocks until data available)
#[inline(never)]
fn fifo_read_word() -> u32 {
    unsafe {
        // Wait for RX_VALID
        while (fifo_read_status() & RX_VALID) == 0 {
            core::hint::spin_loop();
        }
        read_volatile(FIFO_DATA as *const u32)
    }
}

/// Write a word to the FIFO (blocks until ready)
#[inline(never)]
fn fifo_write_word(word: u32) {
    unsafe {
        // Wait for TX_READY (always ready in simulation, but good practice)
        while (fifo_read_status() & TX_READY) == 0 {
            core::hint::spin_loop();
        }
        write_volatile(FIFO_DATA as *mut u32, word);
    }
}

/// Send a packet via FIFO
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
    // Serialize packet
    let bytes = to_bytes::<RkyvError>(packet).map_err(|_| "Serialization failed")?;

    // Send as u32 words
    for chunk in bytes.as_ref().chunks(4) {
        let mut word: u32 = 0;
        for (i, &byte) in chunk.iter().enumerate() {
            word |= (byte as u32) << (i * 8);
        }
        fifo_write_word(word);
    }

    Ok(())
}

/// Receive an Echo packet from FIFO
fn receive_echo_packet() -> Result<EchoPacket, &'static str> {
    const MAX_PACKET_WORDS: usize = 64; // 256 bytes / 4
    let mut bytes = Vec::new();

    // Read words until we have enough data or max reached
    for _ in 0..MAX_PACKET_WORDS {
        let word = fifo_read_word();
        bytes.extend_from_slice(&word.to_le_bytes());

        // Try to deserialize (rkyv will fail if not enough data)
        if bytes.len() >= 20 { // Minimum size for EchoPacket
            if let Ok(packet) = from_bytes::<EchoPacket, RkyvError>(&bytes) {
                return Ok(packet);
            }
        }
    }

    Err("Failed to receive echo packet")
}

/// Receive a DataU32 packet from FIFO
fn receive_data_u32_packet() -> Result<DataU32Packet, &'static str> {
    const MAX_PACKET_WORDS: usize = 64;
    let mut bytes = Vec::new();

    for _ in 0..MAX_PACKET_WORDS {
        let word = fifo_read_word();
        bytes.extend_from_slice(&word.to_le_bytes());

        // Try to deserialize
        if bytes.len() >= 16 { // Minimum size for DataU32Packet
            if let Ok(packet) = from_bytes::<DataU32Packet, RkyvError>(&bytes) {
                return Ok(packet);
            }
        }
    }

    Err("Failed to receive data packet")
}

/// Entry point for the packet test program
#[entry]
fn main() -> ! {
    const SUCCESS_CODE: u32 = 42;
    const FAILURE_CODE: u32 = 1;

    // Test 1: Receive an Echo packet, increment sequence, and send it back
    match receive_echo_packet() {
        Ok(echo) => {
            let response = EchoPacket {
                header: PacketHeader::new(PacketType::Echo, 0),
                sequence: echo.sequence + 1,
                timestamp: echo.timestamp + 100,
            };
            if send_packet(&response).is_err() {
                write_tohost(FAILURE_CODE);
            }
        }
        Err(_) => write_tohost(FAILURE_CODE),
    }

    // Test 2: Send a Debug packet
    let debug = DebugPacket {
        header: PacketHeader::new(PacketType::Debug, 0),
        level: DebugLevel::Info,
        reserved: [0; 3],
        message: "Hello from CPU!".to_string(),
    };
    if send_packet(&debug).is_err() {
        write_tohost(FAILURE_CODE);
    }

    // Test 3: Receive a DataU32 packet and send back doubled value
    match receive_data_u32_packet() {
        Ok(data) => {
            let response = DataU32Packet {
                header: PacketHeader::new(PacketType::DataU32, 0),
                value: data.value.wrapping_mul(2),
                tag: data.tag,
            };
            if send_packet(&response).is_err() {
                write_tohost(FAILURE_CODE);
            }
        }
        Err(_) => write_tohost(FAILURE_CODE),
    }

    // Test 4: Send an Assert packet indicating success
    let assert_pkt = AssertPacket {
        header: PacketHeader::new(PacketType::Assert, 0),
        passed: true,
        reserved: [0; 3],
        test_id: 1,
        expected: 42,
        actual: 42,
        message: "All tests passed".to_string(),
    };
    if send_packet(&assert_pkt).is_err() {
        write_tohost(FAILURE_CODE);
    }

    // All tests passed
    write_tohost(SUCCESS_CODE);
}
