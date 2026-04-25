use crate::config::update_config;
use crate::{
    BoxFuture, LxAppUpdateQuery, ReleaseType, RuntimeCompatibilityError, UpdatePackageInfo,
    UpdateTarget, Version,
};
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tokio::time::timeout;

use super::error::UpdateError;

const FOREGROUND_UPDATE_CHECK_TIMEOUT: Duration = Duration::from_secs(3);

pub trait LxAppUpdateHost: Clone + Send + Sync + 'static {
    fn spawn_detached(&self, task: BoxFuture<'static, ()>);
    fn target_appid(&self) -> &str;
    fn release_type(&self) -> ReleaseType;
    fn runtime_version(&self) -> &str;
    fn current_version_hint(&self) -> Option<String>;
    fn installed_version<'a>(&'a self) -> BoxFuture<'a, Result<Option<String>, UpdateError>>;
    fn is_installed<'a>(&'a self) -> BoxFuture<'a, Result<bool, UpdateError>>;
    fn check_latest_update<'a>(
        &'a self,
        current_version: Option<&'a str>,
    ) -> BoxFuture<'a, Result<Option<UpdatePackageInfo>, UpdateError>>;
    fn check_exact_update<'a>(
        &'a self,
        target_version: &'a str,
    ) -> BoxFuture<'a, Result<Option<UpdatePackageInfo>, UpdateError>>;
    fn has_downloaded_update<'a>(
        &'a self,
        version: &'a str,
    ) -> BoxFuture<'a, Result<bool, UpdateError>>;
    fn download_update<'a>(
        &'a self,
        update: &'a UpdatePackageInfo,
    ) -> BoxFuture<'a, Result<(), UpdateError>>;
    fn wait_for_or_start_force_download<'a>(
        &'a self,
        update: &'a UpdatePackageInfo,
    ) -> BoxFuture<'a, Result<(), UpdateError>>;
    fn emit_update_ready(&self, version: &str, is_force_update: bool) -> Result<(), UpdateError>;
    fn emit_update_failed(
        &self,
        update: &UpdatePackageInfo,
        error: &str,
    ) -> Result<(), UpdateError>;
    fn is_bundled_available(&self) -> bool;
    fn register_builtin_bundle(&self) -> Result<(), UpdateError>;
    fn has_update_provider(&self) -> bool;
    fn log_warning(&self, detail: &str);
}

pub fn lxapp_update_scope_key(target_appid: &str, release_type: ReleaseType) -> String {
    UpdateTarget::lxapp(
        target_appid,
        release_type,
        LxAppUpdateQuery::latest(None::<String>),
    )
    .scope_key()
}

struct ActiveLxAppUpdateCheck {
    scope: String,
}

impl Drop for ActiveLxAppUpdateCheck {
    fn drop(&mut self) {
        if let Ok(mut active) = active_lxapp_update_checks().lock() {
            active.remove(&self.scope);
        }
    }
}

fn active_lxapp_update_checks() -> &'static Mutex<HashSet<String>> {
    static ACTIVE_CHECKS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    ACTIVE_CHECKS.get_or_init(|| Mutex::new(HashSet::new()))
}

fn try_begin_lxapp_update_check(scope: String) -> Option<ActiveLxAppUpdateCheck> {
    let mut active = active_lxapp_update_checks()
        .lock()
        .unwrap_or_else(|err| err.into_inner());
    if !active.insert(scope.clone()) {
        return None;
    }
    Some(ActiveLxAppUpdateCheck { scope })
}

async fn with_foreground_update_timeout<T, F>(future: F, context: &str) -> Result<T, UpdateError>
where
    F: std::future::Future<Output = Result<T, UpdateError>>,
{
    match timeout(FOREGROUND_UPDATE_CHECK_TIMEOUT, future).await {
        Ok(result) => result,
        Err(_) => Err(UpdateError::runtime(format!(
            "{} timed out after {}s",
            context,
            FOREGROUND_UPDATE_CHECK_TIMEOUT.as_secs()
        ))),
    }
}

