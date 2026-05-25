//! On-disk JSON cache for image freshness state, backed by `tauri-plugin-store`.
//!
//! File: `image-digests.json` in the app's per-OS store dir. One entry per
//! `image_ref` (e.g. `"nginx:1.25"`).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;

/// One cache row.
///
/// `latest_digest` and `highest_semver_tag` are populated opportunistically —
/// only when the current-tag digest is unchanged but stale (>24h). See
/// `commands::check_image` for the logic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageCacheEntry {
    pub digest: String,
    pub latest_digest: Option<String>,
    pub highest_semver_tag: Option<String>,
    /// Epoch milliseconds — when the registry's newest tag was published.
    /// Compare against a resource's `last_deployed_at` to decide whether a
    /// `:latest`-pinned container is likely outdated.
    #[serde(default)]
    pub latest_pushed_at: Option<i64>,
    /// Epoch milliseconds — when we last hit the registry for this image.
    pub checked_at: i64,
}

const STORE_FILE: &str = "image-digests.json";

/// Read a single entry by image ref. Returns `None` if missing or malformed.
pub async fn read_entry<R: Runtime>(
    app: &AppHandle<R>,
    image_ref: &str,
) -> Option<ImageCacheEntry> {
    let store = app.store(STORE_FILE).ok()?;
    let value = store.get(image_ref)?;
    serde_json::from_value(value).ok()
}

/// Write a single entry. Persists synchronously to disk via `store.save()`.
pub async fn write_entry<R: Runtime>(
    app: &AppHandle<R>,
    image_ref: &str,
    entry: ImageCacheEntry,
) {
    let Ok(store) = app.store(STORE_FILE) else {
        return;
    };
    let Ok(value) = serde_json::to_value(&entry) else {
        return;
    };
    store.set(image_ref.to_string(), value);
    let _ = store.save();
}

/// Snapshot of every cached entry.
pub async fn entries<R: Runtime>(app: &AppHandle<R>) -> HashMap<String, ImageCacheEntry> {
    let mut out = HashMap::new();
    let Ok(store) = app.store(STORE_FILE) else {
        return out;
    };
    for (key, value) in store.entries() {
        if let Ok(entry) = serde_json::from_value::<ImageCacheEntry>(value.clone()) {
            out.insert(key, entry);
        } else {
            // Be tolerant of legacy or partial values: skip them silently.
            let _ = value as JsonValue;
        }
    }
    out
}
