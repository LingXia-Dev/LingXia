//! Global WebKit website-data operations on the shared default data store.
//!
//! Browser tabs (BrowserRelaxed policy) share `WKWebsiteDataStore.default`,
//! so privacy actions ("clear cache", "clear cookies & site data") operate on
//! the store directly — no live webview needed. All WK calls are dispatched
//! to the main thread and complete through a oneshot, mirroring the per-view
//! patterns in `webview.rs`.

use crate::WebViewError;
use block2::StackBlock;
use dispatch2::DispatchQueue;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_foundation::{NSArray, NSDate, NSHTTPCookie, NSSet, NSString};
use objc2_web_kit::{
    WKWebsiteDataRecord, WKWebsiteDataStore, WKWebsiteDataTypeCookies, WKWebsiteDataTypeDiskCache,
    WKWebsiteDataTypeFetchCache, WKWebsiteDataTypeIndexedDBDatabases,
    WKWebsiteDataTypeLocalStorage, WKWebsiteDataTypeMemoryCache,
    WKWebsiteDataTypeOfflineWebApplicationCache, WKWebsiteDataTypeSessionStorage,
    WKWebsiteDataTypeWebSQLDatabases,
};
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time::timeout;

const DATA_STORE_TIMEOUT: Duration = Duration::from_secs(10);

/// Regenerable caches — clearing never logs anyone out.
fn cache_types() -> Retained<NSSet<NSString>> {
    // SAFETY: WebKit's data-type constants are immutable NSString statics.
    unsafe {
        NSSet::from_slice(&[
            WKWebsiteDataTypeDiskCache,
            WKWebsiteDataTypeMemoryCache,
            WKWebsiteDataTypeFetchCache,
            WKWebsiteDataTypeOfflineWebApplicationCache,
        ])
    }
}

/// Cookies plus site state (Chrome's "cookies and other site data").
fn site_data_types() -> Retained<NSSet<NSString>> {
    // SAFETY: WebKit's data-type constants are immutable NSString statics.
    unsafe {
        NSSet::from_slice(&[
            WKWebsiteDataTypeCookies,
            WKWebsiteDataTypeSessionStorage,
            WKWebsiteDataTypeLocalStorage,
            WKWebsiteDataTypeWebSQLDatabases,
            WKWebsiteDataTypeIndexedDBDatabases,
        ])
    }
}

fn send_once<T>(state: &Arc<Mutex<Option<oneshot::Sender<T>>>>, value: T) {
    if let Some(sender) = state.lock().ok().and_then(|mut guard| guard.take()) {
        let _ = sender.send(value);
    }
}

async fn await_store_op<T: Send + 'static>(
    rx: oneshot::Receiver<Result<T, WebViewError>>,
    what: &str,
) -> Result<T, WebViewError> {
    match timeout(DATA_STORE_TIMEOUT, rx).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err(WebViewError::WebView(format!("{what} was canceled"))),
        Err(_) => Err(WebViewError::WebView(format!("{what} timed out"))),
    }
}

fn count_records(
    types: impl Fn() -> Retained<NSSet<NSString>> + Send + 'static,
    tx: oneshot::Sender<Result<usize, WebViewError>>,
) {
    DispatchQueue::main().exec_async(move || unsafe {
        let Some(mtm) = MainThreadMarker::new() else {
            let _ = tx.send(Err(WebViewError::WebView("Not on main thread".to_string())));
            return;
        };
        let store = WKWebsiteDataStore::defaultDataStore(mtm);
        let tx_state = Arc::new(Mutex::new(Some(tx)));
        let tx_for_block = Arc::clone(&tx_state);
        let completion = StackBlock::new(move |records: NonNull<NSArray<WKWebsiteDataRecord>>| {
            send_once(&tx_for_block, Ok(records.as_ref().count()));
        })
        .copy();
        store.fetchDataRecordsOfTypes_completionHandler(&types(), &completion);
    });
}

/// Count of sites with cached data in the default store.
pub async fn cache_site_count() -> Result<usize, WebViewError> {
    let (tx, rx) = oneshot::channel();
    count_records(cache_types, tx);
    await_store_op(rx, "cache usage query").await
}

/// Count of sites with cookies/site data, plus the total cookie count.
pub async fn site_data_usage() -> Result<(usize, usize), WebViewError> {
    let (tx, rx) = oneshot::channel();
    count_records(site_data_types, tx);
    let sites = await_store_op(rx, "site data usage query").await?;

    let (tx, rx) = oneshot::channel::<Result<usize, WebViewError>>();
    DispatchQueue::main().exec_async(move || unsafe {
        let Some(mtm) = MainThreadMarker::new() else {
            let _ = tx.send(Err(WebViewError::WebView("Not on main thread".to_string())));
            return;
        };
        let cookie_store = WKWebsiteDataStore::defaultDataStore(mtm).httpCookieStore();
        let tx_state = Arc::new(Mutex::new(Some(tx)));
        let tx_for_block = Arc::clone(&tx_state);
        let completion = StackBlock::new(move |cookies: NonNull<NSArray<NSHTTPCookie>>| {
            send_once(&tx_for_block, Ok(cookies.as_ref().count()));
        })
        .copy();
        cookie_store.getAllCookies(&completion);
    });
    let cookies = await_store_op(rx, "cookie count query").await?;
    Ok((sites, cookies))
}

fn clear_types(
    types: impl Fn() -> Retained<NSSet<NSString>> + Send + 'static,
    since_unix_ms: Option<u64>,
) -> oneshot::Receiver<Result<(), WebViewError>> {
    let (tx, rx) = oneshot::channel();
    DispatchQueue::main().exec_async(move || unsafe {
        let Some(mtm) = MainThreadMarker::new() else {
            let _ = tx.send(Err(WebViewError::WebView("Not on main thread".to_string())));
            return;
        };
        let store = WKWebsiteDataStore::defaultDataStore(mtm);
        let tx_state = Arc::new(Mutex::new(Some(tx)));
        let tx_for_block = Arc::clone(&tx_state);
        let completion = StackBlock::new(move || {
            send_once(&tx_for_block, Ok(()));
        })
        .copy();
        let since = since_unix_ms
            .map(|milliseconds| NSDate::dateWithTimeIntervalSince1970(milliseconds as f64 / 1000.0))
            .unwrap_or_else(NSDate::distantPast);
        store.removeDataOfTypes_modifiedSince_completionHandler(&types(), &since, &completion);
    });
    rx
}

/// Clear all regenerable caches in the default store.
pub async fn clear_cache() -> Result<(), WebViewError> {
    clear_cache_since(None).await
}

pub async fn clear_cache_since(since_unix_ms: Option<u64>) -> Result<(), WebViewError> {
    let rx = clear_types(cache_types, since_unix_ms);
    await_store_op(rx, "clear cache").await
}

/// Clear cookies and site data (logs sites out) in the default store.
pub async fn clear_site_data() -> Result<(), WebViewError> {
    clear_site_data_since(None).await
}

pub async fn clear_site_data_since(since_unix_ms: Option<u64>) -> Result<(), WebViewError> {
    let rx = clear_types(site_data_types, since_unix_ms);
    await_store_op(rx, "clear cookies & site data").await
}
