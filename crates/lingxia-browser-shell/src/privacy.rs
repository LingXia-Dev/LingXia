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
    site_data: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClearBrowsingDataResult {
    history_removed: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClearSiteDataInput {
    tab_id: String,
    #[serde(default)]
    cache: bool,
    #[serde(default)]
    site_data: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClearSiteDataResult {
    cache_cleared: bool,
    site_data_cleared: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SiteDataContextInput {
    tab_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SiteDataContext {
    url: String,
    host: String,
}

fn map_webview_error(action: &str, err: WebViewError) -> LxAppError {
    match err {
        WebViewError::Unsupported(_) => {
            LxAppError::UnsupportedOperation(format!("{action}: {err}"))
        }
        _ => LxAppError::Runtime(format!("{action}: {err}")),
    }
}

#[lingxia::native("privacy.getUsage")]
async fn get_privacy_usage(app: Arc<LxApp>) -> HostResult<PrivacyUsage> {
    crate::require_builtin_browser(&app)?;
    let history_entries = crate::history::count_in(&app.app_data_dir())?;
    let cache_sites = lingxia_webview::data_store::cache_site_count()
        .await
        .map_err(|e| map_webview_error("privacy.getUsage", e))?;
    let usage = lingxia_webview::data_store::site_data_usage()
        .await
        .map_err(|e| map_webview_error("privacy.getUsage", e))?;
    Ok(PrivacyUsage {
        history_entries: history_entries as u64,
        cache_sites: cache_sites as u64,
        site_data_sites: usage.sites as u64,
        cookies: usage.cookies as u64,
    })
}

#[lingxia::native("privacy.clearCache")]
async fn clear_cache(app: Arc<LxApp>) -> HostResult<()> {
    crate::require_builtin_browser(&app)?;
    lingxia_webview::data_store::clear_cache(None)
        .await
        .map_err(|e| map_webview_error("privacy.clearCache", e))
}

#[lingxia::native("privacy.clearAllSiteData")]
async fn clear_all_site_data(app: Arc<LxApp>) -> HostResult<()> {
    crate::require_builtin_browser(&app)?;
    lingxia_webview::data_store::clear_all_site_data(None)
        .await
        .map_err(|e| map_webview_error("privacy.clearAllSiteData", e))
}

#[lingxia::native("privacy.clearBrowsingData")]
async fn clear_browsing_data(
    app: Arc<LxApp>,
    input: ClearBrowsingDataInput,
) -> HostResult<ClearBrowsingDataResult> {
    crate::require_builtin_browser(&app)?;
    if !input.history && !input.cache && !input.site_data {
        return Err(LxAppError::InvalidParameter(
            "select at least one browsing data category".to_string(),
        ));
    }
    // Clearing runs sequentially; when a later category fails, say which
    // earlier ones already succeeded so the error is not misread as "nothing
    // was cleared".
    fn with_progress(cleared: &[&str], error: LxAppError) -> LxAppError {
        if cleared.is_empty() {
            error
        } else {
            LxAppError::Runtime(format!("{} cleared; {error}", cleared.join(", ")))
        }
    }
    let since_ms = input.time_range.since_ms();
    let mut cleared: Vec<&str> = Vec::new();
    if input.cache {
        lingxia_webview::data_store::clear_cache(since_ms)
            .await
            .map_err(|error| {
                with_progress(
                    &cleared,
                    map_webview_error("privacy.clearBrowsingData.cache", error),
                )
            })?;
        cleared.push("cache");
    }
    if input.site_data {
        lingxia_webview::data_store::clear_all_site_data(since_ms)
            .await
            .map_err(|error| {
                with_progress(
                    &cleared,
                    map_webview_error("privacy.clearBrowsingData.siteData", error),
                )
            })?;
        cleared.push("site data");
    }
    let history_removed = if input.history {
        crate::history::clear_since_in(&app.app_data_dir(), since_ms)
            .map_err(|error| with_progress(&cleared, error))?
    } else {
        0
    };
    Ok(ClearBrowsingDataResult { history_removed })
}

/// Clears data for the current site only, then reloads the tab so the page
/// reflects the cleared state.
#[lingxia::native("privacy.clearSiteData")]
async fn clear_site_data(
    app: Arc<LxApp>,
    input: ClearSiteDataInput,
) -> HostResult<ClearSiteDataResult> {
    crate::require_builtin_browser(&app)?;
    if !input.cache && !input.site_data {
        return Err(LxAppError::InvalidParameter(
            "select site data or cache to clear".to_string(),
        ));
    }
    let result = lingxia_browser::clear_site_data(
        &input.tab_id,
        lingxia_webview::ClearSiteDataOptions {
            cache: input.cache,
            site_data: input.site_data,
        },
    )
    .await
    .map_err(|error| match error {
        // Bad tab state is caller input, not a runtime fault — matches
        // getSiteDataContext's mapping for the same conditions.
        lingxia_browser::BrowserAutomationError::NativeInput(message) => {
            LxAppError::InvalidParameter(message)
        }
        other => LxAppError::Runtime(format!("privacy.clearSiteData: {other}")),
    })?;
    lingxia_browser::reload(&input.tab_id)
        .map_err(|error| LxAppError::Runtime(format!("privacy.clearSiteData.reload: {error}")))?;
    Ok(ClearSiteDataResult {
        cache_cleared: result.cache_cleared,
        site_data_cleared: result.site_data_cleared,
    })
}

#[lingxia::native("privacy.getSiteDataContext")]
async fn get_site_data_context(
    app: Arc<LxApp>,
    input: SiteDataContextInput,
) -> HostResult<SiteDataContext> {
    crate::require_builtin_browser(&app)?;
    let url = lingxia_browser::current_url(&input.tab_id)
        .await
        .map_err(|error| LxAppError::Runtime(format!("privacy.getSiteDataContext: {error}")))?
        .ok_or_else(|| {
            LxAppError::InvalidParameter("current tab has no website URL".to_string())
        })?;
    let uri = url.parse::<http::Uri>().map_err(|_| {
        LxAppError::InvalidParameter("current tab URL is not a website".to_string())
    })?;
    if !matches!(uri.scheme_str(), Some("http" | "https")) {
        return Err(LxAppError::InvalidParameter(
            "current tab URL is not a website".to_string(),
        ));
    }
    let host = uri
        .host()
        .filter(|host| !host.is_empty())
        .ok_or_else(|| LxAppError::InvalidParameter("current tab URL has no host".to_string()))?
        .to_string();
    Ok(SiteDataContext { url, host })
}

pub(crate) fn register() {
    lxapp::host::register_host_entry(get_privacy_usage_host());
    lxapp::host::register_host_entry(clear_cache_host());
    lxapp::host::register_host_entry(clear_all_site_data_host());
    lxapp::host::register_host_entry(clear_browsing_data_host());
    lxapp::host::register_host_entry(clear_site_data_host());
    lxapp::host::register_host_entry(get_site_data_context_host());
}