fn runtime_compatibility_to_update_error(error: RuntimeCompatibilityError) -> UpdateError {
    match error {
        RuntimeCompatibilityError::InvalidCurrentRuntimeVersion { .. } => {
            UpdateError::runtime(error.to_string())
        }
        RuntimeCompatibilityError::InvalidRequiredRuntimeVersion { .. }
        | RuntimeCompatibilityError::RequiresRuntimeUpgrade { .. } => {
            UpdateError::unsupported(error.to_string())
        }
    }
}

fn ensure_runtime_version_compatible<H: LxAppUpdateHost>(
    host: &H,
    pkg: &UpdatePackageInfo,
) -> Result<(), UpdateError> {
    pkg.ensure_runtime_compatible(host.runtime_version(), host.target_appid())
        .map_err(runtime_compatibility_to_update_error)
}

pub fn spawn_background_update_check<H: LxAppUpdateHost>(host: H, current_version: Option<String>) {
    let runner = host.clone();
    host.spawn_detached(Box::pin(async move {
        let scope = lxapp_update_scope_key(runner.target_appid(), runner.release_type());
        let Some(_active_check) = try_begin_lxapp_update_check(scope) else {
            return;
        };

        let resolved_current_version = match current_version {
            Some(version) => Some(version),
            None => match runner.installed_version().await {
                Ok(version) => version,
                Err(error) => {
                    runner.log_warning(&format!(
                        "Failed to resolve installed version for {}: {}",
                        runner.target_appid(),
                        error
                    ));
                    None
                }
            },
        };

        let update = match runner
            .check_latest_update(resolved_current_version.as_deref())
            .await
        {
            Ok(update) => update,
            Err(error) => {
                runner.log_warning(&format!(
                    "Background update check failed for {}: {}",
                    runner.target_appid(),
                    error
                ));
                None
            }
        };

        let Some(pkg) = update else {
            return;
        };

        if !UpdatePackageInfo::should_replace_version(
            &pkg.version,
            resolved_current_version.as_deref(),
        ) {
            return;
        }

        if let Err(error) = ensure_runtime_version_compatible(&runner, &pkg) {
            let _ = runner.emit_update_failed(&pkg, &error.to_string());
            return;
        }

        match runner.has_downloaded_update(&pkg.version).await {
            Ok(true) => {
                let _ = runner.emit_update_ready(&pkg.version, pkg.is_force_update);
            }
            Ok(false) => match runner.download_update(&pkg).await {
                Ok(()) => {
                    let _ = runner.emit_update_ready(&pkg.version, pkg.is_force_update);
                }
                Err(error) => {
                    let _ = runner.emit_update_failed(&pkg, &error.to_string());
                }
            },
            Err(error) => {
                let _ = runner.emit_update_failed(&pkg, &error.to_string());
            }
        }
    }));
}

pub async fn ensure_first_install<H: LxAppUpdateHost>(host: &H) -> Result<(), UpdateError> {
    if host.release_type() != ReleaseType::Release {
        return Ok(());
    }

    if host.is_installed().await? {
        return Ok(());
    }

    if host.is_bundled_available() {
        host.register_builtin_bundle()?;
        return Ok(());
    }

    if !host.has_update_provider() {
        return Err(UpdateError::unsupported(format!(
            "lxapp '{}' is not installed; remote install unavailable",
            host.target_appid()
        )));
    }

    let pkg = with_foreground_update_timeout(
        host.check_latest_update(None),
        &format!("first install update check for {}", host.target_appid()),
    )
    .await?
    .ok_or_else(|| {
        UpdateError::not_found(format!(
            "lxapp '{}' package not found ({})",
            host.target_appid(),
            host.release_type().as_str()
        ))
    })?;

    ensure_runtime_version_compatible(host, &pkg)?;
    host.download_update(&pkg).await
}

