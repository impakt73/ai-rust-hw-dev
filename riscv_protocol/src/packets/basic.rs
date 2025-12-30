use rkyv::{Archive, Deserialize, Serialize};
use crate::header::PacketHeader;

/// No operation / keepalive packet
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
pub struct NopPacket {
    pub header: PacketHeader,
}

/// Echo request/response packet for testing
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
pub struct EchoPacket {
    pub header: PacketHeader,
    pub sequence: u32,    // Sequence number for matching request/response
    pub timestamp: u64,   // Timestamp in cycles or microseconds
}
