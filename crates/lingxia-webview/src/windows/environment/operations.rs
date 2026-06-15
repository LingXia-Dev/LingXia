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
    WebViewError::WebView(format!("{err}"))
}
