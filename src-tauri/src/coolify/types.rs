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
    pub last_online_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub environment: Option<RawEnvironment>,
    pub destination: Option<RawDestination>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawService {
    pub uuid: Option<String>,
    pub name: Option<String>,
    pub status: Option<String>,
    pub fqdn: Option<String>,
    pub docker_compose_raw: Option<String>,
    pub last_online_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub environment: Option<RawEnvironment>,
    pub destination: Option<RawDestination>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawDatabase {
    pub uuid: Option<String>,
    pub name: Option<String>,
    pub status: Option<String>,
    pub image: Option<String>,
    pub last_online_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
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

#[derive(Debug, Deserialize)]
pub(crate) struct RawDestination {
    pub server: Option<RawServer>,
}

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

impl RawApplication {
    pub(crate) fn into_resource(self) -> Resource {
        let (project_uuid, project_name, environment_name) = unpack_environment(self.environment);
        let status = parse_status(self.status.as_deref().unwrap_or(""));
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
            last_deployed_at: self.last_online_at.or(self.updated_at),
            build_pack: self.build_pack,
        }
    }
}

impl RawService {
    pub(crate) fn into_resource(self) -> Resource {
        let (project_uuid, project_name, environment_name) = unpack_environment(self.environment);
        let status = parse_status(self.status.as_deref().unwrap_or(""));
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
            last_deployed_at: self.last_online_at.or(self.updated_at),
            build_pack: None,
        }
    }
}

impl RawDatabase {
    pub(crate) fn into_resource(self) -> Resource {
        let (project_uuid, project_name, environment_name) = unpack_environment(self.environment);
        let status = parse_status(self.status.as_deref().unwrap_or(""));
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
            last_deployed_at: self.last_online_at.or(self.updated_at),
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
