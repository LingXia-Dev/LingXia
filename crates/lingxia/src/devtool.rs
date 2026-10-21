use std::path::PathBuf;
use std::sync::OnceLock;

use lingxia_platform::traits::app_runtime::AppRuntime;

const LXAPP_PATH_ENV: &str = "LINGXIA_LXAPP_PATH";
const RUNNER_WEB_URL_ENV: &str = "LINGXIA_RUNNER_WEB_URL";

mod net;
mod sync;

pub use net::connect_dev_server;

#[derive(Debug, serde::Deserialize)]
#[allow(non_snake_case)]
struct LxAppManifest {
    appId: String,
    version: String,
}

/// Identity of the LxApp currently exposed through host devtool helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LxAppDevIdentity {
    /// LxApp id from `lxapp.json`.
    pub appid: String,
    /// LxApp version from `lxapp.json`.
    pub version: String,
}

/// Explicit host devtool configuration for loading an unpacked LxApp bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LxAppDevConfig {
    /// Root directory that contains the runnable LxApp bundle.
    pub root: PathBuf,
    /// LxApp identity advertised to the host runtime.
    pub identity: LxAppDevIdentity,
}

static LXAPP_DEV_CONFIG: OnceLock<LxAppDevConfig> = OnceLock::new();

/// Installs an explicit devtool bundle override for the current process.
///
/// Returns `true` when the configuration is installed or when the same
/// configuration had already been installed. Returns `false` when a conflicting
/// config was already present.
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

// Simulated-device control lives in `lxapp::device` so both the devtool
// handlers and the `lx.automation()` host tier share one registry. Re-exported
// here to keep the `lingxia::dev::device_*` / `register_device_controller`
// surface (and the runner's call site) unchanged.
pub use lxapp::device::{
    Appearance, DeviceController, DeviceEntry, DeviceState, device_get, device_list, device_set,
    register_device_controller,
};

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

/// Installs devtool config from the `LINGXIA_LXAPP_PATH` environment variable.
///
/// The path may point either at a built `dist/` directory or at a project root
/// that contains one.
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
    build_default_host_app_config(
        runtime,
        dev_config.identity.appid.clone(),
        dev_config.identity.version.clone(),
    )
}

fn build_default_host_app_config(
    runtime: &lingxia_platform::Platform,
    home_app_id: String,
    home_app_version: String,
) -> lingxia_app_context::AppConfig {
    let product_name = runtime
        .get_app_identifier()
        .ok()
        .filter(|value: &String| !value.trim().is_empty())
        .unwrap_or_else(|| "LingXia Host".to_string());

    lingxia_app_context::AppConfig {
        product_name,
        product_names: Default::default(),
        product_version: env!("CARGO_PKG_VERSION").to_string(),
        lingxia_id: None,
        lingxia_server: None,
        lingxia_servers: Default::default(),
        env: lingxia_app_context::AppEnv::Dev,
        home_app_id,
        home_app_version,
        cache_max_size_mb: 1024,
        storage: None,
        splash: None,
        dev_ws_url: None,
        dev_bundle_base_url: None,
        app_links: None,
        theme: None,
        settings_destination: None,
        browser: Default::default(),
        capabilities: None,
        panels: None,
        update_trusted_public_keys: Vec::new(),
        update_channel: None,
        update_channels: Default::default(),
        store_listing_ids: Default::default(),
    }
}

fn build_web_runner_app_config(
    runtime: &lingxia_platform::Platform,
) -> lingxia_app_context::AppConfig {
    build_default_host_app_config(runtime, String::new(), String::new())
}

