//! Cross-OS secure token storage via `keyring` crate.
//!
//! - macOS: Keychain (apple-native)
//! - Windows: Credential Manager (windows-native)
//! - Linux desktop: Secret Service via zbus (pure Rust, no libsecret build dep)
//!
//! Token entries are keyed by `instance_id` (an opaque UUID generated
//! client-side when the user adds a Coolify instance) so multi-instance
//! installations don't collide. Legacy single-tenant entries were keyed
//! by alias — see `migrate_legacy_token` for the one-shot upgrade path.

use keyring::Entry;

const SERVICE: &str = "dev.fubits.coolify-gui";

fn entry_for(instance_id: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, &format!("instance:{}", instance_id)).map_err(|e| e.to_string())
}

pub fn save_token(instance_id: &str, token: &str) -> Result<(), String> {
    entry_for(instance_id)?
        .set_password(token)
        .map_err(|e| e.to_string())
}

pub fn load_token(instance_id: &str) -> Result<Option<String>, String> {
    match entry_for(instance_id)?.get_password() {
        Ok(t) => Ok(Some(t)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

pub fn delete_token(instance_id: &str) -> Result<(), String> {
    match entry_for(instance_id)?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// Migrate a single-tenant token entry (keyed by `alias` only) into the
/// multi-instance scheme (keyed by `instance:{id}`). Returns the token
/// string if a legacy entry existed, after deleting it.
///
/// Frontend calls this once at app boot when it detects an empty
/// `instances.json` alongside a legacy `instance.json`. Idempotent: a
/// no-op when the legacy entry doesn't exist.
#[tauri::command]
pub fn migrate_legacy_token_cmd(alias: String) -> Result<Option<String>, String> {
    migrate_legacy_token(&alias)
}

pub fn migrate_legacy_token(legacy_alias: &str) -> Result<Option<String>, String> {
    let legacy = Entry::new(SERVICE, legacy_alias).map_err(|e| e.to_string())?;
    let token = match legacy.get_password() {
        Ok(t) => t,
        Err(keyring::Error::NoEntry) => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    if let Err(e) = legacy.delete_credential() {
        match e {
            keyring::Error::NoEntry => {}
            other => return Err(other.to_string()),
        }
    }
    Ok(Some(token))
}
