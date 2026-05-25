use std::time::Duration;

use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use thiserror::Error;
use tracing::{debug, warn};
use url::Url;

/// Errors surfaced by the Coolify HTTP client.
///
/// Mapped to user-friendly strings at the `#[tauri::command]` boundary.
/// The variants distinguish auth failures (so the UI can prompt for a
/// re-paste) from transient network/server issues (which the connection
/// strip surfaces as "reconnecting").
#[derive(Debug, Error)]
pub enum CoolifyError {
    #[error("network error: {0}")]
    Network(String),
    #[error("unauthorized — token rejected (401)")]
    Unauthorized,
    #[error("forbidden — token lacks required scope (403)")]
    Forbidden,
    #[error("not found (404)")]
    NotFound,
    #[error("server error: {0}")]
    Server(u16),
    #[error("decode error: {0}")]
    Decode(String),
}

/// HTTP client for a single Coolify Instance.
///
/// Holds the bearer token internally so callers (Tauri commands) never see
/// it. The token never crosses the Tauri IPC boundary — the webview pastes
/// it once during onboarding, then it lives in the OS keyring + this struct.
#[derive(Clone)]
pub struct CoolifyClient {
    base: Url,
    http: reqwest::Client,
    token: String,
}

impl CoolifyClient {
    /// Build a client against the given Coolify base URL.
    ///
    /// `url` may include or omit a trailing slash; we normalise it. The
    /// `/api/v1/` prefix is appended per-request inside `get` / `get_raw`.
    pub fn new(url: &str, token: &str) -> Result<Self, CoolifyError> {
        let trimmed = url.trim_end_matches('/');
        let base = Url::parse(&format!("{}/", trimmed))
            .map_err(|e| CoolifyError::Decode(format!("invalid url: {}", e)))?;
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| CoolifyError::Network(e.to_string()))?;
        Ok(Self {
            base,
            http,
            token: token.to_string(),
        })
    }

    /// Resolve a path like `api/v1/applications` against the base URL.
    fn url(&self, path: &str) -> Result<Url, CoolifyError> {
        let path = path.trim_start_matches('/');
        self.base
            .join(path)
            .map_err(|e| CoolifyError::Decode(format!("invalid path: {}", e)))
    }

    /// GET a path that requires authentication, decoding the JSON body to `T`.
    ///
    /// Retries on network errors and 5xx with exponential backoff
    /// (1s, 2s, 4s, capped at 30s, max 4 attempts). 4xx is returned
    /// immediately — retrying a bad token is pointless.
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, CoolifyError> {
        let body = self.get_raw(path).await?;
        serde_json::from_str::<T>(&body).map_err(|e| CoolifyError::Decode(e.to_string()))
    }

    /// GET a path that requires authentication, returning the raw response body.
    ///
    /// Used by `tail_logs` (plain text response) and by `get` (JSON-decoded).
    pub async fn get_raw(&self, path: &str) -> Result<String, CoolifyError> {
        let url = self.url(path)?;
        let mut delay_ms: u64 = 1000;
        let max_delay_ms: u64 = 30_000;
        let max_attempts = 4;

        for attempt in 1..=max_attempts {
            let result = self
                .http
                .get(url.clone())
                .bearer_auth(&self.token)
                .send()
                .await;

            match result {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        return resp
                            .text()
                            .await
                            .map_err(|e| CoolifyError::Network(e.to_string()));
                    }
                    match status {
                        StatusCode::UNAUTHORIZED => return Err(CoolifyError::Unauthorized),
                        StatusCode::FORBIDDEN => return Err(CoolifyError::Forbidden),
                        StatusCode::NOT_FOUND => return Err(CoolifyError::NotFound),
                        s if s.is_server_error() => {
                            warn!(
                                "coolify {} 5xx (attempt {}/{}): {}",
                                url, attempt, max_attempts, s
                            );
                            if attempt == max_attempts {
                                return Err(CoolifyError::Server(s.as_u16()));
                            }
                        }
                        s => return Err(CoolifyError::Server(s.as_u16())),
                    }
                }
                Err(e) => {
                    warn!(
                        "coolify {} network err (attempt {}/{}): {}",
                        url, attempt, max_attempts, e
                    );
                    if attempt == max_attempts {
                        return Err(CoolifyError::Network(e.to_string()));
                    }
                }
            }

            debug!("backing off {}ms before retry", delay_ms);
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            delay_ms = (delay_ms * 2).min(max_delay_ms);
        }

        // unreachable — the loop returns on the final attempt.
        Err(CoolifyError::Network("exhausted retries".into()))
    }

    /// GET against the unauthenticated `/api/v1/health` endpoint.
    ///
    /// Returns the raw body so the caller can surface a version string if
    /// present. Used by `test_connection` before we exercise the token.
    pub async fn get_unauthenticated_health(base_url: &str) -> Result<String, CoolifyError> {
        let trimmed = base_url.trim_end_matches('/');
        let url = format!("{}/api/v1/health", trimmed);
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| CoolifyError::Network(e.to_string()))?;
        let resp = http
            .get(&url)
            .send()
            .await
            .map_err(|e| CoolifyError::Network(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(CoolifyError::Server(resp.status().as_u16()));
        }
        resp.text()
            .await
            .map_err(|e| CoolifyError::Network(e.to_string()))
    }
}