pub(crate) fn prepare_host_app_config(
    runtime: &std::sync::Arc<lingxia_platform::Platform>,
    load_bundled: impl FnOnce(
        &std::sync::Arc<lingxia_platform::Platform>,
    ) -> Option<lingxia_app_context::AppConfig>,
) -> Option<lingxia_app_context::AppConfig> {
    let _ = install_lxapp_dev_config_from_env();

    let Some(dev_config) = lxapp_dev_config() else {
        if std::env::var(RUNNER_WEB_URL_ENV)
            .ok()
            .is_some_and(|value| !value.trim().is_empty())
        {
            let mut config = match runtime.read_asset("app.json") {
                Ok(_) => load_bundled(runtime)?,
                Err(lingxia_platform::error::PlatformError::AssetNotFound(path))
                    if path == "app.json" =>
                {
                    build_web_runner_app_config(runtime.as_ref())
                }
                Err(error) => {
                    log::error!("Failed to read app.json: {}", error);
                    return None;
                }
            };
            config.home_app_id.clear();
            config.home_app_version.clear();
            return Some(config);
        }
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
    app_config.home_app_id = dev_config.identity.appid.clone();
    app_config.home_app_version = dev_config.identity.version.clone();
    Some(app_config)
}

pub(crate) fn prepare_bundle_sources(runtime: &std::sync::Arc<lingxia_platform::Platform>) {
    lxapp::set_dev_bundle_preparer(prepare_dev_bundle_for_open);
    let home_id = lingxia_app_context::app_config()
        .map(|config| config.home_app_id.trim().to_string())
        .unwrap_or_default();
    if !home_id.is_empty() {
        // Home's own instance asks next, so record the answer: a `listen`
        // session publishes only its lxapp, and an unreachable server must
        // not cost home a second timeout.
        match sync::sync_dev_bundle(runtime.clone(), &home_id) {
            Ok(sync::DevBundleSync::NotPublished) => {
                lock(&UNPUBLISHED).insert(home_id);
            }
            Ok(_) => {}
            Err(err) => {
                *lock(&UNREACHABLE_UNTIL) = Some(std::time::Instant::now() + UNREACHABLE_BACKOFF);
                log::warn!("Failed to sync dev home lxapp bundle: {}", err);
            }
        }
    }
    register_bundle_source_override();
}

/// App ids the dev server had no manifest for, since the dev session last
/// connected. A new connection may serve more, so it clears this.
static UNPUBLISHED: std::sync::Mutex<std::collections::BTreeSet<String>> =
    std::sync::Mutex::new(std::collections::BTreeSet::new());
/// After a failed fetch, opens skip the dev server until then, so an
/// unreachable host costs one timeout and not one per open.
static UNREACHABLE_UNTIL: std::sync::Mutex<Option<std::time::Instant>> =
    std::sync::Mutex::new(None);
const UNREACHABLE_BACKOFF: std::time::Duration = std::time::Duration::from_secs(30);

fn prepare_dev_bundle_for_open(app_id: &str) {
    if lxapp::is_dev_bundle_appid(app_id) || lock(&UNPUBLISHED).contains(app_id) {
        return;
    }
    if lock(&UNREACHABLE_UNTIL).is_some_and(|until| std::time::Instant::now() < until) {
        return;
    }
    let Ok(runtime) = crate::runtime::platform() else {
        return;
    };
    match sync::sync_dev_bundle(runtime, app_id) {
        Ok(sync::DevBundleSync::NotPublished) => {
            lock(&UNPUBLISHED).insert(app_id.to_string());
        }
        Ok(_) => {}
        Err(err) => {
            *lock(&UNREACHABLE_UNTIL) = Some(std::time::Instant::now() + UNREACHABLE_BACKOFF);
            log::warn!("Failed to sync dev lxapp bundle for {app_id}: {err}");
        }
    }
}

/// The dev session (re)connected: what it publishes, and whether it is
/// reachable, may have changed.
pub fn dev_session_connected() {
    lock(&UNPUBLISHED).clear();
    *lock(&UNREACHABLE_UNTIL) = None;
}

fn lock<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Sync `appid`'s bundle from the dev server, and point a live instance at
/// it; a dev restart calls this before it reloads.
///
/// Home stays an error when the server has no manifest. Any other app id with
/// no manifest keeps the source it already opened from.
pub fn sync_dev_bundle(appid: &str) -> Result<(), String> {
    let runtime = crate::runtime::platform().map_err(|err| err.to_string())?;
    let home_id = lingxia_app_context::app_config()
        .map(|config| config.home_app_id.trim().to_string())
        .unwrap_or_default();
    match sync::sync_dev_bundle(runtime, appid)? {
        sync::DevBundleSync::Ready { root } => {
            lock(&UNPUBLISHED).remove(appid);
            if let Some(app) = lxapp::try_get(appid) {
                app.adopt_dev_bundle(root);
            }
            Ok(())
        }
        sync::DevBundleSync::NotPublished if home_id == appid => {
            Err(format!("dev server has no bundle manifest for {appid}"))
        }
        sync::DevBundleSync::NotPublished | sync::DevBundleSync::Idle => Ok(()),
    }
}

fn register_bundle_source_override() {
    let Some(dev_config) = lxapp_dev_config() else {
        return;
    };
    lxapp::register_dev_bundle_source(dev_config.identity.appid.clone(), dev_config.root.clone());
}

/// Reports whether page element input actions are supported on this platform build.
pub fn lxapp_dev_page_input_supported() -> bool {
    cfg!(any(
        target_os = "android",
        target_os = "ios",
        all(target_os = "linux", target_env = "ohos"),
        all(
            feature = "webview-input",
            any(target_os = "macos", target_os = "windows")
        )
    ))
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
