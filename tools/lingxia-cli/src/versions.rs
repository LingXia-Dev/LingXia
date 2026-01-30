//! Version discovery module for LingXia dependencies.
//!
//! Fetches all versions from GitHub:
//! - Public repo: direct raw.githubusercontent.com
//! - Private repo: GitHub API with GITHUB_TOKEN authorization

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;
use std::env;
use std::time::Duration;

/// GitHub raw file URL (for public repos)
const VERSIONS_RAW_URL: &str =
    "https://raw.githubusercontent.com/LingXia-Dev/LingXia/main/lingxia-versions.json";

/// GitHub API URL (for private repos with token)
const VERSIONS_API_URL: &str =
    "https://api.github.com/repos/LingXia-Dev/LingXia/contents/lingxia-versions.json?ref=main";

/// Timeout for network requests (seconds)
const TIMEOUT_SECS: u64 = 10;

/// LingXia component versions used in project templates
#[derive(Debug, Clone)]
pub struct LingXiaVersions {
    /// @lingxia/rong NPM package version
    pub rong: String,
    /// lingxia Rust crate version
    pub rust_crate: String,
    /// Native SDK version (Android/iOS/HarmonyOS)
    pub sdk: String,
}

/// Versions JSON structure (lingxia-versions.json)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct VersionsJson {
    latest: String,
    #[serde(default)]
    android: Option<String>,
    #[serde(default)]
    ios: Option<String>,
    #[serde(default)]
    harmony: Option<String>,
    #[serde(default, rename = "crate")]
    crate_version: Option<String>,
    #[serde(default)]
    rong: Option<String>,
    #[serde(default)]
    min_cli_version: Option<String>,
}

/// Fetch all latest LingXia versions from GitHub
///
/// Supports private repos via GITHUB_TOKEN environment variable.
pub fn fetch_latest_versions() -> Result<LingXiaVersions> {
    let json = fetch_versions_from_github()?;
    let latest = json.latest.clone();
    Ok(LingXiaVersions {
        rong: json.rong.unwrap_or_else(|| latest.clone()),
        rust_crate: json.crate_version.unwrap_or_else(|| latest.clone()),
        sdk: json.android.unwrap_or_else(|| latest.clone()),
    })
}

/// Fetch versions JSON from GitHub (supports private repo with GITHUB_TOKEN)
///
/// Strategy:
/// 1. Try raw URL first (works for public repos)
/// 2. If failed and GITHUB_TOKEN is set, retry with GitHub API (for private repos)
fn fetch_versions_from_github() -> Result<VersionsJson> {
    let github_token = env::var("GITHUB_TOKEN").ok();

    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(TIMEOUT_SECS)))
        .http_status_as_error(false)
        .build();
    let agent = ureq::Agent::new_with_config(config);

    // First, try raw URL (works for public repos)
    let request = agent
        .get(VERSIONS_RAW_URL)
        .header("User-Agent", "lingxia-cli");

    let mut response = request.call().map_err(|e| {
        anyhow!(
            "Failed to connect to GitHub, please check your network connection\n  Cause: {}",
            e
        )
    })?;

    let status = response.status().as_u16();

    // If raw URL failed and we have a token, try GitHub API (for private repos)
    if status != 200 {
        if let Some(token) = &github_token {
            return fetch_via_github_api(&agent, token);
        }

        let hint = match status {
            404 => {
                "Version file not found. For private repos, set GITHUB_TOKEN environment variable"
            }
            401 | 403 => "Access denied, set GITHUB_TOKEN environment variable for private repos",
            500..=599 => "GitHub service temporarily unavailable, please try again later",
            _ => "Request failed",
        };
        return Err(anyhow!(
            "Failed to fetch version info (HTTP {}): {}",
            status,
            hint
        ));
    }

    let json_str = response
        .body_mut()
        .read_to_string()
        .context("Failed to read response body")?;

    serde_json::from_str(&json_str).map_err(|e| anyhow!("Invalid version file format: {e}"))
}

/// Fetch via GitHub API (for private repos)
fn fetch_via_github_api(agent: &ureq::Agent, token: &str) -> Result<VersionsJson> {
    let request = agent
        .get(VERSIONS_API_URL)
        .header("User-Agent", "lingxia-cli")
        .header("Authorization", &format!("Bearer {}", token))
        .header("Accept", "application/vnd.github.raw+json");

    let mut response = request.call().map_err(|e| {
        anyhow!(
            "Failed to connect to GitHub API, please check your network connection\n  Cause: {}",
            e
        )
    })?;

    let status = response.status().as_u16();
    if status != 200 {
        let hint = match status {
            404 => "Version file not found, or GITHUB_TOKEN lacks 'repo' scope",
            401 | 403 => "Access denied, check GITHUB_TOKEN has 'repo' scope for private repos",
            500..=599 => "GitHub service temporarily unavailable, please try again later",
            _ => "Request failed",
        };
        return Err(anyhow!(
            "Failed to fetch version info (HTTP {}): {}",
            status,
            hint
        ));
    }

    let json_str = response
        .body_mut()
        .read_to_string()
        .context("Failed to read response body")?;

    serde_json::from_str(&json_str).map_err(|e| anyhow!("Invalid version file format: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_versions() {
        let json = r#"{"latest":"0.1.1","android":"0.1.1","ios":"0.1.1","harmony":"0.1.1","crate":"0.1.1","rong":"0.1.1","minCliVersion":"0.0.8"}"#;
        let versions: VersionsJson = serde_json::from_str(json).unwrap();
        assert_eq!(versions.latest, "0.1.1");
    }
}
