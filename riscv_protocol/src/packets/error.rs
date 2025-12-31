use crate::header::PacketHeader;
use serde::{Deserialize, Serialize};

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::string::String;

/// Error code enumeration
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ErrorCode {
    InvalidMagic = 1,          // Bad magic number
    InvalidLength = 2,         // Length field doesn't match data
    UnknownPacketType = 3,     // Unrecognized packet type
    DeserializationFailed = 4, // deserialization error
    BufferOverflow = 5,        // Packet too large for buffer
    FifoOverflow = 6,          // FIFO queue full
    InvalidAddress = 7,        // Memory access to invalid address
    InvalidRegister = 8,       // Invalid register index
    PermissionDenied = 9,      // Operation not allowed
}

/// Report errors in packet processing
#[cfg(feature = "alloc")]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ErrorPacket {
    pub header: PacketHeader,
    pub error_code: ErrorCode,
    pub reserved: [u8; 3],
    pub details: String, // Human-readable error description
}
