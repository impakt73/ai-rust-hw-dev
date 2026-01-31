use cpu_sim::*;
use riscv_core::instruction::*;

/// Test that demonstrates writing programmatic instructions to memory without an ELF file
#[test]
fn test_programmatic_instruction_loading() {
    let _ = env_logger::builder().is_test(true).try_init();

    println!("\n=== PROGRAMMATIC INSTRUCTION LOADING TEST ===");
    println!("Demonstrating the new decoupled simulator API");

    // Define a simple program:
    // Address 0x80000000:
    //   addi x10, x0, 42       ; x10 = 42
    //   lui x11, 0x10000000    ; x11 = tohost address (0x10000000)
    //   sw x10, 0(x11)         ; store to tohost (halt)
    //   jal x0, 0              ; infinite loop (stay here)
    let instructions: Vec<u32> = vec![
        addi(10, 0, 42),     // addi x10, x0, 42
        lui(11, 0x10000000), // lui x11, 0x10000000
        sw(11, 10, 0),       // sw x10, 0(x11)
        jal(0, 0),           // jal x0, 0
    ];
    let program: Vec<u8> = instructions
        .iter()
        .flat_map(|inst| inst.to_le_bytes())
        .collect();

    const START_ADDR: u32 = 0x8000_0000;

    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0, // Zero latency
        |sim| {
            println!("✓ Simulator created without loading an ELF file");

            // Write the program to memory starting at 0x80000000
            sim.write_memory_region(START_ADDR, &program, true);

            println!(
                "✓ Programmatic instructions written to memory at 0x{:08x}",
                START_ADDR
            );
            println!("  Program size: {} bytes", program.len());
            println!("  Instruction 1: addi x10, x0, 42");
            println!("  Instruction 2: lui x11, 0x10000000 ; load tohost address");
            println!("  Instruction 3: sw x10, 0(x11) ; store to tohost");
            println!("  Instruction 4: jal x0, 0 ; infinite loop");

            Ok(START_ADDR)
        },
        None::<fn(&cpu_sim::SimulatorView, &cpu_sim::SimulationResult)>,
    )
    .expect("Simulation should succeed");

    println!("\n=== RESULTS ===");
    println!("✓ Simulation completed in {} cycles", result.cycles);
    println!(
        "✓ Tohost value: 0x{:08x} ({})",
        result.tohost_value.unwrap_or(0),
        result.tohost_value.unwrap_or(0)
    );

    // Verify the program executed correctly
    assert_eq!(
        result.tohost_value,
        Some(42),
        "Expected tohost value 42 from programmatic instructions"
    );
    // With serialized bus protocol, memory operations take more cycles
    assert!(
        result.cycles < 100,
        "Expected program to complete in less than 100 cycles, got {}",
        result.cycles
    );

    println!("\n========================================");
    println!("✓ PROGRAMMATIC LOADING TEST PASSED");
    println!("========================================");
    println!("Demonstrated:");
    println!("  1. Creating simulator without ELF file");
    println!("  2. Writing instructions directly to memory");
    println!("  3. Running simulation with custom boot PC");
    println!("  4. Successful program execution");
    println!("========================================");
}

/// Test write_memory_region with various data patterns
#[test]
fn test_write_memory_region_patterns() {
    let _ = env_logger::builder().is_test(true).try_init();

    println!("\n=== MEMORY REGION WRITE TEST ===");

    // Test by writing patterns and then running a program that reads them
    let result = run_program(
        GLOBAL_MAX_CYCLES,
        false,
        false,
        None::<fn(&mut SimulatorView)>,
        None::<fn(&riscv_core::trace::InstructionTrace)>,
        None,
        0, // Zero latency
        |sim| {
            // Test 1: Write a pattern and read it back
            let test_addr = 0x8000_1000;
            let test_data = vec![0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];
            sim.write_memory_region(test_addr, &test_data, true);

            let read_back: Vec<u8> = sim
                .dump_memory_region(test_addr, test_data.len() as u32)
                .collect();
            assert_eq!(
                read_back, test_data,
                "Written data should match read-back data"
            );
            println!("✓ Pattern write/read test passed");

            // Test 2: Write at different addresses
            sim.write_memory_region(0x8000_2000, &[0xAA, 0xBB], true);
            sim.write_memory_region(0x8000_3000, &[0xCC, 0xDD], true);

            let read1: Vec<u8> = sim.dump_memory_region(0x8000_2000, 2).collect();
            let read2: Vec<u8> = sim.dump_memory_region(0x8000_3000, 2).collect();

            assert_eq!(
                read1,
                vec![0xAA, 0xBB],
                "First region should be independent"
            );
            assert_eq!(
                read2,
                vec![0xCC, 0xDD],
                "Second region should be independent"
            );
            println!("✓ Multiple region write test passed");

            // Test 3: Overwrite test
            sim.write_memory_region(test_addr, &[0xFF; 8], true);
            let overwritten: Vec<u8> = sim.dump_memory_region(test_addr, 8).collect();
            assert_eq!(
                overwritten,
                vec![0xFF; 8],
                "Overwrite should replace previous data"
            );
            println!("✓ Overwrite test passed");

            // Return a simple program to satisfy the prep callback
            let instructions: Vec<u32> = vec![
                addi(10, 0, 42),     // addi x10, x0, 42
                lui(11, 0x10000000), // lui x11, 0x10000000
                sw(11, 10, 0),       // sw x10, 0(x11)
                jal(0, 0),           // jal x0, 0
            ];
            let program: Vec<u8> = instructions
                .iter()
                .flat_map(|inst| inst.to_le_bytes())
                .collect();
            sim.write_memory_region(0x8000_0000, &program, true);
            Ok(0x8000_0000)
        },
        None::<fn(&cpu_sim::SimulatorView, &cpu_sim::SimulationResult)>,
    )
    .expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(42),
        "Simple program should complete successfully"
    );

    println!("\n========================================");
    println!("✓ MEMORY REGION WRITE TEST COMPLETE");
    println!("========================================");
}
