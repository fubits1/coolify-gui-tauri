//! `#[tauri::command]` exports for the registry module.
//!
//! Webview-callable surface. Errors are stringified — frontend only sees
//! human-readable messages.

use std::collections::HashMap;

use chrono::Utc;
use tauri::{AppHandle, Runtime};

use super::cache::{self, ImageCacheEntry};
use super::digest::fetch_manifest_digest;
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
    // Re-fetch the digest for the exact tag the user is pinned to.
    let current_digest = fetch_manifest_digest(&image_ref, None)
        .await
        .map_err(|e| e.to_string())?;

    // ALWAYS fetch the `:latest` digest + tag list — without these the
    // frontend's `isStale` verdict has no upstream reference and every
    // image shows up as ✓ fresh on first inspection. Failures are tolerated
    // (private registries, missing :latest tag, anon rate limits) — we just
    // store None and let the badge fall back to "checked, no signal".
    let (repo, current_tag) = split_ref(&image_ref);
    let latest_digest = if current_tag != "latest" {
        fetch_manifest_digest(&format!("{repo}:latest"), None)
            .await
            .ok()
    } else {
        // Already on :latest — the current digest IS the latest, so leave the
        // dedicated field None and let semver tag comparison drive staleness.
        None
    };
    let highest_semver_tag = match list_tags(repo, None).await {
        Ok(tags) => pick_highest_semver(&tags),
        Err(e) => {
            tracing::debug!("tag listing failed for {}: {}", image_ref, e);
            None
        }
    };

    let entry = ImageCacheEntry {
        digest: current_digest,
        latest_digest,
        highest_semver_tag,
        // Epoch milliseconds — matches the JS Date semantics consumed by the
        // frontend (`new Date(checked_at)`, `now - checked_at > DAY_MS`).
        checked_at: Utc::now().timestamp_millis(),
    };

    cache::write_entry(&app, &image_ref, entry.clone()).await;
    Ok(entry)
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
