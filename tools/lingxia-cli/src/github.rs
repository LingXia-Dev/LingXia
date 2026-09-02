//! GitHub API client module.
//!
//! Provides unified access to GitHub resources:
//! - File content fetching (raw URL with API fallback for private repos)
//! - Release asset downloading
//!
//! Supports both public and private repos via GITHUB_TOKEN environment variable.

use anyhow::{Context, Result, anyhow};
use semver::Version;
use serde::Deserialize;
use std::env;
use std::process::Command;

/// GitHub repository owner/name
pub const DEFAULT_GITHUB_REPO: &str = "LingXia-Dev/LingXia";

/// Timeout for large file downloads (seconds)
const DOWNLOAD_TIMEOUT_SECS: u64 = 300;
/// Timeout for lightweight release metadata checks (seconds)
const RELEASE_INFO_TIMEOUT_SECS: u64 = 5;

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

#[derive(Debug, Deserialize)]
struct GitHubReleaseSummary {
    tag_name: String,
    draft: bool,
    prerelease: bool,
}

#[derive(Debug, Clone)]
pub struct CliReleaseTag {
    pub tag: String,
    pub version: String,
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
pub fn release_repo() -> String {
    env::var("LINGXIA_RELEASE_REPO").unwrap_or_else(|_| DEFAULT_GITHUB_REPO.to_string())
}

pub fn download_release_asset_from_repo(
    repo: &str,
    tag: &str,
    asset_name: &str,
) -> Result<Vec<u8>> {
    let token = get_token();
    let agent = create_agent(DOWNLOAD_TIMEOUT_SECS);

    // Step 1: Get release info by tag
    let release_url = format!(
        "https://api.github.com/repos/{}/releases/tags/{}",
        repo, tag
    );

    let response =
        get_api_text(&release_url, token.as_deref(), DOWNLOAD_TIMEOUT_SECS).map_err(|e| {
            anyhow!(
                "Failed to fetch release info\n  Tag: {}\n  Cause: {}",
                tag,
                e
            )
        })?;

    let status = response.status;
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

    let release: GitHubRelease =
        serde_json::from_str(&response.body).context("Failed to parse release info")?;

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

    if let Some(bytes) = download_release_asset_with_system_tool(&asset.url, token.as_deref())? {
        return Ok(bytes);
    }

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

/// A GitHub API response body plus the status it came back with.
struct ApiText {
    status: u16,
    body: String,
}

/// GET a GitHub API endpoint as text.
///
/// Falls back to the system `curl`/`wget` the way asset downloads do: a
/// transport failure here is almost always a TLS trust problem (intercepting
/// proxy, custom root) that the system tools are already configured for, and
/// without the fallback it takes down the whole release lookup — the step every
/// SDK fetch and update check starts with.
fn get_api_text(url: &str, token: Option<&str>, timeout_secs: u64) -> Result<ApiText> {
    let agent = create_agent(timeout_secs);
    let mut request = agent
        .get(url)
        .header("User-Agent", "lingxia-cli")
        .header("Accept", "application/vnd.github+json");
    if let Some(token) = token {
        request = request.header("Authorization", &format!("Bearer {}", token));
    }

    let transport_err = match request.call() {
        Ok(mut response) => {
            let status = response.status().as_u16();
            match response.body_mut().read_to_string() {
                Ok(body) => return Ok(ApiText { status, body }),
                Err(err) => err,
            }
        }
        Err(err) => err,
    };

    match get_api_text_with_system_tool(url, token, timeout_secs) {
        Ok(Some(text)) => Ok(text),
        Ok(None) => Err(anyhow!("{transport_err}{}", transport_hint(&transport_err))),
        Err(system_err) => Err(anyhow!(
            "{transport_err}{}\n  Fallback: {system_err}",
            transport_hint(&transport_err)
        )),
    }
}

/// Extra guidance for the failure mode we cannot work around ourselves.
fn transport_hint(err: &ureq::Error) -> String {
    if err.to_string().contains("certificate") {
        "\n  Hint: TLS trust failure. If this network inspects TLS, install its root CA in the \
         system trust store or point SSL_CERT_FILE at it."
            .to_string()
    } else {
        String::new()
    }
}

fn get_api_text_with_system_tool(
    url: &str,
    token: Option<&str>,
    timeout_secs: u64,
) -> Result<Option<ApiText>> {
    if let Some(text) = get_api_text_with_curl(url, token, timeout_secs)? {
        return Ok(Some(text));
    }
    get_api_text_with_wget(url, token, timeout_secs)
}

fn get_api_text_with_curl(
    url: &str,
    token: Option<&str>,
    timeout_secs: u64,
) -> Result<Option<ApiText>> {
    let mut command = Command::new("curl");
    // The fallback inherits the caller's budget: the update check gives this
    // whole lookup a few seconds and must not hang on a dead network.
    command.args(["--max-time", &timeout_secs.to_string()]);
    command.args([
        "--silent",
        "--show-error",
        "--location",
        "--http1.1",
        "-H",
        "User-Agent: lingxia-cli",
        "-H",
        "Accept: application/vnd.github+json",
        // Keep the status: the callers branch on 404/403 rather than on failure.
        "-w",
        "\n%{http_code}",
    ]);
    if let Some(token) = token {
        command.args(["-H", &format!("Authorization: Bearer {token}")]);
    }
    command.arg(url);

    match command.output() {
        Ok(output) if output.status.success() => {
            split_status_suffix(&String::from_utf8_lossy(&output.stdout)).map(Some)
        }
        Ok(output) => Err(anyhow!(
            "curl failed to fetch {url}\n  Status: {}\n  Stderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).context("Failed to invoke curl for GitHub API request"),
    }
}

fn get_api_text_with_wget(
    url: &str,
    token: Option<&str>,
    timeout_secs: u64,
) -> Result<Option<ApiText>> {
    let mut command = Command::new("wget");
    command.args(["--tries=1", &format!("--timeout={timeout_secs}")]);
    command.args([
        "-qO-",
        "--header=User-Agent: lingxia-cli",
        "--header=Accept: application/vnd.github+json",
    ]);
    if let Some(token) = token {
        command.arg(format!("--header=Authorization: Bearer {token}"));
    }
    command.arg(url);

    match command.output() {
        // wget has no cheap way to hand back the status, and it only exits 0 on
        // a 2xx, so a success is a 200 as far as the callers are concerned.
        Ok(output) if output.status.success() => Ok(Some(ApiText {
            status: 200,
            body: String::from_utf8_lossy(&output.stdout).into_owned(),
        })),
        Ok(output) => Err(anyhow!(
            "wget failed to fetch {url}\n  Status: {}\n  Stderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).context("Failed to invoke wget for GitHub API request"),
    }
}

/// Split curl's `-w '\n%{http_code}'` suffix off the response body.
fn split_status_suffix(raw: &str) -> Result<ApiText> {
    let (body, status) = raw
        .rsplit_once('\n')
        .ok_or_else(|| anyhow!("curl returned no status code"))?;
    let status = status
        .trim()
        .parse::<u16>()
        .with_context(|| format!("curl returned an unreadable status code {status:?}"))?;
    Ok(ApiText {
        status,
        body: body.to_string(),
    })
}

fn download_release_asset_with_system_tool(
    asset_url: &str,
    token: Option<&str>,
) -> Result<Option<Vec<u8>>> {
    if let Some(bytes) = download_with_curl(asset_url, token)? {
        return Ok(Some(bytes));
    }
    if let Some(bytes) = download_with_wget(asset_url, token)? {
        return Ok(Some(bytes));
    }
    Ok(None)
}

fn download_with_curl(asset_url: &str, token: Option<&str>) -> Result<Option<Vec<u8>>> {
    let mut command = Command::new("curl");
    command.args([
        "--fail",
        "--silent",
        "--show-error",
        "--location",
        "--http1.1",
        "-H",
        "User-Agent: lingxia-cli",
        "-H",
        "Accept: application/octet-stream",
    ]);
    if let Some(token) = token {
        command.args(["-H", &format!("Authorization: Bearer {token}")]);
    }
    command.arg(asset_url);

    match command.output() {
        Ok(output) if output.status.success() => Ok(Some(output.stdout)),
        Ok(output) => Err(anyhow!(
            "curl failed to download release asset\n  Status: {}\n  Stderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).context("Failed to invoke curl for release asset download"),
    }
}

fn download_with_wget(asset_url: &str, token: Option<&str>) -> Result<Option<Vec<u8>>> {
    let mut command = Command::new("wget");
    command.args([
        "-qO-",
        "--header=User-Agent: lingxia-cli",
        "--header=Accept: application/octet-stream",
    ]);
    if let Some(token) = token {
        command.arg(format!("--header=Authorization: Bearer {token}"));
    }
    command.arg(asset_url);

    match command.output() {
        Ok(output) if output.status.success() => Ok(Some(output.stdout)),
        Ok(output) => Err(anyhow!(
            "wget failed to download release asset\n  Status: {}\n  Stderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).context("Failed to invoke wget for release asset download"),
    }
}

pub fn latest_cli_release_from_repo(repo: &str) -> Result<CliReleaseTag> {
    let token = get_token();
    let releases_url = format!(
        "https://api.github.com/repos/{}/releases?per_page=100",
        repo
    );

    let response = get_api_text(&releases_url, token.as_deref(), RELEASE_INFO_TIMEOUT_SECS)
        .map_err(|e| {
            anyhow!(
                "Failed to fetch release list\n  Repo: {}\n  Cause: {}",
                repo,
                e
            )
        })?;

    if response.status != 200 {
        return Err(anyhow!(
            "Failed to fetch release list (HTTP {})\n  Repo: {}\n  Hint: set GITHUB_TOKEN for private repos",
            response.status,
            repo
        ));
    }

    let releases: Vec<GitHubReleaseSummary> =
        serde_json::from_str(&response.body).context("Failed to parse release list")?;

    select_latest_cli_release(releases)
        .ok_or_else(|| anyhow!("No valid CLI release found in GitHub release list"))
}

fn select_latest_cli_release(releases: Vec<GitHubReleaseSummary>) -> Option<CliReleaseTag> {
    releases
        .into_iter()
        .filter(|release| !release.draft && !release.prerelease)
        .filter_map(|release| {
            let version = release.tag_name.strip_prefix("lingxia-cli-v")?;
            let parsed = Version::parse(version).ok()?;
            Some((parsed, release.tag_name))
        })
        .max_by(|(left, _), (right, _)| left.cmp(right))
        .map(|(version, tag)| CliReleaseTag {
            tag,
            version: version.to_string(),
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_status_suffix_keeps_a_multiline_body() {
        let parsed = split_status_suffix("{\n  \"tag_name\": \"v1\"\n}\n200").unwrap();
        assert_eq!(parsed.status, 200);
        assert_eq!(parsed.body, "{\n  \"tag_name\": \"v1\"\n}");
    }

    #[test]
    fn split_status_suffix_reports_an_error_status() {
        let parsed = split_status_suffix("Not Found\n404").unwrap();
        assert_eq!(parsed.status, 404);
        assert_eq!(parsed.body, "Not Found");
    }

    #[test]
    fn split_status_suffix_rejects_output_without_a_status() {
        assert!(split_status_suffix("no status here").is_err());
        assert!(split_status_suffix("body\nnot-a-status").is_err());
    }

    #[test]
    fn select_latest_cli_release_uses_highest_semver() {
        let release = select_latest_cli_release(vec![
            GitHubReleaseSummary {
                tag_name: "lingxia-cli-v0.4.2".to_string(),
                draft: false,
                prerelease: false,
            },
            GitHubReleaseSummary {
                tag_name: "lingxia-cli-v0.4.10".to_string(),
                draft: false,
                prerelease: false,
            },
            GitHubReleaseSummary {
                tag_name: "lingxia-cli-v0.4.3".to_string(),
                draft: false,
                prerelease: false,
            },
        ])
        .expect("expected a valid release");

        assert_eq!(release.tag, "lingxia-cli-v0.4.10");
        assert_eq!(release.version, "0.4.10");
    }

    #[test]
    fn select_latest_cli_release_ignores_invalid_and_prerelease_tags() {
        let release = select_latest_cli_release(vec![
            GitHubReleaseSummary {
                tag_name: "lingxia-cli-v0.5.0-beta.1".to_string(),
                draft: false,
                prerelease: true,
            },
            GitHubReleaseSummary {
                tag_name: "lingxia-cli-vnot-a-version".to_string(),
                draft: false,
                prerelease: false,
            },
            GitHubReleaseSummary {
                tag_name: "lingxia-sdk-v0.5.0".to_string(),
                draft: false,
                prerelease: false,
            },
            GitHubReleaseSummary {
                tag_name: "lingxia-cli-v0.4.9".to_string(),
                draft: false,
                prerelease: false,
            },
        ])
        .expect("expected a valid release");

        assert_eq!(release.tag, "lingxia-cli-v0.4.9");
        assert_eq!(release.version, "0.4.9");
    }
}
