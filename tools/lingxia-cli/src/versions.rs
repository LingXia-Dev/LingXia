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
    pub lingxia_crate: String,
    /// Native SDK version (Android/iOS/HarmonyOS)
    pub sdk: String,
}

/// Versions JSON structure (lingxia-versions.json)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct VersionsJson {
    android: String,
    ios: String,
    harmony: String,
    lingxia: String,
    rong: String,
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
        lingxia_crate: json.lingxia,
        sdk: json.android,
    })
}
