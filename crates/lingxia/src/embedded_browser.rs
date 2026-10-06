//! Host-managed browser sessions whose network access requires a local proxy.

use lingxia_platform::traits::app_runtime::AppRuntime;
use lingxia_webview::{ProxyApplyStatus, ProxyConfig, runtime};

/// Configure a loopback HTTP CONNECT proxy before presenting browser content.
/// Unsupported platforms return an error; callers must not open the page.
/// The proxy remains installed if the local listener stops, so disconnection
/// does not turn into direct access. Strict product pages use separate stores.
pub fn require_local_proxy(port: u16) -> crate::Result<()> {
    let config = ProxyConfig::new("127.0.0.1", port)
        .map_err(|e| crate::Error::invalid_request(e.to_string()))?;
    let report = runtime::apply_proxy_to_current_runtime(Some(config))
        .map_err(|e| crate::Error::invalid_request(e.to_string()))?;
    if report.status != ProxyApplyStatus::Applied {
        return Err(crate::Error::invalid_request("browser proxy unsupported"));
    }
    lingxia_browser::configure_download_proxy(Some(format!("http://127.0.0.1:{port}")));
    Ok(())
}

/// Open an external URL, or the trusted new-tab page for empty input.
pub fn open(url: &str, tab_id: Option<&str>) -> crate::Result<String> {
    let result = if url.is_empty() {
        lingxia_browser::open_trusted(
            crate::browser::native_control_authority()
                .map_err(|e| crate::Error::invalid_request(e.to_string()))?,
            "lingxia://newtab",
            tab_id,
        )
    } else {
        lingxia_browser::open(url, tab_id)
    };
    match result {
        Err(lxapp::LxAppError::ResourceNotFound(_)) if tab_id.is_some() => open(url, None),
        result => result.map_err(|e| crate::Error::invalid_request(e.to_string())),
    }
}

/// Open a tab and bring its native browser controller onscreen.
/// Creating or activating a runtime tab does not present the phone UI.
/// Reopening a tab after the user leaves the browser must present it again.
pub async fn open_and_present(url: &str, tab_id: Option<&str>) -> crate::Result<String> {
    let tab = open(url, tab_id)?;
    crate::runtime::platform()?
        .activate_browser_tab(tab.clone())
        .await
        .map_err(crate::Error::from)?;
    Ok(tab)
}

/// Pause browser downloads without removing the fail-closed WebView proxy.
pub fn block_downloads() {
    lingxia_browser::require_proxy_for_downloads();
}

/// Explicitly use the system network, including an active OS VPN.
pub fn use_system_network() -> crate::Result<()> {
    let report = runtime::apply_proxy_to_current_runtime(None)
        .map_err(|e| crate::Error::invalid_request(e.to_string()))?;
    if report.status != ProxyApplyStatus::Cleared {
        return Err(crate::Error::invalid_request("browser proxy unsupported"));
    }
    lingxia_browser::configure_download_proxy(None);
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadInfo {
    pub id: String,
    pub file_name: String,
    pub status: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub can_retry: bool,
}

/// Download history scoped to the native browser, without URLs, credentials,
/// request headers or filesystem paths crossing into the control page.
pub fn downloads() -> crate::Result<Vec<DownloadInfo>> {
    let runtime = crate::runtime::platform()?;
    let snapshot = lingxia_transfer::snapshot(&runtime.app_data_dir())
        .map_err(|e| crate::Error::internal(e.to_string()))?;
    Ok(snapshot
        .downloads
        .into_iter()
        .filter(|d| d.owner.kind == lingxia_transfer::user_cache::DownloadOwnerKind::Browser)
        .take(100)
        .map(|d| DownloadInfo {
            id: d.task_id,
            file_name: d.file_name,
            status: match d.status {
                lingxia_transfer::DownloadStatus::Downloading => "downloading",
                lingxia_transfer::DownloadStatus::Paused => "paused",
                lingxia_transfer::DownloadStatus::Completed => "completed",
                lingxia_transfer::DownloadStatus::Failed => "failed",
                lingxia_transfer::DownloadStatus::Removed => "removed",
            }
            .into(),
            downloaded_bytes: d.downloaded_bytes,
            total_bytes: d.total_bytes,
            can_retry: d.retry,
        })
        .collect())
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DownloadAction {
    Pause,
    Cancel,
    Resume,
    Retry,
    Remove,
    Share,
}

/// Act on an existing browser download; callers cannot supply arbitrary paths.
pub async fn download_action(id: &str, action: DownloadAction) -> crate::Result<()> {
    let runtime = crate::runtime::platform()?;
    let root = runtime.app_data_dir();
    let record = lingxia_transfer::record(&root, id)
        .map_err(|e| crate::Error::internal(e.to_string()))?
        .filter(|d| d.owner.kind == lingxia_transfer::user_cache::DownloadOwnerKind::Browser)
        .ok_or_else(|| crate::Error::invalid_request("download not found"))?;
    if matches!(action, DownloadAction::Share) {
        if record.status != lingxia_transfer::DownloadStatus::Completed {
            return Err(crate::Error::invalid_request("download is not complete"));
        }
        use lingxia_platform::traits::share::{ShareRequest, ShareService};
        runtime
            .share(ShareRequest {
                title: Some(record.file_name),
                text: None,
                url: None,
                files: vec![record.target_path],
            })
            .await
            .map_err(crate::Error::from)?;
        return Ok(());
    }
    let result = match action {
        DownloadAction::Pause => lingxia_transfer::pause(&root, id),
        DownloadAction::Cancel => lingxia_transfer::cancel(&root, id),
        DownloadAction::Resume => lingxia_transfer::resume(&root, id),
        DownloadAction::Retry => lingxia_transfer::retry(&root, id),
        DownloadAction::Remove => lingxia_transfer::remove(&root, id),
        DownloadAction::Share => unreachable!(),
    };
    result.map_err(|e| crate::Error::invalid_request(e.to_string()))
}
