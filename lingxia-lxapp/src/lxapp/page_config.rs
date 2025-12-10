use crate::lxapp::LxApp;
use crate::lxapp::navbar::{NavigationBarConfig, NavigationBarState};
use crate::warn;
use serde::{Deserialize, Serialize};

/// Page configuration loaded from page.json (immutable)
/// This is the single source of truth for page configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PageConfig {
    /// Navigation bar configuration
    #[serde(flatten)]
    pub navigation_bar: NavigationBarConfig,

    /// Enable pull-to-refresh
    #[serde(default)]
    pub enable_pull_down_refresh: bool,
}

impl PageConfig {
    /// Create PageConfig from JSON config file path
    /// This is the single entry point for loading page configuration.
    pub fn from_json(lxapp: &LxApp, path: &str) -> Self {
        let json_path = path_to_json_path(path);
        match lxapp.read_json(&json_path) {
            Ok(json_value) => match serde_json::from_value::<PageConfig>(json_value) {
                Ok(config) => config,
                Err(e) => {
                    warn!("Failed to parse page config for {}: {}", path, e);
                    Self::default()
                }
            },
            Err(e) => {
                warn!(
                    "Page config read failed for {} ({}); falling back to default",
                    path, e
                );
                // No page config file or read error - use default (navbar enabled, no pull-to-refresh)
                Self::default()
            }
        }
    }

    /// Create NavigationBarState from this config
    /// This converts immutable config to mutable runtime state.
    pub fn create_navbar_state(&self) -> NavigationBarState {
        NavigationBarState::from_config(&self.navigation_bar)
    }

    /// Check if pull-to-refresh is enabled
    pub fn is_pull_down_refresh_enabled(&self) -> bool {
        self.enable_pull_down_refresh
    }
}

/// Convert a page path to its corresponding JSON config path
fn path_to_json_path(path: &str) -> String {
    if path.is_empty() || path == "/" {
        return "pages/index/index.json".to_string();
    }

    let mut trimmed = path.trim_start_matches('/').to_string();
    if trimmed.is_empty() {
        return "pages/index/index.json".to_string();
    }

    // Remove any extension on the last path segment
    if let Some(dot_pos) = trimmed.rfind('.') {
        let last_slash = trimmed.rfind('/');
        if last_slash.map_or(true, |slash| dot_pos > slash) {
            trimmed.truncate(dot_pos);
        }
    }

    format!("{}.json", trimmed)
}
