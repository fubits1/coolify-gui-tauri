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

/// Per-service FQDN cached from `/services/{uuid}/envs` lookup. Coolify
/// service compose templates often declare `SERVICE_URL_<NAME>_<PORT>` as
/// an env pass-through variable; the actual URL lives only in the env
/// store, never in `docker_compose_raw`. We resolve that out-of-band
/// and cache for 60s to avoid hammering Coolify on every poll.
#[derive(Debug, Clone)]
pub struct ServiceFqdnEntry {
    pub fqdn: Option<String>,
    pub fetched_at: Instant,
}

/// Resolved project + environment metadata keyed by Coolify's integer
/// `environment_id`. Coolify's list responses for Services + Applications
/// often ship only `environment_id` without the nested `environment.uuid`
/// or `environment.project.uuid` — but the dashboard's resource pages live
/// at `/project/{project_uuid}/environment/{env_uuid}/...`. We build this
/// map by fanning out `/projects` + `/projects/{uuid}` and cache it for
/// 5 minutes since projects + environments change rarely.
#[derive(Debug, Clone)]
pub struct ProjectEnvLookup {
    pub project_uuid: String,
    pub project_name: Option<String>,
    pub environment_uuid: String,
    pub environment_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProjectEnvCache {
    pub by_env_id: HashMap<i64, ProjectEnvLookup>,
    pub fetched_at: Instant,
}

/// Tauri-managed application state. EVERY field is keyed by
/// `instance_id` (an opaque UUID generated client-side at instance-add
/// time). Today's single-tenant app becomes a degenerate one-entry case;
/// multi-instance simply populates more entries.
pub struct AppState {
    pub clients: RwLock<HashMap<String, CoolifyClient>>,
    pub deploy_cache: RwLock<HashMap<String, HashMap<String, DeployCacheEntry>>>,
    pub service_fqdn_cache: RwLock<HashMap<String, HashMap<String, ServiceFqdnEntry>>>,
    pub project_env_cache: RwLock<HashMap<String, ProjectEnvCache>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            clients: RwLock::new(HashMap::new()),
            deploy_cache: RwLock::new(HashMap::new()),
            service_fqdn_cache: RwLock::new(HashMap::new()),
            project_env_cache: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
