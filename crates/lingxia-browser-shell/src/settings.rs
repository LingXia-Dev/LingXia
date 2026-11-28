use crate::host::{HostCancel, await_or_cancel};
use lingxia_host_config::app_config;
use lingxia_service::file::PickDirectoryRequest;
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

fn download_settings_result(app: &LxApp) -> lingxia_error::Result<DownloadSettingsResult> {
    let effective = lingxia_service::downloads::dir(&app.app_data_dir());
    let configured = lingxia_service::settings::download_dir(&app.app_data_dir())
        .map_err(|e| lingxia_error::Error::internal(e.to_string()))?;
    Ok(DownloadSettingsResult {
        download_dir: effective.to_string_lossy().to_string(),
        uses_default_dir: configured.is_none(),
        // TODO: replace this hardcoded platform check with an AppRuntime
        // capability query (directory-picker support). Windows dialog support
        // is unverified, so behavior is intentionally left unchanged for now.
        can_choose_directory: cfg!(target_os = "macos"),
    })
}

#[lingxia::framework_native("app.getInfo", audience = "control-surface-only")]
fn get_app_info(_app: Arc<LxApp>) -> lingxia_error::Result<AppInfo> {
    let (product_name, version) = match app_config() {
        // About is user-facing, so the name follows the display language the
        // same way the launcher entry does. `productName` is the fallback
        // inside `productNames`, not the string to show.
        Some(cfg) => (
            lingxia_host_config::product_name()
                .unwrap_or(cfg.product_name.as_str())
                .to_string(),
            cfg.product_version.clone(),
        ),
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

#[lingxia::framework_native("downloads.getSettings", audience = "control-surface-only")]
fn get_download_settings(app: Arc<LxApp>) -> lingxia_error::Result<DownloadSettingsResult> {
    download_settings_result(&app)
}

#[lingxia::framework_native("downloads.chooseDirectory", audience = "control-surface-only")]
async fn choose_download_directory(
    app: Arc<LxApp>,
    mut cancel: HostCancel,
) -> lingxia_error::Result<DownloadSettingsResult> {
    let current_dir = lingxia_service::downloads::dir(&app.app_data_dir())
        .to_string_lossy()
        .to_string();
    let app_for_picker = app.clone();
    let result = await_or_cancel(&mut cancel, async move {
        lingxia_service::file::pick_directory(
            &*app_for_picker.runtime,
            PickDirectoryRequest {
                title: Some("Choose Download Folder".to_string()),
                default_path: Some(current_dir),
                owner: app_for_picker.dialog_owner(),
            },
        )
        .await
    })
    .await?;

    if !result.canceled
        && let Some(path) = result.paths().next()
    {
        lingxia_service::downloads::set_dir(&app.app_data_dir(), PathBuf::from(path))
            .map_err(|e| lingxia_error::Error::internal(e.to_string()))?;
    }

    download_settings_result(&app)
}

#[lingxia::framework_native("downloads.resetDirectory", audience = "control-surface-only")]
fn reset_download_directory(app: Arc<LxApp>) -> lingxia_error::Result<DownloadSettingsResult> {
    lingxia_service::downloads::reset_dir(&app.app_data_dir())
        .map_err(|e| lingxia_error::Error::internal(e.to_string()))?;
    download_settings_result(&app)
}
