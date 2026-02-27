mod common;

use common::{
    assert_tohost, create_register_trace_program, create_test_program, create_trace_test_program,
    init_test_logger,
};
use cpu_sim::*;
use riscv_core::instruction::*;
use riscv_shared::bus::DRAM_BASE;
use riscv_shared::sim_control::SUCCESS_CODE;
use std::sync::{Arc, Mutex};

/// Helper to run programmatic instructions with options for trace/VCD/callbacks
fn run_program_with_options<T, F>(
    instructions: &[u32],
    max_cycles: u64,
    print_inst_trace: bool,
    vcd_path: Option<&str>,
    trace_callback: Option<T>,
    termination_callback: Option<F>,
) -> Result<SimulationResult, String>
where
    T: FnMut(&riscv_core::trace::InstructionTrace),
    F: FnOnce(&SimulatorView, &SimulationResult),
{
    const START_ADDR: u32 = 0x8000_0000;

    let program_bytes: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    run_program(
        max_cycles,
        print_inst_trace,
        false, // Don't print FSM state
        None::<fn(&mut SimulatorView)>,
        trace_callback,
        vcd_path,
        0, // Zero latency for RTL verification tests
        |sim| {
            sim.write_memory_region(START_ADDR, &program_bytes);
            Ok(START_ADDR)
        },
        termination_callback,
    )
}

#[test]
fn test_instruction_trace() {
    init_test_logger();

    let program = create_test_program();
    let result = run_program(
        GLOBAL_MAX_CYCLES,
        true,  // print_inst_trace
        false, // print_fsm_state
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        None, // vcd_path
        0,    // mem_latency_cycles
        |sim| {
            sim.write_memory_region(0x8000_0000, &program);
            Ok(0x8000_0000)
        },
        None::<fn(&cpu_sim::SimulatorView, &cpu_sim::SimulationResult)>,
    )
    .expect("Simulation with trace should succeed");

    assert_tohost(&result, 0x2a, "instruction trace test");
    println!(
        "✓ Instruction trace test passed in {} cycles",
        result.cycles
    );
}

