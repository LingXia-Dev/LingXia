//! Plugin management module for LingXia.
//!
//! Plugins are lightweight page packages that can be downloaded and loaded
//! within the current LxApp's WebView. They are managed separately from LxApps
//! and stored in the `plugins` directory.
//!
//! # State Model
//!
//! Plugin state is distributed across multiple locations by design:
//!
//! - **Download state** (`DOWNLOAD_TRACKER` here): Global/process-level. Ensures
//!   each plugin@version is downloaded only once, shared across all LxApps.
//!
//! - **Loaded plugins** (`loaded_plugins` in `runtime_ctx.rs`): Per-LxApp/JSContext.
//!   Tracks which logic.js files have been evaluated in each context.

use crate::archive;
use crate::error::LxAppError;
use crate::lxapp::config::LxPlugin;
use crate::lxapp::{LINGXIA_DIR, PLUGINS_DIR};
use crate::provider::UpdateCheckResult;
use crate::warn;
use dashmap::DashMap;
use lingxia_platform::{AppRuntime, Platform};
use rong::service_executor;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use tokio::sync::watch;

const PLUGIN_URL_SCHEME: &str = "plugin://";
const PLUGIN_PAGE_PATH_PREFIX: &str = "@plugin/";
fn plugin_key(name: &str, version: &str) -> String {
    format!("{}@{}", name, version)
}

/// Plugin download state
#[derive(Clone, Debug, PartialEq)]
pub enum PluginDownloadState {
    /// Plugin is being downloaded
    Downloading,
    /// Plugin download completed successfully
    Completed,
    /// Plugin download failed with error message
    Failed(String),
}

/// Global plugin download state tracker
/// Prevents duplicate downloads and allows UI to check download status
struct PluginDownloadTracker {
    /// Map of plugin key (name@version) -> (state, notifier)
    /// The notifier allows waiting for download completion
    downloads: DashMap<String, (PluginDownloadState, watch::Sender<PluginDownloadState>)>,
}

impl PluginDownloadTracker {
    fn new() -> Self {
        Self {
            downloads: DashMap::new(),
        }
    }

    /// Try to start downloading a plugin. Returns None if download is already in progress.
    /// Returns Some(receiver) if this call initiated the download.
    fn try_start_download(&self, key: &str) -> Option<watch::Receiver<PluginDownloadState>> {
        // Check if already downloading
        if self.downloads.contains_key(key) {
            return None;
        }

        // Start new download
        let (tx, rx) = watch::channel(PluginDownloadState::Downloading);
        self.downloads
            .insert(key.to_string(), (PluginDownloadState::Downloading, tx));
        Some(rx)
    }

    /// Mark download as completed
    fn mark_completed(&self, key: &str) {
        if let Some(mut entry) = self.downloads.get_mut(key) {
            entry.0 = PluginDownloadState::Completed;
            let _ = entry.1.send(PluginDownloadState::Completed);
        }
        // Remove from tracking after completion
        self.downloads.remove(key);
    }

    /// Mark download as failed
    fn mark_failed(&self, key: &str, error: String) {
        if let Some(mut entry) = self.downloads.get_mut(key) {
            let state = PluginDownloadState::Failed(error.clone());
            entry.0 = state.clone();
            let _ = entry.1.send(state);
        }
        // Remove from tracking after failure
        self.downloads.remove(key);
    }

    /// Get a receiver to wait for download completion
    fn get_download_receiver(&self, key: &str) -> Option<watch::Receiver<PluginDownloadState>> {
        self.downloads.get(key).map(|e| e.1.subscribe())
    }
}

static DOWNLOAD_TRACKER: OnceLock<PluginDownloadTracker> = OnceLock::new();

fn get_tracker() -> &'static PluginDownloadTracker {
    DOWNLOAD_TRACKER.get_or_init(PluginDownloadTracker::new)
}

/// Get a receiver to wait for a plugin's download completion
/// Returns None if the plugin is not being downloaded
pub fn wait_for_download(
    plugin_name: &str,
    version: &str,
) -> Option<watch::Receiver<PluginDownloadState>> {
    get_tracker().get_download_receiver(&plugin_key(plugin_name, version))
}

