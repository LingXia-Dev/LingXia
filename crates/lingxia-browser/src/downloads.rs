//! Browser-owned downloads: starting tab downloads, retrying persisted
//! downloads, and mapping runtime errors into the downloads error model.

use crate::BUILTIN_BROWSER_APPID;
use crate::policy::extract_url_scheme;
use crate::tabs::{
    browser_tab_exists, browser_tab_info, ensure_browser_lxapp, normalize_optional_string,
    normalize_runtime_tab_id,
};
use lingxia_platform::traits::app_runtime::AppRuntime;
use lingxia_transfer as transfer;
use lingxia_webview::DownloadRequest;
use lxapp::{LxApp, LxAppError, publish_app_event};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

fn publish_browser_download_event(event_name: &str, payload: serde_json::Value) {
    let payload_str = Some(payload.to_string());
    let _ = publish_app_event(BUILTIN_BROWSER_APPID, event_name, payload_str);
}

pub(crate) async fn browser_download_resource(
    owner: Arc<LxApp>,
    tab_id: String,
    request: DownloadRequest,
) {
    let task_id = Uuid::new_v4().to_string();
    let cancel_rx = transfer::runtime::register_active_download(&task_id);
    let task = transfer::runtime::DownloadTask::for_browser(
        request,
        transfer::runtime::browser_download_root(&owner.runtime.app_data_dir()),
        Some(rong::get_user_agent()),
    )
    .with_browser_persistence(owner.runtime.app_data_dir(), task_id.clone());
    let tab_id_for_event = tab_id.clone();

    let result = transfer::runtime::run_browser_download_task(
        task,
        &task_id,
        &tab_id_for_event,
        cancel_rx,
        |event_name, payload| {
            if let Err(err) = transfer::runtime::record_bridge_event(
                &owner.runtime.app_data_dir(),
                event_name,
                &payload,
            ) {
                lxapp::warn!(
                    "[InternalBrowser] failed to record download event task_id={} event={} error={}",
                    task_id,
                    event_name,
                    err
                );
            }
            publish_browser_download_event(event_name, payload);
        },
    )
    .await;
    transfer::runtime::unregister_active_download(&task_id);
    if let Err(err) = result {
        if err.error == "Download paused" {
            return;
        }
        lxapp::warn!(
            "[InternalBrowser] download task failed tab_id={} url={} reason={}",
            tab_id,
            err.url,
            err.error
        );
    }
}

fn map_lxapp_error_to_downloads(err: LxAppError) -> transfer::DownloadsError {
    match err {
        LxAppError::InvalidParameter(message) => transfer::DownloadsError::InvalidParameter(message),
        LxAppError::ResourceNotFound(message) => transfer::DownloadsError::ResourceNotFound(message),
        LxAppError::UnsupportedOperation(message) => {
            transfer::DownloadsError::UnsupportedOperation(message)
        }
        LxAppError::IoError(message)
        | LxAppError::Runtime(message)
        | LxAppError::ChannelError(message)
        | LxAppError::ResourceExhausted(message)
        | LxAppError::Bridge(message)
        | LxAppError::RongJS(message)
        | LxAppError::PluginNotConfigured(message)
        | LxAppError::PluginDownloadFailed(message)
        | LxAppError::InvalidJsonFile(message)
        | LxAppError::WebView(message) => transfer::DownloadsError::Runtime(message),
        LxAppError::RongJSHost { code, message, .. } => {
            transfer::DownloadsError::Runtime(format!("{code}: {message}"))
        }
    }
}

