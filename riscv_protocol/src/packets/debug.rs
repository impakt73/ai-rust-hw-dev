use rkyv::{Archive, Deserialize, Serialize};
use crate::header::PacketHeader;

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::string::String;

/// Debug level enumeration
#[derive(Archive, Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DebugLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warning = 3,
    Error = 4,
}

/// General debug messages from CPU to host
#[cfg(feature = "alloc")]
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
pub struct DebugPacket {
    pub header: PacketHeader,
    pub level: DebugLevel,
    pub reserved: [u8; 3],
    pub message: String,
}

/// Report test assertion results
#[cfg(feature = "alloc")]
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
pub struct AssertPacket {
    pub header: PacketHeader,
    pub passed: bool,         // True if assertion passed
    pub reserved: [u8; 3],
    pub test_id: u32,         // Test case identifier
    pub expected: u32,        // Expected value
    pub actual: u32,          // Actual value
    pub message: String,      // Optional description
}
