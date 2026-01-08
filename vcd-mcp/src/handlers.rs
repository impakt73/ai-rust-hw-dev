use anyhow::{anyhow, Result};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::domain::{filter_signals_by_scope, parse_vcd, EdgeType, VcdAnalysis};
use crate::state::AppState;
use crate::tools::{
    CountEdgesArgs, GetFileInfoArgs, GetSignalSummaryArgs, GetValuesArgs, InspectHeaderArgs,
    ListSignalsArgs,
};

// Mutex to coordinate concurrent parsing of the same file
lazy_static::lazy_static! {
    static ref PARSE_LOCKS: Arc<Mutex<std::collections::HashMap<String, Arc<Mutex<()>>>>> =
        Arc::new(Mutex::new(std::collections::HashMap::new()));
}

/// Helper function to get or parse a VCD file, using the cache.
/// Uses a check-lock-check pattern to avoid parsing the same file multiple times concurrently.
async fn get_or_parse(state: &AppState, file_path: String) -> Result<Arc<VcdAnalysis>> {
    // First check: read lock (fast path)
    {
        let cache = state.cache.read().await;
        if let Some(analysis) = cache.get(&file_path) {
            return Ok(Arc::clone(analysis));
        }
    }

    // Get or create a lock for this specific file path
    let file_lock = {
        let mut locks = PARSE_LOCKS.lock().await;
        locks
            .entry(file_path.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };

    // Acquire the file-specific lock
    let _guard = file_lock.lock().await;

    // Second check: another task might have parsed while we waited
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
        cache.insert(file_path.clone(), Arc::clone(&analysis));
    }

    // Clean up the parse lock entry
    {
        let mut locks = PARSE_LOCKS.lock().await;
        locks.remove(&file_path);
    }

    Ok(analysis)
}

/// Handler for inspect_vcd_header tool
pub async fn handle_inspect_header(
    state: AppState,
    args: InspectHeaderArgs,
) -> Result<serde_json::Value> {
    // Validate file path
    args.validate()
        .map_err(|e| anyhow!("Invalid file path: {}", e))?;

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

    // Collect top-level scopes (scopes directly under the header root)
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
    // Validate file path
    args.validate()
        .map_err(|e| anyhow!("Invalid file path: {}", e))?;

    let analysis = get_or_parse(&state, args.file_path).await?;

    let signals = filter_signals_by_scope(&analysis, args.scope_filter.as_deref());

    let result = json!({
        "signals": signals,
        "count": signals.len(),
    });

    Ok(result)
}

/// Handler for get_file_info tool
pub async fn handle_get_file_info(
    state: AppState,
    args: GetFileInfoArgs,
) -> Result<serde_json::Value> {
    // Validate file path
    args.validate()
        .map_err(|e| anyhow!("Invalid file path: {}", e))?;

    let analysis = get_or_parse(&state, args.file_path.clone()).await?;
    let metadata = analysis.get_file_metadata();

    // Get file size if possible
    let file_size_bytes = std::fs::metadata(&args.file_path).ok().map(|m| m.len());

    // Extract top-level scopes
    let mut top_scopes = Vec::new();
    for item in &analysis.header.items {
        if let vcd::ScopeItem::Scope(scope) = item {
            top_scopes.push(scope.identifier.clone());
        }
    }

    let result = json!({
        "timescale": metadata.timescale,
        "first_time": metadata.first_time,
        "last_time": metadata.last_time,
        "signal_count": metadata.signal_count,
        "top_scopes": top_scopes,
        "file_size_bytes": file_size_bytes,
    });

    Ok(result)
}

