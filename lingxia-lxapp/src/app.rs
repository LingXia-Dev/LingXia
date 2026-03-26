pub mod settings;
pub(crate) mod state;

use std::io::Read;
use std::sync::Arc;
use std::sync::OnceLock;

use crate::config::{AppConfig, default_cache_max_age_days, default_cache_max_size_mb};
use crate::error::LxAppError;
use lingxia_platform::Platform;
use lingxia_platform::traits::app_runtime::AppRuntime;

static APP_CONFIG: OnceLock<AppConfig> = OnceLock::new();

pub fn app_config() -> Option<&'static AppConfig> {
    APP_CONFIG.get()
}

pub fn product_name() -> Option<&'static str> {
    APP_CONFIG.get().map(|c| c.product_name.as_str())
}

pub fn product_version() -> Option<&'static str> {
    APP_CONFIG.get().map(|c| c.product_version.as_str())
}

pub fn lingxia_id() -> Option<&'static str> {
    APP_CONFIG
        .get()
        .and_then(|c| c.lingxia_id.as_deref())
        .filter(|s| !s.is_empty())
}

pub fn cache_max_age_days() -> u64 {
    APP_CONFIG
        .get()
        .map(|c| c.cache_max_age_days)
        .unwrap_or_else(default_cache_max_age_days)
}

pub fn cache_max_age() -> std::time::Duration {
    std::time::Duration::from_secs(cache_max_age_days() * 86400)
}

pub fn cache_max_size_bytes() -> u64 {
    const MIB: u64 = 1024 * 1024;
    APP_CONFIG
        .get()
        .map(|c| c.cache_max_size_mb.saturating_mul(MIB))
        .unwrap_or_else(|| default_cache_max_size_mb().saturating_mul(MIB))
}

/// Read, parse, validate and cache app.json from the assets directory.
pub(crate) fn load_app_config(controller: Arc<Platform>) -> Result<AppConfig, LxAppError> {
    let mut reader = controller.read_asset("app.json")?;
    let mut content = String::new();
    reader
        .read_to_string(&mut content)
        .map_err(|e| LxAppError::IoError(format!("Failed to read app.json: {}", e)))?;

    let config = AppConfig::parse_and_validate(&content)?;
    let _ = APP_CONFIG.set(config.clone());
    Ok(config)
}
