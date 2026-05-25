//! Docker Hub Hub API client.
//!
//! The Hub API (`hub.docker.com/v2/repositories/<ns>/<repo>/tags`) returns
//! every tag's digest + `last_updated` + size in a single response. That's
//! materially cheaper than the OCI Distribution v2 flow (1 call vs 3 per
//! image) AND it dodges the anonymous registry rate limit (100/6h on
//! `registry-1.docker.io`) — the Hub API runs on a separate quota.
//!
//! For a worst-case Coolify Service (e.g. supabase, ~13 images), this is
//! the difference between consuming 39% of the anon budget per check and
//! consuming roughly nothing. Non-Docker-Hub registries (GHCR, quay,
//! private) still go through the OCI v2 path.

use std::time::Duration;

use serde::Deserialize;

const HUB_BASE: &str = "https://hub.docker.com/v2";

#[derive(Debug, Deserialize)]
pub struct HubTagsPage {
    pub results: Vec<HubTag>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct HubTag {
    pub name: String,
    /// Manifest digest for the tag. Hub sometimes returns an empty string
    /// when the tag points to a multi-arch index — fall back to the first
    /// `images[].digest` in that case.
    #[serde(default)]
    pub digest: String,
    pub last_updated: Option<String>,
    #[serde(default)]
    pub images: Vec<HubArchImage>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct HubArchImage {
    #[serde(default)]
    pub digest: String,
    #[serde(default)]
    pub architecture: String,
    #[serde(default)]
    pub os: String,
}

/// True for an image ref hosted on Docker Hub (no explicit registry, or the
/// explicit `docker.io` / `index.docker.io`). Refs with a `.` or `:port` in
/// the first segment are treated as custom registries (GHCR, quay, etc).
pub fn is_docker_hub_ref(image_ref: &str) -> bool {
    // Strip everything after the tag separator.
    let without_tag = match image_ref.rfind(':') {
        Some(i) if !image_ref[i + 1..].contains('/') => &image_ref[..i],
        _ => image_ref,
    };
    let first_segment = without_tag.split('/').next().unwrap_or("");
    if first_segment == "docker.io" || first_segment == "index.docker.io" {
        return true;
    }
    // A first segment containing `.` (`ghcr.io`) or `:` (port) is a registry host.
    !first_segment.contains('.') && !first_segment.contains(':')
}

/// Split a Docker Hub image ref into `(namespace, repository)`, defaulting
/// missing namespace to `"library"` (so `nginx` → `library/nginx`).
pub fn split_hub_ref(image_ref: &str) -> (String, String) {
    let without_tag = match image_ref.rfind(':') {
        Some(i) if !image_ref[i + 1..].contains('/') => &image_ref[..i],
        _ => image_ref,
    };
    // Drop an explicit docker.io prefix if present.
    let stripped = without_tag
        .strip_prefix("docker.io/")
        .or_else(|| without_tag.strip_prefix("index.docker.io/"))
        .unwrap_or(without_tag);
    match stripped.split_once('/') {
        Some((ns, repo)) => (ns.to_string(), repo.to_string()),
        None => ("library".to_string(), stripped.to_string()),
    }
}

/// Fetch up to `page_size` most-recently-updated tags for a Hub repo.
/// `ordering=last_updated` puts the newest publication first — index 0 of
/// the result is the "latest" tag (under that sort), regardless of whether
/// the repo publishes a literal `:latest` tag.
pub async fn fetch_tags(
    namespace: &str,
    repository: &str,
    page_size: u32,
) -> Result<HubTagsPage, String> {
    let url = format!(
        "{}/repositories/{}/{}/tags?page_size={}&ordering=last_updated",
        HUB_BASE, namespace, repository, page_size
    );
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = http
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("hub api {}: {}", url, resp.status()));
    }
    resp.json::<HubTagsPage>().await.map_err(|e| e.to_string())
}

/// Best-effort digest for a tag — prefer the top-level `digest` and fall
/// back to the first amd64/linux arch entry under `images`. Returns `None`
/// when no candidate is available (the tag is essentially opaque to us).
pub fn primary_digest_for(tag: &HubTag) -> Option<String> {
    if !tag.digest.is_empty() {
        return Some(tag.digest.clone());
    }
    // Prefer amd64+linux; fall back to whatever's there.
    let preferred = tag
        .images
        .iter()
        .find(|i| i.architecture == "amd64" && i.os == "linux")
        .and_then(|i| {
            if i.digest.is_empty() {
                None
            } else {
                Some(i.digest.clone())
            }
        });
    if preferred.is_some() {
        return preferred;
    }
    tag.images
        .iter()
        .find(|i| !i.digest.is_empty())
        .map(|i| i.digest.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_plain_hub_refs() {
        assert!(is_docker_hub_ref("nginx"));
        assert!(is_docker_hub_ref("nginx:1.27"));
        assert!(is_docker_hub_ref("supabase/studio:2026.04.27-sha-5f60601"));
        assert!(is_docker_hub_ref("docker.io/nginx"));
        assert!(is_docker_hub_ref("index.docker.io/library/nginx"));
    }

    #[test]
    fn rejects_non_hub_refs() {
        assert!(!is_docker_hub_ref("ghcr.io/foo/bar"));
        assert!(!is_docker_hub_ref("quay.io/foo/bar:v1"));
        assert!(!is_docker_hub_ref("registry.example.com:5000/foo:tag"));
    }

    #[test]
    fn splits_namespace_and_repo() {
        assert_eq!(split_hub_ref("nginx"), ("library".into(), "nginx".into()));
        assert_eq!(
            split_hub_ref("supabase/studio:2026.04.27-sha-5f60601"),
            ("supabase".into(), "studio".into())
        );
        assert_eq!(
            split_hub_ref("docker.io/library/redis:7"),
            ("library".into(), "redis".into())
        );
    }
}
