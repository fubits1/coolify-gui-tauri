pub mod client;
pub mod ops;
pub mod types;

use tokio::sync::RwLock;

use client::CoolifyClient;

/// Tauri-managed application state holding the active Coolify HTTP client.
///
/// The client is constructed via the `set_credentials` command after the
/// onboarding screen verifies a `{url, token}` pair. Commands acquire a read
/// lock and clone the inner client (which wraps a `reqwest::Client` — cheap
/// to clone because reqwest pools internally).
pub struct AppState {
    pub client: RwLock<Option<CoolifyClient>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            client: RwLock::new(None),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
