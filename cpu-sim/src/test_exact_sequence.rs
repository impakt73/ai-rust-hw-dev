// Test with the exact instruction sequence from trace_test.elf

use crate::bus::SystemBus;
use crate::sim::Simulator;
use riscv_core::trace::{InstructionTrace, InstructionType};
use std::sync::{Arc, Mutex};

#[test]
fn test_exact_trace_test_sequence() {
    let runtime = riscv_core::create_cpu_runtime().expect("Failed to create CPU runtime");
    let bus = SystemBus::new();

    // Collect instruction traces via callback
    let traces = Arc::new(Mutex::new(Vec::new()));
    let traces_clone = Arc::clone(&traces);

    let trace_callback = move |trace: &InstructionTrace| {
        traces_clone.lock().unwrap().push(trace.clone());
    };

    let mut sim = Simulator::new(
        &runtime,
        bus,
        false, // Don't print trace
        None::<fn(u32)>,
        Some(trace_callback),
    )
    .expect("Failed to create simulator");

    // Use the EXACT instruction sequence from trace_test.elf
    let instructions: Vec<u32> = vec![
        0x00a00093, // 0x80000000: addi x1, x0, 10
        0x01400113, // 0x80000004: addi x2, x0, 20
        0x00500193, // 0x80000008: addi x3, x0, 5
        0x00208233, // 0x8000000c: add x4, x1, x2
        0x403102b3, // 0x80000010: sub x5, x2, x3
        0x0ff0f313, // 0x80000014: andi x6, x1, 0xff
        0x00116393, // 0x80000018: ori x7, x2, 1   ← THIS ONE
        0x12345437, // 0x8000001c: lui s0, 0x12345  ← MISSING ONE
        0x00102023, // 0x80000020: sw x1, 0(x0)
        0x00100073, // 0x80000024: EBREAK
    ];

    // Write instructions to memory programmatically
    let base_addr = 0x80000000u32;
    let mut instruction_bytes = Vec::new();
    for instr in &instructions {
        instruction_bytes.extend_from_slice(&instr.to_le_bytes());
    }
    sim.write_memory_region(base_addr, &instruction_bytes);

    println!("\n=== PC Debug Test (original sequence) ===\n");

    // Run simulation
    sim.reset(base_addr);
    let _result = sim.run(base_addr, 200).expect("Simulation should succeed");

    let final_traces = traces.lock().unwrap();
    println!("\nTotal traces (first 15): {}", final_traces.len().min(15));

    for (i, trace) in final_traces.iter().enumerate().take(15) {
        println!(
            "Trace[{}]: PC=0x{:08x}, Instruction=0x{:08x}, Type={:?}",
            i, trace.pc, trace.instruction, trace.inst_type
        );
    }

    // Expected sequence
    let expected = vec![
        (0x80000000, InstructionType::Addi),
        (0x80000004, InstructionType::Addi),
        (0x80000008, InstructionType::Addi),
        (0x8000000c, InstructionType::Add),
        (0x80000010, InstructionType::Sub),
        (0x80000014, InstructionType::Andi),
        (0x80000018, InstructionType::Ori), // This executes
        (0x8000001c, InstructionType::Lui), // This should execute next!
        (0x80000020, InstructionType::Sw),
        (0x80000024, InstructionType::Ebreak),
    ];

    // Check each expected instruction
    for (i, (exp_pc, exp_type)) in expected.iter().enumerate() {
        if i >= final_traces.len() {
            panic!(
                "Missing trace at index {}: expected PC 0x{:08x} ({:?})",
                i, exp_pc, exp_type
            );
        }

        let trace = &final_traces[i];
        assert_eq!(
            trace.pc, *exp_pc,
            "Trace[{}]: expected PC 0x{:08x}, got 0x{:08x}",
            i, exp_pc, trace.pc
        );
        assert_eq!(
            trace.inst_type, *exp_type,
            "Trace[{}]: expected type {:?}, got {:?}",
            i, exp_type, trace.inst_type
        );
    }

    println!("\n✓ All instructions traced correctly!");
}
