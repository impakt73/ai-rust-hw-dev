use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use vcd::{Command, Parser, Value};

#[derive(Debug)]
pub struct VcdStatistics {
    pub total_cycles: u64,
    pub total_instructions: u64,
    pub memory_reads: u64,
    pub memory_writes: u64,
    pub unique_pcs: Vec<u32>,
    pub pc_range: (u32, u32),
    pub clock_transitions: u64,
    pub max_timestamp: u64,
    pub instruction_distribution: HashMap<String, u64>,
}

pub fn analyze_vcd(vcd_path: &str) -> Result<VcdStatistics, String> {
    let file = File::open(vcd_path).map_err(|e| format!("Failed to open VCD file: {}", e))?;
    let reader = BufReader::new(file);
    let mut parser = Parser::new(reader);

    // Parse header
    let header = parser
        .parse_header()
        .map_err(|e| format!("Failed to parse VCD header: {}", e))?;

    // Extract signal IDs from header
    fn find_signals(items: &[vcd::ScopeItem]) -> HashMap<String, vcd::IdCode> {
        let mut signals = HashMap::new();
        for item in items {
            match item {
                vcd::ScopeItem::Var(var) => {
                    signals.insert(var.reference.clone(), var.code);
                }
                vcd::ScopeItem::Scope(scope) => {
                    let nested = find_signals(&scope.children);
                    signals.extend(nested);
                }
            }
        }
        signals
    }

    let signals = find_signals(&header.items);

    let clk_id = signals.get("clk").copied();
    let dmem_we_id = signals.get("dmem_we").copied();
    let dmem_re_id = signals.get("dmem_re").copied();
    let instr_complete_id = signals.get("instr_complete").copied();
    let debug_pc_id = signals.get("debug_pc").copied();

    // Track statistics
    let mut clock_transitions = 0u64;
    let mut memory_reads = 0u64;
    let mut memory_writes = 0u64;
    let mut total_instructions = 0u64;
    let mut unique_pcs = Vec::new();
    let mut pc_min = u32::MAX;
    let mut pc_max = 0u32;
    let mut max_timestamp = 0u64;

    let mut prev_clk = Value::V0;

    // Parse value changes
    for command_result in parser {
        let command = command_result.map_err(|e| format!("Failed to parse VCD command: {}", e))?;

        match command {
            Command::Timestamp(ts) => {
                max_timestamp = max_timestamp.max(ts);
            }
            Command::ChangeScalar(id, value) => {
                // Count clock transitions (rising edges)
                if Some(id) == clk_id {
                    if prev_clk == Value::V0 && value == Value::V1 {
                        clock_transitions += 1;
                    }
                    prev_clk = value;
                }

                // Count memory writes (on rising edge)
                if Some(id) == dmem_we_id && value == Value::V1 {
                    memory_writes += 1;
                }

                // Count memory reads (on rising edge)
                if Some(id) == dmem_re_id && value == Value::V1 {
                    memory_reads += 1;
                }

                // Count instructions (on rising edge)
                if Some(id) == instr_complete_id && value == Value::V1 {
                    total_instructions += 1;
                }
            }
            Command::ChangeVector(id, values) => {
                // Track PC values
                if Some(id) == debug_pc_id {
                    if let Some(pc) = vector_to_u32(&values) {
                        if !unique_pcs.contains(&pc) {
                            unique_pcs.push(pc);
                        }
                        pc_min = pc_min.min(pc);
                        pc_max = pc_max.max(pc);
                    }
                }
            }
            _ => {}
        }
    }

    unique_pcs.sort();

    Ok(VcdStatistics {
        total_cycles: clock_transitions,
        total_instructions,
        memory_reads,
        memory_writes,
        unique_pcs,
        pc_range: if pc_min == u32::MAX {
            (0, 0)
        } else {
            (pc_min, pc_max)
        },
        clock_transitions,
        max_timestamp,
        instruction_distribution: HashMap::new(),
    })
}

fn vector_to_u32(values: &[Value]) -> Option<u32> {
    let mut result = 0u32;
    for (i, val) in values.iter().rev().enumerate() {
        match val {
            Value::V1 => result |= 1 << i,
            Value::V0 => {}
            _ => return None, // Unknown or high-Z
        }
    }
    Some(result)
}