/// Parse a plugin:// URL into (plugin_name, page_path).
///
/// # Example
/// ```
/// let url = "plugin://myPlugin/pages/index";
/// let (name, path) = parse_plugin_url(url).unwrap();
/// assert_eq!(name, "myPlugin");
/// assert_eq!(path, "pages/index");
/// ```
pub fn parse_plugin_url(url: &str) -> Option<(String, String)> {
    if !url.starts_with(PLUGIN_URL_SCHEME) {
        return None;
    }

    let rest = &url[PLUGIN_URL_SCHEME.len()..];
    let (plugin_name, page_path) = if let Some(idx) = rest.find('/') {
        (rest[..idx].to_string(), rest[idx + 1..].to_string())
    } else {
        (rest.to_string(), String::new())
    };

    if plugin_name.is_empty() {
        return None;
    }

    // Clean up the path (trim leading/trailing whitespace)
    let page_path = page_path
        .trim_start_matches('/')
        .trim_end_matches(&[' ', '\t'][..])
        .to_string();

    Some((plugin_name, page_path))
}

/// Parse an internal plugin page path: `@plugin/<name>/<path>`.
///
/// Returns `(plugin_name, page_path)` where `page_path` may be empty.
pub fn parse_plugin_page_path(path: &str) -> Option<(String, String)> {
    if !path.starts_with(PLUGIN_PAGE_PATH_PREFIX) {
        return None;
    }

    let rest = &path[PLUGIN_PAGE_PATH_PREFIX.len()..];
    let (plugin_name, page_path) = if let Some(idx) = rest.find('/') {
        (rest[..idx].to_string(), rest[idx + 1..].to_string())
    } else {
        (rest.to_string(), String::new())
    };

    if plugin_name.is_empty() {
        return None;
    }

    Some((plugin_name, page_path.trim_start_matches('/').to_string()))
}

/// Build an internal plugin page path: `@plugin/<name>` or `@plugin/<name>/<path>`.
pub fn build_plugin_page_path(plugin_name: &str, page_path: &str) -> String {
    let name = plugin_name.trim_matches('/');
    let path = page_path.trim_matches('/');
    if path.is_empty() {
        format!("{}{}", PLUGIN_PAGE_PATH_PREFIX, name)
    } else {
        format!("{}{}/{}", PLUGIN_PAGE_PATH_PREFIX, name, path)
    }
}

/// Resolve a page alias to the actual internal path using plugin's pages mapping.
///
/// If the page_path matches a key in the plugin's pages config, returns the mapped value.
/// Otherwise returns the original page_path unchanged.
pub fn resolve_plugin_page_path(config: &LxPlugin, page_path: &str) -> String {
    // Check if page_path is an alias in the pages mapping
    if let Some(internal_path) = config.pages.get(page_path) {
        internal_path.clone()
    } else {
        page_path.to_string()
    }
}

/// Get the logic.js path for a plugin if it exists.
pub fn get_plugin_logic_js(
    runtime: &Arc<Platform>,
    plugin_name: &str,
    config: &LxPlugin,
) -> Option<PathBuf> {
    let plugin_dir = get_plugin_dir(runtime, plugin_name, &config.version);
    let entry = plugin_entry_js(config);

    // Plugin packages are extracted directly to the plugin directory
    let logic_path = plugin_dir.join(&entry);

    if logic_path.exists() {
        Some(logic_path)
    } else {
        None
    }
}

fn plugin_entry_js(config: &LxPlugin) -> String {
    let entry = config.main.trim();
    if entry.is_empty() || !is_safe_plugin_entry(entry) {
        "logic.js".to_string()
    } else {
        entry.to_string()
    }
}

fn is_safe_plugin_entry(entry: &str) -> bool {
    if entry.contains('\\') {
        return false;
    }
    std::path::Path::new(entry)
        .components()
        .all(|c| matches!(c, std::path::Component::Normal(_)))
}

/// Get the plugins directory path.
pub fn get_plugins_dir(runtime: &Arc<Platform>) -> PathBuf {
    runtime.app_data_dir().join(LINGXIA_DIR).join(PLUGINS_DIR)
}

/// Get the installation directory for a specific plugin and version.
pub fn get_plugin_dir(runtime: &Arc<Platform>, plugin_name: &str, version: &str) -> PathBuf {
    get_plugins_dir(runtime).join(plugin_name).join(version)
}

