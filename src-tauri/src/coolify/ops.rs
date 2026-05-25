use serde::Serialize;
use tauri::State;

use super::client::CoolifyClient;
use super::types::{
    parse_status, EnvVar, HealthCheck, RawApplication, RawDatabase, RawEnvVar, RawService,
    Resource, ResourceDetail, ResourceKind,
};
use super::AppState;

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
                r.last_deployed_at = deploys.get(&r.uuid).cloned().flatten();
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
            out.extend(list.into_iter().map(RawService::into_resource));
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
    state: State<'_, AppState>,
) -> Result<String, String> {
    let segment = action_segment(&kind)?;
    let capped = lines.min(5000);
    let client = clone_client(&state).await?;
    let path = format!("api/v1/{}/{}/logs?lines={}", segment, uuid, capped);
    // Coolify returns JSON `{"logs": "<text>"}`, not plain text. Decode
    // accordingly; if the call 404s (services without log support) bubble
    // a friendly message instead of the raw error.
    let raw: Result<serde_json::Value, _> = client.get(&path).await;
    match raw {
        Ok(v) => Ok(v
            .get("logs")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string())
            .unwrap_or_default()),
        Err(crate::coolify::client::CoolifyError::NotFound) => {
            Err("Logs are not available for this resource type via the Coolify API".to_string())
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

    const TTL: Duration = Duration::from_secs(60);
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
            let path = format!("api/v1/deployments/applications/{}?take=1", uuid);
            let res: Result<serde_json::Value, _> = client.get(&path).await;
            let ts = match res {
                Ok(v) => extract_first_deployment_timestamp(&v),
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

/// `/deployments/applications/{uuid}?take=1` returns either an array or an
/// object with a `data` key; both shapes have at most one deployment record
/// when `take=1`. Extract `created_at` and parse with the same loose
/// datetime parser used elsewhere (RFC 3339 or MySQL `YYYY-MM-DD HH:MM:SS`).
fn extract_first_deployment_timestamp(
    v: &serde_json::Value,
) -> Option<chrono::DateTime<chrono::Utc>> {
    let first = v
        .as_array()
        .and_then(|a| a.first())
        .or_else(|| v.get("data").and_then(|d| d.as_array()).and_then(|a| a.first()))
        .or_else(|| if v.is_object() { Some(v) } else { None })?;
    let raw = first.get("created_at").and_then(|x| x.as_str())?;
    parse_loose_datetime(raw)
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

    ResourceDetail {
        uuid,
        name,
        kind,
        project_uuid,
        project_name,
        environment_name,
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
        env_vars,
        healthcheck,
        server_name,
    }
}

