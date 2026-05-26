//! `#[tauri::command]` exports for the registry module.
//!
//! Webview-callable surface. Errors are stringified — frontend only sees
//! human-readable messages.

use std::collections::HashMap;

use chrono::Utc;
use tauri::{AppHandle, Runtime};

use super::cache::{self, ImageCacheEntry};
use super::digest::fetch_manifest_digest;
use super::hub::{self, is_docker_hub_ref, split_hub_ref};
use super::tags::{list_tags, pick_highest_semver};

/// Fetch fresh digest + (optionally) latest-tag digest + highest semver tag
/// for an image ref. Writes the result to the on-disk cache. Returns the
/// stored entry.
///
/// `auth` is currently always `None` — registry credentials will be wired
/// through later (per-registry keyring lookup happens caller-side and is
/// then threaded through; for now we exercise the anonymous path).
///
/// Concurrency control (max-in-flight cap) is the caller's job — this
/// command is intentionally single-shot.
#[tauri::command]
pub async fn check_image<R: Runtime>(
    image_ref: String,
    app: AppHandle<R>,
) -> Result<ImageCacheEntry, String> {
    let entry = if is_docker_hub_ref(&image_ref) {
        check_via_hub_api(&image_ref).await?
    } else {
        check_via_oci(&image_ref).await?
    };
    cache::write_entry(&app, &image_ref, entry.clone()).await;
    Ok(entry)
}

/// Single-call Docker Hub path: one Hub API request returns digest +
/// `last_updated` for every recent tag in the repo, sorted newest-first.
/// We pick the tag matching the user's current ref for `digest`, and tag
/// index 0 (most-recently-updated) for `latest_digest`. If the current
/// tag isn't in the first page, we mark digest empty — the badge then
/// surfaces this as "unknown" via the existing isStale logic.
async fn check_via_hub_api(image_ref: &str) -> Result<ImageCacheEntry, String> {
    let (namespace, repository) = split_hub_ref(image_ref);
    let current_tag = split_ref(image_ref).1.to_string();

    let page = hub::fetch_tags(&namespace, &repository, 100)
        .await
        .map_err(|e| format!("docker hub api: {}", e))?;

    // First try: find the pinned tag in the most-recent page.
    let mut current_digest = page
        .results
        .iter()
        .find(|t| t.name == current_tag)
        .and_then(hub::primary_digest_for)
        .unwrap_or_default();

    // Fallback: if the pinned tag is OLDER than the 100 most-recent
    // publications, it's not on the first page. Fetch it by name. One
    // extra round-trip per cache miss; results are cached on disk so
    // subsequent reads hit the cache.
    if current_digest.is_empty() {
        match hub::fetch_tag(&namespace, &repository, &current_tag).await {
            Ok(tag) => {
                if let Some(d) = hub::primary_digest_for(&tag) {
                    current_digest = d;
                }
            }
            Err(e) => {
                tracing::debug!(
                    "hub fetch_tag fallback failed for {}:{} - {}",
                    repository,
                    current_tag,
                    e
                );
            }
        }
    }

    let newest = page.results.first();
    let latest_digest = match newest {
        Some(t) if t.name != current_tag => hub::primary_digest_for(t),
        Some(_) => {
            if current_digest.is_empty() {
                None
            } else {
                Some(current_digest.clone())
            }
        }
        None => None,
    };

    // For `:latest`-pinned images, the publish timestamp of the user's CURRENT
    // tag is what tells us "is the registry's current :latest newer than what
    // the user has deployed?". For pinned-version tags, the newest tag's
    // last_updated is what matters.
    let timestamp_source = if current_tag == "latest" {
        page.results.iter().find(|t| t.name == "latest")
    } else {
        newest
    };
    let latest_pushed_at = timestamp_source
        .and_then(|t| t.last_updated.as_deref())
        .and_then(parse_hub_timestamp);

    let highest_semver_tag = pick_highest_semver(
        &page.results.iter().map(|t| t.name.clone()).collect::<Vec<_>>(),
    );

    Ok(ImageCacheEntry {
        digest: current_digest,
        latest_digest,
        highest_semver_tag,
        latest_pushed_at,
        checked_at: Utc::now().timestamp_millis(),
    })
}

/// Hub API timestamps are RFC 3339 (`"2026-04-27T17:14:09.123456Z"`).
fn parse_hub_timestamp(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&Utc).timestamp_millis())
}

/// Fallback OCI Distribution v2 path for non-Docker-Hub registries
/// (GHCR, quay, private). Three round-trips per image; failures are
/// tolerated for `:latest` + tag listing so private registries still
/// at least surface the current digest.
async fn check_via_oci(image_ref: &str) -> Result<ImageCacheEntry, String> {
    let current_digest = fetch_manifest_digest(image_ref, None)
        .await
        .map_err(|e| e.to_string())?;
    let (repo, current_tag) = split_ref(image_ref);
    let latest_digest = if current_tag != "latest" {
        fetch_manifest_digest(&format!("{repo}:latest"), None)
            .await
            .ok()
    } else {
        None
    };
    let highest_semver_tag = match list_tags(repo, None).await {
        Ok(tags) => pick_highest_semver(&tags),
        Err(e) => {
            tracing::debug!("oci tag listing failed for {}: {}", image_ref, e);
            None
        }
    };
    Ok(ImageCacheEntry {
        digest: current_digest,
        latest_digest,
        highest_semver_tag,
        latest_pushed_at: None,
        checked_at: Utc::now().timestamp_millis(),
    })
}

/// Return the full cache snapshot for the frontend store to hydrate badges.
#[tauri::command]
pub async fn read_image_cache<R: Runtime>(
    app: AppHandle<R>,
) -> Result<HashMap<String, ImageCacheEntry>, String> {
    Ok(cache::entries(&app).await)
}

/// Split `"registry.example.com/team/svc:tag"` → `("registry.example.com/team/svc", "tag")`.
/// If no tag is present, defaults to `"latest"`.
///
/// Note: this is intentionally simpler than full OCI reference parsing — we
/// only need to split off the trailing `:tag`. We use the rightmost `:` that
/// isn't followed by a `/`, since registry hosts may include a port (`:5000`).
fn split_ref(image_ref: &str) -> (&str, &str) {
    if let Some(colon) = image_ref.rfind(':') {
        let after = &image_ref[colon + 1..];
        if !after.contains('/') {
            return (&image_ref[..colon], after);
        }
    }
    (image_ref, "latest")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_ref_handles_plain_image() {
        assert_eq!(split_ref("nginx:1.25"), ("nginx", "1.25"));
    }

    #[test]
    fn split_ref_handles_registry_with_port() {
        assert_eq!(
            split_ref("registry.example.com:5000/foo/bar:v1"),
            ("registry.example.com:5000/foo/bar", "v1")
        );
    }

    #[test]
    fn split_ref_defaults_to_latest_when_missing() {
        assert_eq!(split_ref("nginx"), ("nginx", "latest"));
    }

    #[test]
    fn split_ref_with_port_no_tag_defaults() {
        // Port-only with no tag is ambiguous; we accept it as latest.
        assert_eq!(
            split_ref("registry.example.com:5000/foo"),
            ("registry.example.com:5000/foo", "latest")
        );
    }
}
