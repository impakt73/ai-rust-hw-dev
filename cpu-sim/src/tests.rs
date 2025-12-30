use super::*;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

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

/// Helper function to assert tohost value matches expected
fn assert_tohost(result: &SimulationResult, expected: u32, test_name: &str) {
    assert_eq!(
        result.tohost_value,
        Some(expected),
        "Expected tohost value 0x{:x} ({}) from {}",
        expected,
        expected,
        test_name
    );
}

/// Helper function to create a FIFO data collector
fn create_fifo_collector() -> (Arc<Mutex<Vec<u8>>>, impl FnMut(u32)) {
    let fifo_data = Arc::new(Mutex::new(Vec::new()));
    let fifo_data_clone = Arc::clone(&fifo_data);

    let callback = move |word: u32| {
        // Convert u32 word to bytes (little-endian)
        let bytes = [
            (word & 0xFF) as u8,
            ((word >> 8) & 0xFF) as u8,
            ((word >> 16) & 0xFF) as u8,
            ((word >> 24) & 0xFF) as u8,
        ];
        let mut fifo = fifo_data_clone
            .lock()
            .expect("Failed to lock FIFO data mutex in create_fifo_collector callback");
        fifo.extend_from_slice(&bytes);
    };

    (fifo_data, callback)
}

/// Helper function to extract string from FIFO data (removes trailing nulls)
fn fifo_data_to_string(data: &[u8]) -> String {
    let end_index = data.iter().rposition(|&b| b != 0);
    let trimmed_data = match end_index {
        Some(idx) => &data[..=idx],
        None => &[],
    };
    String::from_utf8(trimmed_data.to_vec()).expect("FIFO data should be valid UTF-8")
}

#[test]
fn test_comprehensive_elf() {
    init_test_logger();

    let elf_path = test_program_path("test.elf");
    let result = run_elf(&elf_path, 500, false).expect("Simulation should succeed");

    assert_tohost(&result, 0x2a, "comprehensive test");
    println!(
        "✓ Comprehensive test ELF executed successfully in {} cycles",
        result.cycles
    );
}

#[test]
fn test_instruction_trace() {
    init_test_logger();

    let elf_path = test_program_path("test.elf");
    let result = run_elf(&elf_path, 500, true).expect("Simulation with trace should succeed");

    assert_tohost(&result, 0x2a, "instruction trace test");
    println!(
        "✓ Instruction trace test passed in {} cycles",
        result.cycles
    );
}

#[test]
fn test_register_trace_audit() {
    init_test_logger();

    let elf_path = test_program_path("register_trace_audit.elf");

    println!("\n========================================");
    println!("REGISTER TRACE AUDIT TEST");
    println!("========================================");
    println!("This test verifies that instruction trace correctly displays");
    println!("source and destination register values.");
    println!();
    println!("VERIFICATION GUIDE:");
    println!("- Each ADD instruction: rd_value should equal rs1_value + rs2_value");
    println!("- Each SUB instruction: rd_value should equal rs1_value - rs2_value");
    println!("- Each ADDI instruction: rd_value should equal rs1_value + immediate");
    println!("- Load/Store instructions: verify address calculations and data values");
    println!();
    println!("Expected patterns to verify:");
    println!("  Phase 1: Fibonacci-like sequence (1, 2, 3, 5, 8, 13, 21)");
    println!("  Phase 2: Round numbers (10, 20, 30, 50, 80, 100)");
    println!("  Phase 3: Powers of 2 (1, 2, 4, 8, 16, 32, 64, 128, 256)");
    println!("  Phase 4: Subtraction (100-40=60, 60-40=20)");
    println!("  Phase 5: Load/Store with value 123 (0x7b)");
    println!("========================================\n");

    let result =
        run_elf(&elf_path, 500, true).expect("Register trace audit simulation should succeed");

    assert_tohost(&result, 0x2a, "register trace audit program");
    println!(
        "\n✓ Register trace audit test passed in {} cycles",
        result.cycles
    );
    println!("========================================");
    println!("AUDIT COMPLETE");
    println!("========================================");
    println!("Review the trace output above to verify:");
    println!("1. All ADD results match rs1 + rs2");
    println!("2. All SUB results match rs1 - rs2");
    println!("3. All ADDI results match rs1 + immediate");
    println!("4. Load/store operations show correct register values");
    println!("5. Register values progress through expected sequences");
    println!("========================================\n");
}

