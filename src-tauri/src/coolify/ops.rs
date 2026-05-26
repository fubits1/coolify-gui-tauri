use serde::Serialize;
use tauri::State;

use std::collections::HashMap;
use std::time::Instant;

use super::client::CoolifyClient;
use super::types::{
    parse_status, EnvVar, HealthCheck, RawApplication, RawDatabase, RawEnvVar, RawService,
    Resource, ResourceDetail, ResourceKind,
};
use super::{AppState, ProjectEnvCache, ProjectEnvLookup};

/// Result of an onboarding "Test connection" probe.
///
/// `ok=true` means both `/health` (no auth) and `/teams` (with token)
/// returned 2xx. `team_name` populates the connection-strip label.
#[derive(Debug, Serialize)]
pub struct TestConnectionResult {
    pub ok: bool,
    pub version: Option<String>,
    pub team_name: Option<String>,
}

/// Bundle returned by `list_resources` so the frontend can surface
/// per-endpoint failures without losing the resources that succeeded.
#[derive(Debug, Serialize)]
pub struct ListResourcesResult {
    pub resources: Vec<Resource>,
    /// Map of `endpoint -> error string` for endpoints that failed.
    /// Empty when everything was 2xx + decoded cleanly.
    pub errors: std::collections::HashMap<String, String>,
}

/// GET `/api/v1/health` then `/api/v1/teams` to validate a `{url, token}` pair.
///
/// Used by the onboarding screen before persisting the token to the keyring.
/// We deliberately swallow the structured error and return `ok=false` instead
/// of bubbling it — the frontend renders a toast either way.
#[tauri::command]
pub async fn test_connection(url: String, token: String) -> Result<TestConnectionResult, String> {
    let health_body = match CoolifyClient::get_unauthenticated_health(&url).await {
        Ok(b) => b,
        Err(e) => return Err(e.to_string()),
    };
    let version = extract_version(&health_body);

    let client = CoolifyClient::new(&url, &token).map_err(|e| e.to_string())?;
    let teams: serde_json::Value = client
        .get("api/v1/teams")
        .await
        .map_err(|e| e.to_string())?;
    let team_name = extract_first_team_name(&teams);

    Ok(TestConnectionResult {
        ok: true,
        version,
        team_name,
    })
}

/// Construct a fresh `CoolifyClient` and store it in `AppState`.
///
/// Called after onboarding succeeds. Persists the token to the OS keyring
/// under `alias` (default: `"default"`) so subsequent boots can rehydrate
/// the client via `load_credentials`. Replaces any existing client
/// (handles token rotation).
#[tauri::command]
pub async fn set_credentials(
    url: String,
    token: String,
    alias: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let alias = alias.unwrap_or_else(|| "default".to_string());
    crate::secrets::save_token(&alias, &token)?;
    let client = CoolifyClient::new(&url, &token).map_err(|e| e.to_string())?;
    let mut guard = state.client.write().await;
    *guard = Some(client);
    Ok(())
}

/// Rehydrate the `CoolifyClient` from a token stored in the OS keyring.
///
/// Called on app boot once the persisted `{url, alias}` has been read from
/// the plugin-store. Returns `true` if a token was found and the client is
/// now live; `false` if no token exists (the frontend then shows
/// `ConnectScreen`). Errors surface only on keyring access failures.
#[tauri::command]
pub async fn load_credentials(
    url: String,
    alias: Option<String>,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    // Fast path: if a client is already built for this process, return ok
    // without touching the OS keyring. Prevents a Keychain prompt on every
    // Svelte HMR / Cmd+R during dev iteration.
    {
        let guard = state.client.read().await;
        if guard.is_some() {
            return Ok(true);
        }
    }

    let alias = alias.unwrap_or_else(|| "default".to_string());
    match crate::secrets::load_token(&alias)? {
        Some(token) => {
            let client = CoolifyClient::new(&url, &token).map_err(|e| e.to_string())?;
            let mut guard = state.client.write().await;
            *guard = Some(client);
            Ok(true)
        }
        None => Ok(false),
    }
}

/// Sign out: drop the stored token from the OS keyring and clear in-memory
/// client state. Frontend is expected to also wipe the `instance` store
/// (URL + alias) so the next boot drops the user back at `ConnectScreen`.
#[tauri::command]
pub async fn clear_credentials(
    alias: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let alias = alias.unwrap_or_else(|| "default".to_string());
    crate::secrets::delete_token(&alias)?;
    *state.client.write().await = None;
    Ok(())
}

