mod common;

use bus_shared::{Fifo, FifoDataSource};
use common::{
    create_test_runtime_with_registrations, load_and_boot_elf, resolve_test_elf_path,
    wait_for_tohost, LONG_TIMEOUT,
};
use cpu_sim::packet_transport;
use device_runtime::BusDeviceRegistration;
use riscv_shared::protocol::{
    DataU32Packet, DebugLevel, EchoPacket, PacketHeader, PacketType, PACKET_MAGIC,
};
use riscv_shared::FIFO_BASE;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[test]
fn test_packet_protocol_end_to_end() {
    let elf_path = resolve_test_elf_path("packet_test");

    // Shared state for collecting FIFO TX data
    let fifo_tx_data: Arc<Mutex<Vec<u32>>> = Arc::new(Mutex::new(Vec::new()));
    let fifo_tx_data_clone = fifo_tx_data.clone();

    // Track whether we've sent the test packets yet
    let packets_sent = Arc::new(Mutex::new(false));
    let packets_sent_clone = packets_sent.clone();

    let fifo_source = Arc::new(Mutex::new(FifoDataSource::new()));
    let fifo_source_for_callback = fifo_source.clone();

    // Callback that handles bidirectional packet communication
    let fifo_callback = move |word: u32| {
        fifo_tx_data_clone
            .lock()
            .expect("fifo_tx_data lock poisoned in callback")
            .push(word);

        // On first invocation, send the test packets back into the FIFO source
        let mut sent = packets_sent_clone
            .lock()
            .expect("packets_sent lock poisoned in callback");
        if !*sent {
            // Send Echo packet (seq=100)
            let echo_request = EchoPacket {
                header: PacketHeader::new(PacketType::Echo, 0),
                sequence: 100,
                timestamp: 12345,
            };
            let mut temp_rx = VecDeque::new();
            packet_transport::send_echo_packet(&echo_request, &mut temp_rx)
                .expect("Failed to serialize Echo packet for CPU");
            while let Some(w) = temp_rx.pop_front() {
                fifo_source_for_callback
                    .lock()
                    .expect("FIFO source lock poisoned in callback")
                    .write_word(w);
            }

            // Send DataU32 packet (value=1000)
            let data_request = DataU32Packet {
                header: PacketHeader::new(PacketType::DataU32, 0),
                value: 1000,
                tag: 55,
            };
            let mut temp_rx = VecDeque::new();
            packet_transport::send_data_u32_packet(&data_request, &mut temp_rx)
                .expect("Failed to serialize DataU32 packet for CPU");
            while let Some(w) = temp_rx.pop_front() {
                fifo_source_for_callback
                    .lock()
                    .expect("FIFO source lock poisoned in callback")
                    .write_word(w);
            }

            *sent = true;
        }
    };

    let mut runtime = create_test_runtime_with_registrations(Some(vec![BusDeviceRegistration {
        base_addr: FIFO_BASE,
        device: Box::new(Fifo::new_with_callback(
            fifo_source,
            Box::new(fifo_callback),
        )),
    }]));

    load_and_boot_elf(runtime.as_mut(), &elf_path);
    let tohost_value = wait_for_tohost(runtime.as_mut(), LONG_TIMEOUT);

    assert_eq!(
        tohost_value, 42,
        "Program should complete with success code 42"
    );

    // Parse and verify received packets
    let fifo_words = fifo_tx_data.lock().expect("fifo_tx_data lock poisoned");
    let mut fifo_tx: VecDeque<u32> = fifo_words.iter().copied().collect();

    let debug_pkt = packet_transport::receive_debug_packet(&mut fifo_tx)
        .expect("Failed to parse Debug packet")
        .expect("Should receive Debug packet");
    assert_eq!(debug_pkt.header.magic, PACKET_MAGIC);
    assert_eq!(debug_pkt.message, "CPU Started");

    let echo_pkt = packet_transport::receive_echo_packet(&mut fifo_tx)
        .expect("Failed to parse Echo packet")
        .expect("Should receive Echo packet");
    assert_eq!(echo_pkt.header.magic, PACKET_MAGIC);
    assert_eq!(
        echo_pkt.sequence, 101,
        "Echo sequence should be incremented"
    );

    let data_pkt = packet_transport::receive_data_u32_packet(&mut fifo_tx)
        .expect("Failed to parse DataU32 packet")
        .expect("Should receive DataU32 packet");
    assert_eq!(data_pkt.header.magic, PACKET_MAGIC);
    assert_eq!(data_pkt.value, 2000, "DataU32 value should be doubled");

    let assert_pkt = packet_transport::receive_assert_packet(&mut fifo_tx)
        .expect("Failed to parse Assert packet")
        .expect("Should receive Assert packet");
    assert_eq!(assert_pkt.header.magic, PACKET_MAGIC);
    assert!(
        assert_pkt.passed,
        "Assert packet should indicate test passed"
    );
}

#[test]
fn test_println_macro() {
    let elf_path = resolve_test_elf_path("println_test");

    // Collect FIFO TX words from CPU
    let fifo_data: Arc<Mutex<Vec<u32>>> = Arc::new(Mutex::new(Vec::new()));
    let fifo_data_clone = fifo_data.clone();
    let fifo_source = Arc::new(Mutex::new(FifoDataSource::new()));
    let fifo_callback = move |word: u32| {
        fifo_data_clone
            .lock()
            .expect("fifo_data lock poisoned in callback")
            .push(word);
    };

    let mut runtime = create_test_runtime_with_registrations(Some(vec![BusDeviceRegistration {
        base_addr: FIFO_BASE,
        device: Box::new(Fifo::new_with_callback(
            fifo_source,
            Box::new(fifo_callback),
        )),
    }]));

    load_and_boot_elf(runtime.as_mut(), &elf_path);
    let tohost_value = wait_for_tohost(runtime.as_mut(), LONG_TIMEOUT);

    assert_eq!(
        tohost_value, 42,
        "Program should complete with success code 42"
    );

    let fifo_words = fifo_data.lock().expect("fifo_data lock poisoned");
    let mut fifo_tx: VecDeque<u32> = fifo_words.iter().copied().collect();

    let expected_messages = [
        ("Hello from RISC-V CPU!\n", DebugLevel::Info),
        ("The answer is 42\n", DebugLevel::Info),
        ("Testing println macro\n", DebugLevel::Info),
    ];

    for (expected_msg, expected_level) in &expected_messages {
        let pkt = packet_transport::receive_debug_packet(&mut fifo_tx)
            .expect("Failed to parse DebugPacket")
            .expect("Should receive DebugPacket");
        assert_eq!(pkt.level, *expected_level);
        assert_eq!(pkt.message, *expected_msg);
        assert_eq!(pkt.header.magic, PACKET_MAGIC);
        assert_eq!(pkt.header.packet_type, PacketType::Debug);
    }
}
