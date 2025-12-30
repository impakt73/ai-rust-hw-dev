use riscv_protocol::*;
use rkyv::{from_bytes, rancor::Error, to_bytes};

#[test]
fn test_nop_packet_roundtrip() {
    let packet = NopPacket {
        header: PacketHeader::new(PacketType::Nop, 8),
    };

    let bytes = to_bytes::<Error>(&packet).unwrap();
    let deserialized: NopPacket = from_bytes::<NopPacket, Error>(&bytes).unwrap();

    assert_eq!(deserialized.header.magic, PACKET_MAGIC);
    assert_eq!(deserialized.header.packet_type, PacketType::Nop);
}

#[test]
fn test_echo_packet_roundtrip() {
    let packet = EchoPacket {
        header: PacketHeader::new(PacketType::Echo, 20),
        sequence: 42,
        timestamp: 1234567890,
    };

    let bytes = to_bytes::<Error>(&packet).unwrap();
    let deserialized: EchoPacket = from_bytes::<EchoPacket, Error>(&bytes).unwrap();

    assert_eq!(deserialized.header.packet_type, PacketType::Echo);
    assert_eq!(deserialized.sequence, 42);
    assert_eq!(deserialized.timestamp, 1234567890);
}

#[test]
fn test_data_u32_packet_roundtrip() {
    let packet = DataU32Packet {
        header: PacketHeader::new(PacketType::DataU32, 16),
        value: 0xDEADBEEF,
        tag: 100,
    };

    let bytes = to_bytes::<Error>(&packet).unwrap();
    let deserialized: DataU32Packet = from_bytes::<DataU32Packet, Error>(&bytes).unwrap();

    assert_eq!(deserialized.header.packet_type, PacketType::DataU32);
    assert_eq!(deserialized.value, 0xDEADBEEF);
    assert_eq!(deserialized.tag, 100);
}

#[test]
fn test_data_i32_packet_roundtrip() {
    let packet = DataI32Packet {
        header: PacketHeader::new(PacketType::DataI32, 16),
        value: -42,
        tag: 200,
    };

    let bytes = to_bytes::<Error>(&packet).unwrap();
    let deserialized: DataI32Packet = from_bytes::<DataI32Packet, Error>(&bytes).unwrap();

    assert_eq!(deserialized.header.packet_type, PacketType::DataI32);
    assert_eq!(deserialized.value, -42);
    assert_eq!(deserialized.tag, 200);
}

#[test]
fn test_debug_packet_roundtrip() {
    let packet = DebugPacket {
        header: PacketHeader::new(PacketType::Debug, 0),
        level: DebugLevel::Info,
        reserved: [0; 3],
        message: "Hello from CPU!".to_string(),
    };

    let bytes = to_bytes::<Error>(&packet).unwrap();
    let deserialized: DebugPacket = from_bytes::<DebugPacket, Error>(&bytes).unwrap();

    assert_eq!(deserialized.header.packet_type, PacketType::Debug);
    assert_eq!(deserialized.level, DebugLevel::Info);
    assert_eq!(deserialized.message, "Hello from CPU!");
}

#[test]
fn test_halt_packet_roundtrip() {
    let packet = HaltPacket {
        header: PacketHeader::new(PacketType::Halt, 12),
        exit_code: 0,
    };

    let bytes = to_bytes::<Error>(&packet).unwrap();
    let deserialized: HaltPacket = from_bytes::<HaltPacket, Error>(&bytes).unwrap();

    assert_eq!(deserialized.header.packet_type, PacketType::Halt);
    assert_eq!(deserialized.exit_code, 0);
}
