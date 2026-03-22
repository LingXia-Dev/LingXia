//! Version discovery module for LingXia dependencies.

use crate::runtime;
use anyhow::{Context, Result, anyhow};
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

#[derive(Debug, Deserialize)]
struct CratesIoResponse {
    #[serde(rename = "crate")]
    crate_meta: CratesIoCrateMeta,
}

#[derive(Debug, Deserialize)]
struct CratesIoCrateMeta {
    newest_version: String,
}

const CRATES_IO_URL: &str = "https://crates.io/api/v1/crates/lingxia";
const RONG_PACKAGE: &str = "@lingxia/rong";
const HTTP_TIMEOUT_SECS: u64 = 10;

/// Fetch latest LingXia versions from registry metadata.
///
/// - `lingxia_crate`: latest version from crates.io
/// - `sdk`: same as lingxia crate version
/// - `rong`: same as lingxia crate version, must exist on npm
pub fn fetch_latest_versions() -> Result<LingXiaVersions> {
    let mut resp = crate::http_client::create_agent(HTTP_TIMEOUT_SECS)
        .get(CRATES_IO_URL)
        .header("User-Agent", "lingxia-cli")
        .call()
        .map_err(|e| anyhow!("Failed to query crates.io: {e}"))?;
    if resp.status().as_u16() != 200 {
        return Err(anyhow!(
            "Failed to query crates.io (HTTP {})",
            resp.status().as_u16()
        ));
    }

    let body = resp
        .body_mut()
        .read_to_string()
        .context("Failed to read crates.io response body")?;
    let json: CratesIoResponse =
        serde_json::from_str(&body).map_err(|e| anyhow!("Invalid crates.io response: {e}"))?;
    let version = json.crate_meta.newest_version;

    runtime::ensure_npm_package_version_exists(RONG_PACKAGE, &version).map_err(|e| {
        anyhow!(
            "Latest lingxia crate version is {version}, but {RONG_PACKAGE}@{version} is unavailable.\n\
Publish {RONG_PACKAGE}@{version} before running `lingxia new`.\n\
Details: {e}"
        )
    })?;

    Ok(LingXiaVersions {
        rong: version.clone(),
        lingxia_crate: version.clone(),
        sdk: version,
    })
}
