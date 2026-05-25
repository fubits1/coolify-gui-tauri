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
    /// Primary image reference for single-image Resources (databases, apps
    /// with `image:tag` build pack). `None` for compose Services + apps.
    pub image_ref: Option<String>,
    /// Full list of `image:tag` refs this Resource depends on. Populated
    /// from compose YAML for Services + dockercompose Apps; from
    /// `image_ref` for Databases + image-based Apps. Empty when Coolify
    /// hasn't supplied enough info (e.g. unset Application with no image).
    pub image_refs: Vec<String>,
    /// Heartbeat — when the container last reported online. Constantly
    /// refreshed for running containers (≈ "now" while up). Useful only for
    /// non-running rows ("died X minutes ago").
    pub last_online_at: Option<DateTime<Utc>>,
    /// True last-deployment timestamp, fetched from
    /// `/deployments/applications/{uuid}?take=1`. None for Services/Databases
    /// (no equivalent endpoint) and Applications that have never deployed.
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
    /// For Applications built directly from a registry image (build_pack
    /// "dockerimage"), Coolify ships the image name + tag separately.
    pub docker_registry_image_name: Option<String>,
    pub docker_registry_image_tag: Option<String>,
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
    /// Coolify nests per-container FQDNs here when the service composes
    /// multiple apps; top-level `fqdn` is often null while the first
    /// `service_application` carries the user-facing domain.
    pub service_applications: Option<Vec<RawServiceContainerFqdn>>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct RawServiceContainerFqdn {
    pub fqdn: Option<String>,
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

/// Resolve the "Last deploy" timestamp for the overview row.
///
/// `last_online_at` is the only field that actually tracks deployments;
/// `updated_at` is bumped on any config touch (env vars, healthcheck change,
/// FQDN edit, etc.) and would falsely show "just now" for resources that
/// haven't deployed in months. So we ONLY use `last_online_at`. If the
/// resource has never come online, the cell renders `—`.
///
/// Accepts RFC 3339 and MySQL `YYYY-MM-DD HH:MM:SS` (with or without `T`).
fn pick_last_deployed(
    last_online: Option<String>,
    _updated: Option<String>,
) -> Option<DateTime<Utc>> {
    last_online.as_deref().and_then(parse_loose_datetime)
}

impl RawApplication {
    pub(crate) fn into_resource(self) -> Resource {
        let (project_uuid, project_name, environment_name) = unpack_environment(self.environment);
        let status = parse_status(self.status.as_deref().unwrap_or(""));
        let last_deployed_at = pick_last_deployed(self.last_online_at, self.updated_at);
        // Image refs: prefer registry image (build_pack=dockerimage), else
        // scrape compose YAML (build_pack=dockercompose). Git-built apps
        // have no static image ref to watch.
        let single_image = build_image_ref(
            self.docker_registry_image_name.as_deref(),
            self.docker_registry_image_tag.as_deref(),
        );
        let image_refs = if let Some(r) = single_image.clone() {
            vec![r]
        } else if let Some(yaml) = self.docker_compose_raw.as_deref() {
            scrape_compose_images(yaml)
        } else {
            Vec::new()
        };
        Resource {
            uuid: self.uuid.unwrap_or_default(),
            name: self.name.unwrap_or_default(),
            kind: ResourceKind::Application,
            project_uuid,
            project_name,
            environment_name,
            status,
            fqdn: self.fqdn,
            image_ref: single_image,
            image_refs,
            last_online_at: last_deployed_at,
            last_deployed_at: None,
            build_pack: self.build_pack,
        }
    }
}

impl RawService {
    pub(crate) fn into_resource(self) -> Resource {
        let (project_uuid, project_name, environment_name) = unpack_environment(self.environment);
        let status = parse_status(self.status.as_deref().unwrap_or(""));
        let last_deployed_at = pick_last_deployed(self.last_online_at, self.updated_at);
        // Coolify's GET /services list does NOT surface FQDN at top-level for
        // services. Fall through in order:
        //   1. top-level `fqdn` (only set on simple services)
        //   2. nested `service_applications[*].fqdn` (rarely populated on list)
        //   3. scrape `docker_compose_raw` for `coolify.fqdn=https://…` labels
        //      or `SERVICE_FQDN_<NAME>=https://…` env declarations
        let fqdn = self
            .fqdn
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                self.service_applications.and_then(|apps| {
                    apps.into_iter()
                        .filter_map(|a| a.fqdn)
                        .find(|s| !s.trim().is_empty())
                })
            })
            .or_else(|| {
                self.docker_compose_raw
                    .as_deref()
                    .and_then(scrape_service_fqdn)
            });
        let image_refs = self
            .docker_compose_raw
            .as_deref()
            .map(scrape_compose_images)
            .unwrap_or_default();
        Resource {
            uuid: self.uuid.unwrap_or_default(),
            name: self.name.unwrap_or_default(),
            kind: ResourceKind::Service,
            project_uuid,
            project_name,
            environment_name,
            status,
            fqdn,
            image_ref: None,
            image_refs,
            last_online_at: last_deployed_at,
            last_deployed_at: None,
            build_pack: None,
        }
    }
}

/// Combine Coolify's split registry image name + tag into a `name:tag` ref.
/// Returns None when name is missing — tag-only is meaningless.
fn build_image_ref(name: Option<&str>, tag: Option<&str>) -> Option<String> {
    let name = name.map(|s| s.trim()).filter(|s| !s.is_empty())?;
    let tag = tag
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("latest");
    Some(format!("{}:{}", name, tag))
}

