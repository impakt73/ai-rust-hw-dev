mod common;

use common::init_test_logger;
use cpu_sim::*;

#[test]
fn test_packet_protocol_infrastructure() {
    use riscv_shared::protocol::*;

    init_test_logger();

    println!("\n========================================");
    println!("PACKET PROTOCOL INFRASTRUCTURE TEST");
    println!("========================================");
    println!("Testing packet serialization and transport...\n");

    // Create a simple test without running actual CPU code
    // Just test the packet transport infrastructure

    // Test Echo packet
    let echo_packet = EchoPacket {
        header: PacketHeader::new(PacketType::Echo, 20),
        sequence: 42,
        timestamp: 123456789,
    };

    let mut fifo_rx = std::collections::VecDeque::new();
    packet_transport::send_echo_packet(&echo_packet, &mut fifo_rx).unwrap();

    // Simulate CPU echoing the packet back
    let mut fifo_tx = std::collections::VecDeque::new();
    while let Some(word) = fifo_rx.pop_front() {
        fifo_tx.push_back(word);
    }

    let received_echo = packet_transport::receive_echo_packet(&mut fifo_tx)
        .unwrap()
        .expect("Should receive echo packet");

    assert_eq!(received_echo.sequence, 42);
    assert_eq!(received_echo.timestamp, 123456789);
    println!(
        "✓ Echo packet: sequence={}, timestamp={}",
        received_echo.sequence, received_echo.timestamp
    );

    // Test DataU32 packet
    let data_packet = DataU32Packet {
        header: PacketHeader::new(PacketType::DataU32, 16),
        value: 0xDEADBEEF,
        tag: 100,
    };

    let mut fifo_rx2 = std::collections::VecDeque::new();
    packet_transport::send_data_u32_packet(&data_packet, &mut fifo_rx2).unwrap();

    let mut fifo_tx2 = std::collections::VecDeque::new();
    while let Some(word) = fifo_rx2.pop_front() {
        fifo_tx2.push_back(word);
    }

    let received_data = packet_transport::receive_data_u32_packet(&mut fifo_tx2)
        .unwrap()
        .expect("Should receive data packet");

    assert_eq!(received_data.value, 0xDEADBEEF);
    assert_eq!(received_data.tag, 100);
    println!(
        "✓ DataU32 packet: value=0x{:08x}, tag={}",
        received_data.value, received_data.tag
    );

    // Test Debug packet
    let debug_packet = DebugPacket {
        header: PacketHeader::new(PacketType::Debug, 0),
        level: DebugLevel::Info,
        reserved: [0; 3],
        message: "Hello from CPU!".to_string(),
    };

    let mut fifo_rx3 = std::collections::VecDeque::new();
    packet_transport::send_debug_packet(&debug_packet, &mut fifo_rx3).unwrap();

    let mut fifo_tx3 = std::collections::VecDeque::new();
    while let Some(word) = fifo_rx3.pop_front() {
        fifo_tx3.push_back(word);
    }

    let received_debug = packet_transport::receive_debug_packet(&mut fifo_tx3)
        .unwrap()
        .expect("Should receive debug packet");

    assert_eq!(received_debug.level, DebugLevel::Info);
    assert_eq!(received_debug.message, "Hello from CPU!");
    println!(
        "✓ Debug packet: level={:?}, message=\"{}\"",
        received_debug.level, received_debug.message
    );

    println!("\n========================================");
    println!("PACKET PROTOCOL TEST COMPLETE");
    println!("========================================");
    println!("✓ All packet types serialized and deserialized correctly");
    println!("✓ Transport layer working as expected");
    println!("========================================\n");
}
