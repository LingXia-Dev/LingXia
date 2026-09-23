//! Mutable service environment, distinct from the immutable build env.
//!
//! `app.json::env` is the build. `lingxiaServers` is the switchable map. A
//! persisted override is resolved before cloud/provider init; changing it
//! does not hot-swap an already-running process.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{AppConfig, AppEnv, LingxiaServers};

const PERSIST_FILE: &str = "service-env.json";

static STATE: OnceLock<Installed> = OnceLock::new();
static FILE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Error)]
pub enum ServiceEnvError {
    #[error("service env is not initialized")]
    NotInitialized,
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Persist(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceEnvSnapshot {
    /// Immutable environment baked into the host package.
    pub build_env: AppEnv,
    /// Environment this process is running; switches never mutate it.
    pub service_env: AppEnv,
    /// Environment selected for the next process launch.
    pub next_launch_env: AppEnv,
    pub available: Vec<AppEnv>,
    pub lingxia_server: Option<String>,
    pub build_lingxia_server: Option<String>,
}

impl ServiceEnvSnapshot {
    /// Whether this build can switch away from its running service environment.
    pub fn can_toggle(&self) -> bool {
        self.build_env == AppEnv::Prod
            && self.available.contains(&self.build_env)
            && self.available.contains(&toggle_target(self.service_env))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceEnvSwitch {
    /// The process is already on `snapshot.service_env`. The persist file may
    /// still have been rewritten so the next launch matches this process.
    Unchanged(ServiceEnvSnapshot),
    RestartRequired(ServiceEnvSnapshot),
}

#[derive(Debug, Clone)]
struct Installed {
    data_dir: PathBuf,
    snapshot: ServiceEnvSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedOverride {
    #[serde(rename = "serviceEnv")]
    service_env: String,
}

/// Resolve the effective service environment and remember it for this process.
pub fn install(
    app_data_dir: &Path,
    config: &AppConfig,
) -> Result<ServiceEnvSnapshot, ServiceEnvError> {
    let persisted = {
        let _guard = file_lock();
        read_override(app_data_dir).ok().flatten()
    };
    let snapshot = resolve(config, persisted);
    let installed = Installed {
        data_dir: app_data_dir.to_path_buf(),
        snapshot,
    };
    install_once(&STATE, installed)
}

fn install_once(
    state: &OnceLock<Installed>,
    installed: Installed,
) -> Result<ServiceEnvSnapshot, ServiceEnvError> {
    if let Err(requested) = state.set(installed) {
        let existing = state.get().expect("initialized service environment");
        if existing.snapshot != requested.snapshot || existing.data_dir != requested.data_dir {
            return Err(ServiceEnvError::Invalid(
                "service env is already initialized".to_string(),
            ));
        }
    }
    Ok(state
        .get()
        .expect("initialized service environment")
        .snapshot
        .clone())
}

/// Boot-time state; reading the current environment never touches the filesystem.
fn running_snapshot() -> ServiceEnvSnapshot {
    STATE
        .get()
        .map(|state| state.snapshot.clone())
        .unwrap_or_else(fallback_snapshot)
}

/// Running state plus the latest environment saved for the next launch.
pub fn snapshot() -> ServiceEnvSnapshot {
    let _guard = file_lock();
    STATE
        .get()
        .map(|state| overlay_pending(&state.data_dir, &state.snapshot))
        .unwrap_or_else(fallback_snapshot)
}

pub fn service_env() -> AppEnv {
    running_snapshot().service_env
}

fn dev_service_mark(build_env: AppEnv, service_env: AppEnv) -> bool {
    build_env == AppEnv::Prod && service_env == AppEnv::Dev
}

/// Prod build currently talking to the dev service.
///
/// Hosts draw a non-interactive status-bar label from this. A dev build
/// never shows it: that package is already the dev server.
pub fn dev_service_banner() -> bool {
    let snap = running_snapshot();
    dev_service_mark(snap.build_env, snap.service_env)
}

/// Effective cloud server for discovery, auth, MQTT, and product HTTP.
pub fn service_lingxia_server() -> Option<String> {
    running_snapshot()
        .lingxia_server
        .filter(|server| !server.is_empty())
}

/// Build-time server. Host self-update keeps the release channel.
pub fn build_lingxia_server() -> Option<String> {
    running_snapshot()
        .build_lingxia_server
        .or_else(|| crate::app_config().and_then(|config| config.lingxia_server.clone()))
        .filter(|server| !server.is_empty())
}

/// Persist a service-env override. In-process connections stay on the boot env.
///
/// Repeating the **running** env keeps that env for the next launch: an already
/// applied override is left in place; a not-yet-applied pending file is
/// rewritten so the next launch matches the process that is still running.
/// Persisting the build env clears the override, same as [`prepare_restore`].
pub fn prepare_switch(target: AppEnv) -> Result<ServiceEnvSwitch, ServiceEnvError> {
    let installed = installed()?;
    persist_target(&installed.data_dir, &installed.snapshot, target)
}

/// Drop the override so the next launch uses the build env.
pub fn prepare_restore() -> Result<ServiceEnvSwitch, ServiceEnvError> {
    let installed = installed()?;
    let _guard = file_lock();
    // Always delete the file, including a corrupt one that resolve ignored.
    clear_override_file(&installed.data_dir)?;
    let pending = overlay_pending(&installed.data_dir, &installed.snapshot);
    if installed.snapshot.service_env == installed.snapshot.build_env {
        return Ok(ServiceEnvSwitch::Unchanged(pending));
    }
    Ok(ServiceEnvSwitch::RestartRequired(pending))
}

/// Flip the running service env to the other configured env.
///
/// A dev build has no other env, so this returns the same error as switching
/// it away from its build env and does not write the file.
pub fn prepare_toggle() -> Result<ServiceEnvSnapshot, ServiceEnvError> {
    let installed = installed()?;
    let target = toggle_target(installed.snapshot.service_env);
    match persist_target(&installed.data_dir, &installed.snapshot, target)? {
        ServiceEnvSwitch::RestartRequired(snapshot) => Ok(snapshot),
        ServiceEnvSwitch::Unchanged(_) => {
            unreachable!("toggle always selects the other environment")
        }
    }
}

pub fn toggle_target(running: AppEnv) -> AppEnv {
    match running {
        AppEnv::Dev => AppEnv::Prod,
        AppEnv::Prod => AppEnv::Dev,
    }
}

/// A prod build can flip when the other env has a configured LingXia server.
pub fn can_toggle_service_env() -> bool {
    let snap = running_snapshot();
    snap.can_toggle()
}

/// In-app App Link hosts for the service env this process booted with.
///
/// `hostsByEnv` wins when the package has it. Older packages only stored the
/// build-env list in `appLinks.hosts`. An omitted per-env key means that env
/// has no hosts. The signed associated-domain list stays `appLinks.hosts`.
pub fn service_app_link_hosts() -> Vec<String> {
    crate::app_config()
        .map(|config| app_link_hosts_for(config, service_env()))
        .unwrap_or_default()
}

pub fn app_link_hosts_for(config: &AppConfig, service_env: AppEnv) -> Vec<String> {
    let Some(links) = config.app_links.as_ref() else {
        return Vec::new();
    };
    if let Some(hosts) = links.hosts_by_env.get(service_env) {
        return nonempty_hosts(hosts);
    }
    if links.hosts_by_env.is_empty() {
        return nonempty_hosts(&links.hosts);
    }
    Vec::new()
}

fn nonempty_hosts(hosts: &[String]) -> Vec<String> {
    hosts
        .iter()
        .map(|host| host.trim().to_string())
        .filter(|host| !host.is_empty())
        .collect()
}

pub fn resolve(config: &AppConfig, persisted: Option<AppEnv>) -> ServiceEnvSnapshot {
    let build_env = config.env;
    let build_lingxia_server = config
        .lingxia_server
        .as_deref()
        .map(str::trim)
        .filter(|server| !server.is_empty())
        .map(str::to_string);
    let available = available_envs(
        &config.lingxia_servers,
        build_env,
        build_lingxia_server.as_deref(),
    );
    // Only a prod build may leave its build server. A dev package stays on
    // the dev server even if an older override file is still on disk.
    let override_env = (build_env == AppEnv::Prod && available.contains(&build_env))
        .then(|| persisted.filter(|env| available.contains(env) && *env != build_env))
        .flatten();
    let service_env = override_env.unwrap_or(build_env);
    let lingxia_server = config
        .lingxia_servers
        .get(service_env)
        .map(str::to_string)
        .or_else(|| build_lingxia_server.clone());
    ServiceEnvSnapshot {
        build_env,
        service_env,
        next_launch_env: service_env,
        available,
        lingxia_server,
        build_lingxia_server,
    }
}

fn available_envs(
    servers: &LingxiaServers,
    build_env: AppEnv,
    build_server: Option<&str>,
) -> Vec<AppEnv> {
    if build_env != AppEnv::Prod {
        return vec![build_env];
    }
    let mut available = servers.available();
    if available.is_empty() {
        if build_server.is_some() {
            available.push(build_env);
        }
        return available;
    }
    if !available.contains(&build_env) && build_server.is_some() {
        available.push(build_env);
        available.sort_by_key(|env| env.as_str());
    }
    available
}

fn persist_target(
    app_data_dir: &Path,
    running: &ServiceEnvSnapshot,
    target: AppEnv,
) -> Result<ServiceEnvSwitch, ServiceEnvError> {
    let _guard = file_lock();
    if running.build_env != AppEnv::Prod && target != running.build_env {
        return Err(ServiceEnvError::Invalid(
            "only a prod build can switch the service environment".to_string(),
        ));
    }
    if target != running.build_env && !running.available.contains(&running.build_env) {
        return Err(ServiceEnvError::Invalid(
            "the build environment has no configured LingXia server".to_string(),
        ));
    }
    if !running.available.contains(&target) {
        return Err(ServiceEnvError::Invalid(format!(
            "service environment '{target}' is not configured in this build"
        )));
    }
    let desired = desired_override(running.build_env, target);
    let raw = read_override(app_data_dir);
    let pending = match &raw {
        Ok(value) => (*value).filter(|env| running.available.contains(env)),
        Err(_) => None,
    };
    // A corrupt file is not "already the desired override"; replace it.
    let dirty = raw.is_err() || pending != desired;
    if dirty {
        write_override_file(app_data_dir, desired)?;
    }
    let snapshot = overlay_pending(app_data_dir, running);
    if target == running.service_env {
        Ok(ServiceEnvSwitch::Unchanged(snapshot))
    } else {
        Ok(ServiceEnvSwitch::RestartRequired(snapshot))
    }
}

fn file_lock() -> std::sync::MutexGuard<'static, ()> {
    FILE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn desired_override(build_env: AppEnv, target: AppEnv) -> Option<AppEnv> {
    (target != build_env).then_some(target)
}

fn installed() -> Result<&'static Installed, ServiceEnvError> {
    STATE.get().ok_or(ServiceEnvError::NotInitialized)
}

fn fallback_snapshot() -> ServiceEnvSnapshot {
    let config = crate::app_config();
    ServiceEnvSnapshot {
        build_env: crate::env(),
        service_env: crate::env(),
        next_launch_env: crate::env(),
        available: config
            .map(|config| {
                available_envs(
                    &config.lingxia_servers,
                    config.env,
                    config
                        .lingxia_server
                        .as_deref()
                        .map(str::trim)
                        .filter(|server| !server.is_empty()),
                )
            })
            .unwrap_or_default(),
        lingxia_server: config.and_then(|config| config.lingxia_server.clone()),
        build_lingxia_server: config.and_then(|config| config.lingxia_server.clone()),
    }
}

fn persist_path(app_data_dir: &Path) -> PathBuf {
    crate::app_state_file(app_data_dir, PERSIST_FILE)
}

fn read_override(app_data_dir: &Path) -> Result<Option<AppEnv>, ServiceEnvError> {
    let path = persist_path(app_data_dir);
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path)
        .map_err(|err| ServiceEnvError::Persist(format!("read {}: {err}", path.display())))?;
    let parsed: PersistedOverride = serde_json::from_str(&raw)
        .map_err(|err| ServiceEnvError::Persist(format!("parse {}: {err}", path.display())))?;
    AppEnv::parse(&parsed.service_env).map(Some).ok_or_else(|| {
        ServiceEnvError::Invalid(format!(
            "service environment {:?} is not dev or prod",
            parsed.service_env
        ))
    })
}

fn write_override_file(app_data_dir: &Path, value: Option<AppEnv>) -> Result<(), ServiceEnvError> {
    match value {
        None => clear_override_file(app_data_dir),
        Some(env) => {
            let path = persist_path(app_data_dir);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|err| {
                    ServiceEnvError::Persist(format!("create {}: {err}", parent.display()))
                })?;
            }
            let body = serde_json::to_string_pretty(&PersistedOverride {
                service_env: env.as_str().to_string(),
            })
            .map_err(|err| ServiceEnvError::Persist(err.to_string()))?;
            write_atomic(&path, body.as_bytes())
        }
    }
}

fn write_atomic(path: &Path, body: &[u8]) -> Result<(), ServiceEnvError> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, body)
        .map_err(|err| ServiceEnvError::Persist(format!("write {}: {err}", tmp.display())))?;
    if std::fs::rename(&tmp, path).is_ok() {
        let _ = std::fs::remove_file(path.with_extension("json.bak"));
        return Ok(());
    }
    // Windows refuses to rename over an existing file. Move the live file aside
    // and put it back if the replacement does not land.
    let backup = path.with_extension("json.bak");
    let _ = std::fs::remove_file(&backup);
    if path.exists()
        && let Err(err) = std::fs::rename(path, &backup)
    {
        let _ = std::fs::remove_file(&tmp);
        return Err(ServiceEnvError::Persist(format!(
            "write {}: {err}",
            path.display()
        )));
    }
    if let Err(err) = std::fs::rename(&tmp, path) {
        let _ = std::fs::rename(&backup, path);
        let _ = std::fs::remove_file(&tmp);
        return Err(ServiceEnvError::Persist(format!(
            "write {}: {err}",
            path.display()
        )));
    }
    let _ = std::fs::remove_file(&backup);
    Ok(())
}

