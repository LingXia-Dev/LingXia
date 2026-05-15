use serde::{Deserialize, Serialize};
use std::sync::{OnceLock, RwLock};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateConfig {
    #[serde(default = "default_enabled")]
    pub force_update_gate: bool,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
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
