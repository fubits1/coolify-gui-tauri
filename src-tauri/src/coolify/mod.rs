pub mod client;
pub mod ops;
pub mod types;

use std::collections::HashMap;
use std::time::Instant;

use chrono::{DateTime, Utc};
use tokio::sync::RwLock;

use client::CoolifyClient;

/// One entry in the last-deployment cache. Coolify's
/// `/deployments/applications/{uuid}?take=1` is a fresh round-trip per app,
/// so we cache the result for 60s instead of re-fetching every 5s poll.
#[derive(Debug, Clone)]
pub struct DeployCacheEntry {
    pub last_deployed_at: Option<DateTime<Utc>>,
    pub fetched_at: Instant,
}

/// Tauri-managed application state holding the active Coolify HTTP client
/// and the per-app last-deployment cache.
pub struct AppState {
    pub client: RwLock<Option<CoolifyClient>>,
    pub deploy_cache: RwLock<HashMap<String, DeployCacheEntry>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            client: RwLock::new(None),
            deploy_cache: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
