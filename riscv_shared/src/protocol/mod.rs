//! FIFO protocol definitions for host-CPU communication
//!
//! This module defines the packet protocol used for communication between
//! the simulated RISC-V CPU and the host via the MMIO FIFO.

pub mod header;
pub mod packets;

pub use header::{PacketHeader, PacketType, PACKET_MAGIC};
pub use packets::*;
