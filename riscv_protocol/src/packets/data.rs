use rkyv::{Archive, Deserialize, Serialize};
use crate::header::PacketHeader;

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::{vec::Vec, string::String};

/// Transfer a single unsigned 32-bit value
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
pub struct DataU32Packet {
    pub header: PacketHeader,
    pub value: u32,
    pub tag: u32,         // Optional identifier/tag
}

/// Transfer a single signed 32-bit value
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
pub struct DataI32Packet {
    pub header: PacketHeader,
    pub value: i32,
    pub tag: u32,         // Optional identifier/tag
}

/// Transfer arbitrary binary data
#[cfg(feature = "alloc")]
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
pub struct DataBufferPacket {
    pub header: PacketHeader,
    pub buffer_id: u32,   // Buffer identifier
    pub offset: u32,      // Offset within buffer (for partial transfers)
    pub data: Vec<u8>,    // Variable-length payload
}

/// Transfer UTF-8 text strings
#[cfg(feature = "alloc")]
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
pub struct DataStringPacket {
    pub header: PacketHeader,
    pub string_id: u32,   // String identifier
    pub text: String,     // UTF-8 string (variable-length)
}