pub async fn ensure_target_version_ready<H: LxAppUpdateHost>(
    host: &H,
    target_version: &str,
) -> Result<(), UpdateError> {
    let target_version = target_version.trim();
    if target_version.is_empty() {
        return Err(UpdateError::invalid_parameter(
            "targetVersion cannot be empty",
        ));
    }

    let target_semver = Version::parse(target_version).map_err(|_| {
        UpdateError::invalid_parameter(format!(
            "targetVersion must be semantic version: {}",
            target_version
        ))
    })?;

    let is_installed = host.is_installed().await?;
    let current_version = if is_installed {
        host.installed_version().await?
    } else {
        None
    };

    if host.release_type() == ReleaseType::Release && update_config().force_update_gate {
        match with_foreground_update_timeout(
            host.check_latest_update(current_version.as_deref()),
            &format!("force-update gate check for {}", host.target_appid()),
        )
        .await
        {
            Ok(Some(pkg)) if pkg.is_force_update => {
                let force_version = Version::parse(&pkg.version).map_err(|_| {
                    UpdateError::unsupported(format!(
                        "invalid forced update version '{}' for {}",
                        pkg.version,
                        host.target_appid()
                    ))
                })?;
                if target_semver < force_version {
                    return Err(UpdateError::unsupported(format!(
                        "targetVersion {} is lower than required forced version {} for {} ({})",
                        target_version,
                        pkg.version,
                        host.target_appid(),
                        host.release_type().as_str()
                    )));
                }
            }
            Ok(_) => {}
            Err(error) => {
                host.log_warning(&format!(
                    "targetVersion force-update check failed (fail-open) for {}: {}",
                    host.target_appid(),
                    error
                ));
            }
        }
    }

    if current_version.as_deref() == Some(target_version) {
        return Ok(());
    }

    let pkg = with_foreground_update_timeout(
        host.check_exact_update(target_version),
        &format!(
            "exact version update check for {}@{}",
            host.target_appid(),
            target_version
        ),
    )
    .await?
    .ok_or_else(|| {
        UpdateError::not_found(format!(
            "No package available for {}@{} ({})",
            host.target_appid(),
            target_version,
            host.release_type().as_str()
        ))
    })?;

    ensure_runtime_version_compatible(host, &pkg)?;

    if host.has_downloaded_update(&pkg.version).await? {
        return Ok(());
    }

    host.download_update(&pkg).await
}

pub async fn ensure_force_update_for_installed<H: LxAppUpdateHost>(
    host: &H,
) -> Result<(), UpdateError> {
    if host.release_type() != ReleaseType::Release {
        return Ok(());
    }

    if !update_config().force_update_gate {
        return Ok(());
    }

    if !host.is_installed().await? {
        return Ok(());
    }

    let current_version = match host.installed_version().await? {
        Some(version) => version,
        None => {
            host.log_warning(&format!(
                "Installed lxapp has no recorded version; skip force-update gating: {}",
                host.target_appid()
            ));
            return Ok(());
        }
    };

    let update = match with_foreground_update_timeout(
        host.check_latest_update(Some(current_version.as_str())),
        &format!(
            "installed app force-update check for {}",
            host.target_appid()
        ),
    )
    .await
    {
        Ok(update) => update,
        Err(error) => {
            host.log_warning(&format!(
                "force-update check failed (fail-open) for {}: {}",
                host.target_appid(),
                error
            ));
            return Ok(());
        }
    };

    let Some(pkg) = update else {
        return Ok(());
    };

    if let Err(error) = ensure_runtime_version_compatible(host, &pkg) {
        if pkg.is_force_update {
            return Err(error);
        }
        host.log_warning(&format!(
            "optional update blocked by runtime version gate for {}: {}",
            host.target_appid(),
            error
        ));
        return Ok(());
    }

    if !pkg.is_force_update || pkg.version == current_version {
        return Ok(());
    }

    if host.has_downloaded_update(&pkg.version).await? {
        return Ok(());
    }

    host.wait_for_or_start_force_download(&pkg).await
}