/// Fan out to `/applications`, `/services`, `/databases` in parallel and merge.
///
/// Returns one flat `Vec<Resource>` for the overview table. Each list is
/// independently fetched via `tokio::join!`; if one endpoint errors the
/// whole call fails (callers retry via the polling loop).
#[tauri::command]
pub async fn list_resources(
    state: State<'_, AppState>,
) -> Result<ListResourcesResult, String> {
    let client = clone_client(&state).await?;
    let apps_fut = client.get::<Vec<RawApplication>>("api/v1/applications");
    let svcs_fut = client.get::<Vec<RawService>>("api/v1/services");
    let dbs_fut = client.get::<Vec<RawDatabase>>("api/v1/databases");

    let (apps, svcs, dbs) = tokio::join!(apps_fut, svcs_fut, dbs_fut);

    let mut out: Vec<Resource> = Vec::new();
    let mut errors: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    match apps {
        Ok(list) => {
            tracing::debug!("list_resources: /applications -> {} items", list.len());
            // Pull in real last-deployment timestamps (60s cache) so the
            // overview shows when each app actually deployed, not the
            // last_online_at heartbeat which is constantly refreshed.
            let resources: Vec<Resource> = list
                .into_iter()
                .map(RawApplication::into_resource)
                .collect();
            let uuids: Vec<String> = resources.iter().map(|r| r.uuid.clone()).collect();
            let deploys = fetch_last_deployments(&client, &state, &uuids).await;
            for mut r in resources {
                // Only OVERRIDE the updated_at fallback when the deploys
                // lookup actually returned a timestamp. A None means
                // either no deploy history yet OR a transient 429 — in
                // both cases the updated_at value is better than "—".
                if let Some(Some(ts)) = deploys.get(&r.uuid).cloned() {
                    r.last_deployed_at = Some(ts);
                }
                out.push(r);
            }
        }
        Err(e) => {
            tracing::warn!("list_resources: /applications failed: {}", e);
            errors.insert("applications".to_string(), e.to_string());
        }
    }
    match svcs {
        Ok(list) => {
            tracing::debug!("list_resources: /services -> {} items", list.len());
            let resources: Vec<Resource> = list.into_iter().map(RawService::into_resource).collect();
            // For services whose FQDN we couldn't scrape from the compose YAML
            // (Coolify nocodb-style templates only declare SERVICE_URL_* as
            // env passthroughs), fall back to the per-service envs endpoint
            // and look for a URL there.
            let needing_fqdn: Vec<String> = resources
                .iter()
                .filter(|r| r.fqdn.is_none() || r.fqdn.as_deref().map(|s| s.is_empty()).unwrap_or(true))
                .map(|r| r.uuid.clone())
                .collect();
            let resolved = fetch_service_fqdns(&client, &state, &needing_fqdn).await;
            for mut r in resources {
                if r.fqdn.as_deref().map(|s| s.is_empty()).unwrap_or(true) {
                    if let Some(Some(url)) = resolved.get(&r.uuid).cloned() {
                        r.fqdn = Some(url);
                    }
                }
                out.push(r);
            }
        }
        Err(e) => {
            tracing::warn!("list_resources: /services failed: {}", e);
            errors.insert("services".to_string(), e.to_string());
        }
    }
    match dbs {
        Ok(list) => {
            tracing::debug!("list_resources: /databases -> {} items", list.len());
            out.extend(list.into_iter().map(RawDatabase::into_resource));
        }
        Err(e) => {
            tracing::warn!("list_resources: /databases failed: {}", e);
            errors.insert("databases".to_string(), e.to_string());
        }
    }

    // Enrich resources with project_uuid + environment_uuid via the
    // /projects → /projects/{uuid} fan-out. Coolify's list responses for
    // Services + Databases ship only `environment_id` (int) without the
    // nested env+project UUIDs the dashboard deep-link URL needs.
    enrich_project_env(&client, &state, &mut out).await;

    if out.is_empty() && !errors.is_empty() {
        let combined = errors
            .iter()
            .map(|(k, v)| format!("{}: {}", k, v))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(combined);
    }
    Ok(ListResourcesResult {
        resources: out,
        errors,
    })
}