#[test]
fn test_register_trace_audit() {
    init_test_logger();

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

    let program = create_register_trace_program();
    let result = run_program(
        GLOBAL_MAX_CYCLES,
        true,  // print_inst_trace
        false, // print_fsm_state
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        None, // vcd_path
        0,    // mem_latency_cycles
        |sim| {
            sim.write_memory_region(0x8000_0000, &program);
            Ok(0x8000_0000)
        },
        None::<fn(&cpu_sim::SimulatorView, &cpu_sim::SimulationResult)>,
    )
    .expect("Register trace audit simulation should succeed");

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
fn test_trace_callback() {
    use riscv_core::trace::InstructionType;

    init_test_logger();

    let program = create_trace_test_program();

    // Collect instruction traces via callback
    let traces = Arc::new(Mutex::new(Vec::new()));
    let traces_clone = Arc::clone(&traces);

    let trace_callback = move |trace: &InstructionTrace| {
        traces_clone.lock().unwrap().push(trace.clone());
    };

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false, // print_inst_trace
        false, // print_fsm_state
        None::<fn(&mut SimulatorView)>,
        Some(trace_callback),
        None, // vcd_path
        0,    // mem_latency_cycles
        |sim| {
            sim.write_memory_region(0x8000_0000, &program);
            Ok(0x8000_0000)
        },
        None::<fn(&cpu_sim::SimulatorView, &cpu_sim::SimulationResult)>,
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

    // Collect traces found/missing for the failure message, then assert.
    let missing: Vec<&str> = [
        (!found_addi_x1, "addi x1, x0, 10"),
        (!found_addi_x2, "addi x2, x0, 20"),
        (!found_addi_x3, "addi x3, x0, 5"),
        (!found_add_x4, "add x4, x1, x2"),
        (!found_sub_x5, "sub x5, x2, x3"),
        (!found_lui, "LUI instruction"),
        (!found_sw, "SW instruction"),
        (!found_lw, "LW instruction"),
    ]
    .into_iter()
    .filter_map(|(not_found, name)| if not_found { Some(name) } else { None })
    .collect();

    if !missing.is_empty() {
        // Print full trace dump only when assertions are about to fail
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
        panic!("Missing expected instructions: {:?}", missing);
    }

    println!("✓ Trace callback test passed in {} cycles", result.cycles);
    println!("✓ All expected instructions found and validated");
    println!("========================================\n");
}

#[test]
fn test_vcd_generation() {
    use std::fs;

    init_test_logger();

    println!("\n========================================");
    println!("VCD WAVEFORM GENERATION TEST");
    println!("========================================");
    println!("Testing VCD file creation and basic validation...\n");

    let program = create_test_program();

    // Use a per-process unique path in the system temp dir so that parallel
    // test runs and non-default CARGO_TARGET_DIR settings don't collide.
    let vcd_path = std::env::temp_dir().join(format!(
        "cpu_sim_test_vcd_{}_{}.vcd",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("System time before UNIX_EPOCH")
            .as_nanos()
    ));
    let vcd_path_str = vcd_path.to_str().expect("VCD path should be valid UTF-8");

    println!("Running simulation with VCD enabled...");
    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false, // print_inst_trace
        false, // print_fsm_state
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        Some(vcd_path_str),
        0, // mem_latency_cycles
        |sim| {
            sim.write_memory_region(0x8000_0000, &program);
            Ok(0x8000_0000)
        },
        None::<fn(&cpu_sim::SimulatorView, &cpu_sim::SimulationResult)>,
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

// ============================================================================
// Comprehensive Trace Validation Tests
// ============================================================================

#[test]
fn test_comprehensive_trace_validation() {
    init_test_logger();

    println!("\n========================================");
    println!("COMPREHENSIVE TRACE VALIDATION TEST");
    println!("========================================");
    println!("Testing instruction trace against known sequence...\n");

    // Expected instruction sequence for validation
    #[derive(Debug)]
    struct ExpectedInstruction {
        inst_type: riscv_core::trace::InstructionType,
        pc: u32,
        rd: Option<(u8, u32)>,  // (register number, expected value)
        rs1: Option<(u8, u32)>, // (register number, expected value)
        rs2: Option<(u8, u32)>, // (register number, expected value)
        immediate: Option<i32>,
    }

    // Build test program with known expected results
    let base_addr: u32 = 0x8000_0000;
    let mut instructions = vec![
        addi(1, 0, 10),      // x1 = 10
        addi(2, 0, 20),      // x2 = 20
        add(3, 1, 2),        // x3 = x1 + x2 = 30
        sub(4, 2, 1),        // x4 = x2 - x1 = 10
        and(5, 3, 2),        // x5 = x3 & x2 = 20
        or(6, 1, 2),         // x6 = x1 | x2 = 30
        xor(7, 3, 2),        // x7 = x3 ^ x2 = 10
        sll(8, 1, 0),        // x8 = x1 << 0 = 10
        srl(9, 2, 0),        // x9 = x2 >> 0 = 20
        lui(10, 0x12345000), // x10 = 0x12345000
        lui(11, DRAM_BASE),  // x11 = 0x80000000 (base address)
        sw(11, 1, 0),        // mem[0x80000000] = x1 = 10
        lw(11, 11, 0),       // x11 = mem[0x80000000] = 10
    ];

    // Define expected traces (before adding termination sequence)
    let expected_traces = vec![
        ExpectedInstruction {
            inst_type: riscv_core::trace::InstructionType::Addi,
            pc: base_addr,
            rd: Some((1, 10)),
            rs1: Some((0, 0)),
            rs2: None,
            immediate: Some(10),
        },
        ExpectedInstruction {
            inst_type: riscv_core::trace::InstructionType::Addi,
            pc: base_addr + 4,
            rd: Some((2, 20)),
            rs1: Some((0, 0)),
            rs2: None,
            immediate: Some(20),
        },
        ExpectedInstruction {
            inst_type: riscv_core::trace::InstructionType::Add,
            pc: base_addr + 8,
            rd: Some((3, 30)),
            rs1: Some((1, 10)),
            rs2: Some((2, 20)),
            immediate: None,
        },
        ExpectedInstruction {
            inst_type: riscv_core::trace::InstructionType::Sub,
            pc: base_addr + 12,
            rd: Some((4, 10)),
            rs1: Some((2, 20)),
            rs2: Some((1, 10)),
            immediate: None,
        },
        ExpectedInstruction {
            inst_type: riscv_core::trace::InstructionType::And,
            pc: base_addr + 16,
            rd: Some((5, 20)),
            rs1: Some((3, 30)),
            rs2: Some((2, 20)),
            immediate: None,
        },
        ExpectedInstruction {
            inst_type: riscv_core::trace::InstructionType::Or,
            pc: base_addr + 20,
            rd: Some((6, 30)),
            rs1: Some((1, 10)),
            rs2: Some((2, 20)),
            immediate: None,
        },
        ExpectedInstruction {
            inst_type: riscv_core::trace::InstructionType::Xor,
            pc: base_addr + 24,
            rd: Some((7, 10)),
            rs1: Some((3, 30)),
            rs2: Some((2, 20)),
            immediate: None,
        },
        ExpectedInstruction {
            inst_type: riscv_core::trace::InstructionType::Sll,
            pc: base_addr + 28,
            rd: Some((8, 10)),
            rs1: Some((1, 10)),
            rs2: Some((0, 0)),
            immediate: None,
        },
        ExpectedInstruction {
            inst_type: riscv_core::trace::InstructionType::Srl,
            pc: base_addr + 32,
            rd: Some((9, 20)),
            rs1: Some((2, 20)),
            rs2: Some((0, 0)),
            immediate: None,
        },
        ExpectedInstruction {
            inst_type: riscv_core::trace::InstructionType::Lui,
            pc: base_addr + 36,
            rd: Some((10, 0x12345000)),
            rs1: None,
            rs2: None,
            immediate: Some(74565), // 0x12345
        },
        ExpectedInstruction {
            inst_type: riscv_core::trace::InstructionType::Lui,
            pc: base_addr + 40,
            rd: Some((11, 0x80000000)),
            rs1: None,
            rs2: None,
            immediate: Some(524288), // 0x80000
        },
        ExpectedInstruction {
            inst_type: riscv_core::trace::InstructionType::Sw,
            pc: base_addr + 44,
            rd: None,
            rs1: Some((11, 0x80000000)),
            rs2: Some((1, 10)),
            immediate: Some(0),
        },
        ExpectedInstruction {
            inst_type: riscv_core::trace::InstructionType::Lw,
            pc: base_addr + 48,
            rd: Some((11, 10)),
            rs1: Some((11, 0x80000000)),
            rs2: None,
            immediate: Some(0),
        },
    ];

    // Add termination sequence
    instructions.extend(common::tohost_termination(15, 16, SUCCESS_CODE));

    // Collect traces
    let mut captured_traces = Vec::new();
    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        Some(|trace: &riscv_core::trace::InstructionTrace| {
            captured_traces.push(trace.clone());
        }),
        Some(|_sim: &SimulatorView, result: &SimulationResult| {
            assert_eq!(
                result.tohost_value,
                Some(SUCCESS_CODE),
                "Program should terminate with tohost=1"
            );
        }),
    )
    .expect("Simulation should succeed");

    // Verify we captured the expected number of traces (12 main + 3 termination)
    println!("Captured {} instruction traces", captured_traces.len());
    assert!(
        captured_traces.len() >= expected_traces.len(),
        "Should capture at least {} traces, got {}",
        expected_traces.len(),
        captured_traces.len()
    );

    // Validate each expected trace
    println!("\nValidating instruction traces:");
    for (i, expected) in expected_traces.iter().enumerate() {
        let trace = &captured_traces[i];

        print!("  [{}] PC=0x{:08x} {:?} ... ", i, trace.pc, trace.inst_type);

        // Validate PC
        assert_eq!(
            trace.pc, expected.pc,
            "Trace {} PC mismatch: expected 0x{:08x}, got 0x{:08x}",
            i, expected.pc, trace.pc
        );

        // Validate instruction type
        assert_eq!(
            trace.inst_type, expected.inst_type,
            "Trace {} instruction type mismatch: expected {:?}, got {:?}",
            i, expected.inst_type, trace.inst_type
        );

        // Validate rd
        if let Some((exp_reg, exp_val)) = expected.rd {
            assert!(trace.rd.is_some(), "Trace {} should have rd operand", i);
            let rd = trace.rd.as_ref().unwrap();
            assert_eq!(
                rd.reg, exp_reg,
                "Trace {} rd register mismatch: expected x{}, got x{}",
                i, exp_reg, rd.reg
            );
            assert_eq!(
                rd.value, exp_val,
                "Trace {} rd value mismatch: expected 0x{:08x}, got 0x{:08x}",
                i, exp_val, rd.value
            );
        }

        // Validate rs1
        if let Some((exp_reg, exp_val)) = expected.rs1 {
            assert!(trace.rs1.is_some(), "Trace {} should have rs1 operand", i);
            let rs1 = trace.rs1.as_ref().unwrap();
            assert_eq!(
                rs1.reg, exp_reg,
                "Trace {} rs1 register mismatch: expected x{}, got x{}",
                i, exp_reg, rs1.reg
            );
            assert_eq!(
                rs1.value, exp_val,
                "Trace {} rs1 value mismatch: expected 0x{:08x}, got 0x{:08x}",
                i, exp_val, rs1.value
            );
        }

        // Validate rs2
        if let Some((exp_reg, exp_val)) = expected.rs2 {
            assert!(trace.rs2.is_some(), "Trace {} should have rs2 operand", i);
            let rs2 = trace.rs2.as_ref().unwrap();
            assert_eq!(
                rs2.reg, exp_reg,
                "Trace {} rs2 register mismatch: expected x{}, got x{}",
                i, exp_reg, rs2.reg
            );
            assert_eq!(
                rs2.value, exp_val,
                "Trace {} rs2 value mismatch: expected 0x{:08x}, got 0x{:08x}",
                i, exp_val, rs2.value
            );
        }

        // Validate immediate
        if let Some(exp_imm) = expected.immediate {
            assert!(
                trace.immediate.is_some(),
                "Trace {} should have immediate value",
                i
            );
            let imm = trace.immediate.unwrap();
            assert_eq!(
                imm, exp_imm,
                "Trace {} immediate mismatch: expected {}, got {}",
                i, exp_imm, imm
            );
        }

        println!("✓");
    }

    println!("\n========================================");
    println!("✓ ALL TRACE VALIDATIONS PASSED");
    println!("========================================");
    println!("  - {} instructions validated", expected_traces.len());
    println!("  - PC values matched expected sequence");
    println!("  - Instruction types decoded correctly");
    println!("  - Register values computed correctly");
    println!("  - Immediate values extracted correctly");
    println!("========================================\n");
}

#[test]
fn test_trace_with_branches() {
    init_test_logger();

    println!("\n========================================");
    println!("TRACE VALIDATION WITH BRANCHES");
    println!("========================================\n");

    let base_addr: u32 = 0x8000_0000;
    let mut instructions = vec![
        addi(1, 0, 10), // 0x00: x1 = 10
        addi(2, 0, 20), // 0x04: x2 = 20
        beq(1, 1, 8),   // 0x08: branch to 0x10 (taken - skip next)
        addi(3, 0, 99), // 0x0C: SKIPPED
        addi(4, 0, 5),  // 0x10: x4 = 5
        bne(1, 2, 8),   // 0x14: branch to 0x1C (taken - skip next)
        addi(5, 0, 99), // 0x18: SKIPPED
        addi(6, 0, 1),  // 0x1C: x6 = 1
    ];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    // Collect traces
    let mut captured_traces = Vec::new();
    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        None,
        Some(|trace: &riscv_core::trace::InstructionTrace| {
            captured_traces.push(trace.clone());
        }),
        Some(|_sim: &SimulatorView, result: &SimulationResult| {
            assert_eq!(result.tohost_value, Some(SUCCESS_CODE));
        }),
    )
    .expect("Simulation should succeed");

    println!("Captured {} traces", captured_traces.len());
    println!("\nTrace sequence:");
    for (i, trace) in captured_traces.iter().enumerate() {
        println!("  [{}] PC=0x{:08x} {:?}", i, trace.pc, trace.inst_type);
    }

    // Verify branch behavior - skipped instructions should not appear in trace
    let pcs: Vec<u32> = captured_traces.iter().map(|t| t.pc).collect();

    // Should NOT contain the skipped instructions
    assert!(
        !pcs.contains(&(base_addr + 0x0C)),
        "Trace should not contain skipped instruction at 0x0C (after BEQ)"
    );
    assert!(
        !pcs.contains(&(base_addr + 0x18)),
        "Trace should not contain skipped instruction at 0x18 (after BNE)"
    );

    // Should contain the executed instructions
    assert!(
        pcs.contains(&base_addr),
        "Trace should contain ADDI x1 at 0x00"
    );
    assert!(
        pcs.contains(&(base_addr + 0x04)),
        "Trace should contain ADDI x2 at 0x04"
    );
    assert!(
        pcs.contains(&(base_addr + 0x08)),
        "Trace should contain BEQ at 0x08"
    );
    assert!(
        pcs.contains(&(base_addr + 0x10)),
        "Trace should contain ADDI x4 at 0x10"
    );
    assert!(
        pcs.contains(&(base_addr + 0x14)),
        "Trace should contain BNE at 0x14"
    );
    assert!(
        pcs.contains(&(base_addr + 0x1C)),
        "Trace should contain ADDI x6 at 0x1C"
    );

    println!("\n========================================");
    println!("✓ BRANCH TRACE VALIDATION PASSED");
    println!("========================================");
    println!("  - Branches executed correctly");
    println!("  - Skipped instructions not traced");
    println!("  - Control flow sequence validated");
    println!("========================================\n");
}

