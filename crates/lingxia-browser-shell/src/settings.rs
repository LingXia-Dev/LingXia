use crate::host::{HostCancel, HostResult, StreamContext, await_or_cancel};
use crate::platform_error::map_platform_error;
use lingxia_app_context::app_config;
use lingxia_service::file::ChooseDirectoryRequest;
use lxapp::LxApp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::broadcast;

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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LanguageSettingsResult {
    language: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetLanguageInput {
    language: String,
}

fn language_channel() -> &'static broadcast::Sender<LanguageSettingsResult> {
    static CHANNEL: OnceLock<broadcast::Sender<LanguageSettingsResult>> = OnceLock::new();
    CHANNEL.get_or_init(|| broadcast::channel(16).0)
}

/// Native chrome hook fired after the display language changes, so platform
/// shells can re-render translated labels (webui pages follow the
/// `settings.watchLanguage` stream instead).
static LANGUAGE_CHANGE_LISTENER: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

pub fn set_display_language_change_listener(listener: Box<dyn Fn() + Send + Sync>) {
    let _ = LANGUAGE_CHANGE_LISTENER.set(listener);
}

fn language_settings_result(app: &LxApp) -> HostResult<LanguageSettingsResult> {
    let language = lingxia_service::settings::display_language(&app.app_data_dir())
        .map_err(|error| lxapp::LxAppError::Runtime(error.to_string()))?;
    Ok(LanguageSettingsResult { language })
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

#[lingxia::native("app.getInfo")]
fn get_app_info(app: Arc<LxApp>) -> HostResult<AppInfo> {
    crate::require_builtin_browser(&app)?;
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

#[lingxia::native("downloads.getSettings")]
fn get_download_settings(app: Arc<LxApp>) -> HostResult<DownloadSettingsResult> {
    crate::require_builtin_browser(&app)?;
    download_settings_result(&app)
}

#[lingxia::native("downloads.chooseDirectory")]
async fn choose_download_directory(
    app: Arc<LxApp>,
    mut cancel: HostCancel,
) -> HostResult<DownloadSettingsResult> {
    crate::require_builtin_browser(&app)?;
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

#[lingxia::native("downloads.resetDirectory")]
fn reset_download_directory(app: Arc<LxApp>) -> HostResult<DownloadSettingsResult> {
    crate::require_builtin_browser(&app)?;
    lingxia_service::downloads::reset_dir(&app.app_data_dir())
        .map_err(|e| lxapp::LxAppError::Runtime(e.to_string()))?;
    download_settings_result(&app)
}

#[lingxia::native("settings.getLanguage")]
fn get_display_language(app: Arc<LxApp>) -> HostResult<LanguageSettingsResult> {
    crate::require_builtin_browser(&app)?;
    language_settings_result(&app)
}

#[lingxia::native("settings.setLanguage")]
fn set_display_language(
    app: Arc<LxApp>,
    input: SetLanguageInput,
) -> HostResult<LanguageSettingsResult> {
    crate::require_builtin_browser(&app)?;
    if input.language != "auto" && input.language != "en-US" && input.language != "zh-CN" {
        return Err(lxapp::LxAppError::InvalidParameter(
            "language must be auto, en-US, or zh-CN".to_string(),
        ));
    }
    let language = (input.language != "auto").then_some(input.language);
    lingxia_service::settings::set_display_language(&app.app_data_dir(), language.as_deref())
        .map_err(|error| lxapp::LxAppError::Runtime(error.to_string()))?;
    lxapp::set_display_language(language.clone());
    if let Some(listener) = LANGUAGE_CHANGE_LISTENER.get() {
        listener();
    }
    let result = LanguageSettingsResult { language };
    let _ = language_channel().send(result.clone());
    Ok(result)
}

#[lingxia::native("settings.watchLanguage", stream)]
async fn watch_display_language(
    app: Arc<LxApp>,
    mut stream: StreamContext<LanguageSettingsResult>,
) -> HostResult<()> {
    crate::require_builtin_browser(&app)?;
    let mut receiver = language_channel().subscribe();
    stream.send(language_settings_result(&app)?)?;
    loop {
        tokio::select! {
            _ = stream.canceled() => return Ok(()),
            received = receiver.recv() => {
                match received {
                    Ok(language) => stream.send(language)?,
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        stream.send(language_settings_result(&app)?)?;
                    }
                    Err(broadcast::error::RecvError::Closed) => return stream.end(()),
                }
            }
        }
    }
}

pub(crate) fn register() {
    lxapp::host::register_host_entry(get_app_info_host());
    lxapp::host::register_host_entry(get_download_settings_host());
    lxapp::host::register_host_entry(choose_download_directory_host());
    lxapp::host::register_host_entry(reset_download_directory_host());
    lxapp::host::register_host_entry(get_display_language_host());
    lxapp::host::register_host_entry(set_display_language_host());
    lxapp::host::register_host_entry(watch_display_language_host());
}