/// Build (and cache, 5 min) a `environment_id → (project_uuid, env_uuid, …)`
/// map by fanning out `/projects` then `/projects/{uuid}` per project.
///
/// Used to fill `project_uuid` + `environment_uuid` on Resources whose
/// list response stripped the nested `environment.project` object (common
/// for Services + Databases). Cached aggressively because projects +
/// environments rarely change.
async fn build_project_env_cache(client: &CoolifyClient) -> Option<ProjectEnvCache> {
    let projects: Vec<serde_json::Value> = match client.get("api/v1/projects").await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("project_env_cache: /projects failed: {}", e);
            return None;
        }
    };
    let mut by_env_id: HashMap<i64, ProjectEnvLookup> = HashMap::new();
    for proj in projects {
        let proj_uuid = match proj.get("uuid").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let proj_name = proj
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let path = format!("api/v1/projects/{}", proj_uuid);
        let detail: serde_json::Value = match client.get(&path).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("project_env_cache: {} failed: {}", path, e);
                continue;
            }
        };
        let envs = match detail.get("environments").and_then(|v| v.as_array()) {
            Some(a) => a,
            None => continue,
        };
        for env in envs {
            let env_id = match env.get("id").and_then(|v| v.as_i64()) {
                Some(n) => n,
                None => continue,
            };
            let env_uuid = match env.get("uuid").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };
            let env_name = env
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            by_env_id.insert(
                env_id,
                ProjectEnvLookup {
                    project_uuid: proj_uuid.clone(),
                    project_name: proj_name.clone(),
                    environment_uuid: env_uuid,
                    environment_name: env_name,
                },
            );
        }
    }
    Some(ProjectEnvCache {
        by_env_id,
        fetched_at: Instant::now(),
    })
}

/// Fill `project_uuid` + `environment_uuid` on each Resource whose
/// list-response shape only carried `environment_id`. Cache TTL 5 min.
async fn enrich_project_env(client: &CoolifyClient, state: &State<'_, AppState>, out: &mut [Resource]) {
    const TTL_SECS: u64 = 300;
    let cached: Option<ProjectEnvCache> = {
        let guard = state.project_env_cache.read().await;
        guard.as_ref().and_then(|c| {
            if c.fetched_at.elapsed().as_secs() < TTL_SECS {
                Some(c.clone())
            } else {
                None
            }
        })
    };
    let cache = match cached {
        Some(c) => c,
        None => match build_project_env_cache(client).await {
            Some(c) => {
                *state.project_env_cache.write().await = Some(c.clone());
                c
            }
            None => return,
        },
    };
    for r in out.iter_mut() {
        let env_id = match r.environment_id {
            Some(id) => id,
            None => continue,
        };
        let lookup = match cache.by_env_id.get(&env_id) {
            Some(l) => l,
            None => continue,
        };
        if r.project_uuid.is_none() {
            r.project_uuid = Some(lookup.project_uuid.clone());
        }
        if r.project_name.is_none() {
            r.project_name = lookup.project_name.clone();
        }
        if r.environment_uuid.is_none() {
            r.environment_uuid = Some(lookup.environment_uuid.clone());
        }
        if r.environment_name.is_none() {
            r.environment_name = lookup.environment_name.clone();
        }
    }
}

/// Fetch one Resource by `{kind, uuid}` and expand its detail fields.
///
/// `kind` is the lowercase singular discriminator the frontend already
/// carries on the table row (`"application" | "service" | "database"`).
#[tauri::command]
pub async fn get_resource_detail(
    uuid: String,
    kind: String,
    state: State<'_, AppState>,
) -> Result<ResourceDetail, String> {
    let resource_kind = ResourceKind::from_str(&kind)
        .ok_or_else(|| format!("unknown resource kind: {}", kind))?;
    let client = clone_client(&state).await?;
    let path = format!("api/v1/{}/{}", resource_kind.path_segment(), uuid);
    let raw: serde_json::Value = client.get(&path).await.map_err(|e| e.to_string())?;
    Ok(build_detail(raw, resource_kind))
}

