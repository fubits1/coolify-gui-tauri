//! Manifest digest fetch for a single `name:tag` image reference.

use oci_client::client::ClientConfig;
use oci_client::errors::OciDistributionError;
use oci_client::secrets::RegistryAuth;
use oci_client::{Client, Reference};

use super::RegistryError;

/// Fetch the manifest digest (sha256 string) for a given image reference.
///
/// `image_ref` may be:
/// - `"nginx:latest"` (defaults to Docker Hub `library/`)
/// - `"ghcr.io/foo/bar:1.2.3"`
/// - `"registry.example.com/team/svc:tag"`
///
/// `auth` switches between anonymous (None) and HTTP Basic (Some).
pub async fn fetch_manifest_digest(
    image_ref: &str,
    auth: Option<(&str, &str)>,
) -> Result<String, RegistryError> {
    let reference: Reference = image_ref
        .parse()
        .map_err(|e: oci_client::ParseError| RegistryError::Parse(e.to_string()))?;

    let registry_auth = match auth {
        Some((user, pass)) => RegistryAuth::Basic(user.to_string(), pass.to_string()),
        None => RegistryAuth::Anonymous,
    };

    // Default ClientConfig uses HTTPS with rustls (we opted into rustls-tls feature).
    let client = Client::new(ClientConfig::default());

    let (_manifest, digest) = client
        .pull_manifest(&reference, &registry_auth)
        .await
        .map_err(map_oci_err)?;

    Ok(digest)
}

pub(super) fn map_oci_err(e: OciDistributionError) -> RegistryError {
    match e {
        OciDistributionError::AuthenticationFailure(_)
        | OciDistributionError::UnauthorizedError { .. } => RegistryError::Unauthorized,
        OciDistributionError::ImageManifestNotFoundError(_) => RegistryError::NotFound,
        OciDistributionError::ServerError { code: 429, .. } => RegistryError::RateLimited,
        OciDistributionError::ServerError { code: 404, .. } => RegistryError::NotFound,
        OciDistributionError::ServerError {
            code: 401 | 403, ..
        } => RegistryError::Unauthorized,
        other => {
            let s = other.to_string();
            if s.to_lowercase().contains("toomanyrequests") {
                RegistryError::RateLimited
            } else {
                RegistryError::Network(s)
            }
        }
    }
}
