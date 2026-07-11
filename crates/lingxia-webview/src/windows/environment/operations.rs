//! Per-webview WebView2 operations.

use super::*;

pub(crate) fn set_user_agent(webview: &ICoreWebView2, ua: &str) -> StdResult<()> {
    unsafe {
        let settings = webview
            .Settings()
            .map_err(|err| WebViewError::WebView(format!("Settings failed: {err}")))?;
        let settings2: ICoreWebView2Settings2 = settings
            .cast()
            .map_err(|err| WebViewError::WebView(format!("Settings2 cast failed: {err}")))?;
        let ua = CoTaskMemPWSTR::from(ua);
        settings2
            .SetUserAgent(*ua.as_ref().as_pcwstr())
            .map_err(|err| WebViewError::WebView(format!("SetUserAgent failed: {err}")))?;
    }
    Ok(())
}

pub(crate) fn clear_browsing_data(webview: &ICoreWebView2) -> StdResult<()> {
    let webview13: ICoreWebView2_13 = webview
        .cast()
        .map_err(|err| WebViewError::WebView(format!("WebView profile cast failed: {err}")))?;
    let profile = unsafe {
        webview13
            .Profile()
            .map_err(|err| WebViewError::WebView(format!("Profile failed: {err}")))?
    };
    let profile2: ICoreWebView2Profile2 = profile
        .cast()
        .map_err(|err| WebViewError::WebView(format!("Profile2 cast failed: {err}")))?;

    let (tx, rx) = mpsc::channel();
    unsafe {
        profile2
            .ClearBrowsingDataAll(&ClearBrowsingDataCompletedHandler::create(Box::new(
                move |result| {
                    tx.send(result)
                        .map_err(|_| windows::core::Error::from(E_POINTER))?;
                    Ok(())
                },
            )))
            .map_err(|err| WebViewError::WebView(format!("ClearBrowsingDataAll failed: {err}")))?;
    }

    rx.recv()
        .map_err(|_| WebViewError::WebView("Clear browsing data callback failed".to_string()))?
        .map_err(|err| WebViewError::WebView(format!("Clear browsing data failed: {err}")))
}

pub(crate) fn current_url(webview: &ICoreWebView2) -> StdResult<Option<String>> {
    unsafe {
        let mut uri = PWSTR::null();
        webview
            .Source(&mut uri)
            .map_err(|err| WebViewError::WebView(format!("Source failed: {err}")))?;
        Ok(non_empty(CoTaskMemPWSTR::from(uri).to_string()))
    }
}

pub(crate) enum HistoryDirection {
    Back,
    Forward,
}

pub(crate) fn go_history(webview: &ICoreWebView2, direction: HistoryDirection) -> StdResult<()> {
    unsafe {
        let mut can_go = BOOL::default();
        match direction {
            HistoryDirection::Back => {
                webview
                    .CanGoBack(&mut can_go)
                    .map_err(|err| WebViewError::WebView(format!("CanGoBack failed: {err}")))?;
                if can_go.as_bool() {
                    webview
                        .GoBack()
                        .map_err(|err| WebViewError::WebView(format!("GoBack failed: {err}")))?;
                }
            }
            HistoryDirection::Forward => {
                webview
                    .CanGoForward(&mut can_go)
                    .map_err(|err| WebViewError::WebView(format!("CanGoForward failed: {err}")))?;
                if can_go.as_bool() {
                    webview
                        .GoForward()
                        .map_err(|err| WebViewError::WebView(format!("GoForward failed: {err}")))?;
                }
            }
        }
    }
    Ok(())
}

fn send_once<T>(sent: &std::sync::Arc<std::sync::atomic::AtomicBool>, resp: &Sender<T>, value: T) {
    if !sent.swap(true, std::sync::atomic::Ordering::AcqRel) {
        let _ = resp.send(value);
    }
}

fn cookie_manager(webview: &ICoreWebView2) -> StdResult<ICoreWebView2CookieManager> {
    let webview2: ICoreWebView2_2 = webview
        .cast()
        .map_err(|err| WebViewError::WebView(format!("WebView2_2 cast failed: {err}")))?;
    unsafe { webview2.CookieManager() }
        .map_err(|err| WebViewError::WebView(format!("CookieManager failed: {err}")))
}

