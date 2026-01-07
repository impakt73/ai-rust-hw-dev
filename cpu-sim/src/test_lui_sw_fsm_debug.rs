#[cfg(test)]
mod tests {
    use crate::*;
    use std::collections::HashMap;

#[test]
fn test_lui_sw_fsm_debug() {
    println!("\n========================================");
    println!("LUI+SW FSM DEBUG TEST");
    println!("========================================");

    let mut memory: HashMap<u32, u32> = HashMap::new();
    let base = 0x80000000;
    
    // ORI x7, x2, 1 at 0x80000018
    memory.insert(base + 0x18, 0x00116393); // ori x7, x2, 1
    
    // LUI x8, 0x12345 at 0x8000001c
    memory.insert(base + 0x1c, 0x12345437); // lui x8, 0x12345
    
    // SW x1, 0(x0) at 0x80000020
    memory.insert(base + 0x20, 0x00102023); // sw x1, 0(x0)
    
    // EBREAK at 0x80000024 to halt
    memory.insert(base + 0x24, 0x00100073); // ebreak

    let mut trace_count = 0;
    let trace_callback = move |trace: &sim::CpuTraceData| {
        trace_count += 1;
        println!("TRACE #{}: PC={:08x}, Instr={:08x}", 
                 trace_count, trace.pc, trace.instruction);
    };

    let mut sim = sim::CpuSim::new(base, memory.clone());
    sim.set_trace_callback(trace_callback);
    sim.enable_fsm_debug(true); // Enable FSM debug output

    println!("\nStarting execution from PC={:08x}", base + 0x18);
    
    // Run simulation
    sim.run_until_halt(1000);
    
    println!("\n========================================");
    println!("Total instructions traced: {}", trace_count);
    println!("========================================\n");
    
    assert_eq!(trace_count, 3, "Should trace 3 instructions (ORI, LUI, SW)");
}
}
