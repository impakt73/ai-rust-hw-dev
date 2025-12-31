/// Packet transport utilities for FIFO-based communication
use riscv_protocol::*;
use postcard::{from_bytes, to_allocvec};
use std::collections::VecDeque;

/// Helper macro to send any packet type
macro_rules! impl_send_packet {
    ($name:ident, $packet_type:ty) => {
        pub fn $name(packet: &$packet_type, fifo_rx: &mut VecDeque<u32>) -> Result<(), String> {
            let bytes: Vec<u8> =
                to_allocvec(packet).map_err(|e| format!("Serialization failed: {:?}", e))?;

            let mut i = 0;
            while i < bytes.len() {
                let mut word: u32 = 0;
                for j in 0..4 {
                    if i + j < bytes.len() {
                        word |= (bytes[i + j] as u32) << (j * 8);
                    }
                }
                fifo_rx.push_back(word);
                i += 4;
            }

            Ok(())
        }
    };
}

/// Helper macro to receive any packet type
macro_rules! impl_receive_packet {
    ($name:ident, $packet_type:ty) => {
        pub fn $name(fifo_tx: &mut VecDeque<u32>) -> Result<Option<$packet_type>, String> {
            // For simplicity, try to read up to 256 bytes (64 words)
            // This should be enough for most packets
            const MAX_PACKET_WORDS: usize = 64;

            if fifo_tx.is_empty() {
                return Ok(None);
            }

            // Collect available words (up to max)
            let available_words = fifo_tx.len().min(MAX_PACKET_WORDS);
            let mut bytes = Vec::new();

            // Peek at the data without removing it yet
            for i in 0..available_words {
                if let Some(&word) = fifo_tx.get(i) {
                    bytes.extend_from_slice(&word.to_le_bytes());
                }
            }

            // Try to deserialize
            match from_bytes::<$packet_type>(&bytes) {
                Ok(packet) => {
                    // Success! Now remove the words we actually consumed
                    // For now, remove all peeked words
                    // TODO: Calculate exact consumed bytes from postcard
                    for _ in 0..available_words {
                        fifo_tx.pop_front();
                    }
                    Ok(Some(packet))
                }
                Err(_) => {
                    // Not enough data yet or invalid packet
                    Ok(None)
                }
            }
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
