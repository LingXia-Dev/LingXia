use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const DEFAULT_PROXY_SOCKS_PORT: u16 = 1080;

#[derive(Debug, Error)]
pub enum ProxySettingsError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProxyMode {
    #[default]
    Direct,
    Global,
    GfwList,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProxyRuleAction {
    #[default]
    Proxy,
    Direct,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AutoSwitchRule {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub pattern: String,
    #[serde(default)]
    pub action: ProxyRuleAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxySettings {
    #[serde(default)]
    pub mode: ProxyMode,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub socks_host: String,
    #[serde(
        default = "default_proxy_socks_port",
        skip_serializing_if = "is_default_proxy_socks_port"
    )]
    pub socks_port: u16,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub username: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub password: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub gfwlist_source_url: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auto_switch_rules: Vec<AutoSwitchRule>,
}

impl Default for ProxySettings {
    fn default() -> Self {
        Self {
            mode: ProxyMode::Direct,
            enabled: false,
            socks_host: String::new(),
            socks_port: DEFAULT_PROXY_SOCKS_PORT,
            username: String::new(),
            password: String::new(),
            gfwlist_source_url: String::new(),
            auto_switch_rules: Vec::new(),
        }
    }
}

impl ProxySettings {
    pub fn normalized(mut self) -> Self {
        if matches!(self.mode, ProxyMode::Direct) && self.enabled {
            self.mode = ProxyMode::Global;
        }
        self.enabled = !matches!(self.mode, ProxyMode::Direct);
        self.socks_host = self.socks_host.trim().to_string();
        self.username = self.username.trim().to_string();
        self.gfwlist_source_url = self.gfwlist_source_url.trim().to_string();
        self.auto_switch_rules = self
            .auto_switch_rules
            .into_iter()
            .map(|rule| AutoSwitchRule {
                pattern: rule.pattern.trim().to_string(),
                action: rule.action,
            })
            .filter(|rule| !rule.pattern.is_empty())
            .collect();
        self
    }
}

const fn default_proxy_socks_port() -> u16 {
    DEFAULT_PROXY_SOCKS_PORT
}

fn is_default_proxy_socks_port(value: &u16) -> bool {
    *value == DEFAULT_PROXY_SOCKS_PORT
}

pub fn proxy_settings_path(app_data_dir: &Path) -> PathBuf {
    lingxia_app_context::app_state_file(app_data_dir, "proxy-settings.json")
}

pub fn load_proxy_settings(app_data_dir: &Path) -> Result<ProxySettings, ProxySettingsError> {
    let path = proxy_settings_path(app_data_dir);
    match std::fs::read(&path) {
        Ok(bytes) => Ok(serde_json::from_slice::<ProxySettings>(&bytes)?.normalized()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(ProxySettings::default()),
        Err(err) => Err(ProxySettingsError::Io(err)),
    }
}

pub fn save_proxy_settings(
    app_data_dir: &Path,
    settings: &ProxySettings,
) -> Result<(), ProxySettingsError> {
    let path = proxy_settings_path(app_data_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(&settings.clone().normalized())?;
    std::fs::write(path, bytes)?;
    Ok(())
}
