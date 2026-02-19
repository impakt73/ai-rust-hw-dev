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

#[test]
fn test_packet_protocol_end_to_end() {
    use riscv_shared::protocol::*;

    init_test_logger();

    println!("\n========================================");
    println!("PACKET PROTOCOL END-TO-END TEST");
    println!("========================================");
    println!("Testing bidirectional CPU↔Host packet communication...\n");

    let elf_path = sim_tests::test_program_path("packet_test").expect("Failed to find packet_test");

    // Shared state for collecting FIFO TX data
    let fifo_tx_data = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let fifo_tx_data_clone = fifo_tx_data.clone();

    // Track whether we've sent the test packets yet
    let packets_sent = std::sync::Arc::new(std::sync::Mutex::new(false));
    let packets_sent_clone = packets_sent.clone();

    let fifo_source = std::sync::Arc::new(std::sync::Mutex::new(FifoDataSource::new()));
    let fifo_source_for_callback = fifo_source.clone();

    // Callback that handles bidirectional packet communication
    let fifo_callback = move |word: u32| {
        fifo_tx_data_clone.lock().unwrap().push(word);

        // After collecting some data, send test packets once
        let tx_word_count = fifo_tx_data_clone.lock().unwrap().len();
        let mut sent = packets_sent_clone.lock().unwrap();
        if !*sent && tx_word_count > 0 {
            // Initial Debug packet received, now send Echo and DataU32 packets

            // Send Echo packet (seq=100)
            let echo_request = EchoPacket {
                header: PacketHeader::new(PacketType::Echo, 0),
                sequence: 100,
                timestamp: 12345,
            };
            let mut temp_rx = std::collections::VecDeque::new();
            cpu_sim::packet_transport::send_echo_packet(&echo_request, &mut temp_rx)
                .expect("Failed to serialize Echo packet for CPU");
            while let Some(word) = temp_rx.pop_front() {
                fifo_source_for_callback
                    .lock()
                    .expect("FIFO source lock poisoned in callback")
                    .write_word(word);
            }
            println!("\nStep 2: Sent Echo packet (seq=100) to CPU");

            // Send DataU32 packet (value=1000)
            let data_request = DataU32Packet {
                header: PacketHeader::new(PacketType::DataU32, 0),
                value: 1000,
                tag: 55,
            };
            let mut temp_rx = std::collections::VecDeque::new();
            cpu_sim::packet_transport::send_data_u32_packet(&data_request, &mut temp_rx)
                .expect("Failed to serialize DataU32 packet for CPU");
            while let Some(word) = temp_rx.pop_front() {
                fifo_source_for_callback
                    .lock()
                    .expect("FIFO source lock poisoned in callback")
                    .write_word(word);
            }
            println!("Step 3: Sent DataU32 packet (value=1000) to CPU");

            *sent = true;
        }
    };

    // Run the simulation
    let result = run_elf(
        &elf_path,
        GLOBAL_MAX_CYCLES,
        false, // print_inst_trace
        false, // print_fsm_state
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        None, // vcd_path
        0,    // mem_latency_cycles
        Some(move |view: &mut SimulatorView| {
            view.register_device(
                FIFO_BASE,
                Box::new(Fifo::new_with_callback(
                    fifo_source,
                    Box::new(fifo_callback),
                )),
            )
            .expect("Failed to register FIFO device");
        }),
        None::<fn(&cpu_sim::SimulatorView, &cpu_sim::SimulationResult)>,
    )
    .expect("Simulation should succeed");

    // Verify results
    println!("\n========================================");
    println!("VERIFICATION");
    println!("========================================");

    let fifo_words = fifo_tx_data.lock().unwrap();
    println!(
        "Total packets received from CPU: {} words ({} bytes)",
        fifo_words.len(),
        fifo_words.len() * 4
    );

    // Convert words to VecDeque for packet parsing
    let mut fifo_tx = std::collections::VecDeque::new();
    for &word in fifo_words.iter() {
        fifo_tx.push_back(word);
    }

    println!("Parsing received packets using postcard...");

    // Parse packets using packet_transport functions
    let mut found_debug = false;
    let mut found_echo = false;
    let mut found_data = false;
    let mut found_assert = false;

    // Try to receive Debug packet (first packet from CPU)
    if let Ok(Some(debug_pkt)) = cpu_sim::packet_transport::receive_debug_packet(&mut fifo_tx) {
        println!(
            "  ✓ Debug packet: level={:?}, message='{}'",
            debug_pkt.level, debug_pkt.message
        );
        assert_eq!(
            debug_pkt.header.magic, 0x52565043,
            "Debug packet should have correct magic"
        );
        assert_eq!(
            debug_pkt.message, "CPU Started",
            "Debug message should match"
        );
        found_debug = true;
    } else {
        println!("  ✗ Failed to deserialize Debug packet");
    }

    // Try to receive Echo response (should have sequence=101)
    if let Ok(Some(echo_pkt)) = cpu_sim::packet_transport::receive_echo_packet(&mut fifo_tx) {
        println!(
            "  ✓ Echo response: sequence={} (expected 101)",
            echo_pkt.sequence
        );
        assert_eq!(
            echo_pkt.header.magic, 0x52565043,
            "Echo packet should have correct magic"
        );
        assert_eq!(
            echo_pkt.sequence, 101,
            "Echo sequence should be incremented"
        );
        found_echo = true;
    } else {
        println!("  ✗ Failed to deserialize Echo packet");
    }

    // Try to receive DataU32 response (should have value=2000, which is 1000*2)
    if let Ok(Some(data_pkt)) = cpu_sim::packet_transport::receive_data_u32_packet(&mut fifo_tx) {
        println!(
            "  ✓ DataU32 response: value={} (expected 2000)",
            data_pkt.value
        );
        assert_eq!(
            data_pkt.header.magic, 0x52565043,
            "DataU32 packet should have correct magic"
        );
        assert_eq!(data_pkt.value, 2000, "DataU32 value should be doubled");
        found_data = true;
    } else {
        println!("  ✗ Failed to deserialize DataU32 packet");
    }

    // Try to receive Assert packet
    if let Ok(Some(assert_pkt)) = cpu_sim::packet_transport::receive_assert_packet(&mut fifo_tx) {
        println!(
            "  ✓ Assert packet: passed={}, message='{}'",
            assert_pkt.passed, assert_pkt.message
        );
        assert_eq!(
            assert_pkt.header.magic, 0x52565043,
            "Assert packet should have correct magic"
        );
        assert!(
            assert_pkt.passed,
            "Assert packet should indicate test passed"
        );
        found_assert = true;
    } else {
        println!("  ✗ Failed to deserialize Assert packet");
    }

    // Verify we received all expected packets
    assert!(found_debug, "Should receive Debug packet from CPU");
    assert!(found_echo, "Should receive Echo response from CPU");
    assert!(found_data, "Should receive DataU32 response from CPU");
    assert!(found_assert, "Should receive Assert packet from CPU");

    // Verify successful completion
    assert_eq!(
        result.tohost_value,
        Some(42),
        "Program should complete with success code 42"
    );

    println!("\n========================================");
    println!("END-TO-END TEST COMPLETE ✓");
    println!("========================================");
    println!("✓ Bidirectional communication verified");
    println!("✓ Host→CPU: Echo and DataU32 packets sent");
    println!("✓ CPU→Host: Debug, Echo, DataU32, and Assert packets received");
    println!("✓ Echo sequence incremented correctly (100 → 101)");
    println!("✓ DataU32 value doubled correctly (1000 → 2000)");
    println!("✓ All packet types validated");
    println!("✓ Program completed with success code 42");
    println!("========================================\n");
}