/// Handler for get_signal_summary tool
pub async fn handle_get_signal_summary(
    state: AppState,
    args: GetSignalSummaryArgs,
) -> Result<serde_json::Value> {
    // Validate args
    args.validate()
        .map_err(|e| anyhow!("Invalid arguments: {}", e))?;

    let analysis = get_or_parse(&state, args.file_path).await?;

    // Determine end time (use last timestamp if not specified)
    let end_time = args.end_time.unwrap_or_else(|| {
        analysis
            .time_changes
            .last()
            .map(|(t, _)| *t)
            .unwrap_or(u64::MAX)
    });

    // Validate that end_time >= start_time
    if end_time < args.start_time {
        return Err(anyhow!(
            "end_time ({}) must be greater than or equal to start_time ({})",
            end_time,
            args.start_time
        ));
    }

    let mut summaries = serde_json::Map::new();
    let mut missing_signals = Vec::new();

    for signal_name in &args.signal_names {
        let signal_id = match analysis.name_to_id.get(signal_name) {
            Some(id) => *id,
            None => {
                missing_signals.push(signal_name.clone());
                continue;
            }
        };

        if let Some(summary) = analysis.get_signal_summary(signal_id, args.start_time, end_time) {
            let mut summary_obj = serde_json::Map::new();
            summary_obj.insert("change_count".to_string(), json!(summary.change_count));
            summary_obj.insert("bit_width".to_string(), json!(summary.bit_width));

            if let Some(fct) = summary.first_change_time {
                summary_obj.insert("first_change_time".to_string(), json!(fct));
            }
            if let Some(lct) = summary.last_change_time {
                summary_obj.insert("last_change_time".to_string(), json!(lct));
            }
            if let Some(lv) = summary.last_value {
                summary_obj.insert("last_value".to_string(), json!(lv.format()));
            }

            summaries.insert(signal_name.clone(), json!(summary_obj));
        }
    }

    let mut result = json!({ "summaries": summaries });

    if !missing_signals.is_empty() {
        result["missing_signals"] = json!(missing_signals);
        result["warning"] = json!(format!(
            "Some signals were not found: {:?}",
            missing_signals
        ));
    }

    Ok(result)
}

/// Handler for count_signal_edges tool
pub async fn handle_count_edges(
    state: AppState,
    args: CountEdgesArgs,
) -> Result<serde_json::Value> {
    // Validate args
    args.validate()
        .map_err(|e| anyhow!("Invalid arguments: {}", e))?;

    let analysis = get_or_parse(&state, args.file_path).await?;

    // Get signal ID
    let signal_id = analysis
        .name_to_id
        .get(&args.signal_name)
        .ok_or_else(|| anyhow!("Signal not found: {}", args.signal_name))?;

    // Parse edge type
    let edge_type = match args.edge_type.as_str() {
        "rising" => EdgeType::Rising,
        "falling" => EdgeType::Falling,
        "both" => EdgeType::Both,
        _ => return Err(anyhow!("Invalid edge type: {}", args.edge_type)),
    };

    // Determine end time
    let end_time = args.end_time.unwrap_or_else(|| {
        analysis
            .time_changes
            .last()
            .map(|(t, _)| *t)
            .unwrap_or(u64::MAX)
    });

    // Validate that end_time >= start_time
    if end_time < args.start_time {
        return Err(anyhow!(
            "end_time ({}) must be greater than or equal to start_time ({})",
            end_time,
            args.start_time
        ));
    }

    let count = analysis.count_signal_edges(
        *signal_id,
        edge_type,
        args.bit_index,
        args.start_time,
        end_time,
    );

    let result = json!({
        "signal": args.signal_name,
        "edge_type": args.edge_type,
        "bit_index": args.bit_index,
        "start_time": args.start_time,
        "end_time": end_time,
        "count": count,
    });

    Ok(result)
}

/// Handler for get_signal_values tool
pub async fn handle_get_values(state: AppState, args: GetValuesArgs) -> Result<serde_json::Value> {
    // Validate file path
    args.validate()
        .map_err(|e| anyhow!("Invalid file path: {}", e))?;

    // Validate that end_time >= start_time if end_time is provided
    if let Some(end_time) = args.end_time {
        if end_time < args.start_time {
            return Err(anyhow!(
                "end_time ({}) must be greater than or equal to start_time ({})",
                end_time,
                args.start_time
            ));
        }
    }

    let analysis = get_or_parse(&state, args.file_path).await?;

    let mut results = Vec::new();
    let mut missing_signals = Vec::new();
    let mut truncated = false;

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
            let mut changes = analysis.get_signal_changes(signal_id, args.start_time, end_time);

            // If only_changes is true, filter out values from before start_time (keep only actual changes within the query range)
            if args.only_changes {
                changes.retain(|(timestamp, _)| *timestamp >= args.start_time);
            }

            // Apply limit if specified
            if let Some(limit) = args.limit {
                if changes.len() > limit {
                    changes.truncate(limit);
                    truncated = true;
                }
            }

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

    if truncated {
        response["truncated"] = json!(true);
        response["info"] = json!(format!(
            "Results were limited to {} values per signal. Use 'limit' parameter to control this.",
            args.limit.unwrap_or(0)
        ));
    }

    if !missing_signals.is_empty() {
        response["missing_signals"] = json!(missing_signals);
        response["warning"] = json!(format!(
            "Some signals were not found: {:?}",
            missing_signals
        ));
    }

    Ok(response)
}