/// Load plugin manifest (lxplugin.json) from the plugin directory.
///
/// Returns the pages mapping if the manifest exists and can be parsed.
pub fn load_plugin_manifest_pages(
    runtime: &Arc<Platform>,
    plugin_name: &str,
    config: &LxPlugin,
) -> Option<std::collections::BTreeMap<String, String>> {
    let plugin_dir = get_plugin_dir(runtime, plugin_name, &config.version);
    let manifest_path = plugin_dir.join("lxplugin.json");
    if !manifest_path.exists() {
        return None;
    }

    match std::fs::read_to_string(&manifest_path) {
        Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(json) => {
                if let Some(pages) = json.get("pages").and_then(|p| p.as_object()) {
                    let mut result = std::collections::BTreeMap::new();
                    for (k, v) in pages {
                        if let Some(path) = v.as_str() {
                            result.insert(k.clone(), path.to_string());
                        }
                    }
                    return Some(result);
                }
            }
            Err(e) => {
                warn!(
                    "Failed to parse plugin manifest {}: {}",
                    manifest_path.display(),
                    e
                );
            }
        },
        Err(e) => {
            warn!(
                "Failed to read plugin manifest {}: {}",
                manifest_path.display(),
                e
            );
        }
    }

    None
}

pub fn resolve_plugin_page(
    runtime: &Arc<Platform>,
    plugins: &BTreeMap<String, LxPlugin>,
    plugin_name: &str,
    page_path: &str,
) -> Result<String, LxAppError> {
    let plugin_cfg = plugins
        .get(plugin_name)
        .ok_or_else(|| LxAppError::PluginNotConfigured(plugin_name.to_string()))?;

    if !plugin_cfg.pages.is_empty() {
        return Ok(resolve_plugin_page_path(plugin_cfg, page_path));
    }

    Ok(load_plugin_manifest_pages(runtime, plugin_name, plugin_cfg)
        .and_then(|pages| pages.get(page_path).cloned())
        .unwrap_or_else(|| page_path.to_string()))
}

pub fn resolve_plugin_resource_path(
    runtime: &Arc<Platform>,
    plugins: &BTreeMap<String, LxPlugin>,
    plugin_name: &str,
    relative_path: &str,
) -> Result<PathBuf, LxAppError> {
    let plugin_cfg = plugins
        .get(plugin_name)
        .ok_or_else(|| LxAppError::PluginNotConfigured(plugin_name.to_string()))?;
    let plugin_dir = get_plugin_dir(runtime, plugin_name, &plugin_cfg.version);

    if relative_path.is_empty() {
        if plugin_dir.exists() {
            return Ok(plugin_dir);
        }
        return Err(LxAppError::ResourceNotFound(format!(
            "Plugin directory not found: {}",
            plugin_name
        )));
    }

    let full_path = plugin_dir.join(relative_path);
    if let Ok(canonical) = full_path.canonicalize() {
        let plugin_dir_canonical = plugin_dir
            .canonicalize()
            .unwrap_or_else(|_| plugin_dir.clone());
        if canonical.starts_with(&plugin_dir_canonical) {
            return Ok(canonical);
        }
    }
    if full_path.exists() {
        return Ok(full_path);
    }

    Err(LxAppError::ResourceNotFound(format!(
        "Plugin resource not found: @plugin/{}/{}",
        plugin_name, relative_path
    )))
}

pub fn resolve_plugin_resource_path_from_internal_path(
    runtime: &Arc<Platform>,
    plugins: &BTreeMap<String, LxPlugin>,
    path: &str,
) -> Result<Option<PathBuf>, LxAppError> {
    let Some((plugin_name, rel_path)) = parse_plugin_page_path(path) else {
        return Ok(None);
    };
    resolve_plugin_resource_path(runtime, plugins, &plugin_name, &rel_path).map(Some)
}

