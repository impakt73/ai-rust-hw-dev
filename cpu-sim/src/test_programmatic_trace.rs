use riscv_core::trace::{InstructionTrace, InstructionType};
use std::sync::{Arc, Mutex};

#[test]
fn test_programmatic_instruction_sequence() {
    // This test programmatically loads a known instruction sequence
    // and verifies that all instructions appear in the trace in order

    let runtime = riscv_core::create_cpu_runtime().expect("Failed to create CPU runtime");
    let bus = crate::bus::SystemBus::new();

    // Collect instruction traces via callback
    let traces = Arc::new(Mutex::new(Vec::new()));
    let traces_clone = Arc::clone(&traces);

    let trace_callback = move |trace: &InstructionTrace| {
        traces_clone.lock().unwrap().push(trace.clone());
    };

    let mut sim = crate::sim::Simulator::new(
        &runtime,
        bus,
        false, // Don't print trace
        None::<fn(u32)>,
        Some(trace_callback),
    )
    .expect("Failed to create simulator");

    // Build a precise instruction sequence:
    // All instructions are at base 0x80000000
    let instructions: Vec<u32> = vec![
        0x00a00093, // 0x80000000: addi x1, x0, 10
        0x01400113, // 0x80000004: addi x2, x0, 20
        0x00500193, // 0x80000008: addi x3, x0, 5
        0x00208233, // 0x8000000c: add x4, x1, x2
        0x403102b3, // 0x80000010: sub x5, x2, x3
        0x0ff0f313, // 0x80000014: andi x6, x1, 0xff
        0x00116393, // 0x80000018: ori x7, x2, 1
        0x12345437, // 0x8000001c: lui x8, 0x12345
        0x00102023, // 0x80000020: sw x1, 0(x0)
        0x00002483, // 0x80000024: lw x9, 0(x0)
        0x02a00513, // 0x80000028: addi x10, x0, 42
        0xff000593, // 0x8000002c: addi x11, x0, -16
        0x00a5a023, // 0x80000030: sw x10, 0(x11)  # tohost write
    ];

    // Write instructions to memory at 0x80000000
    let base_addr = 0x80000000u32;
    let mut instruction_bytes = Vec::new();
    for instr in &instructions {
        instruction_bytes.extend_from_slice(&instr.to_le_bytes());
    }
    sim.write_memory_region(base_addr, &instruction_bytes);

    // Run simulation
    sim.reset(base_addr);
    let result = sim.run(base_addr, 1000).expect("Simulation should succeed");

    // Verify tohost
    assert_eq!(result.tohost_value, Some(42), "Should halt with tohost=42");

    // Verify traces
    let captured_traces = traces.lock().unwrap();

    println!("\n========================================");
    println!("PROGRAMMATIC TRACE TEST");
    println!("========================================");
    println!("Total instructions traced: {}", captured_traces.len());

    // Print all captured traces
    for (i, trace) in captured_traces.iter().enumerate() {
        println!(
            "  [{}] PC=0x{:08x}, Instr=0x{:08x}, Type={:?}",
            i, trace.pc, trace.instruction, trace.inst_type
        );
    }
    println!();

    // Verify sequence
    let expected_sequence = vec![
        (0x80000000, InstructionType::Addi),
        (0x80000004, InstructionType::Addi),
        (0x80000008, InstructionType::Addi),
        (0x8000000c, InstructionType::Add),
        (0x80000010, InstructionType::Sub),
        (0x80000014, InstructionType::Andi),
        (0x80000018, InstructionType::Ori),
        (0x8000001c, InstructionType::Lui),
        (0x80000020, InstructionType::Sw),
        (0x80000024, InstructionType::Lw),
        (0x80000028, InstructionType::Addi),
        (0x8000002c, InstructionType::Addi),
        (0x80000030, InstructionType::Sw),
    ];

    for (i, (expected_pc, expected_type)) in expected_sequence.iter().enumerate() {
        assert!(
            i < captured_traces.len(),
            "Expected at least {} traces, got {}",
            i + 1,
            captured_traces.len()
        );
        let trace = &captured_traces[i];
        assert_eq!(
            trace.pc, *expected_pc,
            "Trace[{}]: expected PC 0x{:08x}, got 0x{:08x}",
            i, expected_pc, trace.pc
        );
        assert_eq!(
            trace.inst_type, *expected_type,
            "Trace[{}]: expected {:?}, got {:?}",
            i, expected_type, trace.inst_type
        );
    }

    println!(
        "✓ All {} instructions traced in correct order",
        expected_sequence.len()
    );
    println!("✓ LUI instruction correctly captured at PC 0x8000001c");
    println!("========================================\n");
}
