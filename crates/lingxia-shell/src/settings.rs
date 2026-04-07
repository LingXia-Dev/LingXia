use crate::host::{HostCancel, HostResult, await_or_cancel};
use crate::platform_error::map_platform_error;
use lingxia_app_context::app_config;
use lingxia_platform::traits::file::{ChooseDirectoryRequest, FileService};
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
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadSettingsResult {
    download_dir: String,
    uses_default_dir: bool,
    can_choose_directory: bool,
}

fn download_settings_result(app: &LxApp) -> HostResult<DownloadSettingsResult> {
    let effective = lingxia_transfer::dir(&app.app_data_dir());
    let configured = lingxia_settings::get_download_dir(&app.app_data_dir())
        .map_err(|e| lxapp::LxAppError::Runtime(e.to_string()))?;
    Ok(DownloadSettingsResult {
        download_dir: effective.to_string_lossy().to_string(),
        uses_default_dir: configured.is_none(),
        can_choose_directory: cfg!(target_os = "macos"),
    })
}

#[lingxia::host("app.getInfo")]
fn get_app_info(_app: Arc<LxApp>) -> HostResult<AppInfo> {
    let (product_name, version) = match app_config() {
        Some(cfg) => (cfg.product_name.clone(), cfg.product_version.clone()),
        None => (String::new(), String::new()),
    };
    Ok(AppInfo {
        product_name,
        version,
        sdk_version: lxapp::SDK_RUNTIME_VERSION.to_string(),
    })
}

#[lingxia::host("downloads.getSettings")]
fn get_download_settings(app: Arc<LxApp>) -> HostResult<DownloadSettingsResult> {
    download_settings_result(&app)
}

#[lingxia::host("downloads.chooseDirectory")]
async fn choose_download_directory(
    app: Arc<LxApp>,
    mut cancel: HostCancel,
) -> HostResult<DownloadSettingsResult> {
    let current_dir = lingxia_transfer::dir(&app.app_data_dir())
        .to_string_lossy()
        .to_string();
    let app_for_picker = app.clone();
    let result = await_or_cancel(&mut cancel, async move {
        app_for_picker
            .runtime
            .choose_directory(ChooseDirectoryRequest {
                title: Some("Choose Download Folder".to_string()),
                default_path: Some(current_dir),
            })
            .await
            .map_err(|e| map_platform_error("downloads.chooseDirectory", e))
    })
    .await?;

    if !result.canceled
        && let Some(path) = result.paths.first()
    {
        lingxia_transfer::set_dir(&app.app_data_dir(), PathBuf::from(path))
            .map_err(|e| lxapp::LxAppError::Runtime(e.to_string()))?;
    }

    download_settings_result(&app)
}

#[lingxia::host("downloads.resetDirectory")]
fn reset_download_directory(app: Arc<LxApp>) -> HostResult<DownloadSettingsResult> {
    lingxia_transfer::reset_dir(&app.app_data_dir())
        .map_err(|e| lxapp::LxAppError::Runtime(e.to_string()))?;
    download_settings_result(&app)
}

pub(crate) fn register() {
    crate::register_hosts![
        get_app_info,
        get_download_settings,
        choose_download_directory,
        reset_download_directory,
    ];
}
