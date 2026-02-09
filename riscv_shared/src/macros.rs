//! Formatted print macros for RISC-V bare-metal programs
//!
//! This module provides println-like macros that send messages from the
//! simulated RISC-V CPU to the host via the MMIO FIFO.

use crate::protocol::{DebugLevel, DebugPacket, PacketHeader, PacketType};
use alloc::string::String;
use core::ptr::write_volatile;
use postcard::to_allocvec;

/// MMIO addresses for FIFO communication
const FIFO_DATA: u32 = 0x4000_3000;

/// Send a DebugPacket to the host via MMIO FIFO
///
/// This function serializes a DebugPacket using postcard and writes it
/// word-by-word to the FIFO_DATA register.
///
/// # Error Handling
///
/// If postcard serialization fails, the function silently returns without
/// sending any data. In a bare-metal `no_std` environment, there are limited
/// options for error handling, and the macro invocation sites cannot easily
/// handle errors. The function is designed to be fail-safe: if serialization
/// fails, the program continues execution without the debug message.
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

/// Print formatted output to the host console (Info level) without newline
///
/// Usage: `rvprint!("Hello, {}!", "world");`
///
/// This macro works like print! but sends the output to the host
/// via the MMIO FIFO using DebugPacket protocol.
#[macro_export]
macro_rules! rvprint {
    ($($arg:tt)*) => {{
        let msg = $crate::alloc::format!($($arg)*);
        $crate::macros::send_debug_message($crate::protocol::DebugLevel::Info, msg);
    }};
}

/// Print formatted output to the host console (Info level) with newline
///
/// Usage: `rvprintln!("Hello, {}!", "world");`
///
/// This macro works like println! but sends the output to the host
/// via the MMIO FIFO using DebugPacket protocol.
#[macro_export]
macro_rules! rvprintln {
    ($($arg:tt)*) => {{
        let mut msg = $crate::alloc::format!($($arg)*);
        msg.push('\n');
        $crate::macros::send_debug_message($crate::protocol::DebugLevel::Info, msg);
    }};
}

/// Print formatted debug output to the host console (Debug level) without newline
#[macro_export]
macro_rules! rvdebug {
    ($($arg:tt)*) => {{
        let msg = $crate::alloc::format!($($arg)*);
        $crate::macros::send_debug_message($crate::protocol::DebugLevel::Debug, msg);
    }};
}

/// Print formatted debug output to the host console (Debug level) with newline
#[macro_export]
macro_rules! rvdebugln {
    ($($arg:tt)*) => {{
        let mut msg = $crate::alloc::format!($($arg)*);
        msg.push('\n');
        $crate::macros::send_debug_message($crate::protocol::DebugLevel::Debug, msg);
    }};
}

/// Print formatted error output to the host console (Error level) without newline
#[macro_export]
macro_rules! rverror {
    ($($arg:tt)*) => {{
        let msg = $crate::alloc::format!($($arg)*);
        $crate::macros::send_debug_message($crate::protocol::DebugLevel::Error, msg);
    }};
}

/// Print formatted error output to the host console (Error level) with newline
#[macro_export]
macro_rules! rverrorln {
    ($($arg:tt)*) => {{
        let mut msg = $crate::alloc::format!($($arg)*);
        msg.push('\n');
        $crate::macros::send_debug_message($crate::protocol::DebugLevel::Error, msg);
    }};
}
