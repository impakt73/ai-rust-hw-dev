use cpu_sim::*;
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
fn create_fifo_collector() -> (Arc<Mutex<Vec<u8>>>, impl FnMut(&mut SimulatorView)) {
    let fifo_data = Arc::new(Mutex::new(Vec::new()));
    let fifo_data_clone = Arc::clone(&fifo_data);

    let callback = move |view: &mut SimulatorView| {
        // Drain all words from TX FIFO and collect them as bytes
        while let Some(word) = view.fifo_read_tx() {
            // Convert u32 word to bytes (little-endian)
            let bytes = [
                (word & 0xFF) as u8,
                ((word >> 8) & 0xFF) as u8,
                ((word >> 16) & 0xFF) as u8,
                ((word >> 24) & 0xFF) as u8,
            ];
            let mut data = fifo_data_clone
                .lock()
                .expect("Failed to lock FIFO data mutex in create_fifo_collector callback");
            data.extend_from_slice(&bytes);
        }
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

/// Helper function to run ELF with FSM state printing enabled for debugging
#[allow(dead_code)]
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
fn test_fp_math_elf() {
    init_test_logger();

    let elf_path = test_program_path("test_fp_math.elf");
    let result = run_elf(&elf_path, 1000, false)
        .expect("Floating-point math test simulation should succeed");

    assert_tohost(&result, 0x2a, "FP math test program");
    println!(
        "✓ Floating-point math test ELF executed successfully in {} cycles",
        result.cycles
    );
}

#[test]
fn test_fifo_hello_world() {
    init_test_logger();

    let elf_path = test_program_path("hello_world.elf");
    let (fifo_data, callback) = create_fifo_collector();

    let test_string = "Qu1ck_Br0wn-F0x!Jump5*0v3r@Lazy#D0g$2024%";
    let result = run_program(
        10000,
        false, // print_inst_trace
        false, // print_fsm_state
        Some(callback),
        None::<fn(&InstructionTrace)>,
        None, // vcd_path
        0,    // mem_latency_cycles
        |sim| {
            // Write test string to FIFO RX before loading ELF
            sim.fifo_write_rx_string(test_string);
            load_elf(sim, &elf_path).map_err(|e| e.to_string())
        },
        |_sim, _result| {},
    )
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

    // NOTE: Replaced run_elf_with_trace_callback() with run_program()
    // to use the unified execution API
    let result = run_program(
        500,
        false, // print_inst_trace
        false, // print_fsm_state
        None::<fn(&mut SimulatorView)>,
        Some(trace_callback),
        None, // vcd_path
        0,    // mem_latency_cycles
        |sim| load_elf(sim, &elf_path).map_err(|e| e.to_string()),
        |_sim, _result| {},
    )
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

    // DEBUG: Print ALL captured traces with full details
    println!("\nDEBUG: All captured instruction traces:");
    for (i, trace) in captured_traces.iter().enumerate() {
        println!(
            "  [{}] PC=0x{:08x}, Type={:?}",
            i, trace.pc, trace.inst_type
        );
        if let Some(rd) = &trace.rd {
            println!("      rd: x{} = 0x{:08x}", rd.reg, rd.value);
        }
        if let Some(rs1) = &trace.rs1 {
            println!("      rs1: x{} = 0x{:08x}", rs1.reg, rs1.value);
        }
        if let Some(rs2) = &trace.rs2 {
            println!("      rs2: x{} = 0x{:08x}", rs2.reg, rs2.value);
        }
        if let Some(imm) = &trace.immediate {
            println!("      immediate: {}", imm);
        }
    }
    println!();

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
fn test_vcd_generation() {
    use std::fs;
    use std::path::PathBuf;

    init_test_logger();

    println!("\n========================================");
    println!("VCD WAVEFORM GENERATION TEST");
    println!("========================================");
    println!("Testing VCD file creation and basic validation...\n");

    let elf_path = test_program_path("simple_test.elf");

    // Create a temporary VCD file path in the target directory
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let vcd_path = manifest_dir.join("../target/test_vcd_output.vcd");
    let vcd_path_str = vcd_path.to_str().expect("VCD path should be valid UTF-8");

    // Remove any existing VCD file from previous test runs
    if vcd_path.exists() {
        fs::remove_file(&vcd_path).expect("Should be able to remove old VCD file");
    }

    println!("Running simulation with VCD enabled...");
    let result = run_program(
        500,
        false, // print_inst_trace
        false, // print_fsm_state
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        Some(vcd_path_str),
        0, // mem_latency_cycles
        |sim| load_elf(sim, &elf_path).map_err(|e| e.to_string()),
        |_sim, _result| {},
    )
    .expect("Simulation with VCD should succeed");

    assert_tohost(&result, 0x2a, "VCD generation test");

    // Verify VCD file was created
    assert!(
        vcd_path.exists(),
        "VCD file should be created at {}",
        vcd_path_str
    );

    // Read and validate VCD file contents
    let vcd_contents = fs::read_to_string(&vcd_path).expect("Should be able to read VCD file");

    println!(
        "VCD file created: {} ({} bytes)",
        vcd_path_str,
        vcd_contents.len()
    );

    // Validate VCD file has proper header
    assert!(
        vcd_contents.contains("$version"),
        "VCD file should contain version header"
    );
    assert!(
        vcd_contents.contains("$timescale"),
        "VCD file should contain timescale"
    );
    assert!(
        vcd_contents.contains("$scope module"),
        "VCD file should contain module scope"
    );

    // Validate essential CPU signals are present
    let expected_signals = vec![
        "clk",
        "rst_n",
        "boot_addr",
        "imem_addr",
        "imem_data",
        "dmem_addr",
        "dmem_wdata",
        "dmem_rdata",
        "dmem_we",
        "dmem_re",
    ];

    for signal in &expected_signals {
        assert!(
            vcd_contents.contains(signal),
            "VCD file should contain signal '{}'",
            signal
        );
    }

    // Validate timestamps are present
    assert!(
        vcd_contents.contains("#0"),
        "VCD file should contain timestamp #0 (reset state)"
    );
    assert!(
        vcd_contents.contains("#1"),
        "VCD file should contain timestamp #1 (first cycle)"
    );

    // Count number of timestamps to verify multiple cycles were captured
    let timestamp_count = vcd_contents.matches("\n#").count();
    println!("VCD file contains {} timestamps", timestamp_count);
    assert!(
        timestamp_count >= result.cycles as usize,
        "VCD should have at least {} timestamps for {} cycles",
        result.cycles,
        result.cycles
    );

    println!("✓ VCD file created successfully");
    println!("✓ VCD file contains proper headers and structure");
    println!("✓ All essential CPU signals are present");
    println!(
        "✓ Reset sequence and {} execution cycles captured",
        result.cycles
    );
    println!("\n========================================");
    println!("VCD GENERATION TEST COMPLETE ✓");
    println!("========================================\n");

    // Clean up test VCD file
    fs::remove_file(&vcd_path).expect("Should be able to remove test VCD file");
}

#[test]
fn test_memory_dump() {
    init_test_logger();

    println!("\n========================================");
    println!("MEMORY DUMP TEST");
    println!("========================================");

    let elf_path = test_program_path("test_memory_pattern.elf");

    // Run simulation and access memory in callback
    let result = run_program(
        10000,
        false, // print_inst_trace
        false, // print_fsm_state
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        None, // vcd_path
        0,    // mem_latency_cycles
        |sim| load_elf(sim, &elf_path).map_err(|e| e.to_string()),
        |sim, result| {
            assert_tohost(result, 42, "memory pattern test");
            println!("✓ Program executed successfully");

            // Dump memory region where test pattern was written
            const TEST_MEMORY_BASE: u32 = 0x8000_1000;
            const TEST_PATTERN_SIZE: u32 = 256;

            let memory_data: Vec<u8> = sim
                .dump_memory_region(TEST_MEMORY_BASE, TEST_PATTERN_SIZE)
                .collect();

            // Verify we got the expected amount of data
            assert_eq!(
                memory_data.len(),
                TEST_PATTERN_SIZE as usize,
                "Should dump exactly {} bytes",
                TEST_PATTERN_SIZE
            );

            // Verify magic bytes at the start (0xDE, 0xAD, 0xBE, 0xEF)
            assert_eq!(memory_data[0], 0xDE, "First magic byte should be 0xDE");
            assert_eq!(memory_data[1], 0xAD, "Second magic byte should be 0xAD");
            assert_eq!(memory_data[2], 0xBE, "Third magic byte should be 0xBE");
            assert_eq!(memory_data[3], 0xEF, "Fourth magic byte should be 0xEF");

            println!("✓ Magic bytes verified: 0xDEADBEEF");

            // Verify the pattern for a few more bytes (offset modulo 256)
            #[allow(clippy::needless_range_loop)]
            for i in 4..20 {
                assert_eq!(
                    memory_data[i], i as u8,
                    "Byte at offset {} should match pattern",
                    i
                );
            }

            println!("✓ Memory pattern verified");

            // Test writing the memory dump to a file
            let dump_path = "/tmp/test_memory_dump.bin";
            std::fs::write(dump_path, &memory_data).expect("Should write memory dump to file");
            println!("✓ Memory dump written to: {}", dump_path);

            // Verify file was written correctly
            let read_back = std::fs::read(dump_path).expect("Should read back memory dump");
            assert_eq!(read_back, memory_data, "Read-back data should match");

            // Clean up test file
            std::fs::remove_file(dump_path).expect("Should be able to remove test file");
        },
    )
    .expect("Simulation should succeed");

    // Verify result outside callback - tohost check already done in callback
    println!("✓ Test completed in {} cycles", result.cycles);

    println!("\n========================================");
    println!("✓ MEMORY DUMP TEST PASSED");
    println!("========================================");
}

#[test]
fn test_image_dump() {
    init_test_logger();

    println!("\n========================================");
    println!("IMAGE DUMP TEST");
    println!("========================================");

    let elf_path = test_program_path("test_image_data.elf");

    // Run simulation and access memory in callback
    let result = run_program(
        10000,
        false, // print_inst_trace
        false, // print_fsm_state
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        None, // vcd_path
        0,    // mem_latency_cycles
        |sim| load_elf(sim, &elf_path).map_err(|e| e.to_string()),
        |sim, result| {
            assert_tohost(result, 42, "image data test");
            println!("✓ Program executed successfully");

            // Dump memory region as an image
            const TEST_IMAGE_BASE: u32 = 0x8000_2000;
            const IMAGE_WIDTH: u32 = 4;
            const IMAGE_HEIGHT: u32 = 4;

            let image_path = "/tmp/test_image.png";
            sim.dump_memory_region_as_image(TEST_IMAGE_BASE, IMAGE_WIDTH, IMAGE_HEIGHT, image_path)
                .expect("Should dump image successfully");

            println!("✓ Image dumped to: {}", image_path);

            // Verify the image file was created
            assert!(
                std::path::Path::new(image_path).exists(),
                "Image file should exist"
            );

            // Verify the image can be loaded and has correct dimensions
            use image::GenericImageView;
            let img = image::open(image_path).expect("Should load dumped image");
            assert_eq!(img.width(), IMAGE_WIDTH, "Image width should match");
            assert_eq!(img.height(), IMAGE_HEIGHT, "Image height should match");

            println!(
                "✓ Image dimensions verified: {}x{}",
                img.width(),
                img.height()
            );

            // Verify some pixel values to ensure data integrity
            // Row 0, Pixel 0: Red (255, 0, 0, 255)
            let pixel = img.get_pixel(0, 0);
            assert_eq!(
                pixel[0], 255,
                "Pixel (0,0) red channel should be 255 (bright red)"
            );
            assert_eq!(pixel[1], 0, "Pixel (0,0) green channel should be 0");
            assert_eq!(pixel[2], 0, "Pixel (0,0) blue channel should be 0");
            assert_eq!(pixel[3], 255, "Pixel (0,0) alpha channel should be 255");

            // Row 1, Pixel 0: Green (0, 255, 0, 255)
            let pixel = img.get_pixel(0, 1);
            assert_eq!(pixel[0], 0, "Pixel (0,1) red channel should be 0");
            assert_eq!(
                pixel[1], 255,
                "Pixel (0,1) green channel should be 255 (bright green)"
            );
            assert_eq!(pixel[2], 0, "Pixel (0,1) blue channel should be 0");
            assert_eq!(pixel[3], 255, "Pixel (0,1) alpha channel should be 255");

            // Row 2, Pixel 0: Blue (0, 0, 255, 255)
            let pixel = img.get_pixel(0, 2);
            assert_eq!(pixel[0], 0, "Pixel (0,2) red channel should be 0");
            assert_eq!(pixel[1], 0, "Pixel (0,2) green channel should be 0");
            assert_eq!(
                pixel[2], 255,
                "Pixel (0,2) blue channel should be 255 (bright blue)"
            );
            assert_eq!(pixel[3], 255, "Pixel (0,2) alpha channel should be 255");

            // Row 3, Pixel 0: White (255, 255, 255, 255)
            let pixel = img.get_pixel(0, 3);
            assert_eq!(
                pixel[0], 255,
                "Pixel (0,3) red channel should be 255 (white)"
            );
            assert_eq!(
                pixel[1], 255,
                "Pixel (0,3) green channel should be 255 (white)"
            );
            assert_eq!(
                pixel[2], 255,
                "Pixel (0,3) blue channel should be 255 (white)"
            );
            assert_eq!(pixel[3], 255, "Pixel (0,3) alpha channel should be 255");

            println!("✓ Pixel data verified (red, green, blue, white gradients)");

            // Clean up test file
            std::fs::remove_file(image_path).expect("Should be able to remove test file");
        },
    )
    .expect("Simulation should succeed");

    // Verify result outside callback - tohost check already done in callback
    println!("✓ Test completed in {} cycles", result.cycles);

    println!("\n========================================");
    println!("✓ IMAGE DUMP TEST PASSED");
    println!("========================================");
}

#[test]
fn test_panic_handler() {
    init_test_logger();

    println!("\n========================================");
    println!("PANIC HANDLER TEST");
    println!("========================================");

    let elf_path = test_program_path("test_panic.elf");

    // Run the panic test program with sufficient cycles
    let result = run_elf(&elf_path, 5000, false).expect("Simulation should succeed");

    // Verify that the panic handler wrote 0xDEAD to tohost
    assert_eq!(
        result.tohost_value,
        Some(0xDEAD),
        "Panic handler should write 0xDEAD to tohost (got {:x?})",
        result.tohost_value
    );

    println!("✓ Panic handler correctly signaled with 0xDEAD");
    println!("✓ Test completed in {} cycles", result.cycles);
    println!("\n========================================");
    println!("✓ PANIC HANDLER TEST PASSED");
    println!("========================================");
}

// ============================================================================
// Hung State Detection Integration Tests
// ============================================================================

#[test]
fn test_hung_detection_with_elf_auto_range() {
    init_test_logger();

    println!("\n========================================");
    println!("HUNG DETECTION: ELF AUTO-RANGE TEST");
    println!("========================================");

    // Test that run_elf automatically sets valid PC range from ELF
    // and hung detection works correctly with it
    let elf_path = test_program_path("test.elf");

    // This should succeed with hung detection enabled and auto PC range
    let result = run_elf(&elf_path, 500, false);

    assert!(
        result.is_ok(),
        "Should successfully run ELF with auto-detected PC range: {:?}",
        result.err()
    );

    println!("✓ Valid PC range automatically detected from ELF");
    println!("✓ Hung detection enabled by default");
    println!(
        "✓ Simulation completed in {} cycles",
        result.unwrap().cycles
    );
    println!("\n========================================");
    println!("✓ HUNG DETECTION ELF AUTO-RANGE TEST PASSED");
    println!("========================================");
}

// Tests that verify hung states ARE detected (not just false positives)
#[test]
fn test_hung_detection_catches_infinite_loop() {
    init_test_logger();

    println!("\n========================================");
    println!("HUNG DETECTION: INFINITE LOOP DETECTION");
    println!("========================================");

    // Use run_program to create a simple infinite loop programmatically
    use riscv_core::instruction::jal;

    // Create an infinite loop: JAL x0, 0 (jump to self)
    let infinite_loop_instr = jal(0, 0);
    let start_addr = 0x8000_0000;
    let program_bytes: Vec<u8> = [infinite_loop_instr]
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    let result = run_program(
        10000, // max_cycles
        false, // Don't print instruction trace
        false, // Don't print FSM state
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        None, // No VCD
        0,    // Zero latency
        |sim| {
            sim.write_memory_region(start_addr, &program_bytes, true);
            Ok(start_addr)
        },
        |_sim, _result| {},
    );

    // Should get an error about PC stuck
    assert!(result.is_err(), "Should detect infinite loop");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("PC stuck") || err_msg.contains("Hung state"),
        "Error should mention PC stuck or hung state, got: {}",
        err_msg
    );

    println!("✓ Successfully detected infinite loop");
    println!("✓ Error message: {}", err_msg);
    println!("\n========================================");
    println!("✓ HUNG DETECTION INFINITE LOOP TEST PASSED");
    println!("========================================");
}

#[test]
fn test_hung_detection_catches_out_of_bounds_pc() {
    init_test_logger();

    println!("\n========================================");
    println!("HUNG DETECTION: OUT OF BOUNDS PC");
    println!("========================================");

    use riscv_core::instruction::jal;

    // Create a jump that goes outside the loaded program
    // We'll load a single instruction and jump far beyond it
    let start_addr = 0x8000_0000;

    // Jump forward by 0x10000 bytes (64KB), which is way outside our 4-byte program
    let jump_instr = jal(0, 0x10000);
    let program_bytes: Vec<u8> = [jump_instr]
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    // write_memory_region will set valid PC range to [start_addr, start_addr + 4)
    // The jump will go to start_addr + 0x10000, which is outside this range
    let result = run_program(
        10000,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        None,
        0,
        |sim| {
            sim.write_memory_region(start_addr, &program_bytes, true);
            Ok(start_addr)
        },
        |_sim, _result| {},
    );

    // Should get an error about PC out of bounds
    assert!(result.is_err(), "Should detect PC out of bounds");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("outside valid")
            || err_msg.contains("PcOutOfBounds")
            || err_msg.contains("Hung state"),
        "Error should mention PC out of bounds, got: {}",
        err_msg
    );

    println!("✓ Successfully detected out-of-bounds PC jump");
    println!("✓ Error message: {}", err_msg);
    println!("\n========================================");
    println!("✓ HUNG DETECTION OUT OF BOUNDS TEST PASSED");
    println!("========================================");
}

#[test]
fn test_hung_detection_catches_long_instruction() {
    init_test_logger();

    println!("\n========================================");
    println!("HUNG DETECTION: LONG INSTRUCTION DETECTION");
    println!("========================================");

    // Use memory latency to make an instruction take too many cycles
    // We'll use a load instruction that will access memory with high latency
    use riscv_core::instruction::lw;

    let start_addr = 0x8000_0000;

    // Program:
    // 1. ADDI x2, x0, <low 12 bits of data_addr>   - Load low part of address into x2
    // 2. LUI x2, <high 20 bits of data_addr>        - Would be needed for full address, but we'll use a simpler approach
    // Actually, let's just use LW with offset from x0 which is always 0
    // We'll place data at a small offset that fits in 12-bit immediate

    // Simpler approach: Use data at address that fits in 12-bit offset from x0
    let simple_data_addr = 0x100u32; // Small address that fits in LW immediate

    // LW x1, 0x100(x0) - load word from address 0x100 into x1
    let load_instr = lw(1, 0, simple_data_addr as i32);
    let program_bytes: Vec<u8> = [load_instr]
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    // Set memory latency to exceed max_cycles_per_instruction (default 10000)
    // This will cause the load instruction to take too long
    let mem_latency_cycles = 15000;

    let result = run_program(
        100000, // High max_cycles so we don't hit that limit first
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        None,
        mem_latency_cycles, // Set memory latency high enough to trigger long instruction detection
        |sim| {
            sim.write_memory_region(start_addr, &program_bytes, true);

            // Write data at simple_data_addr
            let data: Vec<u8> = vec![0x12, 0x34, 0x56, 0x78];
            sim.write_memory_region(simple_data_addr, &data, false);

            Ok(start_addr)
        },
        |_sim, _result| {},
    );

    // Should get an error about instruction taking too long
    assert!(result.is_err(), "Should detect long instruction");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("LongInstruction")
            || err_msg.contains("taken") && err_msg.contains("cycles"),
        "Error should mention long instruction, got: {}",
        err_msg
    );

    println!("✓ Successfully detected instruction taking too many cycles");
    println!("✓ Error message: {}", err_msg);
    println!("\n========================================");
    println!("✓ HUNG DETECTION LONG INSTRUCTION TEST PASSED");
    println!("========================================");
}

