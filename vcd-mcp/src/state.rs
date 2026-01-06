use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::domain::VcdAnalysis;

/// Application state containing an in-memory cache of VCD analyses.
///
/// Note: This is a simple unbounded `HashMap` protected by an `RwLock`,
/// not an LRU cache. Entries are never evicted automatically, so callers
/// must ensure the cache does not grow without bound in long-running
/// processes.
#[derive(Clone, Debug)]
pub struct AppState {
    /// Cache of parsed VCD files, keyed by file path
    pub cache: Arc<RwLock<HashMap<String, Arc<VcdAnalysis>>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
