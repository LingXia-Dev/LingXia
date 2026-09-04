use crate::host::{HostCancel, HostResult, await_or_cancel};
use crate::platform_error::map_platform_error;
use lingxia_app_context::app_config;
use lingxia_service::file::ChooseDirectoryRequest;
use lxapp::LxApp;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    product_name: String,
    version: String,
    sdk_version: String,
    webui_version: String,
    git_sha: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadSettingsResult {
    download_dir: String,
    uses_default_dir: bool,
    can_choose_directory: bool,
}

fn download_settings_result(app: &LxApp) -> HostResult<DownloadSettingsResult> {
    let effective = lingxia_service::downloads::dir(&app.app_data_dir());
    let configured = lingxia_service::settings::download_dir(&app.app_data_dir())
        .map_err(|e| lxapp::LxAppError::Runtime(e.to_string()))?;
    Ok(DownloadSettingsResult {
        download_dir: effective.to_string_lossy().to_string(),
        uses_default_dir: configured.is_none(),
        // TODO: replace this hardcoded platform check with an AppRuntime
        // capability query (directory-picker support). Windows dialog support
        // is unverified, so behavior is intentionally left unchanged for now.
        can_choose_directory: cfg!(target_os = "macos"),
    })
}

#[lingxia::framework_native("app.getInfo", audience = "browser-control-only")]
fn get_app_info(_app: Arc<LxApp>) -> HostResult<AppInfo> {
    let (product_name, version) = match app_config() {
        Some(cfg) => (cfg.product_name.clone(), cfg.product_version.clone()),
        None => (String::new(), String::new()),
    };
    Ok(AppInfo {
        product_name,
        version,
        sdk_version: lxapp::SDK_RUNTIME_VERSION.to_string(),
        webui_version: crate::bundled_webui_version().unwrap_or_default(),
        git_sha: env!("LINGXIA_GIT_SHA_SHORT").to_string(),
    })
}

#[lingxia::framework_native("downloads.getSettings", audience = "browser-control-only")]
fn get_download_settings(app: Arc<LxApp>) -> HostResult<DownloadSettingsResult> {
    download_settings_result(&app)
}

#[lingxia::framework_native("downloads.chooseDirectory", audience = "browser-control-only")]
async fn choose_download_directory(
    app: Arc<LxApp>,
    mut cancel: HostCancel,
) -> HostResult<DownloadSettingsResult> {
    let current_dir = lingxia_service::downloads::dir(&app.app_data_dir())
        .to_string_lossy()
        .to_string();
    let app_for_picker = app.clone();
    let result = await_or_cancel(&mut cancel, async move {
        lingxia_service::file::choose_directory(
            &*app_for_picker.runtime,
            ChooseDirectoryRequest {
                title: Some("Choose Download Folder".to_string()),
                default_path: Some(current_dir),
            },
        )
        .await
        .map_err(|e| map_platform_error("downloads.chooseDirectory", e))
    })
    .await?;

    if !result.canceled
        && let Some(path) = result.paths.first()
    {
        lingxia_service::downloads::set_dir(&app.app_data_dir(), PathBuf::from(path))
            .map_err(|e| lxapp::LxAppError::Runtime(e.to_string()))?;
    }

    download_settings_result(&app)
}

#[lingxia::framework_native("downloads.resetDirectory", audience = "browser-control-only")]
fn reset_download_directory(app: Arc<LxApp>) -> HostResult<DownloadSettingsResult> {
    lingxia_service::downloads::reset_dir(&app.app_data_dir())
        .map_err(|e| lxapp::LxAppError::Runtime(e.to_string()))?;
    download_settings_result(&app)
}

pub(crate) fn register_routes() {
    lxapp::host::register_host_entry(get_app_info_host());
    lxapp::host::register_host_entry(get_download_settings_host());
    lxapp::host::register_host_entry(choose_download_directory_host());
    lxapp::host::register_host_entry(reset_download_directory_host());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framework_routes_are_browser_control_only() {
        for route in [
            get_app_info_host(),
            get_download_settings_host(),
            choose_download_directory_host(),
            reset_download_directory_host(),
        ] {
            assert_eq!(
                route.audience(),
                lxapp::host::RouteAudience::BrowserControlOnly
            );
        }
    }
}