pub(crate) fn retry_browser_owned_download(task_id: &str) -> transfer::Result<()> {
    let owner = ensure_browser_lxapp().map_err(map_lxapp_error_to_downloads)?;
    let app_data_dir = owner.runtime.app_data_dir();
    let record = transfer::runtime::get_record(&app_data_dir, task_id)?.ok_or_else(|| {
        transfer::DownloadsError::ResourceNotFound(format!("download not found: {task_id}"))
    })?;
    if !matches!(
        record.status,
        transfer::DownloadStatus::Failed | transfer::DownloadStatus::Paused
    ) {
        return Err(transfer::DownloadsError::UnsupportedOperation(
            "download is not retryable".to_string(),
        ));
    }
    if !record.retry {
        return Err(transfer::DownloadsError::UnsupportedOperation(
            "download cannot be retried".to_string(),
        ));
    }
    if transfer::runtime::has_active_download(task_id) {
        return Err(transfer::DownloadsError::UnsupportedOperation(
            "download is already active".to_string(),
        ));
    }

    let request_context = transfer::runtime::get_request_context(&app_data_dir, task_id)?
        .ok_or_else(|| {
            transfer::DownloadsError::UnsupportedOperation(
                "download retry context is unavailable".to_string(),
            )
        })?;

    if matches!(
        record.owner.kind,
        transfer::user_cache::DownloadOwnerKind::LxApp
    ) {
        let task_id_owned = task_id.to_string();
        let app_data_dir_clone = app_data_dir.clone();
        let owner_appid = record.owner.appid.clone();
        let url = record.url.clone();
        let headers = request_context.headers.clone();
        let user_agent = request_context.user_agent.clone();
        let target_path = PathBuf::from(&record.target_path);
        let behavior = request_context.behavior;

        rong::RongExecutor::global().spawn(async move {
            let persistence = transfer::user_cache::DownloadPersistence::new(
                app_data_dir_clone.clone(),
                task_id_owned.clone(),
                transfer::user_cache::DownloadOwner {
                    kind: transfer::user_cache::DownloadOwnerKind::LxApp,
                    appid: owner_appid,
                    page_path: None,
                    tab_id: None,
                },
                true,
            );
            let result = transfer::user_cache::download_to_path_with_behavior(
                Some(persistence),
                target_path,
                transfer::user_cache::UserCacheDownloadRequest { url, headers },
                user_agent,
                behavior,
                |_| {},
            )
            .await;
            if let Err(err) = result {
                if err.error == "Download paused" {
                    return;
                }
                lxapp::warn!(
                    "[Downloads] retry download task failed task_id={} url={} reason={}",
                    task_id_owned,
                    err.url,
                    err.error
                );
            }
        });

        return Ok(());
    }

    let request = DownloadRequest {
        url: record.url.clone(),
        user_agent: request_context.user_agent.clone(),
        content_disposition: None,
        mime_type: record.mime_type.clone(),
        content_length: record.total_bytes,
        suggested_filename: request_context
            .suggested_filename
            .clone()
            .or_else(|| Some(record.file_name.clone())),
        source_page_url: request_context.source_page_url.clone(),
        cookie: request_context.cookie.clone(),
    };
    let cancel_rx = transfer::runtime::register_active_download(task_id);
    let task = transfer::runtime::DownloadTask::for_browser(
        request,
        transfer::runtime::browser_download_root(&app_data_dir),
        Some(rong::get_user_agent()),
    )
    .with_target_path(PathBuf::from(&record.target_path))
    .with_browser_persistence(app_data_dir.clone(), task_id.to_string())
    .with_behavior(request_context.behavior);
    let owner_clone = owner.clone();
    let task_id_owned = task_id.to_string();
    let tab_id = record.tab_id.clone();

    rong::RongExecutor::global().spawn(async move {
        let result = transfer::runtime::run_browser_download_task(
            task,
            &task_id_owned,
            &tab_id,
            cancel_rx,
            |event_name, payload| {
                if let Err(err) = transfer::runtime::record_bridge_event(
                    &owner_clone.runtime.app_data_dir(),
                    event_name,
                    &payload,
                ) {
                    lxapp::warn!(
                        "[InternalBrowser] failed to record retry download event task_id={} event={} error={}",
                        task_id_owned,
                        event_name,
                        err
                    );
                }
                publish_browser_download_event(event_name, payload);
            },
        )
        .await;
        transfer::runtime::unregister_active_download(&task_id_owned);
        if let Err(err) = result {
            if err.error == "Download paused" {
                return;
            }
            lxapp::warn!(
                "[InternalBrowser] retry download task failed task_id={} url={} reason={}",
                task_id_owned,
                err.url,
                err.error
            );
        }
    });

    Ok(())
}

pub(crate) fn start_native_browser_download(
    tab_id: &str,
    url: &str,
    user_agent: Option<&str>,
    suggested_filename: Option<&str>,
    source_page_url: Option<&str>,
    cookie: Option<&str>,
) -> Result<(), LxAppError> {
    let normalized_tab_id = normalize_runtime_tab_id(tab_id).ok_or_else(|| {
        LxAppError::InvalidParameter("tab_id must be a valid runtime browser tab id".to_string())
    })?;

    let normalized_url = url.trim();
    if normalized_url.is_empty() {
        return Err(LxAppError::InvalidParameter("url is required".to_string()));
    }
    if !matches!(
        extract_url_scheme(normalized_url).as_deref(),
        Some("http" | "https")
    ) {
        return Err(LxAppError::InvalidParameter(
            "browser download url must be http(s)".to_string(),
        ));
    }

    let source_page_url = normalize_optional_string(source_page_url)
        .or_else(|| browser_tab_info(&normalized_tab_id).and_then(|info| info.current_url));
    if !browser_tab_exists(&normalized_tab_id) {
        return Err(LxAppError::ResourceNotFound(format!(
            "browser tab not found: {}",
            normalized_tab_id
        )));
    }

    let owner = ensure_browser_lxapp()?;
    let request = DownloadRequest {
        url: normalized_url.to_string(),
        user_agent: normalize_optional_string(user_agent),
        content_disposition: None,
        mime_type: None,
        content_length: None,
        suggested_filename: normalize_optional_string(suggested_filename),
        source_page_url,
        cookie: normalize_optional_string(cookie),
    };

    rong::RongExecutor::global().spawn({
        let owner = owner.clone();
        let tab_id = normalized_tab_id.clone();
        async move {
            browser_download_resource(owner, tab_id, request).await;
        }
    });

    Ok(())
}