#[test]
fn test_atomic_operations() {
    init_test_logger();

    println!("\n========================================");
    println!("ATOMIC OPERATIONS TEST (RV32A)");
    println!("========================================\n");

    // Test 1: Simple atomic operations (AMOADD, AMOSWAP)
    println!("Running test_atomic_simple.elf...");
    let simple_path = test_program_path("test_atomic_simple.elf");
    let result =
        run_elf(&simple_path, 1000, false).expect("test_atomic_simple simulation should succeed");
    assert_tohost(&result, 0x2a, "test_atomic_simple");
    println!("✓ test_atomic_simple passed in {} cycles", result.cycles);

    // Test 2: Comprehensive atomic operations (all AMO ops, LR/SC, compare_exchange)
    println!("\nRunning test_atomic.elf...");
    let full_path = test_program_path("test_atomic.elf");
    let result = run_elf(&full_path, 100000, false).expect("test_atomic simulation should succeed");
    assert_tohost(&result, 0x2a, "test_atomic");
    println!("✓ test_atomic passed in {} cycles", result.cycles);

    println!("\n✓ All atomic operations tested:");
    println!("  - AMOADD.W, AMOSWAP.W");
    println!("  - AMOAND.W, AMOOR.W, AMOXOR.W");
    println!("  - AMOMIN.W, AMOMAX.W, AMOMINU.W, AMOMAXU.W");
    println!("  - LR.W, SC.W (for compare_exchange)");
    println!("\n========================================");
    println!("✓ ATOMIC OPERATIONS TEST PASSED");
    println!("========================================");
}

