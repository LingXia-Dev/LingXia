//! Host-managed browser sessions whose network access requires a local proxy.

use lingxia_platform::traits::app_runtime::AppRuntime;
use lingxia_webview::{ProxyApplyStatus, ProxyConfig, runtime};

/// Configure a loopback HTTP CONNECT proxy before presenting browser content.
///
/// Unsupported platforms (anything but Apple, Android and Harmony) return an
/// error; callers must not open the page. The call also routes browser
/// downloads through the same endpoint.
///
/// - Fail-closed: the proxy stays installed if the local listener stops, so
///   disconnection does not turn into direct access. Apple disables WebKit
///   proxy failover and Android installs no direct fallback rule. Harmony's
///   app proxy (`OH_NetConn_SetAppHttpProxy`) states no fallback behaviour;
///   that is not verified.
/// - Scope: on Apple, the browser data stores (the persistent default store
///   and live private-browsing stores); lxapp pages use separate stores and are
///   not proxied. On Android and Harmony the override is app-wide, so every
///   WebView in the app routes through it.
/// - Idempotence: on Apple an unchanged port is a no-op, while a changed port
///   (or switching from [`use_system_network`]) interrupts in-flight loads in
///   every affected tab. Android and Harmony reinstall the override on every
///   call.
/// - Suspension: nothing re-applies the proxy when the app resumes. See
///   [`reapply_local_proxy`].
/// - Concurrency: callable from any thread. On Apple the caller blocks until
///   the main thread runs the apply, so never call it while the main thread
///   waits on the caller. Racing calls settle on the last apply for both
///   WebViews and downloads.
pub fn require_local_proxy(port: u16) -> crate::Result<()> {
    let config = ProxyConfig::new("127.0.0.1", port)
        .map_err(|e| crate::Error::invalid_request(e.to_string()))?;
    let (report, sequence) = runtime::apply_proxy_to_current_runtime_sequenced(Some(config))
        .map_err(|e| crate::Error::invalid_request(e.to_string()))?;
    if report.status != ProxyApplyStatus::Applied {
        return Err(crate::Error::invalid_request("browser proxy unsupported"));
    }
    route_downloads(sequence, Some(format!("http://127.0.0.1:{port}")));
    Ok(())
}

/// Write the proxy set by [`require_local_proxy`] again even though it is
/// unchanged, for a host that rebuilt its listener on the same port (e.g. on
/// resume from suspension).
///
/// On Apple the configuration is written as one replacement, never cleared
/// first, and the write interrupts in-flight loads like a port change. Whether
/// rewriting an identical value makes WebKit reconnect to the new listener is
/// WebKit's behaviour, not an API guarantee: confirm it on a device before
/// relying on it, or move to a fresh port with [`require_local_proxy`].
/// Android and Harmony have no cache, so this equals calling
/// [`require_local_proxy`] again.
///
/// Download routing is left alone: downloads paused by [`block_downloads`]
/// stay paused until [`require_local_proxy`] is called.
///
/// Errors when no local proxy is required (never set, or after
/// [`use_system_network`]) and where the proxy is unsupported.
pub fn reapply_local_proxy() -> crate::Result<()> {
    let report = runtime::reapply_proxy_to_current_runtime()
        .map_err(|e| crate::Error::invalid_request(e.to_string()))?;
    match report.status {
        ProxyApplyStatus::Applied => Ok(()),
        ProxyApplyStatus::Cleared => Err(crate::Error::invalid_request(
            "no browser proxy is required",
        )),
        ProxyApplyStatus::Unsupported => {
            Err(crate::Error::invalid_request("browser proxy unsupported"))
        }
    }
}

/// The newest download routing decision, by WebView apply sequence. A stale
/// apply that lost a race, or one older than a [`block_downloads`], must not
/// overwrite it: downloads would go direct while WebViews stay proxied.
static DOWNLOAD_SEQUENCE: std::sync::Mutex<u64> = std::sync::Mutex::new(0);

fn route_downloads(sequence: u64, proxy: Option<String>) {
    let mut latest = DOWNLOAD_SEQUENCE.lock().unwrap_or_else(|e| e.into_inner());
    if take_newer(&mut latest, sequence) {
        lingxia_browser::configure_download_proxy(proxy);
    }
}

/// Record `sequence` when it is newer than `latest`. Zero (no update) never is.
fn take_newer(latest: &mut u64, sequence: u64) -> bool {
    if sequence <= *latest {
        return false;
    }
    *latest = sequence;
    true
}

/// Applies up to `finished` already ran; none of them may route afterwards.
fn raise_floor(latest: &mut u64, finished: u64) {
    *latest = (*latest).max(finished);
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
/// They resume on the next [`require_local_proxy`] or [`use_system_network`].
pub fn block_downloads() {
    let mut latest = DOWNLOAD_SEQUENCE.lock().unwrap_or_else(|e| e.into_inner());
    raise_floor(&mut latest, runtime::proxy_apply_sequence());
    lingxia_browser::require_proxy_for_downloads();
}

/// Explicitly use the system network, including an active OS VPN.
/// On Apple this interrupts in-flight loads when a proxy was installed.
pub fn use_system_network() -> crate::Result<()> {
    let (report, sequence) = runtime::apply_proxy_to_current_runtime_sequenced(None)
        .map_err(|e| crate::Error::invalid_request(e.to_string()))?;
    if report.status != ProxyApplyStatus::Cleared {
        return Err(crate::Error::invalid_request("browser proxy unsupported"));
    }
    route_downloads(sequence, None);
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

#[cfg(test)]
mod tests {
    use super::{raise_floor, take_newer};

    #[test]
    fn download_routing_keeps_only_the_newest_apply() {
        let mut latest = 0;
        assert!(!take_newer(&mut latest, 0), "a failed apply routes nothing");
        assert!(take_newer(&mut latest, 2));
        assert!(!take_newer(&mut latest, 1), "a stale apply loses the race");
        assert!(!take_newer(&mut latest, 2));
        // block_downloads after apply 3 finished: 3 must not unblock, 4 may.
        raise_floor(&mut latest, 3);
        assert!(!take_newer(&mut latest, 3));
        assert!(take_newer(&mut latest, 4));
        raise_floor(&mut latest, 1);
        assert_eq!(latest, 4, "the floor never moves back");
    }
}
