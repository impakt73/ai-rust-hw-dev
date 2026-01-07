// Test LUI followed by different memory operations

use crate::bus::SystemBus;
use crate::sim::Simulator;
use riscv_core::trace::{InstructionTrace, InstructionType};
use std::sync::{Arc, Mutex};

#[test]
fn test_lui_followed_by_memory_ops() {
    let sequences: Vec<(&str, Vec<u32>)> = vec![
        (
            "LUI+LW",
            vec![
                0x12345437, // lui s0, 0x12345
                0x00002483, // lw x9, 0(x0)
                0x00100073, // ebreak
            ],
        ),
        (
            "LUI+SW",
            vec![
                0x12345437, // lui s0, 0x12345
                0x00102023, // sw x1, 0(x0)
                0x00100073, // ebreak
            ],
        ),
        (
            "LUI+SH",
            vec![
                0x12345437, // lui s0, 0x12345
                0x00101023, // sh x1, 0(x0)
                0x00100073, // ebreak
            ],
        ),
        (
            "LUI+SB",
            vec![
                0x12345437, // lui s0, 0x12345
                0x00100023, // sb x1, 0(x0)
                0x00100073, // ebreak
            ],
        ),
        (
            "LUI+ADD",
            vec![
                0x12345437, // lui s0, 0x12345
                0x00208233, // add x4, x1, x2
                0x00100073, // ebreak
            ],
        ),
    ];

    for (name, instructions) in sequences {
        print!("{:20}", name);

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

        // Check if LUI was executed
        let lui_found = final_traces
            .iter()
            .any(|t| t.inst_type == InstructionType::Lui);

        if lui_found {
            println!("✓ LUI executed");
        } else {
            println!(
                "✗ LUI skipped! First trace: PC=0x{:08x}",
                final_traces[0].pc
            );
        }
    }
}