#[test]
fn test_packet_protocol_end_to_end() {
    use riscv_protocol::*;

    init_test_logger();

    println!("\n========================================");
    println!("PACKET PROTOCOL END-TO-END TEST");
    println!("========================================");
    println!("Testing bidirectional CPU↔Host packet communication...\n");

    let elf_path = test_program_path("packet_test.elf");

    // Shared state for collecting FIFO TX data
    let fifo_tx_data = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let fifo_tx_data_clone = fifo_tx_data.clone();

    // Track whether we've sent the test packets yet
    let packets_sent = std::sync::Arc::new(std::sync::Mutex::new(false));
    let packets_sent_clone = packets_sent.clone();

    // Callback that handles bidirectional packet communication
    let inst_complete_callback = move |view: &mut SimulatorView| {
        // Collect all TX data
        while let Some(word) = view.fifo_read_tx() {
            fifo_tx_data_clone.lock().unwrap().push(word);
        }

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
            view.send_packet_to_rx(&echo_request)
                .expect("Failed to send Echo packet to CPU");
            println!("\nStep 2: Sent Echo packet (seq=100) to CPU");

            // Send DataU32 packet (value=1000)
            let data_request = DataU32Packet {
                header: PacketHeader::new(PacketType::DataU32, 0),
                value: 1000,
                tag: 55,
            };
            view.send_packet_to_rx(&data_request)
                .expect("Failed to send DataU32 packet to CPU");
            println!("Step 3: Sent DataU32 packet (value=1000) to CPU");

            *sent = true;
        }
    };

    // Run the simulation
    let result = run_program(
        50000,
        false, // print_inst_trace
        false, // print_fsm_state
        Some(inst_complete_callback),
        None::<fn(&InstructionTrace)>,
        None, // vcd_path
        0,    // mem_latency_cycles
        |sim| load_elf(sim, &elf_path).map_err(|e| e.to_string()),
        |_sim, _result| {},
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

    let elf_path = test_program_path("println_test.elf");

    // Create a callback to collect FIFO data from CPU
    let fifo_data = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let fifo_data_clone = fifo_data.clone();
    let inst_complete_callback = move |view: &mut SimulatorView| {
        // Drain all words from TX FIFO and collect them
        while let Some(word) = view.fifo_read_tx() {
            fifo_data_clone.lock().unwrap().push(word);
        }
    };

    // Run the simulation with inst_complete callback
    let result = run_program(
        25000,
        true,  // print_inst_trace
        false, // print_fsm_state
        Some(inst_complete_callback),
        None::<fn(&InstructionTrace)>,
        None, // vcd_path
        0,    // mem_latency_cycles
        |sim| {
            // No pre-configuration needed, just load ELF
            load_elf(sim, &elf_path).map_err(|e| e.to_string())
        },
        |_sim, _result| {},
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
        ("Hello from RISC-V CPU!\n", riscv_protocol::DebugLevel::Info),
        ("The answer is 42\n", riscv_protocol::DebugLevel::Info),
        ("Testing println macro\n", riscv_protocol::DebugLevel::Info),
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