/// Walk a docker-compose YAML and extract every `image:` directive value.
/// Best-effort string parsing — no full YAML AST — but handles standard
/// indented `image: foo:tag` and `image: "foo:tag"` forms.
fn scrape_compose_images(yaml: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in yaml.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            continue;
        }
        // Match a leading `image:` key — skip `image_*:` or label-like
        // occurrences inside other keys.
        let rest = match trimmed.strip_prefix("image:") {
            Some(r) => r,
            None => continue,
        };
        let value = rest.trim().trim_matches(|c| c == '"' || c == '\'');
        if value.is_empty() {
            continue;
        }
        // Skip variable-only refs we can't resolve.
        if value.starts_with('$') {
            continue;
        }
        out.push(value.to_string());
    }
    out.sort();
    out.dedup();
    out
}

/// Pick the first user-facing URL out of a Coolify service's compose YAML.
///
/// Coolify encodes service FQDNs inside `docker_compose_raw` via either:
///   - traefik/coolify labels: `coolify.fqdn=https://app.example.com`
///   - magic env vars: `SERVICE_FQDN_APP=https://app.example.com`
///
/// Returns the first `https?://…` token we find from either convention,
/// stripping a trailing comma/quote/space so the URL is render-ready.
fn scrape_service_fqdn(yaml: &str) -> Option<String> {
    // Pass 1: prefer Coolify's canonical fqdn/url markers. These hold the
    // user-facing domain even when the rest of the file mentions internal
    // hosts in healthchecks.
    for line in yaml.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if lower.contains("coolify.fqdn")
            || lower.contains("coolify.url")
            || lower.contains("service_fqdn_")
            || lower.contains("service_url_")
        {
            if let Some(url) = extract_first_url(line) {
                return Some(url);
            }
        }
    }
    // Pass 2: any traefik Host(...) rule typically encodes the public domain.
    for line in yaml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower.contains("traefik") && lower.contains("host(") {
            if let Some(host) = extract_host_directive(trimmed) {
                return Some(format!("https://{}", host));
            }
        }
    }
    // Last-resort: first non-loopback https:// in the file.
    extract_first_url(yaml)
}

/// Pull the first hostname out of a traefik `Host(\`name.example.com\`)`
/// directive. Coolify (and bare traefik) stamps these as labels for
/// reverse-proxy routing — the hostname is the public FQDN.
fn extract_host_directive(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    let idx = lower.find("host(")?;
    let after = &line[idx + 5..];
    let close = after.find(')')?;
    let inside = &after[..close];
    let trimmed = inside.trim_matches(|c: char| c == '`' || c == '"' || c == '\'' || c.is_whitespace());
    // Take just the first comma-separated host if there are multiple.
    let first = trimmed.split(',').next().unwrap_or(trimmed);
    let host = first.trim_matches(|c: char| c == '`' || c == '"' || c == '\'' || c.is_whitespace());
    if host.is_empty() || host.starts_with("127.") || host == "localhost" {
        None
    } else {
        Some(host.to_string())
    }
}

fn extract_first_url(s: &str) -> Option<String> {
    let mut search_from = 0;
    let bytes = s.as_bytes();
    while search_from < s.len() {
        let rest = &s[search_from..];
        let scheme_hit = ["https://", "http://"]
            .iter()
            .filter_map(|sc| rest.find(sc).map(|i| (i, *sc)))
            .min_by_key(|(i, _)| *i);
        let (rel_start, scheme) = match scheme_hit {
            Some(v) => v,
            None => return None,
        };
        let start = search_from + rel_start;
        let mut end = start + scheme.len();
        while end < bytes.len() {
            let b = bytes[end];
            let is_url_char = b.is_ascii_alphanumeric()
                || matches!(
                    b,
                    b'.' | b'-'
                        | b'_'
                        | b'/'
                        | b':'
                        | b'?'
                        | b'='
                        | b'&'
                        | b'%'
                        | b'+'
                        | b'~'
                        | b'#'
                );
            if !is_url_char {
                break;
            }
            end += 1;
        }
        if end > start + scheme.len() {
            let candidate = s[start..end].trim_end_matches('/').to_string();
            if !is_loopback_url(&candidate) {
                return Some(candidate);
            }
            // Skip loopback (health-check internal URL) and keep searching.
            search_from = end;
            continue;
        }
        search_from = end;
    }
    None
}

fn is_loopback_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    let host_start = lower
        .find("://")
        .map(|i| i + 3)
        .unwrap_or(0);
    let host_end = lower[host_start..]
        .find(['/', ':', '?', '#'])
        .map(|i| host_start + i)
        .unwrap_or(lower.len());
    let host = &lower[host_start..host_end];
    matches!(
        host,
        "localhost" | "127.0.0.1" | "0.0.0.0" | "::1" | "host.docker.internal"
    )
}

impl RawDatabase {
    pub(crate) fn into_resource(self) -> Resource {
        let (project_uuid, project_name, environment_name) = unpack_environment(self.environment);
        let status = parse_status(self.status.as_deref().unwrap_or(""));
        let last_deployed_at = pick_last_deployed(self.last_online_at, self.updated_at);
        let image = self.image.clone();
        let image_refs = image
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(|s| vec![s.to_string()])
            .unwrap_or_default();
        Resource {
            uuid: self.uuid.unwrap_or_default(),
            name: self.name.unwrap_or_default(),
            kind: ResourceKind::Database,
            project_uuid,
            project_name,
            environment_name,
            status,
            fqdn: None,
            image_ref: image,
            image_refs,
            last_online_at: last_deployed_at,
            last_deployed_at: None,
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