fn clear_override_file(app_data_dir: &Path) -> Result<(), ServiceEnvError> {
    let path = persist_path(app_data_dir);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(ServiceEnvError::Persist(format!(
            "remove {}: {err}",
            path.display()
        ))),
    }
}

fn overlay_pending(app_data_dir: &Path, running: &ServiceEnvSnapshot) -> ServiceEnvSnapshot {
    let override_env = read_override(app_data_dir)
        .ok()
        .flatten()
        .filter(|env| running.build_env == AppEnv::Prod && running.available.contains(env));
    ServiceEnvSnapshot {
        next_launch_env: override_env.unwrap_or(running.build_env),
        ..running.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LingxiaServers;
    use std::path::PathBuf;

    fn config(env: AppEnv, servers: LingxiaServers, server: Option<&str>) -> AppConfig {
        AppConfig {
            product_name: "Demo".into(),
            product_names: Default::default(),
            product_version: "1.0.0".into(),
            lingxia_id: None,
            lingxia_server: server.map(str::to_string),
            lingxia_servers: servers,
            env,
            home_app_id: String::new(),
            home_app_version: String::new(),
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

    fn both() -> LingxiaServers {
        LingxiaServers {
            dev: Some("https://dev.example".into()),
            prod: Some("https://prod.example".into()),
        }
    }

    #[test]
    fn repeated_install_checks_the_installed_config_and_data_directory() {
        let state = OnceLock::new();
        let original = Installed {
            data_dir: PathBuf::from("first"),
            snapshot: resolve(
                &config(AppEnv::Prod, both(), Some("https://prod.example")),
                None,
            ),
        };
        assert_eq!(
            install_once(&state, original.clone()).unwrap(),
            original.snapshot
        );
        assert_eq!(
            install_once(&state, original.clone()).unwrap(),
            original.snapshot
        );
        let mut different = original.clone();
        different.data_dir = PathBuf::from("second");
        assert!(install_once(&state, different).is_err());
        let mut different = original.clone();
        different.snapshot = resolve(
            &config(AppEnv::Prod, both(), Some("https://prod.example")),
            Some(AppEnv::Dev),
        );
        assert!(install_once(&state, different).is_err());
        assert_eq!(state.get().unwrap().snapshot, original.snapshot);
    }

    #[test]
    fn no_override_uses_build_env_and_its_server() {
        let snapshot = resolve(
            &config(AppEnv::Prod, both(), Some("https://prod.example")),
            None,
        );
        assert_eq!(snapshot.service_env, AppEnv::Prod);
        assert_eq!(snapshot.next_launch_env, snapshot.build_env);
        assert_eq!(
            snapshot.lingxia_server.as_deref(),
            Some("https://prod.example")
        );
        assert_eq!(
            snapshot.build_lingxia_server.as_deref(),
            Some("https://prod.example")
        );
    }

    #[test]
    fn dev_build_ignores_an_override_and_cannot_switch() {
        let cfg = config(AppEnv::Dev, both(), Some("https://dev.example"));
        let snapshot = resolve(&cfg, Some(AppEnv::Prod));
        assert_eq!(snapshot.service_env, AppEnv::Dev);
        assert_eq!(snapshot.next_launch_env, snapshot.build_env);
        assert_eq!(snapshot.available, vec![AppEnv::Dev]);
        assert!(!dev_service_mark(snapshot.build_env, snapshot.service_env));
        assert_eq!(
            snapshot.lingxia_server.as_deref(),
            Some("https://dev.example")
        );

        let dir = temp_dir("dev-build-blocked");
        let err = persist_target(&dir, &snapshot, AppEnv::Prod).unwrap_err();
        assert!(matches!(err, ServiceEnvError::Invalid(_)));
        assert!(!persist_path(&dir).exists());
        match persist_target(&dir, &snapshot, AppEnv::Dev).unwrap() {
            ServiceEnvSwitch::Unchanged(snapshot) => {
                assert_eq!(snapshot.service_env, AppEnv::Dev);
                assert_eq!(snapshot.next_launch_env, snapshot.build_env);
            }
            other => panic!("expected Unchanged, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn persisted_dev_selects_dev_server_on_prod_build() {
        let snapshot = resolve(
            &config(AppEnv::Prod, both(), Some("https://prod.example")),
            Some(AppEnv::Dev),
        );
        assert_eq!(snapshot.service_env, AppEnv::Dev);
        assert_eq!(snapshot.next_launch_env, AppEnv::Dev);
        assert_eq!(
            snapshot.lingxia_server.as_deref(),
            Some("https://dev.example")
        );
        assert_eq!(
            snapshot.build_lingxia_server.as_deref(),
            Some("https://prod.example")
        );
        assert!(dev_service_mark(snapshot.build_env, snapshot.service_env));
    }

    #[test]
    fn missing_target_is_ignored_at_boot() {
        let servers = LingxiaServers {
            prod: Some("https://prod.example".into()),
            dev: None,
        };
        let snapshot = resolve(
            &config(AppEnv::Prod, servers, Some("https://prod.example")),
            Some(AppEnv::Dev),
        );
        assert_eq!(snapshot.service_env, AppEnv::Prod);
        assert_eq!(snapshot.next_launch_env, snapshot.build_env);
        assert_eq!(snapshot.available, vec![AppEnv::Prod]);
    }

    #[test]
    fn prod_build_without_its_own_server_cannot_switch() {
        let servers = LingxiaServers {
            dev: Some("https://dev.example".into()),
            prod: None,
        };
        let config = config(AppEnv::Prod, servers, None);
        let snapshot = resolve(&config, None);
        assert_eq!(snapshot.available, vec![AppEnv::Dev]);
        assert!(!snapshot.can_toggle());

        let persisted = resolve(&config, Some(AppEnv::Dev));
        assert_eq!(persisted.service_env, AppEnv::Prod);

        let dir = temp_dir("no-prod-server");
        assert!(matches!(
            persist_target(&dir, &snapshot, AppEnv::Dev),
            Err(ServiceEnvError::Invalid(_))
        ));
        assert!(!persist_path(&dir).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("lingxia-service-env-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn override_file_roundtrips_and_clears() {
        let dir = temp_dir("roundtrip");
        write_override_file(&dir, Some(AppEnv::Dev)).unwrap();
        assert_eq!(read_override(&dir).unwrap(), Some(AppEnv::Dev));
        clear_override_file(&dir).unwrap();
        assert_eq!(read_override(&dir).unwrap(), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn repeating_applied_override_keeps_the_file() {
        let dir = temp_dir("keep-override");
        write_override_file(&dir, Some(AppEnv::Dev)).unwrap();
        let running = resolve(
            &config(AppEnv::Prod, both(), Some("https://prod.example")),
            Some(AppEnv::Dev),
        );
        match persist_target(&dir, &running, AppEnv::Dev).unwrap() {
            ServiceEnvSwitch::Unchanged(snapshot) => {
                assert_eq!(snapshot.next_launch_env, AppEnv::Dev);
                assert_eq!(snapshot.service_env, AppEnv::Dev);
            }
            other => panic!("expected Unchanged, got {other:?}"),
        }
        assert_eq!(read_override(&dir).unwrap(), Some(AppEnv::Dev));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn repeating_running_build_cancels_pending_file() {
        let dir = temp_dir("cancel-pending");
        write_override_file(&dir, Some(AppEnv::Dev)).unwrap();
        let running = resolve(
            &config(AppEnv::Prod, both(), Some("https://prod.example")),
            None,
        );
        match persist_target(&dir, &running, AppEnv::Prod).unwrap() {
            ServiceEnvSwitch::Unchanged(snapshot) => {
                assert_eq!(snapshot.next_launch_env, snapshot.build_env);
                assert_eq!(snapshot.service_env, AppEnv::Prod);
            }
            other => panic!("expected Unchanged, got {other:?}"),
        }
        assert_eq!(read_override(&dir).unwrap(), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn switching_away_from_applied_override_clears_and_requires_restart() {
        let dir = temp_dir("switch-back");
        write_override_file(&dir, Some(AppEnv::Dev)).unwrap();
        let running = resolve(
            &config(AppEnv::Prod, both(), Some("https://prod.example")),
            Some(AppEnv::Dev),
        );
        match persist_target(&dir, &running, AppEnv::Prod).unwrap() {
            ServiceEnvSwitch::RestartRequired(snapshot) => {
                assert_eq!(snapshot.next_launch_env, snapshot.build_env);
                assert_eq!(snapshot.service_env, AppEnv::Dev);
            }
            other => panic!("expected RestartRequired, got {other:?}"),
        }
        assert_eq!(read_override(&dir).unwrap(), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_override_is_replaced_when_repeating_the_running_env() {
        let dir = temp_dir("corrupt");
        let path = persist_path(&dir);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"{").unwrap();
        let running = resolve(
            &config(AppEnv::Prod, both(), Some("https://prod.example")),
            None,
        );
        match persist_target(&dir, &running, AppEnv::Prod).unwrap() {
            ServiceEnvSwitch::Unchanged(snapshot) => {
                assert_eq!(snapshot.next_launch_env, snapshot.build_env);
            }
            other => panic!("expected Unchanged, got {other:?}"),
        }
        assert_eq!(read_override(&dir).unwrap(), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn retrying_a_pending_switch_keeps_the_target_and_running_server() {
        let dir = temp_dir("retry-pending");
        let cfg = config(AppEnv::Prod, both(), Some("https://prod.example"));
        let running = resolve(&cfg, None);
        assert!(running.can_toggle());
        for _ in 0..2 {
            let ServiceEnvSwitch::RestartRequired(pending) =
                persist_target(&dir, &running, toggle_target(running.service_env)).unwrap()
            else {
                panic!("toggle must require restart")
            };
            assert_eq!(pending.service_env, AppEnv::Prod);
            assert_eq!(pending.next_launch_env, AppEnv::Dev);
            assert_eq!(pending.lingxia_server, running.lingxia_server);
            assert!(pending.can_toggle());
        }
        assert_eq!(running.next_launch_env, AppEnv::Prod);
        assert_eq!(overlay_pending(&dir, &running).next_launch_env, AppEnv::Dev);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn prod_build_switches_to_dev_and_back_across_launches() {
        let dir = temp_dir("loop");
        let cfg = config(AppEnv::Prod, both(), Some("https://prod.example"));

        let launch = resolve(&cfg, read_override(&dir).unwrap());
        assert_eq!(launch.service_env, AppEnv::Prod);
        assert!(!dev_service_mark(launch.build_env, launch.service_env));
        assert_eq!(
            launch.lingxia_server.as_deref(),
            Some("https://prod.example")
        );

        match persist_target(&dir, &launch, AppEnv::Dev).unwrap() {
            ServiceEnvSwitch::RestartRequired(pending) => {
                assert_eq!(pending.service_env, AppEnv::Prod);
                assert_eq!(pending.next_launch_env, AppEnv::Dev);
                assert_eq!(
                    pending.lingxia_server.as_deref(),
                    Some("https://prod.example")
                );
            }
            other => panic!("expected RestartRequired, got {other:?}"),
        }

        let launch = resolve(&cfg, read_override(&dir).unwrap());
        assert_eq!(launch.service_env, AppEnv::Dev);
        assert!(dev_service_mark(launch.build_env, launch.service_env));
        assert_eq!(
            launch.lingxia_server.as_deref(),
            Some("https://dev.example")
        );
        assert_eq!(
            launch.build_lingxia_server.as_deref(),
            Some("https://prod.example")
        );

        match persist_target(&dir, &launch, AppEnv::Dev).unwrap() {
            ServiceEnvSwitch::Unchanged(snapshot) => {
                assert_eq!(snapshot.next_launch_env, AppEnv::Dev);
                assert_eq!(read_override(&dir).unwrap(), Some(AppEnv::Dev));
            }
            other => panic!("expected Unchanged, got {other:?}"),
        }

        match persist_target(&dir, &launch, AppEnv::Prod).unwrap() {
            ServiceEnvSwitch::RestartRequired(pending) => {
                assert_eq!(pending.service_env, AppEnv::Dev);
                assert_eq!(pending.next_launch_env, pending.build_env);
            }
            other => panic!("expected RestartRequired, got {other:?}"),
        }

        let launch = resolve(&cfg, read_override(&dir).unwrap());
        assert_eq!(launch.service_env, AppEnv::Prod);
        assert!(!dev_service_mark(launch.build_env, launch.service_env));
        assert_eq!(
            launch.lingxia_server.as_deref(),
            Some("https://prod.example")
        );
        assert_eq!(read_override(&dir).unwrap(), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn app_link_hosts_follow_the_service_env_when_both_lists_are_packed() {
        let mut cfg = config(AppEnv::Prod, both(), Some("https://prod.example"));
        cfg.app_links = Some(crate::AppLinksConfig {
            hosts: vec!["app.example.com".into()],
            hosts_by_env: crate::AppLinkHostsByEnv {
                dev: Some(vec!["app-dev.example.com".into()]),
                prod: Some(vec!["app.example.com".into()]),
            },
        });
        assert_eq!(
            app_link_hosts_for(&cfg, AppEnv::Dev),
            vec!["app-dev.example.com".to_string()]
        );
        assert_eq!(
            app_link_hosts_for(&cfg, AppEnv::Prod),
            vec!["app.example.com".to_string()]
        );

        cfg.app_links = Some(crate::AppLinksConfig {
            hosts: vec!["app.example.com".into()],
            hosts_by_env: crate::AppLinkHostsByEnv {
                dev: None,
                prod: Some(vec!["app.example.com".into()]),
            },
        });
        assert!(app_link_hosts_for(&cfg, AppEnv::Dev).is_empty());

        cfg.app_links = Some(crate::AppLinksConfig {
            hosts: vec!["only.example.com".into()],
            hosts_by_env: Default::default(),
        });
        assert_eq!(
            app_link_hosts_for(&cfg, AppEnv::Dev),
            vec!["only.example.com".to_string()]
        );
    }

    #[test]
    fn toggle_target_flips_between_dev_and_prod() {
        assert_eq!(toggle_target(AppEnv::Prod), AppEnv::Dev);
        assert_eq!(toggle_target(AppEnv::Dev), AppEnv::Prod);
    }
}
