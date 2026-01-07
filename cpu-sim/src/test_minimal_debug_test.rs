// Test with FSM debugging enabled to trace the exact issue

use crate::bus::SystemBus;
use crate::sim::Simulator;
use riscv_core::trace::InstructionTrace;
use std::sync::{Arc, Mutex};

#[test]
fn test_lui_sw_with_fsm_debug() {
    println!("\n=== LUI+SW Sequence with FSM Debug ===\n");

    let runtime = riscv_core::create_cpu_runtime().expect("Failed to create CPU runtime");
    let bus = SystemBus::new();

    let traces = Arc::new(Mutex::new(Vec::new()));
    let traces_clone = Arc::clone(&traces);

    let trace_callback = move |trace: &InstructionTrace| {
        traces_clone.lock().unwrap().push(trace.clone());
    };

    let mut sim = Simulator::new(
        &runtime,
        bus,
        true, // Enable instruction trace printing
        None::<fn(u32)>,
        Some(trace_callback),
    )
    .expect("Failed to create simulator");

    // Enable FSM state debugging
    sim.set_print_fsm_state(true);

    let instructions: Vec<u32> = vec![
        0x12345437, // 0x00: lui s0, 0x12345
        0x00102023, // 0x04: sw x1, 0(x0)
        0x00100073, // 0x08: ebreak
    ];

    let base_addr = 0x80000000u32;
    let mut instruction_bytes = Vec::new();
    for &instr in &instructions {
        instruction_bytes.extend_from_slice(&instr.to_le_bytes());
    }
    sim.write_memory_region(base_addr, &instruction_bytes);

    println!("Instructions loaded:");
    println!("  0x80000000: 0x12345437 (LUI)");
    println!("  0x80000004: 0x00102023 (SW)");
    println!("  0x80000008: 0x00100073 (EBREAK)");
    println!("\nStarting simulation with FSM debug enabled...\n");

    sim.reset(base_addr);

    // Run for limited cycles to see the issue
    let result = sim.run(base_addr, 50);

    match result {
        Ok(_) => {
            let final_traces = traces.lock().unwrap();
            println!("\n=== TRACE SUMMARY ===");
            println!("Total instruction traces: {}", final_traces.len());
            for (i, trace) in final_traces.iter().enumerate().take(5) {
                println!(
                    "  [{}] PC=0x{:08x}, Instr=0x{:08x}, Type={:?}",
                    i, trace.pc, trace.instruction, trace.inst_type
                );
            }

            // Check if LUI executed
            let lui_found = final_traces.iter().any(|t| t.pc == 0x80000000);
            if lui_found {
                println!("\n✓ LUI was executed");
            } else {
                println!("\n✗ BUG: LUI at PC 0x80000000 was NOT executed!");
                println!(
                    "   First instruction traced: PC=0x{:08x}",
                    final_traces[0].pc
                );
            }
        }
        Err(e) => {
            println!("Simulation error: {}", e);
        }
    }
}
