//! Privacy host routes: browsing-data usage and clearing.
//!
//! Backed by the shared default WebKit data store (`lingxia_webview::data_store`)
//! — every browser tab writes there, so clearing covers the whole in-app
//! browser without needing a live tab. The settings page hides the Privacy
//! section when `privacy.getUsage` reports the platform as unsupported.

use crate::host::HostResult;
use lingxia_webview::WebViewError;
use lxapp::LxApp;
use lxapp::LxAppError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PrivacyUsage {
    /// Stored browser history entries.
    history_entries: u64,
    /// Sites with cached data.
    cache_sites: u64,
    /// Sites storing cookies or other site data.
    site_data_sites: u64,
    /// Total cookie count across all sites.
    cookies: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
enum TimeRange {
    LastHour,
    Last24Hours,
    Last7Days,
    Last4Weeks,
    AllTime,
}

impl TimeRange {
    fn since_ms(&self) -> Option<u64> {
        let duration_ms = match self {
            Self::LastHour => 60 * 60 * 1_000,
            Self::Last24Hours => 24 * 60 * 60 * 1_000,
            Self::Last7Days => 7 * 24 * 60 * 60 * 1_000,
            Self::Last4Weeks => 28 * 24 * 60 * 60 * 1_000,
            Self::AllTime => return None,
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0);
        Some(now.saturating_sub(duration_ms))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClearBrowsingDataInput {
    time_range: TimeRange,
    #[serde(default)]
    history: bool,
    #[serde(default)]
    cache: bool,
    #[serde(default)]
    cookies: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClearBrowsingDataResult {
    history_removed: usize,
}

fn map_webview_error(action: &str, err: WebViewError) -> LxAppError {
    let message = err.to_string();
    if message.contains("not supported on this platform") {
        LxAppError::UnsupportedOperation(format!("{action}: {message}"))
    } else {
        LxAppError::Runtime(format!("{action}: {message}"))
    }
}

#[lingxia::native("privacy.getUsage")]
async fn get_privacy_usage(app: Arc<LxApp>) -> HostResult<PrivacyUsage> {
    let history_entries = crate::history::count_in(&app.app_data_dir())?;
    let cache_sites = lingxia_webview::data_store::cache_site_count()
        .await
        .map_err(|e| map_webview_error("privacy.getUsage", e))?;
    let (site_data_sites, cookies) = lingxia_webview::data_store::site_data_usage()
        .await
        .map_err(|e| map_webview_error("privacy.getUsage", e))?;
    Ok(PrivacyUsage {
        history_entries: history_entries as u64,
        cache_sites: cache_sites as u64,
        site_data_sites: site_data_sites as u64,
        cookies: cookies as u64,
    })
}

#[lingxia::native("privacy.clearCache")]
async fn clear_cache(_app: Arc<LxApp>) -> HostResult<()> {
    lingxia_webview::data_store::clear_cache()
        .await
        .map_err(|e| map_webview_error("privacy.clearCache", e))
}

#[lingxia::native("privacy.clearCookies")]
async fn clear_cookies(_app: Arc<LxApp>) -> HostResult<()> {
    lingxia_webview::data_store::clear_site_data()
        .await
        .map_err(|e| map_webview_error("privacy.clearCookies", e))
}

#[lingxia::native("privacy.clearBrowsingData")]
async fn clear_browsing_data(
    app: Arc<LxApp>,
    input: ClearBrowsingDataInput,
) -> HostResult<ClearBrowsingDataResult> {
    if !input.history && !input.cache && !input.cookies {
        return Err(LxAppError::InvalidParameter(
            "select at least one browsing data category".to_string(),
        ));
    }
    let since_ms = input.time_range.since_ms();
    if input.cache {
        lingxia_webview::data_store::clear_cache_since(since_ms)
            .await
            .map_err(|error| map_webview_error("privacy.clearBrowsingData.cache", error))?;
    }
    if input.cookies {
        lingxia_webview::data_store::clear_site_data_since(since_ms)
            .await
            .map_err(|error| map_webview_error("privacy.clearBrowsingData.cookies", error))?;
    }
    let history_removed = if input.history {
        crate::history::clear_since_in(&app.app_data_dir(), since_ms)?
    } else {
        0
    };
    Ok(ClearBrowsingDataResult { history_removed })
}

pub(crate) fn register() {
    lxapp::host::register_host_entry(get_privacy_usage_host());
    lxapp::host::register_host_entry(clear_cache_host());
    lxapp::host::register_host_entry(clear_cookies_host());
    lxapp::host::register_host_entry(clear_browsing_data_host());
}
