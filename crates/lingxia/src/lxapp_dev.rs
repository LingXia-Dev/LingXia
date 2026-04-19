use std::path::PathBuf;
use std::sync::OnceLock;

use lingxia_platform::traits::app_runtime::AppRuntime;

const LXAPP_PATH_ENV: &str = "LINGXIA_LXAPP_PATH";

#[derive(Debug, serde::Deserialize)]
#[allow(non_snake_case)]
struct LxAppManifest {
    appId: String,
    version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LxAppDevIdentity {
    pub appid: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LxAppDevConfig {
    pub root: PathBuf,
    pub identity: LxAppDevIdentity,
}

static LXAPP_DEV_CONFIG: OnceLock<LxAppDevConfig> = OnceLock::new();

pub fn install_lxapp_dev_config(config: LxAppDevConfig) -> bool {
    if let Some(existing) = LXAPP_DEV_CONFIG.get() {
        if existing == &config {
            return true;
        }
        log::warn!(
            "Lxapp dev config already set for appid={}, refusing conflicting appid={}",
            existing.identity.appid,
            config.identity.appid
        );
        return false;
    }

    match LXAPP_DEV_CONFIG.set(config.clone()) {
        Ok(()) => {
            log::info!(
                "Installed explicit lxapp dev config: appid={}, version={}, root={}",
                config.identity.appid,
                config.identity.version,
                config.root.display()
            );
            true
        }
        Err(_) => {
            log::warn!("Lxapp dev config already set");
            false
        }
    }
}

pub(crate) fn lxapp_dev_config() -> Option<&'static LxAppDevConfig> {
    LXAPP_DEV_CONFIG.get()
}

fn resolve_runnable_lxapp_path(path: &std::path::Path) -> PathBuf {
    let dist_path = path.join("dist");
    if dist_path.join("lxapp.json").exists() {
        return dist_path;
    }
    path.to_path_buf()
}

fn read_lxapp_manifest(path: &std::path::Path) -> Result<LxAppManifest, String> {
    let manifest_path = path.join("lxapp.json");
    let content = std::fs::read_to_string(&manifest_path)
        .map_err(|e| format!("failed to read {}: {}", manifest_path.display(), e))?;
    let manifest: LxAppManifest = serde_json::from_str(&content)
        .map_err(|e| format!("invalid {}: {}", manifest_path.display(), e))?;
    let appid = manifest.appId.trim();
    if appid.is_empty() {
        return Err(format!(
            r#""appId" must not be empty in {}"#,
            manifest_path.display()
        ));
    }
    let version = manifest.version.trim();
    if version.is_empty() {
        return Err(format!(
            r#""version" must not be empty in {}"#,
            manifest_path.display()
        ));
    }
    Ok(LxAppManifest {
        appId: appid.to_string(),
        version: version.to_string(),
    })
}

pub fn install_lxapp_dev_config_from_env() -> bool {
    let Ok(raw_path) = std::env::var(LXAPP_PATH_ENV) else {
        return false;
    };

    let path = raw_path.trim();
    if path.is_empty() {
        log::warn!("{LXAPP_PATH_ENV} is set but empty; ignoring");
        return false;
    }

    let root = resolve_runnable_lxapp_path(&PathBuf::from(path));
    if !root.exists() {
        log::warn!("{LXAPP_PATH_ENV} path does not exist: {}", root.display());
        return false;
    }
    if !root.join("logic.js").exists() {
        log::warn!(
            "{LXAPP_PATH_ENV} logic.js not found in {} (continuing; build output may be incomplete)",
            root.display()
        );
    }

    let manifest = match read_lxapp_manifest(&root) {
        Ok(manifest) => manifest,
        Err(err) => {
            log::warn!(
                "Failed to initialize lxapp dev config from {}={}: {}",
                LXAPP_PATH_ENV,
                path,
                err
            );
            return false;
        }
    };

    install_lxapp_dev_config(LxAppDevConfig {
        root,
        identity: LxAppDevIdentity {
            appid: manifest.appId,
            version: manifest.version,
        },
    })
}

fn build_host_app_config(
    runtime: &lingxia_platform::Platform,
    dev_config: &LxAppDevConfig,
) -> lingxia_app_context::AppConfig {
    let product_name = runtime
        .get_app_identifier()
        .ok()
        .filter(|value: &String| !value.trim().is_empty())
        .unwrap_or_else(|| "LingXia Host".to_string());

    lingxia_app_context::AppConfig {
        product_name,
        product_version: env!("CARGO_PKG_VERSION").to_string(),
        lingxia_id: None,
        api_server: None,
        home_lxapp_appid: dev_config.identity.appid.clone(),
        home_lxapp_version: dev_config.identity.version.clone(),
        cache_max_age_days: 7,
        cache_max_size_mb: 1024,
        storage: None,
        dev_ws_url: None,
        app_links: None,
        panels: None,
    }
}

pub(crate) fn load_host_app_config(
    runtime: &std::sync::Arc<lingxia_platform::Platform>,
    load_bundled: impl FnOnce(
        &std::sync::Arc<lingxia_platform::Platform>,
    ) -> Option<lingxia_app_context::AppConfig>,
) -> Option<lingxia_app_context::AppConfig> {
    let Some(dev_config) = lxapp_dev_config() else {
        return load_bundled(runtime);
    };

    let mut app_config = match runtime.read_asset("app.json") {
        Ok(_) => load_bundled(runtime)?,
        Err(lingxia_platform::error::PlatformError::AssetNotFound(path)) if path == "app.json" => {
            log::info!(
                "Bootstrapping host in explicit lxapp dev mode using host defaults for {}",
                dev_config.identity.appid
            );
            build_host_app_config(runtime.as_ref(), dev_config)
        }
        Err(e) => {
            log::error!("Failed to read app.json: {}", e);
            return None;
        }
    };
    app_config.home_lxapp_appid = dev_config.identity.appid.clone();
    app_config.home_lxapp_version = dev_config.identity.version.clone();
    Some(app_config)
}

pub(crate) fn register_bundle_source_override() {
    let Some(dev_config) = lxapp_dev_config() else {
        return;
    };
    lxapp::register_dev_bundle_source(dev_config.identity.appid.clone(), dev_config.root.clone());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn read_lxapp_manifest_valid() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("lxapp.json"),
            r#"{"appId":"demo","version":"1.0.0"}"#,
        )
        .unwrap();
        let manifest = read_lxapp_manifest(tmp.path()).unwrap();
        assert_eq!(manifest.appId, "demo");
        assert_eq!(manifest.version, "1.0.0");
    }

    #[test]
    fn read_lxapp_manifest_rejects_empty_appid() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("lxapp.json"),
            r#"{"appId":"","version":"1.0.0"}"#,
        )
        .unwrap();
        let err = read_lxapp_manifest(tmp.path()).unwrap_err();
        assert!(err.contains("appId"), "unexpected error: {err}");
    }

    #[test]
    fn read_lxapp_manifest_rejects_empty_version() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("lxapp.json"),
            r#"{"appId":"demo","version":""}"#,
        )
        .unwrap();
        let err = read_lxapp_manifest(tmp.path()).unwrap_err();
        assert!(err.contains("version"), "unexpected error: {err}");
    }

    #[test]
    fn read_lxapp_manifest_rejects_malformed_json() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("lxapp.json"), "not json").unwrap();
        assert!(read_lxapp_manifest(tmp.path()).is_err());
    }

    #[test]
    fn read_lxapp_manifest_rejects_missing_file() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(read_lxapp_manifest(tmp.path()).is_err());
    }

    #[test]
    fn resolve_runnable_lxapp_path_prefers_dist() {
        let tmp = tempfile::tempdir().unwrap();
        let dist = tmp.path().join("dist");
        fs::create_dir_all(&dist).unwrap();
        fs::write(dist.join("lxapp.json"), "{}").unwrap();
        assert_eq!(resolve_runnable_lxapp_path(tmp.path()), dist);
    }

    #[test]
    fn resolve_runnable_lxapp_path_falls_back_when_dist_missing_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let dist = tmp.path().join("dist");
        fs::create_dir_all(&dist).unwrap();
        assert_eq!(
            resolve_runnable_lxapp_path(tmp.path()),
            tmp.path().to_path_buf()
        );
    }
}
