#[cfg(test)]
mod tests {
    use crate::*;

    /// Test that demonstrates writing programmatic instructions to memory without an ELF file
    #[test]
    fn test_programmatic_instruction_loading() {
        let _ = env_logger::builder().is_test(true).try_init();

        println!("\n=== PROGRAMMATIC INSTRUCTION LOADING TEST ===");
        println!("Demonstrating the new decoupled simulator API");

        // Create system bus with internal DRAM
        let bus = bus::SystemBus::new();

        // Initialize CPU Simulator without an ELF file
        let runtime = riscv_core::create_cpu_runtime().expect("Failed to create CPU runtime");
        let mut sim = sim::Simulator::new(
            &runtime,
            bus,
            false,
            false, // Don't print FSM state,
            None::<fn(u32)>,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            None, // No VCD
            0,    // Zero latency
            Some(HungDetectorConfig::default()),
        )
        .expect("Failed to create simulator");

        println!("✓ Simulator created without loading an ELF file");

        // Define a simple program:
        // Address 0x80000000:
        //   addi x10, x0, 42  ; x10 = 42
        //   sw x10, 0xFFFFFFF0(x0)  ; store to tohost (halt)
        //   jal x0, 0  ; infinite loop (stay here)
        //
        // Let's manually encode these instructions:
        // addi x10, x0, 42 = 0x02a00513
        //   imm[11:0] = 42 = 0x02a
        //   rs1 = x0 = 0
        //   funct3 = 0x0 (ADDI)
        //   rd = x10 = 10 = 0xa
        //   opcode = 0x13 (OP-IMM)
        //   Encoding: 0x02a00513
        //
        // sw x10, -16(x0) = store x10 to address 0xFFFFFFF0
        //   imm[11:5] = 0x7f (sign-extended from -16 >> 5)
        //   rs2 = x10 = 10 = 0xa
        //   rs1 = x0 = 0
        //   funct3 = 0x2 (SW)
        //   imm[4:0] = 0x10
        //   opcode = 0x23 (STORE)
        //   Encoding for sw x10, -16(x0): 0xfea02823
        //
        // jal x0, 0 = jump to current address (infinite loop)
        //   imm[20] = 0, imm[10:1] = 0, imm[11] = 0, imm[19:12] = 0
        //   rd = x0 = 0
        //   opcode = 0x6f (JAL)
        //   Encoding: 0x0000006f

        let program: Vec<u8> = vec![
            // addi x10, x0, 42 (0x02a00513 in little-endian)
            0x13, 0x05, 0xa0, 0x02, // sw x10, -16(x0) (0xfea02823 in little-endian)
            0x23, 0x28, 0xa0, 0xfe, // jal x0, 0 (0x0000006f in little-endian)
            0x6f, 0x00, 0x00, 0x00,
        ];

        // Write the program to memory starting at 0x80000000 (typical RISC-V start address)
        const START_ADDR: u32 = 0x8000_0000;
        sim.write_memory_region(START_ADDR, &program, true);

        println!(
            "✓ Programmatic instructions written to memory at 0x{:08x}",
            START_ADDR
        );
        println!("  Program size: {} bytes", program.len());
        println!("  Instruction 1: addi x10, x0, 42");
        println!("  Instruction 2: sw x10, -16(x0) ; store to tohost");
        println!("  Instruction 3: jal x0, 0 ; infinite loop");

        // Run the simulation with the start address as boot PC
        println!("\n✓ Running simulation with boot PC = 0x{:08x}", START_ADDR);
        let result = sim.run(START_ADDR, 100).expect("Simulation should succeed");

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
        assert!(
            result.cycles < 20,
            "Expected program to complete in less than 20 cycles, got {}",
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

        // Create simulator with system bus
        let bus = bus::SystemBus::new();
        let runtime = riscv_core::create_cpu_runtime().expect("Failed to create CPU runtime");
        let mut sim = sim::Simulator::new(
            &runtime,
            bus,
            false,
            false, // Don't print FSM state,
            None::<fn(u32)>,
            None::<fn(&riscv_core::trace::InstructionTrace)>,
            None, // No VCD
            0,    // Zero latency
            Some(HungDetectorConfig::default()),
        )
        .expect("Failed to create simulator");

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

        println!("\n========================================");
        println!("✓ MEMORY REGION WRITE TEST COMPLETE");
        println!("========================================");
    }
}
