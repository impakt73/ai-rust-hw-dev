use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Arguments for inspecting a VCD file header
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct InspectHeaderArgs {
    /// Absolute path to the VCD file
    pub file_path: String,
}

/// Arguments for listing signals in a VCD file
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ListSignalsArgs {
    /// Absolute path to the VCD file
    pub file_path: String,
    /// Optional scope filter (e.g., "top.cpu") to list only signals within that module
    pub scope_filter: Option<String>,
}

/// Arguments for getting signal values from a VCD file
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct GetValuesArgs {
    /// Absolute path to the VCD file
    pub file_path: String,
    /// Full hierarchical names of signals to query
    pub signal_names: Vec<String>,
    /// Simulation start timestamp
    pub start_time: u64,
    /// Optional end timestamp. If provided, returns all changes between start and end.
    /// If omitted, returns the value exactly at start_time.
    pub end_time: Option<u64>,
}
