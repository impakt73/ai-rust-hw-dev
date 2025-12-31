#![no_std]

// Re-export alloc for macros
pub extern crate alloc;

use alloc::string::String;
use core::ptr::write_volatile;
use postcard::to_allocvec;
use riscv_protocol::{DebugLevel, DebugPacket, PacketHeader, PacketType};

/// MMIO addresses for FIFO communication
const FIFO_DATA: u32 = 0x4000_0000;

/// Send a DebugPacket to the host via MMIO FIFO
/// 
/// This function serializes a DebugPacket using postcard and writes it
/// word-by-word to the FIFO_DATA register.
pub fn send_debug_message(level: DebugLevel, message: String) {
    let packet = DebugPacket {
        header: PacketHeader::new(PacketType::Debug, 0),
        level,
        reserved: [0; 3],
        message,
    };

    // Serialize packet to bytes
    if let Ok(bytes) = to_allocvec(&packet) {
        // Send bytes in 4-byte chunks (u32 words)
        for chunk in bytes.chunks(4) {
            let mut word: u32 = 0;
            for (i, &byte) in chunk.iter().enumerate() {
                word |= (byte as u32) << (i * 8);
            }
            unsafe {
                write_volatile(FIFO_DATA as *mut u32, word);
            }
        }
    }
}

/// Print formatted output to the host console (Info level)
/// 
/// Usage: `cprintln!("Hello, {}!", "world");`
/// 
/// This macro works like println! but sends the output to the host
/// via the MMIO FIFO using DebugPacket protocol.
#[macro_export]
macro_rules! cprintln {
    ($($arg:tt)*) => {{
        let msg = $crate::alloc::format!($($arg)*);
        $crate::send_debug_message($crate::riscv_protocol::DebugLevel::Info, msg);
    }};
}

/// Print formatted debug output to the host console (Debug level)
#[macro_export]
macro_rules! cdebugln {
    ($($arg:tt)*) => {{
        let msg = $crate::alloc::format!($($arg)*);
        $crate::send_debug_message($crate::riscv_protocol::DebugLevel::Debug, msg);
    }};
}

/// Print formatted error output to the host console (Error level)
#[macro_export]
macro_rules! cerrorln {
    ($($arg:tt)*) => {{
        let msg = $crate::alloc::format!($($arg)*);
        $crate::send_debug_message($crate::riscv_protocol::DebugLevel::Error, msg);
    }};
}

// Re-export for convenience
pub use riscv_protocol;
