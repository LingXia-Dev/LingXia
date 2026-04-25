use serde::{Deserialize, Serialize};
use std::sync::{OnceLock, RwLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum UpdateUiMode {
    /// LingXia owns the host app update prompt, download progress UI, and install handoff.
    #[default]
    Builtin,
    /// LingXia never shows built-in host app update UI or installs automatically.
    /// The native host owns check/download/install UX explicitly.
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateConfig {
    /// Whether startup should automatically check host app updates.
    ///
    /// - Builtin mode: auto check can show LingXia prompt/progress and request install.
    /// - Custom mode: auto check only emits availability events; it never downloads or installs.
    /// - Set this to false in custom mode when the native host wants full manual control.
    #[serde(default = "default_enabled")]
    pub auto_check_app: bool,
    #[serde(default)]
    pub ui_mode: UpdateUiMode,
    #[serde(default = "default_enabled")]
    pub force_update_gate: bool,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            auto_check_app: true,
            ui_mode: UpdateUiMode::Builtin,
            force_update_gate: true,
        }
    }
}

fn default_enabled() -> bool {
    true
}

fn config_store() -> &'static RwLock<UpdateConfig> {
    static UPDATE_CONFIG: OnceLock<RwLock<UpdateConfig>> = OnceLock::new();
    UPDATE_CONFIG.get_or_init(|| RwLock::new(UpdateConfig::default()))
}

pub fn update_config() -> UpdateConfig {
    config_store()
        .read()
        .unwrap_or_else(|err| err.into_inner())
        .clone()
}

pub fn configure_update(config: UpdateConfig) {
    *config_store()
        .write()
        .unwrap_or_else(|err| err.into_inner()) = config;
}
