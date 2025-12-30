/// Packet transport utilities for FIFO-based communication
use riscv_protocol::*;
use rkyv::{from_bytes, rancor::Error, to_bytes};
use std::collections::VecDeque;

/// Helper macro to send any packet type
macro_rules! impl_send_packet {
    ($name:ident, $packet_type:ty) => {
        pub fn $name(packet: &$packet_type, fifo_rx: &mut VecDeque<u32>) -> Result<(), String> {
            let bytes = to_bytes::<Error>(packet)
                .map_err(|e| format!("Serialization failed: {:?}", e))?;

            for chunk in bytes.as_ref().chunks(4) {
                let mut word: u32 = 0;
                for (i, &byte) in chunk.iter().enumerate() {
                    word |= (byte as u32) << (i * 8);
                }
                fifo_rx.push_back(word);
            }

            Ok(())
        }
    };
}

/// Helper macro to receive any packet type
macro_rules! impl_receive_packet {
    ($name:ident, $packet_type:ty) => {
        pub fn $name(fifo_tx: &mut VecDeque<u32>) -> Result<Option<$packet_type>, String> {
            if fifo_tx.len() < 2 {
                return Ok(None);
            }

            let mut header_bytes = Vec::new();
            for i in 0..2 {
                if let Some(&word) = fifo_tx.get(i) {
                    header_bytes.extend_from_slice(&word.to_le_bytes());
                } else {
                    return Ok(None);
                }
            }

            let magic = u32::from_le_bytes([
                header_bytes[0],
                header_bytes[1],
                header_bytes[2],
                header_bytes[3],
            ]);
            let length = u16::from_le_bytes([header_bytes[4], header_bytes[5]]) as usize;

            if magic != PACKET_MAGIC {
                return Err(format!("Invalid packet magic: 0x{:08x}", magic));
            }

            let total_words = (length + 3) / 4;

            if fifo_tx.len() < total_words {
                return Ok(None);
            }

            let mut bytes = Vec::new();
            for _ in 0..total_words {
                if let Some(word) = fifo_tx.pop_front() {
                    bytes.extend_from_slice(&word.to_le_bytes());
                }
            }

            bytes.truncate(length);

            let packet: $packet_type = from_bytes::<$packet_type, Error>(&bytes)
                .map_err(|e| format!("Deserialization failed: {:?}", e))?;

            Ok(Some(packet))
        }
    };
}

// Implement send functions for all packet types
impl_send_packet!(send_nop_packet, NopPacket);
impl_send_packet!(send_echo_packet, EchoPacket);
impl_send_packet!(send_data_u32_packet, DataU32Packet);
impl_send_packet!(send_data_i32_packet, DataI32Packet);
impl_send_packet!(send_debug_packet, DebugPacket);
impl_send_packet!(send_halt_packet, HaltPacket);
impl_send_packet!(send_assert_packet, AssertPacket);

// Implement receive functions for all packet types  
impl_receive_packet!(receive_nop_packet, NopPacket);
impl_receive_packet!(receive_echo_packet, EchoPacket);
impl_receive_packet!(receive_data_u32_packet, DataU32Packet);
impl_receive_packet!(receive_data_i32_packet, DataI32Packet);
impl_receive_packet!(receive_debug_packet, DebugPacket);
impl_receive_packet!(receive_halt_packet, HaltPacket);
impl_receive_packet!(receive_assert_packet, AssertPacket);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_receive_echo_packet() {
        let mut fifo_rx = VecDeque::new();
        let mut fifo_tx = VecDeque::new();

        let packet = EchoPacket {
            header: PacketHeader::new(PacketType::Echo, 20),
            sequence: 123,
            timestamp: 456789,
        };

        // Send packet
        send_echo_packet(&packet, &mut fifo_rx).unwrap();

        // Move data from rx to tx (simulating CPU echoing)
        while let Some(word) = fifo_rx.pop_front() {
            fifo_tx.push_back(word);
        }

        // Receive packet
        let received = receive_echo_packet(&mut fifo_tx)
            .unwrap()
            .expect("Should receive packet");

        assert_eq!(received.sequence, 123);
        assert_eq!(received.timestamp, 456789);
    }

    #[test]
    fn test_send_receive_data_u32_packet() {
        let mut fifo_rx = VecDeque::new();
        let mut fifo_tx = VecDeque::new();

        let packet = DataU32Packet {
            header: PacketHeader::new(PacketType::DataU32, 16),
            value: 0xDEADBEEF,
            tag: 42,
        };

        send_data_u32_packet(&packet, &mut fifo_rx).unwrap();

        while let Some(word) = fifo_rx.pop_front() {
            fifo_tx.push_back(word);
        }

        let received = receive_data_u32_packet(&mut fifo_tx)
            .unwrap()
            .expect("Should receive packet");

        assert_eq!(received.value, 0xDEADBEEF);
        assert_eq!(received.tag, 42);
    }

    #[test]
    fn test_send_receive_debug_packet() {
        let mut fifo_rx = VecDeque::new();
        let mut fifo_tx = VecDeque::new();

        let packet = DebugPacket {
            header: PacketHeader::new(PacketType::Debug, 0),
            level: DebugLevel::Info,
            reserved: [0; 3],
            message: "Test message".to_string(),
        };

        send_debug_packet(&packet, &mut fifo_rx).unwrap();

        while let Some(word) = fifo_rx.pop_front() {
            fifo_tx.push_back(word);
        }

        let received = receive_debug_packet(&mut fifo_tx)
            .unwrap()
            .expect("Should receive packet");

        assert_eq!(received.level, DebugLevel::Info);
        assert_eq!(received.message, "Test message");
    }
}