#[test]
fn test_rust_bare_metal_elf() {
    init_test_logger();

    let elf_path = test_program_path("rust_test.elf");
    let result = run_elf(&elf_path, 500, false).expect("Rust bare metal simulation should succeed");

    assert_tohost(&result, 0x2a, "Rust bare metal program");
    println!(
        "✓ Rust bare metal test ELF executed successfully in {} cycles",
        result.cycles
    );
}

#[test]
fn test_fifo_hello_world() {
    init_test_logger();

    let elf_path = test_program_path("hello_world.elf");
    let (fifo_data, callback) = create_fifo_collector();

    let test_string = "Qu1ck_Br0wn-F0x!Jump5*0v3r@Lazy#D0g$2024%";
    let result = run_elf_with_fifo(&elf_path, 10000, false, Some(callback), Some(test_string))
        .expect("FIFO hello world simulation should succeed");

    assert_tohost(&result, 0x2a, "hello_world program");

    let received_data = fifo_data.lock().unwrap();
    let received_string = fifo_data_to_string(&received_data);

    assert_eq!(
        received_string, test_string,
        "Expected to receive echoed test string via FIFO"
    );

    println!("✓ FIFO echo test passed in {} cycles", result.cycles);
    println!("✓ Echoed data via FIFO: '{}'", received_string);
}

