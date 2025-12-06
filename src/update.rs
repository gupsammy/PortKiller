use anyhow::{Context, Result};
use serde::Deserialize;
use std::time::Duration;

const GITHUB_API_URL: &str = "https://api.github.com/repos/gupsammy/PortKiller/releases/latest";
const DOWNLOAD_URL: &str =
    "https://github.com/gupsammy/PortKiller/releases/latest/download/PortKiller.dmg";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, Default)]
pub struct UpdateInfo {
    pub version: String,
    pub download_url: String,
    pub release_notes: Option<String>,
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    prerelease: bool,
    body: Option<String>,
}

/// Check GitHub for a newer version.
/// Returns Some(UpdateInfo) if an update is available, None if current.
pub fn check_for_update() -> Result<Option<UpdateInfo>> {
    let current_version = env!("CARGO_PKG_VERSION");
    log::debug!(
        "Checking for updates, current version: v{}",
        current_version
    );

    let response = ureq::get(GITHUB_API_URL)
        .set("User-Agent", "PortKiller")
        .set("Accept", "application/vnd.github.v3+json")
        .timeout(REQUEST_TIMEOUT)
        .call()
        .context("failed to fetch latest release from GitHub")?;

    let body = response
        .into_string()
        .context("failed to read response body")?;

    let release: GitHubRelease =
        serde_json::from_str(&body).context("failed to parse GitHub release response")?;

    // Skip pre-releases
    if release.prerelease {
        log::debug!(
            "Latest release {} is a pre-release, skipping",
            release.tag_name
        );
        return Ok(None);
    }

    // Remove 'v' prefix if present
    let latest_version = release.tag_name.trim_start_matches('v');

    if is_newer_version(latest_version, current_version) {
        log::info!(
            "Update available: v{} -> v{}",
            current_version,
            latest_version
        );
        Ok(Some(UpdateInfo {
            version: latest_version.to_string(),
            download_url: DOWNLOAD_URL.to_string(),
            release_notes: release.body,
        }))
    } else {
        log::debug!("No update available (latest: v{})", latest_version);
        Ok(None)
    }
}

/// Compare semver versions. Returns true if `latest` is newer than `current`.
fn is_newer_version(latest: &str, current: &str) -> bool {
    let parse_version = |v: &str| -> (u32, u32, u32) {
        let parts: Vec<u32> = v.split('.').filter_map(|p| p.parse().ok()).collect();
        (
            parts.first().copied().unwrap_or(0),
            parts.get(1).copied().unwrap_or(0),
            parts.get(2).copied().unwrap_or(0),
        )
    };

    let (l_major, l_minor, l_patch) = parse_version(latest);
    let (c_major, c_minor, c_patch) = parse_version(current);

    (l_major, l_minor, l_patch) > (c_major, c_minor, c_patch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(is_newer_version("1.0.0", "0.9.0"));
        assert!(is_newer_version("0.2.0", "0.1.5"));
        assert!(is_newer_version("0.1.6", "0.1.5"));
        assert!(!is_newer_version("0.1.5", "0.1.5"));
        assert!(!is_newer_version("0.1.4", "0.1.5"));
        assert!(is_newer_version("1.0.0", "0.99.99"));
    }
}