fn pwstr_field(
    read: impl FnOnce(*mut PWSTR) -> windows::core::Result<()>,
    what: &str,
) -> StdResult<String> {
    let mut value = PWSTR::null();
    read(&mut value).map_err(|err| WebViewError::WebView(format!("{what} failed: {err}")))?;
    Ok(CoTaskMemPWSTR::from(value).to_string())
}

fn webview_cookie_from_platform(cookie: &ICoreWebView2Cookie) -> StdResult<WebViewCookie> {
    unsafe {
        let name = pwstr_field(|out| cookie.Name(out), "cookie Name")?;
        let value = pwstr_field(|out| cookie.Value(out), "cookie Value")?;
        let domain = pwstr_field(|out| cookie.Domain(out), "cookie Domain")?;
        let path = pwstr_field(|out| cookie.Path(out), "cookie Path")?;
        let mut secure = BOOL::default();
        let _ = cookie.IsSecure(&mut secure);
        let mut http_only = BOOL::default();
        let _ = cookie.IsHttpOnly(&mut http_only);
        let mut session = BOOL::default();
        let _ = cookie.IsSession(&mut session);
        let mut expires_seconds = -1f64;
        let _ = cookie.Expires(&mut expires_seconds);
        let mut same_site_kind = COREWEBVIEW2_COOKIE_SAME_SITE_KIND_LAX;
        let same_site = cookie
            .SameSite(&mut same_site_kind)
            .ok()
            .and(match same_site_kind {
                COREWEBVIEW2_COOKIE_SAME_SITE_KIND_STRICT => Some(WebViewCookieSameSite::Strict),
                COREWEBVIEW2_COOKIE_SAME_SITE_KIND_LAX => Some(WebViewCookieSameSite::Lax),
                COREWEBVIEW2_COOKIE_SAME_SITE_KIND_NONE => Some(WebViewCookieSameSite::None),
                _ => None,
            });
        // WebView2 reports a host-only cookie as a bare domain; a Domain
        // attribute always carries the leading dot.
        let host_only = !domain.starts_with('.');
        Ok(WebViewCookie {
            name,
            value,
            domain,
            path,
            host_only,
            secure: secure.as_bool(),
            http_only: http_only.as_bool(),
            session: session.as_bool(),
            expires_unix_ms: (!session.as_bool() && expires_seconds >= 0.0)
                .then_some((expires_seconds * 1000.0) as i64),
            same_site,
        })
    }
}

/// Collect all cookies of the webview's profile (an empty uri filter means
/// "every cookie"), completing through `resp` when the platform callback runs.
pub(crate) fn start_list_cookies(
    webview: &ICoreWebView2,
    resp: Sender<StdResult<Vec<WebViewCookie>>>,
) {
    let manager = match cookie_manager(webview) {
        Ok(manager) => manager,
        Err(err) => {
            let _ = resp.send(Err(err));
            return;
        }
    };
    let sent = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let handler_sent = sent.clone();
    let handler_resp = resp.clone();
    let handler = GetCookiesCompletedHandler::create(Box::new(move |result, list| {
        let response = result
            .map_err(|err| WebViewError::WebView(format!("GetCookies failed: {err}")))
            .and_then(|()| {
                let list: ICoreWebView2CookieList = list.ok_or_else(|| {
                    WebViewError::WebView("GetCookies returned no cookie list".to_string())
                })?;
                let mut count = 0u32;
                unsafe {
                    list.Count(&mut count).map_err(|err| {
                        WebViewError::WebView(format!("cookie list Count failed: {err}"))
                    })?;
                }
                let mut cookies = Vec::with_capacity(count as usize);
                for index in 0..count {
                    let cookie = unsafe {
                        list.GetValueAtIndex(index).map_err(|err| {
                            WebViewError::WebView(format!("cookie at {index} failed: {err}"))
                        })?
                    };
                    cookies.push(webview_cookie_from_platform(&cookie)?);
                }
                Ok(cookies)
            });
        send_once(&handler_sent, &handler_resp, response);
        Ok(())
    }));
    let started = unsafe { manager.GetCookies(PCWSTR::null(), &handler) };
    if let Err(err) = started {
        send_once(
            &sent,
            &resp,
            Err(WebViewError::WebView(format!("GetCookies failed: {err}"))),
        );
    }
}