#[test]
fn test_trace_callback() {
    use riscv_core::trace::InstructionType;

    init_test_logger();

    let elf_path = test_program_path("trace_test.elf");

    // Collect instruction traces via callback
    let traces = Arc::new(Mutex::new(Vec::new()));
    let traces_clone = Arc::clone(&traces);

    let trace_callback = move |trace: &InstructionTrace| {
        traces_clone.lock().unwrap().push(trace.clone());
    };

    let result = run_elf_with_trace_callback(&elf_path, 500, false, Some(trace_callback))
        .expect("Trace test simulation should succeed");

    assert_tohost(&result, 0x2a, "trace test program");

    // Verify we captured traces
    let captured_traces = traces.lock().unwrap();
    assert!(
        !captured_traces.is_empty(),
        "Should have captured instruction traces"
    );

    println!("\n========================================");
    println!("TRACE CALLBACK TEST RESULTS");
    println!("========================================");
    println!("Total instructions traced: {}", captured_traces.len());

    // Track which expected instructions we've found
    let mut found_addi_x1 = false;
    let mut found_addi_x2 = false;
    let mut found_addi_x3 = false;
    let mut found_add_x4 = false;
    let mut found_sub_x5 = false;
    let mut found_lui = false;
    let mut found_sw = false;
    let mut found_lw = false;

    for trace in captured_traces.iter() {
        match trace.inst_type {
            InstructionType::Addi => {
                if let InstructionTrace {
                    rd:
                        Some(riscv_core::trace::RegisterOperand {
                            reg: rd_reg,
                            value: rd_val,
                        }),
                    rs1:
                        Some(riscv_core::trace::RegisterOperand {
                            reg: rs1_reg,
                            value: _,
                        }),
                    immediate: Some(imm),
                    ..
                } = trace
                {
                    if *rd_reg == 1 && *rs1_reg == 0 && *imm == 10 && *rd_val == 10 {
                        found_addi_x1 = true;
                        println!("✓ Found: addi x1, x0, 10 → x1=10");
                    }
                    if *rd_reg == 2 && *rs1_reg == 0 && *imm == 20 && *rd_val == 20 {
                        found_addi_x2 = true;
                        println!("✓ Found: addi x2, x0, 20 → x2=20");
                    }
                    if *rd_reg == 3 && *rs1_reg == 0 && *imm == 5 && *rd_val == 5 {
                        found_addi_x3 = true;
                        println!("✓ Found: addi x3, x0, 5 → x3=5");
                    }
                }
            }
            InstructionType::Add => {
                if let InstructionTrace {
                    rd:
                        Some(riscv_core::trace::RegisterOperand {
                            reg: rd_reg,
                            value: rd_val,
                        }),
                    rs1:
                        Some(riscv_core::trace::RegisterOperand {
                            reg: rs1_reg,
                            value: rs1_val,
                        }),
                    rs2:
                        Some(riscv_core::trace::RegisterOperand {
                            reg: rs2_reg,
                            value: rs2_val,
                        }),
                    ..
                } = trace
                {
                    if *rd_reg == 4 && *rs1_reg == 1 && *rs2_reg == 2 && *rd_val == 30 {
                        found_add_x4 = true;
                        println!(
                            "✓ Found: add x4, x1, x2 → x4={} (x1={} + x2={})",
                            rd_val, rs1_val, rs2_val
                        );
                    }
                }
            }
            InstructionType::Sub => {
                if let InstructionTrace {
                    rd:
                        Some(riscv_core::trace::RegisterOperand {
                            reg: rd_reg,
                            value: rd_val,
                        }),
                    rs1:
                        Some(riscv_core::trace::RegisterOperand {
                            reg: rs1_reg,
                            value: rs1_val,
                        }),
                    rs2:
                        Some(riscv_core::trace::RegisterOperand {
                            reg: rs2_reg,
                            value: rs2_val,
                        }),
                    ..
                } = trace
                {
                    if *rd_reg == 5 && *rs1_reg == 2 && *rs2_reg == 3 && *rd_val == 15 {
                        found_sub_x5 = true;
                        println!(
                            "✓ Found: sub x5, x2, x3 → x5={} (x2={} - x3={})",
                            rd_val, rs1_val, rs2_val
                        );
                    }
                }
            }
            InstructionType::Lui => {
                found_lui = true;
                println!("✓ Found: LUI instruction");
            }
            InstructionType::Sw => {
                found_sw = true;
                println!("✓ Found: SW (store word) instruction");
            }
            InstructionType::Lw => {
                found_lw = true;
                println!("✓ Found: LW (load word) instruction");
            }
            _ => {}
        }
    }

    println!("========================================");

    // Assert that we found the expected instructions
    assert!(found_addi_x1, "Should find addi x1, x0, 10");
    assert!(found_addi_x2, "Should find addi x2, x0, 20");
    assert!(found_addi_x3, "Should find addi x3, x0, 5");
    assert!(found_add_x4, "Should find add x4, x1, x2");
    assert!(found_sub_x5, "Should find sub x5, x2, x3");
    assert!(found_lui, "Should find LUI instruction");
    assert!(found_sw, "Should find SW instruction");
    assert!(found_lw, "Should find LW instruction");

    println!("✓ Trace callback test passed in {} cycles", result.cycles);
    println!("✓ All expected instructions found and validated");
    println!("========================================\n");
}

#[test]
fn test_packet_protocol_infrastructure() {
    use riscv_protocol::*;
    
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
    println!("✓ Echo packet: sequence={}, timestamp={}", received_echo.sequence, received_echo.timestamp);
    
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
    println!("✓ DataU32 packet: value=0x{:08x}, tag={}", received_data.value, received_data.tag);
    
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
    println!("✓ Debug packet: level={:?}, message=\"{}\"", received_debug.level, received_debug.message);
    
    println!("\n========================================");
    println!("PACKET PROTOCOL TEST COMPLETE");
    println!("========================================");
    println!("✓ All packet types serialized and deserialized correctly");
    println!("✓ Transport layer working as expected");
    println!("========================================\n");
}

