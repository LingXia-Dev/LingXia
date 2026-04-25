pub use lingxia_service::update::{
    AppUpdateApply, AppUpdateEvent, AppUpdateStage, UpdateError, UpdatePackageInfo,
};
use lingxia_service::update::{
    HostAppUpdateService, UpdateUiMode, configure_update, update_config,
};

/// Disables built-in host app update UX and startup auto-check.
///
/// Native code can still call [`check_host_app_update`] and [`apply_host_app_update`]
/// explicitly and render its own UI from the returned event stream.
pub fn use_custom_host_app_update() {
    let mut config = update_config();
    config.ui_mode = UpdateUiMode::Custom;
    config.auto_check_app = false;
    configure_update(config);
}

/// Checks the provider for a host app update.
///
/// The current version always comes from the initialized host app config
/// (`productVersion`); native callers must not provide their own version.
pub async fn check_host_app_update() -> crate::Result<Option<UpdatePackageInfo>> {
    host_update_service()?.check().await.map_err(Into::into)
}

/// Applies a checked host app update and returns an event receiver for UI state.
///
/// The package path is intentionally not exposed. Native UI should render progress
/// from [`AppUpdateEvent`] and let LingXia hand off installation to the platform.
pub fn apply_host_app_update(update: UpdatePackageInfo) -> crate::Result<AppUpdateApply> {
    Ok(host_update_service()?.apply(update))
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
