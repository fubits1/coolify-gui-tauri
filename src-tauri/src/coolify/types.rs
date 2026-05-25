use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Discriminator for a Coolify Resource.
///
/// Mirrors the three top-level endpoints (`/applications`, `/services`,
/// `/databases`). The string form is what the frontend sends back when
/// invoking per-row actions, so the variants serialise as lowercase
/// singular tags.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResourceKind {
    Application,
    Service,
    Database,
}

impl ResourceKind {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "application" | "applications" => Some(Self::Application),
            "service" | "services" => Some(Self::Service),
            "database" | "databases" => Some(Self::Database),
            _ => None,
        }
    }

    /// Plural REST path segment used by Coolify (`/applications`, `/services`, `/databases`).
    pub fn path_segment(self) -> &'static str {
        match self {
            Self::Application => "applications",
            Self::Service => "services",
            Self::Database => "databases",
        }
    }
}

/// Parsed view of Coolify's combined status string.
///
/// Coolify returns values like `running:healthy`, `exited:unhealthy`,
/// `degraded`, `starting`, `excluded`. We split on `:` once: left half is
/// the lifecycle state, right half (if present) is the healthcheck verdict.
/// `raw` is preserved verbatim for debugging + unanticipated formats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceStatus {
    pub state: String,
    pub health: Option<String>,
    pub raw: String,
}

/// Split a Coolify combined status string into `{state, health, raw}`.
pub fn parse_status(raw: &str) -> ResourceStatus {
    let trimmed = raw.trim();
    let mut parts = trimmed.splitn(2, ':');
    let state = parts.next().unwrap_or("").to_string();
    let health = parts.next().map(|s| s.to_string()).filter(|s| !s.is_empty());
    ResourceStatus {
        state,
        health,
        raw: trimmed.to_string(),
    }
}

/// Summary view of a Resource — what the overview table renders per row.
///
/// Optional fields tolerate Coolify variants: e.g. Databases have no
/// `fqdn`, raw-compose Services have no `build_pack`, freshly created
/// Resources have no `last_deployed_at` yet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub uuid: String,
    pub name: String,
    pub kind: ResourceKind,
    pub project_uuid: Option<String>,
    pub project_name: Option<String>,
    pub environment_name: Option<String>,
    pub status: ResourceStatus,
    pub fqdn: Option<String>,
    pub image_ref: Option<String>,
    pub last_deployed_at: Option<DateTime<Utc>>,
    pub build_pack: Option<String>,
}

/// A single env var on a Resource, as surfaced by the Env detail tab.
///
/// `is_secret` reflects Coolify's `is_secret` / `is_shown_once` flags; the
/// UI uses it to render a mask + reveal-on-click affordance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVar {
    pub key: String,
    pub value: String,
    pub is_secret: bool,
}

/// Healthcheck configuration as Coolify reports it.
///
/// All fields optional — Coolify only fills the relevant ones for the
/// chosen healthcheck type (HTTP vs CMD vs none).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub enabled: Option<bool>,
    pub path: Option<String>,
    pub port: Option<u32>,
    pub method: Option<String>,
    pub interval: Option<u32>,
    pub timeout: Option<u32>,
    pub retries: Option<u32>,
    pub start_period: Option<u32>,
}

/// Detail view of a Resource — drives the detail pane tabs.
///
/// Superset of `Resource`: same summary fields plus Git provenance, port
/// exposure list, raw compose YAML (for Service and dockercompose
/// Applications), env vars, healthcheck, and the SSH server name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDetail {
    pub uuid: String,
    pub name: String,
    pub kind: ResourceKind,
    pub project_uuid: Option<String>,
    pub project_name: Option<String>,
    pub environment_name: Option<String>,
    pub status: ResourceStatus,
    pub fqdn: Option<String>,
    pub image_ref: Option<String>,
    pub last_deployed_at: Option<DateTime<Utc>>,
    pub build_pack: Option<String>,
    pub git_repository: Option<String>,
    pub git_branch: Option<String>,
    pub git_commit_sha: Option<String>,
    pub ports_exposes: Option<String>,
    pub docker_compose_raw: Option<String>,
    pub env_vars: Vec<EnvVar>,
    pub healthcheck: Option<HealthCheck>,
    pub server_name: Option<String>,
}

// ── Raw Coolify response shapes (internal, deserialise-only) ────────────────
//
// These mirror the OpenAPI payloads loosely; every field is Option so a
// minor upstream change doesn't break parsing. We map them to the public
// `Resource` / `ResourceDetail` structs in ops.rs.