#[test]
fn test_packet_protocol_end_to_end() {
    init_test_logger();

    println!("\n========================================");
    println!("PACKET PROTOCOL END-TO-END TEST");
    println!("========================================");
    println!("Testing CPU→Host packet communication (serialization only)...\n");

    let elf_path = test_program_path("packet_test.elf");
    
    // Initialize DRAM and load ELF
    let mut dram = crate::dram::Dram::new();
    let entry_point = dram
        .load_elf(&elf_path)
        .expect("Failed to load packet_test.elf");

    log::info!("ELF loaded successfully");
    log::info!("Entry point: 0x{:08x}", entry_point);

    // Create system bus with DRAM and FIFO
    let bus = crate::bus::SystemBus::new(dram);

    // Create a callback to collect FIFO data
    let fifo_data = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let fifo_data_clone = fifo_data.clone();
    let fifo_callback = move |word: u32| {
        fifo_data_clone.lock().unwrap().push(word);
    };

    // Initialize CPU Simulator
    let runtime = riscv_core::create_cpu_runtime()
        .expect("Failed to create CPU runtime");
    let mut sim = crate::sim::Simulator::new(
        &runtime,
        bus,
        entry_point,
        false, // Disable instruction trace
        Some(fifo_callback),
        None::<fn(&riscv_core::trace::InstructionTrace)>,
    )
    .expect("Failed to create simulator");

    // Reset the CPU before starting
    sim.reset();

    // Run simulation and wait for packets from CPU
    println!("Running CPU program...");
    let mut debug_received = false;
    let mut data_received = false;
    let mut final_tohost = None;
    
    const DEBUG_VALUE: u32 = 0xDEADBEEF;
    
    for cycle in 0..50000 {
        if let Some(tohost) = sim.step() {
            println!("CPU halted at cycle {} with tohost=0x{:08x}", cycle, tohost);
            final_tohost = Some(tohost);
            // Continue for a few more cycles to ensure we get all packets
            for _ in 0..100 {
                sim.step();
            }
            break;
        }
        
        // Check for Debug packet
        if !debug_received {
            if let Some(debug_pkt) = sim.try_receive_debug_packet().unwrap() {
                println!("✓ Received Debug packet from CPU:");
                println!("  Level: {:?}", debug_pkt.level);
                println!("  Message: \"{}\"", debug_pkt.message);
                debug_received = true;
            }
        }
        
        // Check for DataU32 packet
        if !data_received {
            if let Some(data_pkt) = sim.try_receive_data_u32_packet().unwrap() {
                println!("✓ Received DataU32 packet from CPU:");
                println!("  Value: 0x{:08x}", data_pkt.value);
                println!("  Tag: {}", data_pkt.tag);
                assert_eq!(data_pkt.value, DEBUG_VALUE, "DataU32 value should match expected test value");
                data_received = true;
            }
        }
        
        if debug_received && data_received && final_tohost.is_some() {
            break;
        }
    }
    
    println!("\nDebug info:");
    let fifo_words = fifo_data.lock().unwrap();
    println!("  FIFO TX callback received {} words", fifo_words.len());
    if !fifo_words.is_empty() {
        println!("  First few words: {:?}", &fifo_words[..fifo_words.len().min(5)]);
    }
    println!("  Debug received: {}", debug_received);
    println!("  Data received: {}", data_received);
    println!("  Final tohost: {:?}", final_tohost);
    
    // Check marker at 0xFFFF_FFF4
    let marker = sim.bus.read_word(0xFFFF_FFF4);
    println!("  Marker at 0xFFFF_FFF4: 0x{:04x}", marker);
    
    // Verify results
    assert!(debug_received, "Should have received Debug packet from CPU");
    assert!(data_received, "Should have received DataU32 packet from CPU");
    assert_eq!(final_tohost, Some(42), "Program should complete with success code 42");
    
    println!("\n========================================");
    println!("END-TO-END TEST COMPLETE");
    println!("========================================");
    println!("✓ Debug packet: Received from CPU");
    println!("✓ DataU32 packet: Received with correct value (0x{:08x})", DEBUG_VALUE);
    println!("✓ Program completed with success code 42");
    println!("✓ Packet serialization in bare-metal works!");
    println!("========================================\n");
}