/// Host portion of `url` (`scheme://host[:port]/...`), used to derive a
/// host-only cookie domain when the request declares none.
fn cookie_url_host(url: &str) -> Option<String> {
    let rest = url.trim().split_once("://")?.1;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority.rsplit('@').next().unwrap_or(authority);
    let host = host.split(':').next().unwrap_or(host).trim();
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

pub(crate) fn set_cookie(
    webview: &ICoreWebView2,
    request: &WebViewCookieSetRequest,
) -> StdResult<()> {
    if request.name.trim().is_empty() {
        return Err(WebViewError::WebView("cookie name is required".to_string()));
    }
    // Mirror the Apple semantics: an explicit Domain attribute makes a
    // domain cookie (leading dot); otherwise the cookie is host-only for
    // the request URL's host.
    let domain = match request
        .domain
        .as_deref()
        .map(str::trim)
        .filter(|domain| !domain.is_empty())
    {
        Some(domain) => {
            if domain.starts_with('.') {
                domain.to_string()
            } else {
                format!(".{domain}")
            }
        }
        None => cookie_url_host(&request.url)
            .ok_or_else(|| WebViewError::WebView("cookie url or domain is required".to_string()))?,
    };
    let path = if request.path.trim().is_empty() {
        "/"
    } else {
        request.path.as_str()
    };

    let manager = cookie_manager(webview)?;
    unsafe {
        let name = CoTaskMemPWSTR::from(request.name.as_str());
        // CreateCookie rejects an empty value (E_INVALIDARG); create with a
        // placeholder and clear it through SetValue below.
        let create_value = if request.value.is_empty() {
            "placeholder"
        } else {
            request.value.as_str()
        };
        let value = CoTaskMemPWSTR::from(create_value);
        let domain = CoTaskMemPWSTR::from(domain.as_str());
        let path = CoTaskMemPWSTR::from(path);
        let cookie = manager
            .CreateCookie(
                *name.as_ref().as_pcwstr(),
                *value.as_ref().as_pcwstr(),
                *domain.as_ref().as_pcwstr(),
                *path.as_ref().as_pcwstr(),
            )
            .map_err(|err| WebViewError::WebView(format!("CreateCookie failed: {err}")))?;
        if request.value.is_empty() {
            let empty = CoTaskMemPWSTR::from("");
            cookie
                .SetValue(*empty.as_ref().as_pcwstr())
                .map_err(|err| WebViewError::WebView(format!("SetValue failed: {err}")))?;
        }
        let _ = cookie.SetIsSecure(request.secure);
        let _ = cookie.SetIsHttpOnly(request.http_only);
        if let Some(expires_unix_ms) = request.expires_unix_ms {
            let _ = cookie.SetExpires(expires_unix_ms as f64 / 1000.0);
        }
        if let Some(same_site) = request.same_site {
            let kind = match same_site {
                WebViewCookieSameSite::Strict => COREWEBVIEW2_COOKIE_SAME_SITE_KIND_STRICT,
                WebViewCookieSameSite::Lax => COREWEBVIEW2_COOKIE_SAME_SITE_KIND_LAX,
                WebViewCookieSameSite::None => COREWEBVIEW2_COOKIE_SAME_SITE_KIND_NONE,
            };
            let _ = cookie.SetSameSite(kind);
        }
        manager
            .AddOrUpdateCookie(&cookie)
            .map_err(|err| WebViewError::WebView(format!("AddOrUpdateCookie failed: {err}")))
    }
}

/// Deletes cookies matching `name` + `domain` + `path`. The store keys
/// host-only (bare) and Domain-attribute (dotted) cookies separately;
/// callers pass whichever form they listed, so both are deleted.
pub(crate) fn delete_cookie(
    webview: &ICoreWebView2,
    name: &str,
    domain: &str,
    path: &str,
) -> StdResult<()> {
    let manager = cookie_manager(webview)?;
    let path = if path.trim().is_empty() { "/" } else { path };
    let bare = domain.trim().trim_start_matches('.').to_string();
    for candidate in [bare.clone(), format!(".{bare}")] {
        unsafe {
            let name = CoTaskMemPWSTR::from(name);
            let candidate = CoTaskMemPWSTR::from(candidate.as_str());
            let path = CoTaskMemPWSTR::from(path);
            manager
                .DeleteCookiesWithDomainAndPath(
                    *name.as_ref().as_pcwstr(),
                    *candidate.as_ref().as_pcwstr(),
                    *path.as_ref().as_pcwstr(),
                )
                .map_err(|err| {
                    WebViewError::WebView(format!("DeleteCookiesWithDomainAndPath failed: {err}"))
                })?;
        }
    }
    Ok(())
}

pub(crate) fn clear_cookies(webview: &ICoreWebView2) -> StdResult<()> {
    let manager = cookie_manager(webview)?;
    unsafe {
        manager
            .DeleteAllCookies()
            .map_err(|err| WebViewError::WebView(format!("DeleteAllCookies failed: {err}")))
    }
}

/// Invoke a Chrome DevTools Protocol method, completing through `resp` with
/// the raw JSON result when the platform callback runs.
pub(crate) fn start_call_devtools_protocol(
    webview: &ICoreWebView2,
    method: &str,
    params: &str,
    resp: Sender<StdResult<String>>,
) {
    let sent = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let handler_sent = sent.clone();
    let handler_resp = resp.clone();
    let method_name = method.to_string();
    let handler =
        CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |result, return_json| {
            let response = result
                .map(|()| return_json)
                .map_err(|err| WebViewError::WebView(format!("CDP {method_name} failed: {err}")));
            send_once(&handler_sent, &handler_resp, response);
            Ok(())
        }));
    let started = unsafe {
        let method = CoTaskMemPWSTR::from(method);
        let params = CoTaskMemPWSTR::from(params);
        webview.CallDevToolsProtocolMethod(
            *method.as_ref().as_pcwstr(),
            *params.as_ref().as_pcwstr(),
            &handler,
        )
    };
    if let Err(err) = started {
        send_once(
            &sent,
            &resp,
            Err(WebViewError::WebView(format!(
                "CallDevToolsProtocolMethod failed: {err}"
            ))),
        );
    }
}

