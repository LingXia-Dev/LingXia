//! Version discovery module for LingXia dependencies.

use crate::github;
use anyhow::{Result, anyhow};
use serde::Deserialize;

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
    android: String,
    #[serde(default)]
    ios: Option<String>,
    #[serde(default)]
    harmony: Option<String>,
    #[serde(rename = "crate")]
    crate_version: String,
    rong: String,
    #[serde(default)]
    min_cli_version: Option<String>,
}

/// Fetch all latest LingXia versions from GitHub
///
/// Supports private repos via GITHUB_TOKEN environment variable.
pub fn fetch_latest_versions() -> Result<LingXiaVersions> {
    let content = github::fetch_file_content("lingxia-versions.json", "main")?;
    let json: VersionsJson =
        serde_json::from_str(&content).map_err(|e| anyhow!("Invalid version file format: {e}"))?;

    Ok(LingXiaVersions {
        rong: json.rong,
        rust_crate: json.crate_version,
        sdk: json.android,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_versions() {
        let json = r#"{"android":"0.1.1","ios":"0.1.1","harmony":"0.1.1","crate":"0.1.1","rong":"0.1.1","minCliVersion":"0.0.8"}"#;
        let versions: VersionsJson = serde_json::from_str(json).unwrap();
        assert_eq!(versions.android, "0.1.1");
    }
}
