use crate::protocol::header::PacketHeader;
use serde::{Deserialize, Serialize};

/// No operation / keepalive packet
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NopPacket {
    pub header: PacketHeader,
}

/// Echo request/response packet for testing
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EchoPacket {
    pub header: PacketHeader,
    pub sequence: u32,  // Sequence number for matching request/response
    pub timestamp: u64, // Timestamp in cycles or microseconds
}
