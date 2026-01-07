// Test LUI followed by SW to see if SW is the trigger

use crate::bus::SystemBus;
use crate::sim::Simulator;
use riscv_core::trace::{InstructionTrace, InstructionType};
use std::sync::{Arc, Mutex};

#[test]
fn test_lui_followed_by_sw() {
    let sequences: Vec<(&str, Vec<u32>)> = vec![
        (
            "LUI+EBREAK",
            vec![
                0x12345437, // lui s0, 0x12345
                0x00100073, // ebreak
            ],
        ),
        (
            "LUI+SW+EBREAK",
            vec![
                0x12345437, // lui s0, 0x12345
                0x00102023, // sw x1, 0(x0)
                0x00100073, // ebreak
            ],
        ),
        (
            "ORI+LUI+EBREAK",
            vec![
                0x00116393, // ori x7, x2, 1
                0x12345437, // lui s0, 0x12345
                0x00100073, // ebreak
            ],
        ),
        (
            "ORI+LUI+SW+EBREAK",
            vec![
                0x00116393, // ori x7, x2, 1
                0x12345437, // lui s0, 0x12345
                0x00102023, // sw x1, 0(x0)
                0x00100073, // ebreak
            ],
        ),
        (
            "Full 7+LUI+SW+EBREAK",
            vec![
                0x00a00093, // addi x1, x0, 10
                0x01400113, // addi x2, x0, 20
                0x00500193, // addi x3, x0, 5
                0x00208233, // add x4, x1, x2
                0x403102b3, // sub x5, x2, x3
                0x0ff0f313, // andi x6, x1, 0xff
                0x00116393, // ori x7, x2, 1
                0x12345437, // lui s0, 0x12345
                0x00102023, // sw x1, 0(x0)
                0x00100073, // ebreak
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

        println!("Captured {} traces (showing first 12):", final_traces.len());
        for (i, trace) in final_traces.iter().enumerate().take(12) {
            println!(
                "  [{}] PC=0x{:08x}, Instr=0x{:08x}, Type={:?}",
                i, trace.pc, trace.instruction, trace.inst_type
            );
        }

        // Check if LUI was executed
        let lui_found = final_traces
            .iter()
            .any(|t| t.inst_type == InstructionType::Lui);

        if lui_found {
            println!("  ✓ LUI executed");
        } else {
            println!("  ✗ LUI NOT found - BUG!");
            panic!("{}: LUI instruction was not executed!", name);
        }
    }
}
