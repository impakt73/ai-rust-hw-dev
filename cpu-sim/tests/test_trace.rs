mod common;

use common::{
    assert_tohost, create_register_trace_program, create_test_program, create_trace_test_program,
    init_test_logger,
};
use cpu_sim::*;
use std::sync::{Arc, Mutex};

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

    let elf_path = sim_tests::test_program_path("simple_test").expect("Failed to find simple_test");

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
    let result = run_elf(
        &elf_path,
        GLOBAL_MAX_CYCLES,
        false, // print_inst_trace
        false, // print_fsm_state
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        Some(vcd_path_str),
        0,                              // mem_latency_cycles
        None::<fn(&mut SimulatorView)>, // setup_callback
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