/// Fetch env vars for a resource. Separate command so the detail pane can
/// render IMMEDIATELY after `get_resource_detail` returns, and the EnvTab
/// can populate in a second pass — Coolify's `/envs` endpoint is slow
/// enough to block detail rendering for several seconds when bundled.
#[tauri::command]
pub async fn get_resource_envs(
    uuid: String,
    kind: String,
    state: State<'_, AppState>,
) -> Result<Vec<EnvVar>, String> {
    let resource_kind = ResourceKind::from_str(&kind)
        .ok_or_else(|| format!("unknown resource kind: {}", kind))?;
    let client = clone_client(&state).await?;
    let envs_path = format!(
        "api/v1/{}/{}/envs",
        resource_kind.path_segment(),
        uuid
    );
    match client.get::<serde_json::Value>(&envs_path).await {
        Ok(v) => Ok(parse_envs(&v)),
        Err(e) => {
            tracing::warn!("/envs fetch failed for {}/{}: {}", kind, uuid, e);
            Err(e.to_string())
        }
    }
}

/// Parse Coolify's `/envs` response into our `EnvVar` shape. The endpoint
/// returns `[{ key, value, real_value, is_shown_once, ... }]` — we map
/// `key` straight through and prefer `real_value` (full token) over
/// `value` (masked) since the UI handles its own masking.
fn parse_envs(v: &serde_json::Value) -> Vec<EnvVar> {
    let arr = v
        .as_array()
        .or_else(|| v.get("data").and_then(|d| d.as_array()))
        .or_else(|| v.get("envs").and_then(|d| d.as_array()))
        .or_else(|| v.get("environment_variables").and_then(|d| d.as_array()));
    let arr = match arr {
        Some(a) => a,
        None => return Vec::new(),
    };
    let bool_or = |item: &serde_json::Value, key: &str, default: bool| -> bool {
        item.get(key)
            .and_then(|x| x.as_bool())
            .unwrap_or(default)
    };
    arr.iter()
        .filter_map(|item| {
            let key = item.get("key")?.as_str()?.to_string();
            let value = item
                .get("real_value")
                .and_then(|x| x.as_str())
                .or_else(|| item.get("value").and_then(|x| x.as_str()))
                .unwrap_or("")
                .to_string();
            let is_secret = bool_or(item, "is_shown_once", true);
            let is_preview = bool_or(item, "is_preview", false);
            let is_buildtime = bool_or(item, "is_buildtime", false);
            // Coolify defaults runtime=true unless explicitly false.
            let is_runtime = bool_or(item, "is_runtime", true);
            let is_shared = bool_or(item, "is_shared", false);
            Some(EnvVar {
                key,
                value,
                is_secret,
                is_preview,
                is_buildtime,
                is_runtime,
                is_shared,
            })
        })
        .collect()
}

