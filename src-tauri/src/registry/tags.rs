//! Tag listing + semver picking for an image repository.

use oci_client::client::ClientConfig;
use oci_client::secrets::RegistryAuth;
use oci_client::{Client, Reference};
use semver::Version;

use super::RegistryError;
use super::digest::map_oci_err;

/// List all tags for a given image repository.
///
/// `image_name` may be a bare repo (`"nginx"`, `"library/redis"`) or include a
/// registry host (`"ghcr.io/foo/bar"`). The reference parser tolerates a
/// missing tag — we attach `:latest` only to satisfy `Reference`, then the API
/// call ignores the tag portion (it queries `/v2/<name>/tags/list`).
pub async fn list_tags(
    image_name: &str,
    auth: Option<(&str, &str)>,
) -> Result<Vec<String>, RegistryError> {
    let with_tag = if image_name.contains(':') {
        image_name.to_string()
    } else {
        format!("{image_name}:latest")
    };

    let reference: Reference = with_tag
        .parse()
        .map_err(|e: oci_client::ParseError| RegistryError::Parse(e.to_string()))?;

    let registry_auth = match auth {
        Some((user, pass)) => RegistryAuth::Basic(user.to_string(), pass.to_string()),
        None => RegistryAuth::Anonymous,
    };

    let client = Client::new(ClientConfig::default());

    let response = client
        .list_tags(&reference, &registry_auth, None, None)
        .await
        .map_err(map_oci_err)?;

    Ok(response.tags)
}

/// Pick the highest semver tag from a list, ignoring non-semver entries.
///
/// Tolerates leading `v` (e.g. `v1.2.3`) by stripping it before parse. Returns
/// the original tag string so the caller can show what's in the registry.
pub fn pick_highest_semver(tags: &[String]) -> Option<String> {
    tags.iter()
        .filter_map(|t| {
            let stripped = t.strip_prefix('v').unwrap_or(t);
            Version::parse(stripped).ok().map(|v| (v, t.clone()))
        })
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, original)| original)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_highest_semver_among_mixed_tags() {
        let tags = vec![
            "latest".to_string(),
            "1.2.3".to_string(),
            "v1.2.4".to_string(),
            "1.10.0".to_string(),
            "alpine".to_string(),
            "1.9.99".to_string(),
        ];
        assert_eq!(pick_highest_semver(&tags), Some("1.10.0".to_string()));
    }

    #[test]
    fn returns_none_when_no_semver() {
        let tags = vec!["latest".to_string(), "alpine".to_string()];
        assert_eq!(pick_highest_semver(&tags), None);
    }
}
