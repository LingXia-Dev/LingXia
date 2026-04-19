use std::sync::OnceLock;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct LxAppRuntimeConfig {
    pub home_appid: String,
    pub home_app_version: String,
    pub temp_max_size_bytes: u64,
    pub cache_max_age: Duration,
    pub cache_max_size_bytes: u64,
}

static LXAPP_RUNTIME_CONFIG: OnceLock<LxAppRuntimeConfig> = OnceLock::new();

pub(crate) fn set_runtime_config(config: LxAppRuntimeConfig) {
    let _ = LXAPP_RUNTIME_CONFIG.set(config);
}

pub(crate) fn runtime_config() -> Option<&'static LxAppRuntimeConfig> {
    LXAPP_RUNTIME_CONFIG.get()
}

pub(crate) fn home_appid() -> Option<&'static str> {
    runtime_config().map(|config| config.home_appid.as_str())
}

pub(crate) fn cache_max_age() -> Duration {
    runtime_config()
        .map(|config| config.cache_max_age)
        .unwrap_or_else(|| Duration::from_secs(7 * 86400))
}

pub(crate) fn temp_max_size_bytes() -> u64 {
    runtime_config()
        .map(|config| config.temp_max_size_bytes)
        .unwrap_or(1024 * 1024 * 1024)
}

pub(crate) fn cache_max_size_bytes() -> u64 {
    runtime_config()
        .map(|config| config.cache_max_size_bytes)
        .unwrap_or(1024 * 1024 * 1024)
}