#[test]
fn test_println_macro() {
    init_test_logger();

    println!("\n========================================");
    println!("PRINTLN MACRO TEST");
    println!("========================================");
    println!("Testing rvprintln! macro functionality...\n");

    let elf_path =
        sim_tests::test_program_path("println_test").expect("Failed to find println_test");

    // Create a callback to collect FIFO data from CPU
    let fifo_data = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let fifo_data_clone = fifo_data.clone();
    let fifo_source = std::sync::Arc::new(std::sync::Mutex::new(FifoDataSource::new()));
    let fifo_callback = move |word: u32| {
        fifo_data_clone.lock().unwrap().push(word);
    };

    // Run the simulation with inst_complete callback
    let result = run_elf(
        &elf_path,
        GLOBAL_MAX_CYCLES,
        true,  // print_inst_trace
        false, // print_fsm_state
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        None, // vcd_path
        0,    // mem_latency_cycles
        Some(move |view: &mut SimulatorView| {
            view.register_device(
                FIFO_BASE,
                Box::new(Fifo::new_with_callback(
                    fifo_source,
                    Box::new(fifo_callback),
                )),
            )
            .expect("Failed to register FIFO device");
        }),
        None::<fn(&cpu_sim::SimulatorView, &cpu_sim::SimulationResult)>,
    )
    .expect("Simulation should succeed");

    // Check FIFO data
    let fifo_words = fifo_data.lock().unwrap();
    println!("FIFO TX words received: {} words", fifo_words.len());

    // Convert words to VecDeque for packet parsing
    let mut fifo_tx = std::collections::VecDeque::new();
    for &word in fifo_words.iter() {
        fifo_tx.push_back(word);
    }

    // Expected messages from the test program
    let expected_messages = [
        (
            "Hello from RISC-V CPU!\n",
            riscv_shared::protocol::DebugLevel::Info,
        ),
        (
            "The answer is 42\n",
            riscv_shared::protocol::DebugLevel::Info,
        ),
        (
            "Testing println macro\n",
            riscv_shared::protocol::DebugLevel::Info,
        ),
    ];

    // Try to receive and validate DebugPackets
    let mut packet_count = 0;
    for (expected_msg, expected_level) in expected_messages.iter() {
        match cpu_sim::packet_transport::receive_debug_packet(&mut fifo_tx) {
            Ok(Some(debug_pkt)) => {
                packet_count += 1;

                // Validate packet level
                assert_eq!(
                    debug_pkt.level, *expected_level,
                    "Packet {} should have level {:?}",
                    packet_count, expected_level
                );

                // Validate packet message
                assert_eq!(
                    debug_pkt.message, *expected_msg,
                    "Packet {} should have message '{}'",
                    packet_count, expected_msg
                );

                // Validate packet header magic
                assert_eq!(
                    debug_pkt.header.magic, 0x52565043,
                    "Packet {} should have correct magic number",
                    packet_count
                );

                // Validate packet type
                assert_eq!(
                    debug_pkt.header.packet_type,
                    riscv_shared::protocol::PacketType::Debug,
                    "Packet {} should be a Debug packet",
                    packet_count
                );

                // Print for visibility
                let level_str = match debug_pkt.level {
                    riscv_shared::protocol::DebugLevel::Trace => "[TRACE]",
                    riscv_shared::protocol::DebugLevel::Debug => "[DEBUG]",
                    riscv_shared::protocol::DebugLevel::Info => "[INFO]",
                    riscv_shared::protocol::DebugLevel::Warning => "[WARN]",
                    riscv_shared::protocol::DebugLevel::Error => "[ERROR]",
                };
                print!("{} {}", level_str, debug_pkt.message);
            }
            Ok(None) => {
                panic!("Expected packet {} but received None", packet_count + 1);
            }
            Err(e) => {
                panic!("Failed to deserialize packet {}: {}", packet_count + 1, e);
            }
        }
    }

    println!("\nReceived and validated {} DebugPacket(s)", packet_count);

    // Verify successful completion
    assert_eq!(
        result.tohost_value,
        Some(42),
        "Program should complete with success code 42"
    );

    assert_eq!(
        packet_count, 3,
        "Should have received exactly 3 DebugPackets"
    );

    println!("\n========================================");
    println!("PRINTLN MACRO TEST COMPLETE ✓");
    println!("========================================");
    println!("✓ rvprintln! messages received and validated");
    println!(
        "✓ Program completed successfully in {} cycles",
        result.cycles
    );
    println!("========================================\n");
}