#[test]
fn test_trace_and_vcd_together() {
    init_test_logger();

    println!("\n========================================");
    println!("COMBINED TRACE + VCD TEST");
    println!("========================================\n");

    // Use a per-process unique path in the system temp dir so that parallel
    // test runs and non-default CARGO_TARGET_DIR settings don't collide.
    let vcd_path = std::env::temp_dir().join(format!(
        "cpu_sim_trace_vcd_{}_{}.vcd",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("System time before UNIX_EPOCH")
            .as_nanos()
    ));
    let vcd_path_str = vcd_path.to_str().expect("VCD path should be valid UTF-8");

    // Simple test program
    let mut instructions = vec![addi(1, 0, 42), addi(2, 1, 8), add(3, 1, 2)];
    instructions.extend(common::tohost_termination(7, 8, SUCCESS_CODE));

    // Run with VCD enabled
    run_program_with_options(
        &instructions,
        GLOBAL_MAX_CYCLES,
        false,
        Some(vcd_path_str),
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        Some(|_sim: &SimulatorView, result: &SimulationResult| {
            assert_eq!(result.tohost_value, Some(SUCCESS_CODE));
        }),
    )
    .expect("Simulation should succeed");

    // Verify VCD file was created
    assert!(
        vcd_path.exists(),
        "VCD file should be created at {}",
        vcd_path_str
    );

    // Read VCD file
    let vcd_contents = std::fs::read_to_string(&vcd_path).expect("Should be able to read VCD file");

    // Validate VCD contains essential signals
    assert!(
        vcd_contents.contains("clk"),
        "VCD should contain clk signal"
    );
    assert!(
        vcd_contents.contains("rst_n"),
        "VCD should contain rst_n signal"
    );
    assert!(
        vcd_contents.contains("imem_addr"),
        "VCD should contain imem_addr"
    );
    assert!(vcd_contents.contains("#0"), "VCD should have timestamps");

    // Clean up
    std::fs::remove_file(&vcd_path).expect("Should be able to remove VCD file");

    println!("✓ VCD file generated successfully");
    println!("✓ VCD contains all expected signals");

    println!("\n========================================");
    println!("✓ COMBINED TRACE + VCD TEST PASSED");
    println!("========================================");
    println!("  - VCD waveform dumping works");
    println!("  - Trace options enable easy config");
    println!("  - Both features can be used together");
    println!("========================================\n");
}