pub(crate) fn start_capture_preview_png(webview: &ICoreWebView2, resp: Sender<StdResult<Vec<u8>>>) {
    let stream = match unsafe { CreateStreamOnHGlobal(None, true) } {
        Ok(stream) => stream,
        Err(err) => {
            let _ = resp.send(Err(WebViewError::WebView(format!(
                "CreateStreamOnHGlobal failed: {err}"
            ))));
            return;
        }
    };
    let capture_stream = stream.clone();
    let read_stream = stream.clone();
    let start_resp = resp.clone();
    let sent = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let callback_sent = sent.clone();
    let callback = CapturePreviewCompletedHandler::create(Box::new(move |result| {
        let response = match result {
            Ok(()) => {
                let bytes = read_stream_to_end(&read_stream).map_err(|err| {
                    WebViewError::WebView(format!("read screenshot stream failed: {err}"))
                });
                match bytes {
                    Ok(bytes) if bytes.is_empty() => Err(WebViewError::WebView(
                        "WebView2 screenshot stream was empty".to_string(),
                    )),
                    result => result,
                }
            }
            Err(err) => Err(WebViewError::WebView(format!(
                "WebView2 CapturePreview failed: {err}"
            ))),
        };
        send_once(&callback_sent, &resp, response);
        Ok(())
    }));

    let result = unsafe {
        webview.CapturePreview(
            COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
            &capture_stream,
            &callback,
        )
    };
    if let Err(err) = result {
        send_once(
            &sent,
            &start_resp,
            Err(WebViewError::WebView(format!(
                "CapturePreview failed: {err}"
            ))),
        );
    }
}

/// Run `ExecuteScript` with an async completion handler instead of a nested
/// message pump, sending the mapped result through `resp` when it completes.
pub(crate) fn start_execute_script<T: Send + 'static>(
    webview: &ICoreWebView2,
    js: &str,
    resp: Sender<T>,
    map: fn(std::result::Result<String, WebViewScriptError>) -> T,
) {
    let handler_resp = resp.clone();
    let sent = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let handler_sent = sent.clone();
    let handler = ExecuteScriptCompletedHandler::create(Box::new(move |result, json| {
        let outcome = match result {
            Ok(()) => Ok(json),
            Err(err) => Err(WebViewScriptError::Platform(err.to_string())),
        };
        send_once(&handler_sent, &handler_resp, map(outcome));
        Ok(())
    }));

    let started = unsafe {
        let js = CoTaskMemPWSTR::from(js);
        webview.ExecuteScript(*js.as_ref().as_pcwstr(), &handler)
    };
    if let Err(err) = started {
        send_once(
            &sent,
            &resp,
            map(Err(WebViewScriptError::Platform(err.to_string()))),
        );
    }
}

