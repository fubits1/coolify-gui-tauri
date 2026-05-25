//! Cross-OS secure token storage via `keyring` crate.
//!
//! - macOS: Keychain (apple-native)
//! - Windows: Credential Manager (windows-native)
//! - Linux desktop: Secret Service via zbus (pure Rust, no libsecret build dep)

use keyring::Entry;

const SERVICE: &str = "dev.fubits.coolify-gui";

pub fn save_token(alias: &str, token: &str) -> Result<(), String> {
    Entry::new(SERVICE, alias)
        .map_err(|e| e.to_string())?
        .set_password(token)
        .map_err(|e| e.to_string())
}

pub fn load_token(alias: &str) -> Result<Option<String>, String> {
    match Entry::new(SERVICE, alias)
        .map_err(|e| e.to_string())?
        .get_password()
    {
        Ok(t) => Ok(Some(t)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

pub fn delete_token(alias: &str) -> Result<(), String> {
    match Entry::new(SERVICE, alias)
        .map_err(|e| e.to_string())?
        .delete_credential()
    {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