// Fields below are read at the JSON-walk level in `ops::build_detail`
// (via serde_json::Value) rather than through the typed struct, so the
// compiler flags them as "never read". They are intentionally kept here as
// a schema reference for the upstream OpenAPI payload — once `build_detail`
// is rewritten to consume `Raw*` directly, the warnings will disappear.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct RawApplication {
    pub uuid: Option<String>,
    pub name: Option<String>,
    pub status: Option<String>,
    pub fqdn: Option<String>,
    pub build_pack: Option<String>,
    pub git_repository: Option<String>,
    pub git_branch: Option<String>,
    pub git_commit_sha: Option<String>,
    pub ports_exposes: Option<String>,
    pub docker_compose_raw: Option<String>,
    // Coolify ships datetimes as MySQL-style "YYYY-MM-DD HH:MM:SS" (no `T`, no
    // timezone) — NOT RFC 3339. Deserialising directly as `DateTime<Utc>`
    // fails and serde_json surfaces it as a misleading "premature end of
    // input" error. We accept raw String here and convert via
    // `parse_loose_datetime` in `into_resource`.
    pub last_online_at: Option<String>,
    pub updated_at: Option<String>,
    pub environment: Option<RawEnvironment>,
    pub destination: Option<RawDestination>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct RawService {
    pub uuid: Option<String>,
    pub name: Option<String>,
    pub status: Option<String>,
    pub fqdn: Option<String>,
    pub docker_compose_raw: Option<String>,
    pub last_online_at: Option<String>,
    pub updated_at: Option<String>,
    pub environment: Option<RawEnvironment>,
    pub destination: Option<RawDestination>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct RawDatabase {
    pub uuid: Option<String>,
    pub name: Option<String>,
    pub status: Option<String>,
    pub image: Option<String>,
    pub last_online_at: Option<String>,
    pub updated_at: Option<String>,
    pub environment: Option<RawEnvironment>,
    pub destination: Option<RawDestination>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawEnvironment {
    pub name: Option<String>,
    pub project: Option<RawProject>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawProject {
    pub uuid: Option<String>,
    pub name: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct RawDestination {
    pub server: Option<RawServer>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct RawServer {
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawEnvVar {
    pub key: Option<String>,
    pub value: Option<String>,
    pub is_secret: Option<bool>,
}

/// Accept either RFC 3339 (`2026-05-25T17:57:07Z`) or Coolify's MySQL-flavoured
/// `YYYY-MM-DD HH:MM:SS` format (no `T`, no zone). Unknown formats → `None`.
fn parse_loose_datetime(s: &str) -> Option<DateTime<Utc>> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(d) = DateTime::parse_from_rfc3339(trimmed) {
        return Some(d.with_timezone(&Utc));
    }
    if let Ok(d) = chrono::NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S") {
        return Some(d.and_utc());
    }
    if let Ok(d) = chrono::NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S") {
        return Some(d.and_utc());
    }
    None
}

/// Pick the most recent of `last_online_at` / `updated_at` for the overview's
/// "Last deploy" column. Either may be RFC 3339 or MySQL-style.
fn pick_last_deployed(
    last_online: Option<String>,
    updated: Option<String>,
) -> Option<DateTime<Utc>> {
    last_online
        .as_deref()
        .and_then(parse_loose_datetime)
        .or_else(|| updated.as_deref().and_then(parse_loose_datetime))
}

impl RawApplication {
    pub(crate) fn into_resource(self) -> Resource {
        let (project_uuid, project_name, environment_name) = unpack_environment(self.environment);
        let status = parse_status(self.status.as_deref().unwrap_or(""));
        let last_deployed_at = pick_last_deployed(self.last_online_at, self.updated_at);
        Resource {
            uuid: self.uuid.unwrap_or_default(),
            name: self.name.unwrap_or_default(),
            kind: ResourceKind::Application,
            project_uuid,
            project_name,
            environment_name,
            status,
            fqdn: self.fqdn,
            image_ref: None,
            last_deployed_at,
            build_pack: self.build_pack,
        }
    }
}

impl RawService {
    pub(crate) fn into_resource(self) -> Resource {
        let (project_uuid, project_name, environment_name) = unpack_environment(self.environment);
        let status = parse_status(self.status.as_deref().unwrap_or(""));
        let last_deployed_at = pick_last_deployed(self.last_online_at, self.updated_at);
        Resource {
            uuid: self.uuid.unwrap_or_default(),
            name: self.name.unwrap_or_default(),
            kind: ResourceKind::Service,
            project_uuid,
            project_name,
            environment_name,
            status,
            fqdn: self.fqdn,
            image_ref: None,
            last_deployed_at,
            build_pack: None,
        }
    }
}

impl RawDatabase {
    pub(crate) fn into_resource(self) -> Resource {
        let (project_uuid, project_name, environment_name) = unpack_environment(self.environment);
        let status = parse_status(self.status.as_deref().unwrap_or(""));
        let last_deployed_at = pick_last_deployed(self.last_online_at, self.updated_at);
        Resource {
            uuid: self.uuid.unwrap_or_default(),
            name: self.name.unwrap_or_default(),
            kind: ResourceKind::Database,
            project_uuid,
            project_name,
            environment_name,
            status,
            fqdn: None,
            image_ref: self.image,
            last_deployed_at,
            build_pack: None,
        }
    }
}

fn unpack_environment(env: Option<RawEnvironment>) -> (Option<String>, Option<String>, Option<String>) {
    let Some(e) = env else {
        return (None, None, None);
    };
    let env_name = e.name;
    let (project_uuid, project_name) = match e.project {
        Some(p) => (p.uuid, p.name),
        None => (None, None),
    };
    (project_uuid, project_name, env_name)
}

impl RawEnvVar {
    pub(crate) fn into_env_var(self) -> EnvVar {
        EnvVar {
            key: self.key.unwrap_or_default(),
            value: self.value.unwrap_or_default(),
            is_secret: self.is_secret.unwrap_or(false),
        }
    }
}
