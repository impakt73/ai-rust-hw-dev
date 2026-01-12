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

/// Unit test for packet protocol end-to-end communication
///
/// This test requires direct access to Simulator::new() and internal APIs because it needs to:
/// - Manually step through simulation cycles
/// - Send packets to the simulator during execution
/// - Check FIFO state between steps
///
/// These requirements make it a true unit test that cannot be easily converted to use
/// the public helper functions.
#[test]
fn test_packet_protocol_end_to_end() {
    use riscv_protocol::{EchoPacket, PacketHeader, PacketType};

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
    println!("\nStep 3: Running CPU and waiting for Echo response...");
    let mut received_echo_response = false;
    for _ in 0..PACKET_EXCHANGE_TIMEOUT_CYCLES {
        sim.step().expect("Step failed");
        let words = fifo_data.lock().unwrap();
        // Look for sufficient words to form an Echo packet (header + payload)
        if words.len() >= 12 {
            received_echo_response = true;
            println!("  ✓ Received Echo response ({} words total)", words.len());
            break;
        }
    }
    assert!(
        received_echo_response,
        "Should receive Echo response from CPU"
    );

    // Step 4: Parse and validate the Echo response
    println!("\nStep 4: Validating Echo response packet...");
    let fifo_words = fifo_data.lock().unwrap();
    let mut fifo_tx = std::collections::VecDeque::new();
    for &word in fifo_words.iter() {
        fifo_tx.push_back(word);
    }

    // Expect: Debug packet first, then Echo packet
    let debug_pkt =
        crate::packet_transport::receive_debug_packet(&mut fifo_tx).expect("Should parse Debug");
    println!("  ✓ Received Debug packet");
    if let Some(debug_pkt) = debug_pkt {
        println!(
            "    Message: \"{}\"",
            debug_pkt.message.trim_end_matches('\0')
        );
        println!("    Level: {:?}", debug_pkt.level);
    }

    let echo_response = crate::packet_transport::receive_echo_packet(&mut fifo_tx)
        .expect("Should parse Echo response");
    println!("  ✓ Received Echo response packet");
    if let Some(echo_pkt) = echo_response {
        println!("    Sequence: {}", echo_pkt.sequence);
        println!("    Timestamp: {}", echo_pkt.timestamp);
        assert_eq!(
            echo_pkt.sequence, 101,
            "Echo response sequence should be incremented from request (100 → 101)"
        );
    } else {
        panic!("No Echo packet found in FIFO");
    }

    // Step 5: Run to completion (tohost termination)
    println!("\nStep 5: Running to program completion...");
    let mut final_result = None;
    for _ in 0..PACKET_EXCHANGE_TIMEOUT_CYCLES {
        let step_result = sim.step();
        if let Ok(sr) = step_result {
            if sr.tohost_value.is_some() {
                final_result = sr.tohost_value;
                break;
            }
        }
    }

    assert_eq!(
        final_result,
        Some(42),
        "Program should terminate with tohost=42"
    );
    println!("  ✓ Program terminated with tohost=42");

    println!("\n========================================");
    println!("✓ ALL PACKET PROTOCOL TESTS PASSED");
    println!("========================================");
}

/// Unit test for rvprintln! macro functionality
///
/// This test requires direct access to Simulator::new() and internal APIs because it needs to:
/// - Manually step through simulation and check FIFO state
/// - Parse packet protocol data directly
///
/// These requirements make it a true unit test that cannot be easily converted to use
/// the public helper functions.
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
                let msg = debug_pkt.message.trim_end_matches('\0');
                println!(
                    "✓ Debug Packet {}: Level={:?}, Message=\"{}\"",
                    packet_count, debug_pkt.level, msg
                );

                assert_eq!(
                    msg, *expected_msg,
                    "Debug message {} should match expected",
                    packet_count
                );
                assert_eq!(
                    debug_pkt.level, *expected_level,
                    "Debug level {} should match expected",
                    packet_count
                );
            }
            Ok(None) => {
                panic!("Expected Debug packet {} but got None", packet_count + 1);
            }
            Err(e) => {
                panic!("Failed to parse Debug packet {}: {:?}", packet_count + 1, e);
            }
        }
    }

    assert_eq!(packet_count, 3, "Should receive exactly 3 Debug packets");
    assert_eq!(
        result.tohost_value,
        Some(42),
        "Program should halt with tohost=42"
    );

    println!("\n✓ All println tests passed!");
    println!("========================================");
}
