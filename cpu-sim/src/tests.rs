use crate::hung_detector::HungDetectorConfig;
use std::path::PathBuf;

/// Helper function to initialize test logger (idempotent)
fn init_test_logger() {
    let _ = env_logger::builder().is_test(true).try_init();
}

/// Helper function to get path to a test program ELF file
fn test_program_path(filename: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .expect("CARGO_MANIFEST_DIR should have a parent directory (workspace root)");
    workspace_root.join("test_programs").join(filename)
}

#[test]
fn test_packet_protocol_end_to_end() {
    use riscv_protocol::*;

    init_test_logger();

    // Configurable timeout for packet exchange operations.
    // This value balances reasonable wait time for packet operations against
    // quick failure detection. Adjust based on packet complexity and CPU speed.
    const PACKET_EXCHANGE_TIMEOUT_CYCLES: u32 = 10000;

    println!("\n========================================");
    println!("PACKET PROTOCOL END-TO-END TEST");
    println!("========================================");
    println!("Testing bidirectional CPU↔Host packet communication...\n");

    let elf_path = test_program_path("packet_test.elf");

    // Create system bus with internal DRAM
    let bus = crate::bus::SystemBus::new();

    // Create a callback to collect FIFO data from CPU
    let fifo_data = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let fifo_data_clone = fifo_data.clone();
    let fifo_callback = move |word: u32| {
        fifo_data_clone.lock().unwrap().push(word);
    };

    // Initialize CPU Simulator
    let runtime = riscv_core::create_cpu_runtime().expect("Failed to create CPU runtime");
    let mut sim = crate::sim::Simulator::new(
        &runtime,
        bus,
        false, // Disable instruction trace
        false, // Don't print FSM state
        Some(fifo_callback),
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,                                                      // No VCD
        0,                                                         // Zero latency
        Some(crate::hung_detector::HungDetectorConfig::default()), // Enable hung detection (test properly ends with tohost termination)
    )
    .expect("Failed to create simulator");

    // Load ELF into simulator memory
    let entry_point = crate::load_elf(&mut sim, &elf_path).expect("Failed to load packet_test.elf");

    log::info!("ELF loaded successfully");
    log::info!("Entry point: 0x{:08x}", entry_point);

    // Reset the CPU before starting
    sim.reset(entry_point).expect("Failed to reset simulator");

    println!("Running CPU program and exchanging packets...\n");

    // Step 1: Run CPU until it sends initial Debug packet
    println!("Step 1: Waiting for initial Debug packet from CPU...");
    let mut received_initial_debug = false;
    for _ in 0..PACKET_EXCHANGE_TIMEOUT_CYCLES {
        sim.step().expect("Step failed");
        let words = fifo_data.lock().unwrap();
        if !words.is_empty() {
            received_initial_debug = true;
            println!("  ✓ Received {} words from CPU", words.len());
            break;
        }
    }
    assert!(
        received_initial_debug,
        "Should receive initial Debug packet from CPU"
    );

    // Step 2: Send Echo packet to CPU
    println!("\nStep 2: Sending Echo packet (seq=100) to CPU...");
    let echo_request = EchoPacket {
        header: PacketHeader::new(PacketType::Echo, 0),
        sequence: 100,
        timestamp: 12345,
    };
    sim.send_echo_packet(&echo_request)
        .expect("Failed to send Echo packet");
    println!("  ✓ Echo packet sent to CPU");

    // Step 3: Run CPU and wait for Echo response
    println!("\nStep 3: Waiting for Echo response from CPU...");
    let initial_word_count = fifo_data.lock().unwrap().len();
    for _ in 0..PACKET_EXCHANGE_TIMEOUT_CYCLES {
        sim.step().expect("Step failed");
        let words = fifo_data.lock().unwrap();
        if words.len() > initial_word_count {
            println!("  ✓ Received Echo response ({} words total)", words.len());
            break;
        }
    }

    // Step 4: Send DataU32 packet to CPU
    println!("\nStep 4: Sending DataU32 packet (value=1000) to CPU...");
    let data_request = DataU32Packet {
        header: PacketHeader::new(PacketType::DataU32, 0),
        value: 1000,
        tag: 55,
    };
    sim.send_data_u32_packet(&data_request)
        .expect("Failed to send DataU32 packet");
    println!("  ✓ DataU32 packet sent to CPU");

    // Step 5: Run CPU and wait for DataU32 response
    println!("\nStep 5: Waiting for DataU32 response from CPU...");
    let words_before_data = fifo_data.lock().unwrap().len();
    for _ in 0..PACKET_EXCHANGE_TIMEOUT_CYCLES {
        sim.step().expect("Step failed");
        let words = fifo_data.lock().unwrap();
        if words.len() > words_before_data {
            println!(
                "  ✓ Received DataU32 response ({} words total)",
                words.len()
            );
            break;
        }
    }

    // Step 6: Run until CPU halts
    println!("\nStep 6: Running CPU until halt...");
    let mut final_tohost = None;
    for cycle in 0..50000 {
        let step_result = sim.step().expect("Step failed");
        if let Some(tohost) = step_result.tohost_value {
            println!(
                "  ✓ CPU halted at cycle {} with tohost=0x{:08x}",
                cycle, tohost
            );
            final_tohost = Some(tohost);
            break;
        }
    }

    // Verify results
    println!("\n========================================");
    println!("VERIFICATION");
    println!("========================================");

    let fifo_words = fifo_data.lock().unwrap();
    println!(
        "Total packets received from CPU: {} words ({} bytes)",
        fifo_words.len(),
        fifo_words.len() * 4
    );

    // Convert words to VecDeque for packet parsing (using packet_transport functions)
    let mut fifo_tx = std::collections::VecDeque::new();
    for &word in fifo_words.iter() {
        fifo_tx.push_back(word);
    }

    println!("Parsing received packets using postcard...");

    // Convert first few words to bytes for debug display
    let mut first_bytes = Vec::new();
    for i in 0..fifo_words.len().min(16) {
        first_bytes.extend_from_slice(&fifo_words[i].to_le_bytes());
    }
    println!(
        "First 64 bytes: {:02x?}",
        &first_bytes[..first_bytes.len().min(64)]
    );

    // Parse packets using packet_transport functions
    let mut found_debug = false;
    let mut found_echo = false;
    let mut found_data = false;
    let mut found_assert = false;

    // Try to receive Debug packet (first packet from CPU)
    if let Ok(Some(debug_pkt)) = crate::packet_transport::receive_debug_packet(&mut fifo_tx) {
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
    if let Ok(Some(echo_pkt)) = crate::packet_transport::receive_echo_packet(&mut fifo_tx) {
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
    if let Ok(Some(data_pkt)) = crate::packet_transport::receive_data_u32_packet(&mut fifo_tx) {
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
    if let Ok(Some(assert_pkt)) = crate::packet_transport::receive_assert_packet(&mut fifo_tx) {
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
        final_tohost,
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

    let elf_path = test_program_path("println_test.elf");

    // Create system bus with internal DRAM
    let bus = crate::bus::SystemBus::new();

    // Create a callback to collect FIFO data from CPU
    let fifo_data = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let fifo_data_clone = fifo_data.clone();
    let fifo_callback = move |word: u32| {
        fifo_data_clone.lock().unwrap().push(word);
    };

    // Initialize CPU Simulator
    let runtime = riscv_core::create_cpu_runtime().expect("Failed to create CPU runtime");
    let mut sim = crate::sim::Simulator::new(
        &runtime,
        bus,
        true,  // Enable instruction trace
        false, // Don't print FSM state
        Some(fifo_callback),
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None, // No VCD
        0,    // Zero latency
        Some(HungDetectorConfig::default()),
    )
    .expect("Failed to create simulator");

    // Load ELF into simulator memory
    let entry_point =
        crate::load_elf(&mut sim, &elf_path).expect("Failed to load println_test.elf");

    // Reset the CPU before starting
    sim.reset(entry_point).expect("Failed to reset simulator");

    println!("Running CPU program...\n");

    // Run until halt
    let result = sim
        .run(entry_point, 25000)
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
        ("Hello from RISC-V CPU!\n", riscv_protocol::DebugLevel::Info),
        ("The answer is 42\n", riscv_protocol::DebugLevel::Info),
        ("Testing println macro\n", riscv_protocol::DebugLevel::Info),
    ];

    // Try to receive and validate DebugPackets
    let mut packet_count = 0;
    for (expected_msg, expected_level) in expected_messages.iter() {
        match crate::packet_transport::receive_debug_packet(&mut fifo_tx) {
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
                    riscv_protocol::PacketType::Debug,
                    "Packet {} should be a Debug packet",
                    packet_count
                );

                // Print for visibility
                let level_str = match debug_pkt.level {
                    riscv_protocol::DebugLevel::Trace => "[TRACE]",
                    riscv_protocol::DebugLevel::Debug => "[DEBUG]",
                    riscv_protocol::DebugLevel::Info => "[INFO]",
                    riscv_protocol::DebugLevel::Warning => "[WARN]",
                    riscv_protocol::DebugLevel::Error => "[ERROR]",
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
