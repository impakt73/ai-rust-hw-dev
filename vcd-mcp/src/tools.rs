use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Validates that a file path has a .vcd extension
fn validate_vcd_path(path: &str) -> Result<(), String> {
    if !path.ends_with(".vcd") {
        return Err(format!(
            "File path must have a .vcd extension, got: {}",
            path
        ));
    }
    Ok(())
}

/// Arguments for inspecting a VCD file header
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct InspectHeaderArgs {
    /// Absolute path to the VCD file (must have .vcd extension)
    pub file_path: String,
}

impl InspectHeaderArgs {
    pub fn validate(&self) -> Result<(), String> {
        validate_vcd_path(&self.file_path)
    }
}

/// Arguments for listing signals in a VCD file
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ListSignalsArgs {
    /// Absolute path to the VCD file (must have .vcd extension)
    pub file_path: String,
    /// Optional scope filter (e.g., "top.cpu") to list only signals within that module
    pub scope_filter: Option<String>,
}

impl ListSignalsArgs {
    pub fn validate(&self) -> Result<(), String> {
        validate_vcd_path(&self.file_path)
    }
}

/// Arguments for getting signal values from a VCD file
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct GetValuesArgs {
    /// Absolute path to the VCD file (must have .vcd extension)
    pub file_path: String,
    /// Full hierarchical names of signals to query
    pub signal_names: Vec<String>,
    /// Simulation start timestamp
    pub start_time: u64,
    /// Optional end timestamp. If provided, returns all changes between start and end.
    /// If omitted, returns the value exactly at start_time.
    pub end_time: Option<u64>,
}

impl GetValuesArgs {
    pub fn validate(&self) -> Result<(), String> {
        validate_vcd_path(&self.file_path)
    }
}
