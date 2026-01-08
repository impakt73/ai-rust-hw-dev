/// Test memory latency functionality
use super::*;
use crate::sim::Simulator;
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

/// Test that the simulator works with zero latency (default)
#[test]
fn test_zero_latency_default() {
    init_test_logger();

    let runtime = riscv_core::create_cpu_runtime().expect("Failed to create runtime");
    let bus = bus::SystemBus::new();
    let mut sim = Simulator::new(&runtime, bus, false, false, None::<fn(u32)>, None::<fn(&riscv_core::trace::InstructionTrace)>)
        .expect("Failed to create simulator");

    // Load a simple program: addi x1, x0, 42 then sw x1, -16(x0) to write to 0xFFFFFFF0
    let instructions: Vec<u8> = vec![
        0x93, 0x00, 0xa0, 0x02, // addi x1, x0, 42 (0x02a00093)
        0x23, 0x28, 0x10, 0xfe, // sw x1, -16(x0) (0xfe102823) - writes to 0xFFFFFFF0
    ];
    
    sim.write_memory_region(0x8000_0000, &instructions);

    let result = sim.run(0x8000_0000, 100).expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(42),
        "Should halt with tohost value 42"
    );
    
    // With zero latency, this should complete quickly
    println!("✓ Zero latency test completed in {} cycles", result.cycles);
    assert!(result.cycles < 20, "Should complete in fewer than 20 cycles with zero latency");
}

/// Test that the simulator works with multi-cycle memory latency
#[test]
fn test_multi_cycle_memory_latency() {
    init_test_logger();

    let runtime = riscv_core::create_cpu_runtime().expect("Failed to create runtime");
    let bus = bus::SystemBus::new();
    let mut sim = Simulator::new(&runtime, bus, false, false, None::<fn(u32)>, None::<fn(&riscv_core::trace::InstructionTrace)>)
        .expect("Failed to create simulator");

    // Set memory latency to 3 cycles
    sim.set_memory_latency(3);

    // Load a simple program: addi x1, x0, 42 then sw x1, -16(x0) to write to 0xFFFFFFF0
    let instructions: Vec<u8> = vec![
        0x93, 0x00, 0xa0, 0x02, // addi x1, x0, 42
        0x23, 0x28, 0x10, 0xfe, // sw x1, -16(x0) - writes to 0xFFFFFFF0
    ];
    
    sim.write_memory_region(0x8000_0000, &instructions);

    let result = sim.run(0x8000_0000, 100).expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(42),
        "Should halt with tohost value 42 even with latency"
    );
    
    // With 3-cycle latency, this should take more cycles
    // Each instruction fetch takes 3 cycles, store takes 3 cycles
    println!("✓ Multi-cycle latency test completed in {} cycles", result.cycles);
    assert!(result.cycles > 10, "Should take more cycles with 3-cycle latency");
}

/// Test load/store operations with variable latency
#[test]
fn test_load_store_with_latency() {
    init_test_logger();

    let runtime = riscv_core::create_cpu_runtime().expect("Failed to create runtime");
    let bus = bus::SystemBus::new();
    let mut sim = Simulator::new(&runtime, bus, false, false, None::<fn(u32)>, None::<fn(&riscv_core::trace::InstructionTrace)>)
        .expect("Failed to create simulator");

    // Set memory latency to 2 cycles
    sim.set_memory_latency(2);

    // Load a program that does: 
    // 1. addi x1, x0, 100  (x1 = 100)
    // 2. sw x1, 0(x0)      (store 100 to address 0)
    // 3. lw x2, 0(x0)      (load from address 0 into x2)
    // 4. sw x2, -16(x0)    (write to tohost 0xFFFFFFF0 to halt)
    let instructions: Vec<u8> = vec![
        0x93, 0x00, 0x40, 0x06, // addi x1, x0, 100
        0x23, 0x20, 0x10, 0x00, // sw x1, 0(x0)
        0x03, 0x21, 0x00, 0x00, // lw x2, 0(x0)
        0x23, 0x28, 0x20, 0xfe, // sw x2, -16(x0)
    ];
    
    sim.write_memory_region(0x8000_0000, &instructions);

    let result = sim.run(0x8000_0000, 200).expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(100),
        "Should halt with tohost value 100"
    );
    
    println!("✓ Load/store with latency test completed in {} cycles", result.cycles);
    assert!(result.cycles > 15, "Should take more cycles with memory latency");
}

/// Test that existing ELF programs still work with variable latency
#[test]
fn test_comprehensive_elf_with_latency() {
    init_test_logger();

    let elf_path = test_program_path("test.elf");
    
    // Create runtime and load ELF
    let runtime = riscv_core::create_cpu_runtime().expect("Failed to create runtime");
    let bus = bus::SystemBus::new();
    let mut sim = Simulator::new(&runtime, bus, false, false, None::<fn(u32)>, None::<fn(&riscv_core::trace::InstructionTrace)>)
        .expect("Failed to create simulator");

    // Set memory latency to 2 cycles
    sim.set_memory_latency(2);

    // Load ELF
    let elf_data = std::fs::read(&elf_path).expect("Failed to read ELF file");
    let elf_file = elf::ElfBytes::<elf::endian::AnyEndian>::minimal_parse(&elf_data)
        .expect("Failed to parse ELF");

    // Load program headers into memory
    for phdr in elf_file
        .segments()
        .expect("Failed to get program headers")
        .iter()
        .filter(|p| p.p_type == elf::abi::PT_LOAD)
    {
        let vaddr = phdr.p_vaddr as u32;
        let data = elf_file
            .segment_data(&phdr)
            .expect("Failed to get segment data");
        sim.write_memory_region(vaddr, data);
        log::debug!(
            "Loaded segment at 0x{:08x}, size {} bytes",
            vaddr,
            data.len()
        );
    }

    let entry_point = elf_file.ehdr.e_entry as u32;
    let result = sim.run(entry_point, 1000).expect("Simulation should succeed");

    assert_eq!(
        result.tohost_value,
        Some(0x2a),
        "Should complete with expected tohost value even with memory latency"
    );
    
    println!("✓ Comprehensive ELF with latency completed in {} cycles", result.cycles);
}
