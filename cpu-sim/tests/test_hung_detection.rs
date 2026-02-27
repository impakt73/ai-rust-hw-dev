mod common;

use common::{init_test_logger, run_program, write_memory_region};
use cpu_sim::*;

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
        GLOBAL_MAX_CYCLES, // max_cycles
        false,             // Don't print instruction trace
        false,             // Don't print FSM state
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        None, // No VCD
        0,    // Zero latency
        |sim| {
            write_memory_region(sim, start_addr, &program_bytes);
            Ok(start_addr)
        },
        None::<fn(&cpu_sim::SimulationResult)>,
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

    // Simpler approach: Use data at address within DRAM range
    // Use address 0x80000100 (DRAM base + 0x100)
    let data_addr = 0x80000100u32;

    // LUI x2, 0x80000 (load upper bits of DRAM base)
    // LW x1, 0x100(x2) - load word from address 0x80000100 into x1
    let lui_instr = riscv_core::instruction::lui(2, DRAM_BASE);
    let load_instr = lw(1, 2, 0x100);
    let program_bytes: Vec<u8> = [lui_instr, load_instr]
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    // Set memory latency to exceed max_cycles_per_instruction (default 10000)
    // This will cause the load instruction to take too long
    let mem_latency_cycles = 15000;

    let result = run_program(
        100_000, // High max_cycles so we don't hit that limit first
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&InstructionTrace)>,
        None,
        mem_latency_cycles, // Set memory latency high enough to trigger long instruction detection
        |sim| {
            write_memory_region(sim, start_addr, &program_bytes);

            // Write data at data_addr (0x80000100)
            let data: Vec<u8> = vec![0x12, 0x34, 0x56, 0x78];
            write_memory_region(sim, data_addr, &data);

            Ok(start_addr)
        },
        None::<fn(&cpu_sim::SimulationResult)>,
    );

    // Should get an error about instruction taking too long
    assert!(result.is_err(), "Should detect long instruction");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("LongInstruction")
            || (err_msg.contains("taken") && err_msg.contains("cycles")),
        "Error should mention long instruction, got: {}",
        err_msg
    );

    println!("✓ Successfully detected instruction taking too many cycles");
    println!("✓ Error message: {}", err_msg);
    println!("\n========================================");
    println!("✓ HUNG DETECTION LONG INSTRUCTION TEST PASSED");
    println!("========================================");
}