/// POST-equivalent: Coolify exposes `/restart` as GET. No body.
///
/// Databases share this endpoint shape via `/databases/{uuid}/restart`,
/// but per the spec only Applications + Services route through here in v1.
#[tauri::command]
pub async fn restart_resource(
    uuid: String,
    kind: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let segment = action_segment(&kind)?;
    let client = clone_client(&state).await?;
    let path = format!("api/v1/{}/{}/restart", segment, uuid);
    client.get_raw(&path).await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Stop a running Resource. Mirrors `restart_resource` in transport.
#[tauri::command]
pub async fn stop_resource(
    uuid: String,
    kind: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let segment = action_segment(&kind)?;
    let client = clone_client(&state).await?;
    let path = format!("api/v1/{}/{}/stop", segment, uuid);
    client.get_raw(&path).await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Trigger a Deploy. Coolify takes `uuid` + `force` as query params, not body.
///
/// `force=true` maps to the "Force rebuild" checkbox in the confirm dialog
/// — pulls fresh source / rebuilds the image even if nothing upstream changed.
#[tauri::command]
pub async fn deploy_resource(
    uuid: String,
    force: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let client = clone_client(&state).await?;
    let path = format!("api/v1/deploy?uuid={}&force={}", uuid, force);
    client.get_raw(&path).await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Debug: dump raw body + status length of /applications, /services, /databases.
///
/// Lets us see exactly what Coolify is returning when the typed parser yields
/// empty results — wraps, status codes, shape mismatches all become visible.
/// Frontend invokes via `api.debugDumpEndpoints()`.
#[tauri::command]
pub async fn debug_dump_endpoints(
    state: State<'_, AppState>,
) -> Result<std::collections::HashMap<String, String>, String> {
    let client = clone_client(&state).await?;
    let mut out = std::collections::HashMap::new();
    for path in ["api/v1/applications", "api/v1/services", "api/v1/databases"] {
        let result = client.get_raw(path).await;
        let body = match result {
            Ok(b) => {
                tracing::info!("debug_dump {} ok: {} bytes", path, b.len());
                let preview = if b.len() > 4000 {
                    format!("{}…(truncated, total {} bytes)", &b[..4000], b.len())
                } else {
                    b
                };
                preview
            }
            Err(e) => {
                tracing::warn!("debug_dump {} err: {}", path, e);
                format!("ERROR: {}", e)
            }
        };
        out.insert(path.to_string(), body);
    }
    Ok(out)
}

/// Fetch the last N lines of logs for a Resource as plain text.
///
/// Caps `lines` at 5000 to keep payloads bounded; the Logs tab defaults
/// to 500 per the locked design.
#[tauri::command]
pub async fn tail_logs(
    uuid: String,
    kind: String,
    lines: u32,
    container: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let capped = lines.min(5000);
    let client = clone_client(&state).await?;

    // The Coolify v1 API only exposes a logs endpoint under
    // /applications/{uuid}/logs. Services + Databases have NO documented
    // logs endpoint. The dashboard streams container logs out-of-band
    // (likely SSH/docker-exec through the Soketi WebSocket). For now,
    // surface this gap explicitly rather than 404-chasing.
    let resource_kind = ResourceKind::from_str(&kind)
        .ok_or_else(|| format!("unknown resource kind: {}", kind))?;
    let path = match resource_kind {
        ResourceKind::Application => {
            // Coolify v1 only documents `lines`. Adding `&timestamps=true`
            // causes network errors against `cf.fubits.dev` (verified via
            // tracing logs) — likely a routing/CSRF mismatch. Drop it; the
            // frontend `humanizeTimestamps` still normalizes any RFC 3339
            // prefixes the container itself ships.
            format!("api/v1/applications/{}/logs?lines={}", uuid, capped)
        }
        ResourceKind::Service | ResourceKind::Database => {
            let _ = container; // ignored — see above
            return Err(format!(
                "The Coolify v1 API does not expose a logs endpoint for {} resources. \
                 Use the Coolify dashboard for now.",
                if matches!(resource_kind, ResourceKind::Service) {
                    "service"
                } else {
                    "database"
                }
            ));
        }
    };

    let raw: Result<serde_json::Value, _> = client.get(&path).await;
    match raw {
        Ok(v) => Ok(v
            .get("logs")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string())
            .unwrap_or_default()),
        Err(crate::coolify::client::CoolifyError::NotFound) => {
            Err(format!("404 at {}", path))
        }
        Err(e) => Err(e.to_string()),
    }
}

// ── helpers ─────────────────────────────────────────────────────────────────

async fn clone_client(state: &State<'_, AppState>) -> Result<CoolifyClient, String> {
    let guard = state.client.read().await;
    guard
        .as_ref()
        .cloned()
        .ok_or_else(|| "no Coolify credentials set — call set_credentials first".to_string())
}

/// Pull the most-recent deployment timestamp per application uuid, with a
/// 60s in-memory cache to avoid re-hitting Coolify on every 5s poll.
///
/// Endpoint: `GET /deployments/applications/{uuid}?take=1` returns the
/// freshest deployment record (uses Coolify's own pagination). We only need
/// `created_at` from it.
async fn fetch_last_deployments(
    client: &CoolifyClient,
    state: &State<'_, AppState>,
    uuids: &[String],
) -> std::collections::HashMap<String, Option<chrono::DateTime<chrono::Utc>>> {
    use std::time::{Duration, Instant};

    // 5min cache — Coolify behind Cloudflare 429s aggressively when we hit
    // /deployments/applications/{uuid} per resource on every list refresh.
    const TTL: Duration = Duration::from_secs(300);
    let now = Instant::now();

    // Phase 1: drain anything still fresh from the cache.
    let mut out: std::collections::HashMap<
        String,
        Option<chrono::DateTime<chrono::Utc>>,
    > = std::collections::HashMap::new();
    let mut to_fetch: Vec<String> = Vec::new();
    {
        let cache = state.deploy_cache.read().await;
        for uuid in uuids {
            match cache.get(uuid) {
                Some(entry) if now.duration_since(entry.fetched_at) < TTL => {
                    out.insert(uuid.clone(), entry.last_deployed_at);
                }
                _ => to_fetch.push(uuid.clone()),
            }
        }
    }

    if to_fetch.is_empty() {
        return out;
    }

    // Phase 2: fan out the remaining uuids in parallel.
    let mut futs = Vec::with_capacity(to_fetch.len());
    for uuid in &to_fetch {
        let client = client.clone();
        let uuid = uuid.clone();
        futs.push(async move {
            // take=10: the most recent record may be queued/in_progress/failed,
            // none of which mean "this is the version actually running". Fetch
            // a small window and pick the most recent FINISHED deployment.
            let path = format!("api/v1/deployments/applications/{}?take=10", uuid);
            let res: Result<serde_json::Value, _> = client.get(&path).await;
            let ts = match res {
                Ok(v) => extract_last_finished_deployment_timestamp(&v),
                Err(e) => {
                    tracing::warn!("last_deploy fetch failed for {}: {}", uuid, e);
                    None
                }
            };
            (uuid, ts)
        });
    }
    let results = futures::future::join_all(futs).await;

    // Phase 3: write back to cache + accumulate output.
    let mut cache = state.deploy_cache.write().await;
    let fetched_at = Instant::now();
    for (uuid, ts) in results {
        cache.insert(
            uuid.clone(),
            super::DeployCacheEntry {
                last_deployed_at: ts,
                fetched_at,
            },
        );
        out.insert(uuid, ts);
    }
    out
}

/// Scan a `/deployments/applications/{uuid}` response and return the
/// `created_at` of the most recent deployment whose `status` is
/// `"finished"`. Skips `queued`, `in_progress`, `failed`, and
/// `cancelled-by-user` records — none of those reflect "the version
/// currently running". Returns None when no finished record is in the
/// window (e.g. app never successfully deployed, or all returned records
/// are queued/in-progress).
///
/// Accepts either a bare JSON array OR `{data: [...]}` wrapper shape.
/// Records are assumed to be returned newest-first (Coolify's default
/// ordering); we still walk linearly to be defensive.
fn extract_last_finished_deployment_timestamp(
    v: &serde_json::Value,
) -> Option<chrono::DateTime<chrono::Utc>> {
    let arr = v
        .as_array()
        .or_else(|| v.get("data").and_then(|d| d.as_array()))?;
    for item in arr {
        let status = item.get("status").and_then(|x| x.as_str()).unwrap_or("");
        if status != "finished" {
            continue;
        }
        if let Some(ts) = item
            .get("created_at")
            .and_then(|x| x.as_str())
            .and_then(parse_loose_datetime)
        {
            return Some(ts);
        }
    }
    None
}

/// Look up `SERVICE_URL_*` / `SERVICE_FQDN_*` env-var VALUES for each
/// service uuid (via `/services/{uuid}/envs`), cached for 60s. Returns
/// `uuid -> Some(url)` when a URL-looking value is found, else `None`.
async fn fetch_service_fqdns(
    client: &CoolifyClient,
    state: &State<'_, AppState>,
    uuids: &[String],
) -> std::collections::HashMap<String, Option<String>> {
    use std::time::{Duration, Instant};
    const TTL: Duration = Duration::from_secs(60);
    let now = Instant::now();

    let mut out: std::collections::HashMap<String, Option<String>> =
        std::collections::HashMap::new();
    let mut to_fetch: Vec<String> = Vec::new();
    {
        let cache = state.service_fqdn_cache.read().await;
        for uuid in uuids {
            match cache.get(uuid) {
                Some(entry) if now.duration_since(entry.fetched_at) < TTL => {
                    out.insert(uuid.clone(), entry.fqdn.clone());
                }
                _ => to_fetch.push(uuid.clone()),
            }
        }
    }

    if to_fetch.is_empty() {
        return out;
    }

    let mut futs = Vec::with_capacity(to_fetch.len());
    for uuid in &to_fetch {
        let client = client.clone();
        let uuid = uuid.clone();
        futs.push(async move {
            let path = format!("api/v1/services/{}/envs", uuid);
            let res: Result<serde_json::Value, _> = client.get(&path).await;
            let fqdn = match res {
                Ok(v) => extract_service_url_from_envs(&v),
                Err(_) => None,
            };
            (uuid, fqdn)
        });
    }
    let results = futures::future::join_all(futs).await;

    let mut cache = state.service_fqdn_cache.write().await;
    let fetched_at = Instant::now();
    for (uuid, fqdn) in results {
        cache.insert(
            uuid.clone(),
            super::ServiceFqdnEntry {
                fqdn: fqdn.clone(),
                fetched_at,
            },
        );
        out.insert(uuid, fqdn);
    }
    out
}

/// Walk an `/envs` response (bare array of env-var objects) and pull the
/// first value that LOOKS like a URL out of a `SERVICE_URL_*` or
/// `SERVICE_FQDN_*` key. Loopback hosts are filtered out — those are
/// health-check internal URLs, not the user-facing FQDN.
fn extract_service_url_from_envs(v: &serde_json::Value) -> Option<String> {
    let arr = v
        .as_array()
        .or_else(|| v.get("data").and_then(|d| d.as_array()))?;
    let mut prod_url: Option<String> = None;
    let mut any_url: Option<String> = None;
    for item in arr {
        let key = match item.get("key").and_then(|x| x.as_str()) {
            Some(k) => k,
            None => continue,
        };
        let upper = key.to_ascii_uppercase();
        if !upper.starts_with("SERVICE_URL_") && !upper.starts_with("SERVICE_FQDN_") {
            continue;
        }
        let value = item
            .get("real_value")
            .and_then(|x| x.as_str())
            .or_else(|| item.get("value").and_then(|x| x.as_str()))
            .unwrap_or("")
            .trim();
        if value.is_empty() {
            continue;
        }
        // Coerce bare hostnames into https:// URLs.
        let url = if value.starts_with("http://") || value.starts_with("https://") {
            value.to_string()
        } else {
            format!("https://{}", value)
        };
        // Skip loopback.
        let lower = url.to_ascii_lowercase();
        if lower.contains("://localhost")
            || lower.contains("://127.0.0.1")
            || lower.contains("://0.0.0.0")
        {
            continue;
        }
        // Prefer production scope (is_preview=false) over preview duplicates.
        let is_preview = item
            .get("is_preview")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        if !is_preview && prod_url.is_none() {
            prod_url = Some(url.clone());
        }
        if any_url.is_none() {
            any_url = Some(url);
        }
    }
    prod_url.or(any_url)
}

fn parse_loose_datetime(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(d) = chrono::DateTime::parse_from_rfc3339(trimmed) {
        return Some(d.with_timezone(&chrono::Utc));
    }
    if let Ok(d) = chrono::NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S") {
        return Some(d.and_utc());
    }
    if let Ok(d) = chrono::NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S") {
        return Some(d.and_utc());
    }
    None
}

