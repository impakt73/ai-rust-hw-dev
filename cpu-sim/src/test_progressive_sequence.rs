// Progressive sequence test to find where LUI skipping starts

use crate::bus::SystemBus;
use crate::sim::Simulator;
use riscv_core::trace::{InstructionTrace, InstructionType};
use std::sync::{Arc, Mutex};

#[test]
fn test_progressive_sequence_to_lui() {
    // Start with 2 instructions before LUI, then 3, then 4, etc.
    // to find exactly when the bug appears

    let sequences: Vec<(&str, Vec<u32>)> = vec![
        (
            "1 before LUI",
            vec![
                0x00116393, // 0x00: ori x7, x2, 1
                0x12345437, // 0x04: lui s0, 0x12345
                0x00100073, // 0x08: ebreak
            ],
        ),
        (
            "2 before LUI",
            vec![
                0x0ff0f313, // 0x00: andi x6, x1, 0xff
                0x00116393, // 0x04: ori x7, x2, 1
                0x12345437, // 0x08: lui s0, 0x12345
                0x00100073, // 0x0c: ebreak
            ],
        ),
        (
            "3 before LUI",
            vec![
                0x403102b3, // 0x00: sub x5, x2, x3
                0x0ff0f313, // 0x04: andi x6, x1, 0xff
                0x00116393, // 0x08: ori x7, x2, 1
                0x12345437, // 0x0c: lui s0, 0x12345
                0x00100073, // 0x10: ebreak
            ],
        ),
        (
            "4 before LUI",
            vec![
                0x00208233, // 0x00: add x4, x1, x2
                0x403102b3, // 0x04: sub x5, x2, x3
                0x0ff0f313, // 0x08: andi x6, x1, 0xff
                0x00116393, // 0x0c: ori x7, x2, 1
                0x12345437, // 0x10: lui s0, 0x12345
                0x00100073, // 0x14: ebreak
            ],
        ),
        (
            "5 before LUI",
            vec![
                0x00500193, // 0x00: addi x3, x0, 5
                0x00208233, // 0x04: add x4, x1, x2
                0x403102b3, // 0x08: sub x5, x2, x3
                0x0ff0f313, // 0x0c: andi x6, x1, 0xff
                0x00116393, // 0x10: ori x7, x2, 1
                0x12345437, // 0x14: lui s0, 0x12345
                0x00100073, // 0x18: ebreak
            ],
        ),
        (
            "6 before LUI",
            vec![
                0x01400113, // 0x00: addi x2, x0, 20
                0x00500193, // 0x04: addi x3, x0, 5
                0x00208233, // 0x08: add x4, x1, x2
                0x403102b3, // 0x0c: sub x5, x2, x3
                0x0ff0f313, // 0x10: andi x6, x1, 0xff
                0x00116393, // 0x14: ori x7, x2, 1
                0x12345437, // 0x18: lui s0, 0x12345
                0x00100073, // 0x1c: ebreak
            ],
        ),
        (
            "7 before LUI (full original)",
            vec![
                0x00a00093, // 0x00: addi x1, x0, 10
                0x01400113, // 0x04: addi x2, x0, 20
                0x00500193, // 0x08: addi x3, x0, 5
                0x00208233, // 0x0c: add x4, x1, x2
                0x403102b3, // 0x10: sub x5, x2, x3
                0x0ff0f313, // 0x14: andi x6, x1, 0xff
                0x00116393, // 0x18: ori x7, x2, 1
                0x12345437, // 0x1c: lui s0, 0x12345
                0x00100073, // 0x20: ebreak
            ],
        ),
    ];

    for (name, instructions) in sequences {
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
        let _ = sim.run(base_addr, 200).expect("Simulation should succeed");

        let final_traces = traces.lock().unwrap();

        println!("Captured {} traces (showing first 10):", final_traces.len());
        for (i, trace) in final_traces.iter().enumerate().take(10) {
            println!(
                "  [{}] PC=0x{:08x}, Type={:?}",
                i, trace.pc, trace.inst_type
            );
        }

        // Find LUI instruction - it should be at position (instructions.len() - 2)
        // because last instruction is EBREAK
        let lui_index = instructions.len() - 2;
        let lui_pc = base_addr + (lui_index as u32 * 4);

        let lui_found = final_traces
            .iter()
            .any(|t| t.pc == lui_pc && t.inst_type == InstructionType::Lui);

        if lui_found {
            println!("  ✓ LUI at PC 0x{:08x} executed correctly", lui_pc);
        } else {
            println!("  ✗ LUI at PC 0x{:08x} NOT found - BUG REPRODUCED!", lui_pc);
            // Don't panic yet - let's see all cases
        }
    }
}
