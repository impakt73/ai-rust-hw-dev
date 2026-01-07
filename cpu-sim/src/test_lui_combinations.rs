// Test to see if LUI skipping is specific to ORI or happens with other I-type instructions

use crate::bus::SystemBus;
use crate::sim::Simulator;
use riscv_core::trace::{InstructionTrace, InstructionType};
use std::sync::{Arc, Mutex};

#[test]
fn test_lui_after_various_instructions() {
    // Test LUI after different instruction types
    let test_cases: Vec<(&str, Vec<u32>)> = vec![
        (
            "ADDI+LUI",
            vec![
                0x00a00093, // addi x1, x0, 10
                0x12345437, // lui s0, 0x12345
                0x00100073, // ebreak
            ],
        ),
        (
            "ANDI+LUI",
            vec![
                0x0ff0f313, // andi x6, x1, 0xff
                0x12345437, // lui s0, 0x12345
                0x00100073, // ebreak
            ],
        ),
        (
            "ORI+LUI",
            vec![
                0x00116393, // ori x7, x2, 1
                0x12345437, // lui s0, 0x12345
                0x00100073, // ebreak
            ],
        ),
        (
            "ADD+LUI",
            vec![
                0x00208233, // add x4, x1, x2
                0x12345437, // lui s0, 0x12345
                0x00100073, // ebreak
            ],
        ),
    ];

    for (name, instructions) in test_cases {
        println!("\n=== Testing: {} ===", name);

        let runtime = riscv_core::create_cpu_runtime().expect("Failed to create CPU runtime");
        let bus = SystemBus::new();

        let traces = Arc::new(Mutex::new(Vec::new()));
        let traces_clone = Arc::clone(&traces);

        let trace_callback = move |trace: &InstructionTrace| {
            traces_clone.lock().unwrap().push(trace.clone());
        };

        let mut sim = Simulator::new(&runtime, bus, false, None::<fn(u32)>, Some(trace_callback))
            .expect("Failed to create simulator");

        let base_addr = 0x80000000u32;
        let mut instruction_bytes = Vec::new();
        for &instr in &instructions {
            instruction_bytes.extend_from_slice(&instr.to_le_bytes());
        }
        sim.write_memory_region(base_addr, &instruction_bytes);

        sim.reset(base_addr);
        let _ = sim.run(base_addr, 100).expect("Simulation should succeed");

        let final_traces = traces.lock().unwrap();

        println!("Captured {} traces:", final_traces.len().min(5));
        for (i, trace) in final_traces.iter().enumerate().take(5) {
            println!(
                "  [{}] PC=0x{:08x}, Type={:?}",
                i, trace.pc, trace.inst_type
            );
        }

        // Check if LUI was executed
        let lui_found = final_traces
            .iter()
            .any(|t| t.pc == 0x80000004 && t.inst_type == InstructionType::Lui);

        if lui_found {
            println!("  ✓ LUI executed correctly");
        } else {
            println!("  ✗ LUI NOT found - BUG!");
            panic!(
                "{}: LUI instruction at PC 0x80000004 was not executed!",
                name
            );
        }
    }

    println!("\n✓ All test cases passed!");
}