/// Only Applications + Services accept restart/stop/logs in v1.
fn action_segment(kind: &str) -> Result<&'static str, String> {
    match ResourceKind::from_str(kind) {
        Some(ResourceKind::Application) => Ok("applications"),
        Some(ResourceKind::Service) => Ok("services"),
        Some(ResourceKind::Database) => Ok("databases"),
        None => Err(format!("unknown resource kind: {}", kind)),
    }
}

fn extract_version(health_body: &str) -> Option<String> {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(health_body) {
        if let Some(s) = v.get("version").and_then(|x| x.as_str()) {
            return Some(s.to_string());
        }
    }
    // Some Coolify versions return a bare plaintext "OK" or similar.
    let trimmed = health_body.trim();
    if !trimmed.is_empty() && trimmed.len() < 64 {
        return Some(trimmed.to_string());
    }
    None
}

fn extract_first_team_name(teams: &serde_json::Value) -> Option<String> {
    let arr = teams.as_array()?;
    let first = arr.first()?;
    first
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn build_detail(raw: serde_json::Value, kind: ResourceKind) -> ResourceDetail {
    use serde_json::Value;

    let get_str = |v: &Value, key: &str| -> Option<String> {
        v.get(key).and_then(|x| x.as_str()).map(|s| s.to_string())
    };

    let uuid = get_str(&raw, "uuid").unwrap_or_default();
    let name = get_str(&raw, "name").unwrap_or_default();
    let status_raw = get_str(&raw, "status").unwrap_or_default();
    let status = parse_status(&status_raw);
    let fqdn = get_str(&raw, "fqdn");
    let build_pack = get_str(&raw, "build_pack");
    let image_ref = get_str(&raw, "image");

    // Coolify ships datetimes as MySQL-style "YYYY-MM-DD HH:MM:SS" (no `T`, no
    // timezone). Accept both that and RFC 3339 — same loose parser as in
    // types.rs/parse_loose_datetime, inlined to avoid a cross-module helper.
    let parse_loose = |s: &str| -> Option<chrono::DateTime<chrono::Utc>> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Ok(d) = chrono::DateTime::parse_from_rfc3339(trimmed) {
            return Some(d.with_timezone(&chrono::Utc));
        }
        if let Ok(d) = chrono::NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S") {
            return Some(d.and_utc());
        }
        if let Ok(d) = chrono::NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S") {
            return Some(d.and_utc());
        }
        None
    };
    let last_deployed_at = raw
        .get("last_online_at")
        .or_else(|| raw.get("updated_at"))
        .and_then(|v| v.as_str())
        .and_then(parse_loose);

    // environment + project
    let env_obj = raw.get("environment");
    let environment_name = env_obj
        .and_then(|e| e.get("name"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let environment_uuid = env_obj
        .and_then(|e| e.get("uuid"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let environment_id = raw.get("environment_id").and_then(|v| v.as_i64());
    let project_uuid = env_obj
        .and_then(|e| e.get("project"))
        .and_then(|p| p.get("uuid"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let project_name = env_obj
        .and_then(|e| e.get("project"))
        .and_then(|p| p.get("name"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let server_name = raw
        .get("destination")
        .and_then(|d| d.get("server"))
        .and_then(|s| s.get("name"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let env_vars: Vec<EnvVar> = raw
        .get("environment_variables")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| serde_json::from_value::<RawEnvVar>(item.clone()).ok())
                .map(|raw| raw.into_env_var())
                .collect()
        })
        .unwrap_or_default();

    let healthcheck = raw
        .get("health_check")
        .and_then(|v| serde_json::from_value::<HealthCheck>(v.clone()).ok());

    // Service containers: Coolify's GET /services/{uuid} response nests
    // each compose service under `applications` (HTTP services) and
    // `databases` (stateful services). Each item carries its own coolify
    // uuid — that's the value the per-container logs endpoint
    // (/applications/{uuid}/logs) expects.
    let mut service_containers: Vec<crate::coolify::types::ServiceContainer> = Vec::new();
    for arr_key in [
        "applications",
        "databases",
        // Legacy variants — kept for older Coolify versions.
        "service_applications",
        "service_databases",
    ] {
        if let Some(arr) = raw.get(arr_key).and_then(|v| v.as_array()) {
            for item in arr {
                let uuid = match item.get("uuid").and_then(|x| x.as_str()) {
                    Some(s) if !s.is_empty() => s.to_string(),
                    _ => continue,
                };
                let name = item
                    .get("name")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| uuid.clone());
                let image = item
                    .get("image")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string());
                let fqdn = item
                    .get("fqdn")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string());
                service_containers.push(crate::coolify::types::ServiceContainer {
                    uuid,
                    name,
                    image,
                    fqdn,
                });
            }
        }
    }

    ResourceDetail {
        uuid,
        name,
        kind,
        project_uuid,
        project_name,
        environment_uuid,
        environment_name,
        environment_id,
        status,
        fqdn,
        image_ref,
        last_deployed_at,
        build_pack,
        git_repository: get_str(&raw, "git_repository"),
        git_branch: get_str(&raw, "git_branch"),
        git_commit_sha: get_str(&raw, "git_commit_sha"),
        ports_exposes: get_str(&raw, "ports_exposes"),
        docker_compose_raw: get_str(&raw, "docker_compose_raw"),
        install_command: get_str(&raw, "install_command"),
        build_command: get_str(&raw, "build_command"),
        start_command: get_str(&raw, "start_command"),
        base_directory: get_str(&raw, "base_directory"),
        publish_directory: get_str(&raw, "publish_directory"),
        dockerfile: get_str(&raw, "dockerfile"),
        dockerfile_location: get_str(&raw, "dockerfile_location"),
        dockerfile_target_build: get_str(&raw, "dockerfile_target_build"),
        watch_paths: get_str(&raw, "watch_paths"),
        pre_deployment_command: get_str(&raw, "pre_deployment_command"),
        pre_deployment_command_container: get_str(&raw, "pre_deployment_command_container"),
        post_deployment_command: get_str(&raw, "post_deployment_command"),
        post_deployment_command_container: get_str(&raw, "post_deployment_command_container"),
        custom_docker_run_options: get_str(&raw, "custom_docker_run_options"),
        static_image: get_str(&raw, "static_image"),
        env_vars,
        healthcheck,
        server_name,
        service_containers,
    }
}

