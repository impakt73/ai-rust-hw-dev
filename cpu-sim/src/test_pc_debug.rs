// Test to debug PC sequencing issue

use crate::bus::SystemBus;
use crate::sim::Simulator;
use riscv_core::trace::InstructionTrace;
use std::sync::{Arc, Mutex};

#[test]
fn test_pc_debug() {
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

    // Create a simple sequence of NOPs and one LUI
    let instructions: Vec<u32> = vec![
        0x00000013, // 0x80000000: NOP (ADDI x0, x0, 0)
        0x00000013, // 0x80000004: NOP
        0x00000013, // 0x80000008: NOP
        0x00000013, // 0x8000000c: NOP
        0x00000013, // 0x80000010: NOP
        0x00000013, // 0x80000014: NOP
        0x00000013, // 0x80000018: NOP
        0x12345437, // 0x8000001c: LUI s0, 0x12345
        0x00000013, // 0x80000020: NOP
        0x00100073, // 0x80000024: EBREAK
    ];

    // Write instructions to memory programmatically
    let base_addr = 0x80000000u32;
    let mut instruction_bytes = Vec::new();
    for instr in &instructions {
        instruction_bytes.extend_from_slice(&instr.to_le_bytes());
    }
    sim.write_memory_region(base_addr, &instruction_bytes);

    println!("\n=== PC Debug Test (all NOPs except LUI) ===\n");

    // Run simulation
    sim.reset(base_addr);
    let result = sim.run(base_addr, 200).expect("Simulation should succeed");

    println!("Simulation complete");
    if let Some(tohost) = result.tohost_value {
        println!("tohost = {}", tohost);
    }

    let final_traces = traces.lock().unwrap();
    println!("\nTotal traces: {}", final_traces.len());

    for (i, trace) in final_traces.iter().enumerate() {
        println!(
            "Trace[{}]: PC=0x{:08x}, Instruction=0x{:08x}, Type={:?}",
            i, trace.pc, trace.instruction, trace.inst_type
        );
    }

    // We should see all 10 instructions
    assert_eq!(
        final_traces.len(),
        10,
        "Should trace all 10 instructions including LUI"
    );

    // Verify LUI is at position 7
    assert_eq!(
        final_traces[7].pc, 0x8000001c,
        "Trace[7] should be LUI at PC 0x8000001c"
    );
    assert_eq!(
        final_traces[7].instruction, 0x12345437,
        "Trace[7] should be LUI instruction"
    );
}
