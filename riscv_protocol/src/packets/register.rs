use crate::header::PacketHeader;
use rkyv::{Archive, Deserialize, Serialize};

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

/// Read one or more CPU registers
#[cfg(feature = "alloc")]
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
pub struct RegisterReadPacket {
    pub header: PacketHeader,
    pub register_indices: Vec<u8>, // List of register numbers (0-31)
}

/// Response packet for register read
#[cfg(feature = "alloc")]
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
pub struct RegisterReadResponsePacket {
    pub header: PacketHeader,
    pub values: Vec<u32>, // Register values in same order as request
}

/// Single register write operation
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
pub struct RegisterWrite {
    pub register_index: u8,
    pub reserved: [u8; 3],
    pub value: u32,
}

/// Write one or more CPU registers
#[cfg(feature = "alloc")]
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
pub struct RegisterWritePacket {
    pub header: PacketHeader,
    pub writes: Vec<RegisterWrite>,
}
