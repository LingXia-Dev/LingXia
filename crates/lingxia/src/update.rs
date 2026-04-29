//! Host app update helpers built on LingXia's update service.

use lingxia_service::update::{
    AppUpdateApply, AppUpdateEvent, AppUpdateStage, HostAppUpdateService, UpdatePackageInfo,
    UpdateUiMode, configure_update, update_config,
};
use std::fmt;
use std::path::Path;

pub use lingxia_service::update::HostAppInstall;

/// Disables built-in host app update UX and startup auto-check.
///
/// Native code can still call [`check_host_app_update`] explicitly and render
/// its own UI from the returned update and apply event stream.
pub fn use_custom_host_app_update() {
    let mut config = update_config();
    config.ui_mode = UpdateUiMode::Custom;
    config.auto_check_app = false;
    configure_update(config);
}

/// Registers a custom host app installer.
///
/// The installer is called only after the package has been downloaded and
/// verified. Use [`use_custom_host_app_update`] when the host wants to disable
/// LingXia's built-in startup check and update UI.
///
/// Return [`HostAppInstall::Handled`] when the installer has handled the
/// package, or [`HostAppInstall::Fallback`] to use the default platform
/// installer.
pub fn set_host_app_installer(
    installer: impl Fn(&Path) -> crate::Result<HostAppInstall> + Send + Sync + 'static,
) {
    lingxia_service::update::set_host_app_installer(move |path| {
        installer(path).map_err(|error| lingxia_update::UpdateError::runtime(error.to_string()))
    });
}

/// A checked host app update.
pub struct HostAppUpdate {
    info: UpdatePackageInfo,
    service: HostAppUpdateService,
}

impl HostAppUpdate {
    fn new(info: UpdatePackageInfo, service: HostAppUpdateService) -> Self {
        Self { info, service }
    }

    /// Returns update metadata for custom UI.
    pub fn info(&self) -> HostAppUpdateInfo<'_> {
        HostAppUpdateInfo { inner: &self.info }
    }

    /// Applies this update and returns an event receiver for UI state.
    ///
    /// The package path is intentionally not exposed. Native UI should render
    /// progress from [`HostAppUpdateEvent`] and let LingXia hand off
    /// installation to the platform.
    pub fn apply(self) -> HostAppUpdateApply {
        HostAppUpdateApply {
            inner: self.service.apply(self.info),
        }
    }
}

/// Read-only host app update metadata.
#[derive(Clone, Copy)]
pub struct HostAppUpdateInfo<'a> {
    inner: &'a UpdatePackageInfo,
}

impl HostAppUpdateInfo<'_> {
    /// Returns the target app version.
    pub fn version(&self) -> &str {
        &self.inner.version
    }

    /// Returns the expected package size in bytes when the provider supplied it.
    pub fn package_size_bytes(&self) -> Option<u64> {
        self.inner.size
    }

    /// Returns release notes when the update provider supplied them.
    pub fn release_notes(&self) -> Option<&[String]> {
        self.inner.release_notes.as_deref()
    }

    /// Reports whether this update must be applied before the app can continue.
    pub fn is_force_update(&self) -> bool {
        self.inner.is_force_update
    }
}

impl fmt::Debug for HostAppUpdateInfo<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HostAppUpdateInfo")
            .field("version", &self.version())
            .field("package_size_bytes", &self.package_size_bytes())
            .field("release_notes", &self.release_notes())
            .field("is_force_update", &self.is_force_update())
            .finish()
    }
}

/// Applies a checked host app update and yields UI events.
pub struct HostAppUpdateApply {
    inner: AppUpdateApply,
}

impl HostAppUpdateApply {
    /// Waits for the next update event.
    pub async fn next(&mut self) -> Option<HostAppUpdateEvent> {
        while let Some(event) = self.inner.next().await {
            if let Some(event) = HostAppUpdateEvent::from_service_event(event) {
                return Some(event);
            }
        }
        None
    }
}

/// Host app update state changes for custom native UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostAppUpdateEvent {
    /// The update package is downloading.
    Downloading {
        downloaded_bytes: u64,
        progress: Option<u8>,
    },
    /// The package finished downloading and validation passed.
    Downloaded,
    /// Installation has been handed off to the platform.
    InstallRequested,
    /// Applying the update failed.
    Failed {
        stage: HostAppUpdateStage,
        error: String,
    },
}

impl HostAppUpdateEvent {
    fn from_service_event(event: AppUpdateEvent) -> Option<Self> {
        match event {
            AppUpdateEvent::Available(_) => None,
            AppUpdateEvent::DownloadStarted { .. } => Some(Self::Downloading {
                downloaded_bytes: 0,
                progress: None,
            }),
            AppUpdateEvent::DownloadProgress {
                downloaded_bytes,
                progress,
                ..
            } => Some(Self::Downloading {
                downloaded_bytes,
                progress,
            }),
            AppUpdateEvent::Downloaded { .. } => Some(Self::Downloaded),
            AppUpdateEvent::InstallRequested { .. } => Some(Self::InstallRequested),
            AppUpdateEvent::Failed { stage, error } => Some(Self::Failed {
                stage: stage.into(),
                error,
            }),
        }
    }
}

/// Stage that produced a host app update failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostAppUpdateStage {
    Check,
    Prompt,
    Download,
    Install,
}

impl From<AppUpdateStage> for HostAppUpdateStage {
    fn from(stage: AppUpdateStage) -> Self {
        match stage {
            AppUpdateStage::Check => Self::Check,
            AppUpdateStage::Prompt => Self::Prompt,
            AppUpdateStage::Download => Self::Download,
            AppUpdateStage::Install => Self::Install,
        }
    }
}

/// Checks the provider for a host app update.
///
/// The current version always comes from the initialized host app config
/// (`productVersion`); native callers must not provide their own version.
pub async fn check_host_app_update() -> crate::Result<Option<HostAppUpdate>> {
    let service = host_update_service()?;
    let Some(info) = service.check().await? else {
        return Ok(None);
    };
    Ok(Some(HostAppUpdate::new(info, service)))
}

pub(crate) fn spawn_host_app_update_flow(runtime: std::sync::Arc<lingxia_platform::Platform>) {
    host_update_service_from(runtime).spawn_builtin_flow();
}

fn host_update_service() -> crate::Result<HostAppUpdateService> {
    let runtime = lxapp::get_platform()
        .ok_or_else(|| crate::Error::internal("platform is not initialized"))?;
    Ok(host_update_service_from(runtime))
}

fn host_update_service_from(
    runtime: std::sync::Arc<lingxia_platform::Platform>,
) -> HostAppUpdateService {
    HostAppUpdateService::new(runtime, lxapp::provider::update_provider())
}
