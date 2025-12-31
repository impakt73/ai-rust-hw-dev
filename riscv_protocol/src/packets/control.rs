use crate::header::PacketHeader;
use serde::{Deserialize, Serialize};

/// Reset type enumeration
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResetType {
    Soft = 0, // Software-triggered reset
    Hard = 1, // Hardware reset (full state clear)
}

/// Request CPU reset
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResetPacket {
    pub header: PacketHeader,
    pub reset_type: ResetType,
    pub reserved: [u8; 3],
}

/// Request simulation halt/termination
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HaltPacket {
    pub header: PacketHeader,
    pub exit_code: i32, // Exit code (0 = success, non-zero = error)
}

/// Query or report system status
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StatusPacket {
    pub header: PacketHeader,
    pub cycle_count: u64,  // Current cycle count
    pub pc: u32,           // Current program counter
    pub status_flags: u32, // Bit flags for various status indicators
}