/// Download and install a plugin.
///
/// Uses the registered UpdateProvider to check for updates and download the plugin archive.
/// The plugin's `provider` field is used as the appid for the update check.
///
/// This function tracks download state to prevent duplicate concurrent downloads.
/// If a download is already in progress, it will wait for completion.
pub async fn download_and_install(
    runtime: Arc<Platform>,
    plugin_name: &str,
    config: &LxPlugin,
) -> Result<PathBuf, LxAppError> {
    let version = &config.version;
    let key = plugin_key(plugin_name, version);
    let install_dir = get_plugin_dir(&runtime, plugin_name, version);

    // If already installed, skip
    if install_dir.exists() {
        return Ok(install_dir);
    }

    // Try to start the download; if someone else started it, wait for completion
    let maybe_rx = get_tracker().try_start_download(&key);
    let is_initiator = maybe_rx.is_some();
    let mut rx = maybe_rx.or_else(|| wait_for_download(plugin_name, version));

    if !is_initiator {
        if let Some(mut rx) = rx.take() {
            while rx.changed().await.is_ok() {
                match &*rx.borrow() {
                    PluginDownloadState::Completed => {
                        return Ok(get_plugin_dir(&runtime, plugin_name, version));
                    }
                    PluginDownloadState::Failed(err) => {
                        return Err(LxAppError::PluginDownloadFailed(err.clone()));
                    }
                    PluginDownloadState::Downloading => continue,
                }
            }
            return Err(LxAppError::PluginDownloadFailed(
                "Plugin download tracking closed unexpectedly".to_string(),
            ));
        } else {
            return Err(LxAppError::PluginDownloadFailed(
                "Plugin download already in progress".to_string(),
            ));
        }
    }

    // Perform the actual download
    let result = download_and_install_internal(runtime.clone(), plugin_name, version, config).await;

    // Update tracker based on result
    match &result {
        Ok(_) => get_tracker().mark_completed(&key),
        Err(e) => get_tracker().mark_failed(&key, e.to_string()),
    }

    result
}

/// Internal download and install logic (without tracking)
async fn download_and_install_internal(
    runtime: Arc<Platform>,
    plugin_name: &str,
    version: &str,
    config: &LxPlugin,
) -> Result<PathBuf, LxAppError> {
    let plugin_id = &config.lx_plugin_id;
    let required_version = &config.version;

    // 1. Check for update using the lx_plugin_id
    let provider = crate::get_provider();
    let check_result: UpdateCheckResult = provider
        .check_update(plugin_id, Some(required_version))
        .await
        .map_err(|e| LxAppError::IoError(format!("Plugin update check failed: {}", e)))?;

    let package = check_result.package.ok_or_else(|| {
        LxAppError::IoError(format!(
            "Plugin {} (lxPluginId: {}) not found on server",
            plugin_name, plugin_id
        ))
    })?;

    // 2. Download the archive
    let plugins_dir = get_plugins_dir(&runtime);
    let download_dir = plugins_dir.join("download");
    fs::create_dir_all(&download_dir)?;

    let archive_path = download_dir.join(format!("{}-{}.tar.zst", plugin_name, version));
    if archive_path.exists() {
        let _ = fs::remove_file(&archive_path);
    }

    let receiver =
        service_executor::request_download(package.url.clone(), archive_path.clone(), None, None)
            .map_err(|e| LxAppError::IoError(format!("Failed to start plugin download: {}", e)))?;

    match receiver.await {
        Ok(Ok(())) => {
            // Verify checksum if provided
            if !package.checksum_sha256.is_empty() {
                archive::verify_sha256(&archive_path, &package.checksum_sha256)?;
            }
        }
        Ok(Err(err)) => {
            let _ = fs::remove_file(&archive_path);
            return Err(LxAppError::IoError(format!(
                "Plugin download failed: {}",
                err
            )));
        }
        Err(_) => {
            let _ = fs::remove_file(&archive_path);
            return Err(LxAppError::IoError(
                "Plugin download task cancelled".to_string(),
            ));
        }
    }

    // 3. Install the archive
    let install_path = install_plugin_archive(&runtime, plugin_name, version, &archive_path)?;

    // 5. Clean up archive
    let _ = fs::remove_file(&archive_path);

    Ok(install_path)
}

/// Install a plugin archive to the plugins directory.
fn install_plugin_archive(
    runtime: &Arc<Platform>,
    plugin_name: &str,
    version: &str,
    archive_path: &Path,
) -> Result<PathBuf, LxAppError> {
    let destination = get_plugin_dir(runtime, plugin_name, version);
    archive::extract_tar_zst(archive_path, &destination)?;
    Ok(destination)
}
