use anyhow::Result;
use serde_json::json;
use std::sync::Arc;

use crate::domain::{filter_signals_by_scope, parse_vcd, VcdAnalysis};
use crate::state::AppState;
use crate::tools::{GetValuesArgs, InspectHeaderArgs, ListSignalsArgs};

/// Helper function to get or parse a VCD file, using the cache
async fn get_or_parse(state: &AppState, file_path: String) -> Result<Arc<VcdAnalysis>> {
    // Check cache first (read lock)
    {
        let cache = state.cache.read().await;
        if let Some(analysis) = cache.get(&file_path) {
            return Ok(Arc::clone(analysis));
        }
    }

    // Not in cache, parse it (blocking operation)
    let path_clone = file_path.clone();
    let analysis = tokio::task::spawn_blocking(move || parse_vcd(&path_clone)).await??;

    let analysis = Arc::new(analysis);

    // Store in cache (write lock)
    {
        let mut cache = state.cache.write().await;
        cache.insert(file_path, Arc::clone(&analysis));
    }

    Ok(analysis)
}

/// Handler for inspect_vcd_header tool
pub async fn handle_inspect_header(
    state: AppState,
    args: InspectHeaderArgs,
) -> Result<serde_json::Value> {
    let analysis = get_or_parse(&state, args.file_path).await?;

    let timescale = analysis
        .header
        .timescale
        .map(|ts| format!("{} {:?}", ts.0, ts.1))
        .unwrap_or_else(|| "Not specified".to_string());

    let date = analysis.header.date.as_deref().unwrap_or("Not specified");
    let version = analysis
        .header
        .version
        .as_deref()
        .unwrap_or("Not specified");

    // Collect top-level scopes
    let mut top_scopes = Vec::new();
    for item in &analysis.header.items {
        if let vcd::ScopeItem::Scope(scope) = item {
            top_scopes.push(format!("{} ({:?})", scope.identifier, scope.scope_type));
        }
    }

    let result = json!({
        "timescale": timescale,
        "date": date,
        "version": version,
        "top_level_scopes": top_scopes,
        "total_signals": analysis.id_to_name.len(),
    });

    Ok(result)
}

/// Handler for list_signals tool
pub async fn handle_list_signals(
    state: AppState,
    args: ListSignalsArgs,
) -> Result<serde_json::Value> {
    let analysis = get_or_parse(&state, args.file_path).await?;

    let signals = filter_signals_by_scope(&analysis, args.scope_filter.as_deref());

    let result = json!({
        "signals": signals,
        "count": signals.len(),
    });

    Ok(result)
}

/// Handler for get_signal_values tool
pub async fn handle_get_values(state: AppState, args: GetValuesArgs) -> Result<serde_json::Value> {
    let analysis = get_or_parse(&state, args.file_path).await?;

    let mut results = Vec::new();
    let mut missing_signals = Vec::new();

    for signal_name in &args.signal_names {
        let signal_id = match analysis.name_to_id.get(signal_name) {
            Some(id) => *id,
            None => {
                missing_signals.push(signal_name.clone());
                continue;
            }
        };

        if let Some(end_time) = args.end_time {
            // Get all changes in range
            let changes = analysis.get_signal_changes(signal_id, args.start_time, end_time);

            for (timestamp, value) in changes {
                results.push(json!({
                    "signal": signal_name,
                    "time": timestamp,
                    "value": value.format(),
                }));
            }
        } else {
            // Get value at specific time
            if let Some(value) = analysis.get_signal_value_at(signal_id, args.start_time) {
                results.push(json!({
                    "signal": signal_name,
                    "time": args.start_time,
                    "value": value.format(),
                }));
            }
        }
    }

    let mut response = json!({
        "values": results,
    });

    if !missing_signals.is_empty() {
        response["missing_signals"] = json!(missing_signals);
        response["warning"] = json!(format!(
            "Some signals were not found: {:?}",
            missing_signals
        ));
    }

    Ok(response)
}
