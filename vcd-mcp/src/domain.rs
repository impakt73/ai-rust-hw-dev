use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum VcdError {
    #[error("Failed to open VCD file: {0}")]
    FileOpen(#[from] std::io::Error),
    #[error("Failed to parse VCD: {0}")]
    ParseError(String),
    #[allow(dead_code)]
    #[error("Signal not found: {0}")]
    SignalNotFound(String),
}

/// Represents a value in the VCD file
#[derive(Debug, Clone)]
pub enum VcdValue {
    Scalar(vcd::Value),
    Vector(Vec<vcd::Value>),
    Real(f64),
    String(String),
}

impl VcdValue {
    pub fn format(&self) -> String {
        match self {
            VcdValue::Scalar(v) => format!("{}", v),
            VcdValue::Vector(v) => {
                let s: String = v.iter().map(|val| format!("{}", val)).collect();
                format!("0b{}", s)
            }
            VcdValue::Real(r) => format!("{}", r),
            VcdValue::String(s) => s.clone(),
        }
    }
}

/// Represents a parsed VCD file with queryable data
#[derive(Debug)]
pub struct VcdAnalysis {
    pub header: vcd::Header,
    /// Map signal ID to full hierarchical name
    pub id_to_name: HashMap<vcd::IdCode, String>,
    /// Map full hierarchical name to signal ID
    pub name_to_id: HashMap<String, vcd::IdCode>,
    /// Timeline of value changes: Vec<(timestamp, Vec<(signal_id, value)>)>
    /// Only stores changes, not full snapshots
    pub time_changes: Vec<(u64, Vec<(vcd::IdCode, VcdValue)>)>,
}

impl VcdAnalysis {
    /// Extract all signals from the header
    fn extract_signals_from_header(
        header: &vcd::Header,
    ) -> (HashMap<vcd::IdCode, String>, HashMap<String, vcd::IdCode>) {
        let mut id_to_name = HashMap::new();
        let mut name_to_id = HashMap::new();

        // Recursively traverse the scope hierarchy
        fn traverse_scope_items(
            items: &[vcd::ScopeItem],
            scope_stack: &mut Vec<String>,
            id_to_name: &mut HashMap<vcd::IdCode, String>,
            name_to_id: &mut HashMap<String, vcd::IdCode>,
        ) {
            for item in items {
                match item {
                    vcd::ScopeItem::Scope(scope) => {
                        // Enter this scope
                        scope_stack.push(scope.identifier.clone());
                        // Recursively process children
                        traverse_scope_items(&scope.items, scope_stack, id_to_name, name_to_id);
                        // Exit this scope
                        scope_stack.pop();
                    }
                    vcd::ScopeItem::Var(var) => {
                        let full_name = if scope_stack.is_empty() {
                            var.reference.clone()
                        } else {
                            format!("{}.{}", scope_stack.join("."), var.reference)
                        };
                        id_to_name.insert(var.code, full_name.clone());
                        name_to_id.insert(full_name, var.code);
                    }
                    _ => {} // Handle other variants that may be added in future versions
                }
            }
        }

        let mut scope_stack = Vec::new();
        traverse_scope_items(
            &header.items,
            &mut scope_stack,
            &mut id_to_name,
            &mut name_to_id,
        );

        (id_to_name, name_to_id)
    }

    /// Get the value of a signal at a specific time
    pub fn get_signal_value_at(
        &self,
        signal_id: vcd::IdCode,
        target_time: u64,
    ) -> Option<VcdValue> {
        let mut current_value: Option<VcdValue> = None;

        for (timestamp, changes) in &self.time_changes {
            if *timestamp > target_time {
                break;
            }

            // Update current value if this signal changed
            for (id, value) in changes {
                if *id == signal_id {
                    current_value = Some(value.clone());
                }
            }
        }

        current_value
    }

    /// Get all changes for a signal between start_time and end_time (inclusive).
    ///
    /// Returns a vector of (timestamp, value) tuples for all times when the signal
    /// changed within the specified range. If no changes occur in the range but the
    /// signal had a value before start_time, that value is returned with the actual
    /// timestamp when it was last set (not start_time), to accurately reflect when
    /// the value was established.
    pub fn get_signal_changes(
        &self,
        signal_id: vcd::IdCode,
        start_time: u64,
        end_time: u64,
    ) -> Vec<(u64, VcdValue)> {
        let mut changes = Vec::new();
        let mut last_value: Option<(u64, VcdValue)> = None;

        for (timestamp, change_list) in &self.time_changes {
            // Track value up to start_time
            if *timestamp < start_time {
                for (id, value) in change_list {
                    if *id == signal_id {
                        last_value = Some((*timestamp, value.clone()));
                    }
                }
                continue;
            }

            // Past end_time, stop
            if *timestamp > end_time {
                break;
            }

            // Within range, collect changes
            for (id, value) in change_list {
                if *id == signal_id {
                    changes.push((*timestamp, value.clone()));
                    last_value = Some((*timestamp, value.clone()));
                }
            }
        }

        // If we found a value before start_time but no changes in range, include it
        // with the actual timestamp when it was set
        if changes.is_empty() {
            if let Some((timestamp, value)) = last_value {
                changes.push((timestamp, value));
            }
        }

        changes
    }
}

/// Parse a VCD file and return a queryable analysis structure
pub fn parse_vcd(path: &str) -> Result<VcdAnalysis, VcdError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut parser = vcd::Parser::new(reader);

    // Parse header
    let header = parser
        .parse_header()
        .map_err(|e| VcdError::ParseError(format!("Header parse error: {}", e)))?;

    // Extract signal mappings from header
    let (id_to_name, name_to_id) = VcdAnalysis::extract_signals_from_header(&header);

    // Parse value changes
    let mut time_changes: Vec<(u64, Vec<(vcd::IdCode, VcdValue)>)> = Vec::new();
    let mut current_time: u64 = 0;
    let mut current_changes: Vec<(vcd::IdCode, VcdValue)> = Vec::new();

    for command_result in parser {
        let command = command_result
            .map_err(|e| VcdError::ParseError(format!("Command parse error: {}", e)))?;

        match command {
            vcd::Command::Timestamp(ts) => {
                // Save previous timestamp's changes if any
                if !current_changes.is_empty() {
                    time_changes.push((current_time, current_changes.clone()));
                    current_changes.clear();
                }
                current_time = ts;
            }
            vcd::Command::ChangeScalar(id, value) => {
                current_changes.push((id, VcdValue::Scalar(value)));
            }
            vcd::Command::ChangeVector(id, value) => {
                current_changes.push((id, VcdValue::Vector(value.iter().collect())));
            }
            vcd::Command::ChangeReal(id, value) => {
                current_changes.push((id, VcdValue::Real(value)));
            }
            vcd::Command::ChangeString(id, value) => {
                current_changes.push((id, VcdValue::String(value)));
            }
            vcd::Command::Upscope => {
                // This is a header command that shouldn't appear in the value change section
                // but we'll handle it gracefully
            }
            _ => {}
        }
    }

    // Don't forget the last batch of changes
    if !current_changes.is_empty() {
        time_changes.push((current_time, current_changes));
    }

    Ok(VcdAnalysis {
        header,
        id_to_name,
        name_to_id,
        time_changes,
    })
}

/// Get all signals in a given scope (or all signals if scope is None)
pub fn filter_signals_by_scope(analysis: &VcdAnalysis, scope_filter: Option<&str>) -> Vec<String> {
    let mut signals: Vec<String> = analysis.name_to_id.keys().cloned().collect();

    if let Some(scope) = scope_filter {
        let prefix = format!("{}.", scope);
        signals.retain(|name| name.starts_with(&prefix) || name == scope);
    }

    signals.sort();
    signals
}