pub(crate) fn decode_script_result(
    raw: &str,
) -> std::result::Result<serde_json::Value, WebViewScriptError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(serde_json::Value::Null);
    }
    serde_json::from_str(trimmed).map_err(|err| {
        WebViewScriptError::Platform(format!(
            "WebView2 returned invalid JavaScript result JSON: {err}; raw={trimmed}"
        ))
    })
}

pub(crate) fn read_stream_to_end(stream: &IStream) -> WinResult<Vec<u8>> {
    unsafe {
        let _ = stream.Seek(0, STREAM_SEEK_SET, None);
    }

    let mut result = Vec::new();
    let mut buffer = [0u8; 8192];

    loop {
        let mut bytes_read = 0u32;
        unsafe {
            stream
                .Read(
                    buffer.as_mut_ptr().cast(),
                    buffer.len() as u32,
                    Some(&mut bytes_read),
                )
                .ok()?;
        }

        if bytes_read == 0 {
            break;
        }

        result.extend_from_slice(&buffer[..bytes_read as usize]);
    }

    Ok(result)
}

pub(crate) fn map_webview2_error(err: webview2_com::Error) -> WebViewError {
    if is_webview2_runtime_missing(&err) {
        return WebViewError::WebView(format!(
            "Microsoft Edge WebView2 Runtime is required to run LingXia Windows apps. \
             Install the Evergreen WebView2 Runtime and try again. Original error: {err}"
        ));
    }
    WebViewError::WebView(format!("WebView2 operation failed: {err}"))
}

pub(crate) fn clear_profile_data(
    webview: &ICoreWebView2,
    kind: super::super::data_store::BrowsingDataKind,
    since_unix_ms: Option<u64>,
) -> StdResult<()> {
    let webview13: ICoreWebView2_13 = webview
        .cast()
        .map_err(|err| WebViewError::WebView(format!("WebView profile cast failed: {err}")))?;
    let profile = unsafe {
        webview13
            .Profile()
            .map_err(|err| WebViewError::WebView(format!("Profile failed: {err}")))?
    };
    let profile2: ICoreWebView2Profile2 = profile
        .cast()
        .map_err(|err| WebViewError::WebView(format!("Profile2 cast failed: {err}")))?;
    let data_kinds = match kind {
        super::super::data_store::BrowsingDataKind::Cache => {
            COREWEBVIEW2_BROWSING_DATA_KINDS_DISK_CACHE
                | COREWEBVIEW2_BROWSING_DATA_KINDS_CACHE_STORAGE
        }
        super::super::data_store::BrowsingDataKind::SiteData => {
            COREWEBVIEW2_BROWSING_DATA_KINDS_COOKIES
                | COREWEBVIEW2_BROWSING_DATA_KINDS_ALL_DOM_STORAGE
                | COREWEBVIEW2_BROWSING_DATA_KINDS_SERVICE_WORKERS
        }
    };
    let (tx, rx) = mpsc::channel();
    let handler = ClearBrowsingDataCompletedHandler::create(Box::new(move |result| {
        tx.send(result)
            .map_err(|_| windows::core::Error::from(E_POINTER))?;
        Ok(())
    }));
    unsafe {
        if let Some(since_unix_ms) = since_unix_ms {
            let end = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_secs_f64())
                .unwrap_or(f64::MAX);
            profile2.ClearBrowsingDataInTimeRange(
                data_kinds,
                since_unix_ms as f64 / 1_000.0,
                end,
                &handler,
            )
        } else {
            profile2.ClearBrowsingData(data_kinds, &handler)
        }
        .map_err(|err| WebViewError::WebView(format!("ClearBrowsingData failed: {err}")))?;
    }
    rx.recv()
        .map_err(|_| WebViewError::WebView("Clear browsing data callback failed".to_string()))?
        .map_err(|err| WebViewError::WebView(format!("Clear browsing data failed: {err}")))
}

fn is_webview2_runtime_missing(err: &webview2_com::Error) -> bool {
    const HRESULT_FROM_WIN32_ERROR_FILE_NOT_FOUND: i32 = 0x80070002u32 as i32;
    matches!(
        err,
        webview2_com::Error::WindowsError(err)
            if err.code().0 == HRESULT_FROM_WIN32_ERROR_FILE_NOT_FOUND
    )
}
