use crate::header::PacketHeader;
use rkyv::{Archive, Deserialize, Serialize};

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

/// Read a contiguous memory region
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
pub struct MemoryReadPacket {
    pub header: PacketHeader,
    pub address: u32, // Starting memory address
    pub length: u32,  // Number of bytes to read
}

/// Response packet for memory read
#[cfg(feature = "alloc")]
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
pub struct MemoryReadResponsePacket {
    pub header: PacketHeader,
    pub address: u32,  // Starting address (echoed from request)
    pub data: Vec<u8>, // Memory contents
}

/// Write to a contiguous memory region
#[cfg(feature = "alloc")]
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
pub struct MemoryWritePacket {
    pub header: PacketHeader,
    pub address: u32,  // Starting memory address
    pub data: Vec<u8>, // Data to write
}
