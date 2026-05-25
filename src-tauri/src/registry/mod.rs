//! OCI registry interaction: manifest digest fetch, tag listing, on-disk cache.
//!
//! Powers the "image freshness" check across docker / docker-compose resources.
//! Uses the `oci-client` crate (rename of upstream `oci-distribution`, now
//! maintained under the `oras-project` org).
//!
//! Commands exported:
//! - [`commands::check_image`]
//! - [`commands::read_image_cache`]

pub mod cache;
pub mod commands;
pub mod digest;
pub mod tags;

use thiserror::Error;

/// Errors that can come out of a registry interaction.
///
/// Mapped from `oci_client::errors::OciDistributionError` and `reqwest::Error`.
/// Frontend never sees this directly — Tauri commands stringify via `to_string()`.
#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("network error: {0}")]
    Network(String),
    #[error("rate limited by registry")]
    RateLimited,
    #[error("unauthorized")]
    Unauthorized,
    #[error("image or tag not found")]
    NotFound,
    #[error("failed to parse image reference: {0}")]
    Parse(String),
    #[error("registry returned malformed response: {0}")]
    Malformed(String),
}
