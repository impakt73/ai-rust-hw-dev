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
    /// If true, only return values when the signal actually changed (not initial values at start_time)
    #[serde(default)]
    pub only_changes: bool,
    /// Maximum number of value changes to return per signal (for pagination)
    pub limit: Option<usize>,
}

impl GetValuesArgs {
    pub fn validate(&self) -> Result<(), String> {
        validate_vcd_path(&self.file_path)
    }
}

/// Arguments for getting file metadata
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct GetFileInfoArgs {
    /// Absolute path to the VCD file (must have .vcd extension)
    pub file_path: String,
}

impl GetFileInfoArgs {
    pub fn validate(&self) -> Result<(), String> {
        validate_vcd_path(&self.file_path)
    }
}

/// Arguments for getting signal summary statistics
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct GetSignalSummaryArgs {
    /// Absolute path to the VCD file (must have .vcd extension)
    pub file_path: String,
    /// Signal names to get summaries for
    pub signal_names: Vec<String>,
    /// Start time for the summary range (default: 0)
    #[serde(default)]
    pub start_time: u64,
    /// End time for the summary range (default: end of simulation)
    pub end_time: Option<u64>,
}

impl GetSignalSummaryArgs {
    pub fn validate(&self) -> Result<(), String> {
        validate_vcd_path(&self.file_path)?;
        if self.signal_names.is_empty() {
            return Err("At least one signal name must be provided".to_string());
        }
        Ok(())
    }
}

/// Arguments for counting signal edges
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct CountEdgesArgs {
    /// Absolute path to the VCD file (must have .vcd extension)
    pub file_path: String,
    /// Signal name to analyze
    pub signal_name: String,
    /// Type of edge to count: "rising", "falling", or "both"
    #[serde(default = "default_edge_type")]
    pub edge_type: String,
    /// For vector signals, which bit to analyze (0 = LSB). Omit for scalar signals.
    pub bit_index: Option<usize>,
    /// Start time for counting (default: 0)
    #[serde(default)]
    pub start_time: u64,
    /// End time for counting (default: end of simulation)
    pub end_time: Option<u64>,
}

fn default_edge_type() -> String {
    "rising".to_string()
}

impl CountEdgesArgs {
    pub fn validate(&self) -> Result<(), String> {
        validate_vcd_path(&self.file_path)?;
        match self.edge_type.as_str() {
            "rising" | "falling" | "both" => Ok(()),
            _ => Err(format!(
                "Invalid edge_type '{}'. Must be 'rising', 'falling', or 'both'",
                self.edge_type
            )),
        }
    }
}
