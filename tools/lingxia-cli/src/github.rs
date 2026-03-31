//! GitHub API client module.
//!
//! Provides unified access to GitHub resources:
//! - File content fetching (raw URL with API fallback for private repos)
//! - Release asset downloading
//!
//! Supports both public and private repos via GITHUB_TOKEN environment variable.

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;
use std::env;

/// GitHub repository owner/name
const GITHUB_REPO: &str = "LingXia-Dev/LingXia";

/// Timeout for large file downloads (seconds)
const DOWNLOAD_TIMEOUT_SECS: u64 = 300;

/// GitHub release asset info
#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    url: String,
}

/// GitHub release info
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    assets: Vec<GitHubAsset>,
}

/// Create a ureq agent with specified timeout
fn create_agent(timeout_secs: u64) -> ureq::Agent {
    crate::http_client::create_agent(timeout_secs)
}

/// Get GitHub token from environment
fn get_token() -> Option<String> {
    env::var("GITHUB_TOKEN").ok()
}

/// Download a release asset from GitHub.
///
/// Uses GitHub API to get asset download URL (works for both public and private repos).
pub fn download_release_asset(tag: &str, asset_name: &str) -> Result<Vec<u8>> {
    let token = get_token();
    let agent = create_agent(DOWNLOAD_TIMEOUT_SECS);

    // Step 1: Get release info by tag
    let release_url = format!(
        "https://api.github.com/repos/{}/releases/tags/{}",
        GITHUB_REPO, tag
    );

    let mut request = agent
        .get(&release_url)
        .header("User-Agent", "lingxia-cli")
        .header("Accept", "application/vnd.github+json");

    if let Some(ref token) = token {
        request = request.header("Authorization", &format!("Bearer {}", token));
    }

    let mut response = request.call().map_err(|e| {
        anyhow!(
            "Failed to fetch release info\n  Tag: {}\n  Cause: {}",
            tag,
            e
        )
    })?;

    let status = response.status().as_u16();
    if status != 200 {
        let hint = match status {
            404 => {
                "Release not found. Check if the tag exists, or set GITHUB_TOKEN for private repos"
            }
            401 | 403 => "Access denied. Set GITHUB_TOKEN with 'repo' scope for private repos",
            _ => "Failed to fetch release",
        };
        return Err(anyhow!(
            "Failed to fetch release (HTTP {}): {}\n  Tag: {}",
            status,
            hint,
            tag
        ));
    }

    let body = response
        .body_mut()
        .read_to_string()
        .context("Failed to read release info")?;
    let release: GitHubRelease =
        serde_json::from_str(&body).context("Failed to parse release info")?;

    // Step 2: Find the asset by name
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == asset_name)
        .ok_or_else(|| {
            let available = release
                .assets
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            anyhow!(
                "Asset '{}' not found in release '{}'\n  Available: {}",
                asset_name,
                tag,
                if available.is_empty() {
                    "(none)"
                } else {
                    &available
                }
            )
        })?;

    // Step 3: Download the asset using its API URL
    let mut request = agent
        .get(&asset.url)
        .header("User-Agent", "lingxia-cli")
        .header("Accept", "application/octet-stream");

    if let Some(ref token) = token {
        request = request.header("Authorization", &format!("Bearer {}", token));
    }

    let mut response = request.call().map_err(|e| {
        anyhow!(
            "Failed to download asset\n  Asset: {}\n  Cause: {}",
            asset_name,
            e
        )
    })?;

    let status = response.status().as_u16();

    // Handle redirect (GitHub API redirects to the actual download URL)
    if (status == 302 || status == 301)
        && let Some(location) = response.headers().get("location")
    {
        return download_url_direct(location.to_str().unwrap_or_default());
    }

    if status != 200 {
        return Err(anyhow!(
            "Failed to download asset (HTTP {})\n  Asset: {}",
            status,
            asset_name
        ));
    }

    response
        .body_mut()
        .read_to_vec()
        .context("Failed to read asset data")
}

/// Download from a direct URL (for redirected downloads)
fn download_url_direct(url: &str) -> Result<Vec<u8>> {
    let agent = create_agent(DOWNLOAD_TIMEOUT_SECS);

    let mut response = agent
        .get(url)
        .header("User-Agent", "lingxia-cli")
        .call()
        .map_err(|e| anyhow!("Failed to download\n  Cause: {}", e))?;

    let status = response.status().as_u16();
    if status != 200 {
        return Err(anyhow!("Failed to download (HTTP {})", status));
    }

    response
        .body_mut()
        .read_to_vec()
        .context("Failed to read data")
}
