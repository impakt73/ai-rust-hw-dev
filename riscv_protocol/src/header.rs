use rkyv::{Archive, Deserialize, Serialize};

/// Magic number for packet validation (0x52565043 = "RVPC" in ASCII)
pub const PACKET_MAGIC: u32 = 0x52565043;

/// Common packet header (8 bytes)
#[derive(Archive, Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct PacketHeader {
    /// Magic number for packet validation (0x52565043 = "RVPC" in ASCII)
    pub magic: u32,

    /// Total packet length in bytes (including header)
    pub length: u16,

    /// Packet type identifier
    pub packet_type: PacketType,

    /// Reserved for future use / alignment (set to 0)
    pub reserved: u8,
}

impl PacketHeader {
    /// Create a new packet header with the given type and length
    pub fn new(packet_type: PacketType, length: u16) -> Self {
        Self {
            magic: PACKET_MAGIC,
            length,
            packet_type,
            reserved: 0,
        }
    }
}

/// Packet type enumeration
#[derive(Archive, Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PacketType {
    // Basic communication packets
    Nop = 0,  // No operation / keepalive
    Echo = 1, // Echo request/response for testing

    // Data transfer packets
    DataU32 = 10,    // Single 32-bit unsigned integer
    DataI32 = 11,    // Single 32-bit signed integer
    DataBuffer = 12, // Arbitrary byte buffer
    DataString = 13, // UTF-8 string

    // Control packets
    Reset = 20,  // Request CPU reset
    Halt = 21,   // Request simulation halt
    Status = 22, // Status query/response

    // Register access packets
    RegisterRead = 30,  // Read CPU register(s)
    RegisterWrite = 31, // Write CPU register(s)

    // Memory access packets
    MemoryRead = 40,  // Read memory region
    MemoryWrite = 41, // Write memory region

    // Test/Debug packets
    Assert = 50, // Test assertion result
    Debug = 51,  // Debug message

    // Error packets
    Error = 255, // Error notification
}
