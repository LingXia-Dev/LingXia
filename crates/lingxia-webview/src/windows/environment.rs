//! WebView2 environment/controller creation and per-webview
//! operations (settings, scripts, history, capture).

use super::*;

/// Custom schemes registered on every WebView2 environment.
///
/// All webviews share one user data folder, and WebView2 fails environment
/// creation with 0x8007139F when two environments over the same folder carry
/// different options — so registration must be identical everywhere and is
/// the fixed union of the schemes the runtime serves. Which schemes a given
/// webview actually handles is still decided per webview by its
/// `WebResourceRequested` filters (see `registered_request_schemes`).
const WEBVIEW2_CUSTOM_SCHEME_REGISTRATIONS: &[&str] = &["lingxia", "lx"];

pub(crate) fn create_environment(
    _effective_options: &EffectiveWebViewCreateOptions,
) -> StdResult<ICoreWebView2Environment> {
    let options = CoreWebView2EnvironmentOptions::default();
    let custom_schemes: Vec<String> = WEBVIEW2_CUSTOM_SCHEME_REGISTRATIONS
        .iter()
        .map(|scheme| scheme.to_string())
        .collect();
    let user_data_folder = configured_webview_user_data_dir().map(|path| {
        let _ = std::fs::create_dir_all(&path);
        path.to_string_lossy().to_string()
    });

    unsafe {
        let registrations = custom_schemes
            .into_iter()
            .map(|scheme| {
                let registration = CoreWebView2CustomSchemeRegistration::new(scheme);
                registration.set_has_authority_component(true);
                registration.set_treat_as_secure(true);
                Some(registration.into())
            })
            .collect();
        options.set_scheme_registrations(registrations);
    }
    let options_iface: ICoreWebView2EnvironmentOptions = options.into();

    let (tx, rx) = mpsc::channel();
    CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| unsafe {
            let user_data_folder = user_data_folder
                .as_ref()
                .map(|path| CoTaskMemPWSTR::from(path.as_str()));
            let user_data_folder = user_data_folder
                .as_ref()
                .map(|path| *path.as_ref().as_pcwstr())
                .unwrap_or(PCWSTR::null());
            CreateCoreWebView2EnvironmentWithOptions(
                windows::core::PCWSTR::null(),
                user_data_folder,
                &options_iface,
                &handler,
            )
            .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |result, environment| {
            result?;
            tx.send(environment.ok_or_else(|| windows::core::Error::from(E_POINTER)))
                .map_err(|_| windows::core::Error::from(E_POINTER))?;
            Ok(())
        }),
    )
    .map_err(map_webview2_error)?;

    rx.recv()
        .map_err(|_| WebViewError::WebView("Environment callback channel failed".to_string()))?
        .map_err(|err| WebViewError::WebView(format!("Environment creation failed: {err}")))
}

pub(crate) fn registered_request_schemes(registered_schemes: &[String]) -> Vec<String> {
    let mut schemes = if registered_schemes.is_empty() {
        vec!["lx".to_string()]
    } else {
        registered_schemes.to_vec()
    };
    schemes.sort_unstable();
    schemes.dedup();
    schemes
}

pub(crate) fn webview2_custom_schemes(registered_schemes: &[String]) -> Vec<String> {
    registered_request_schemes(registered_schemes)
        .into_iter()
        .filter(|scheme| scheme != "http" && scheme != "https")
        .collect()
}

pub(crate) fn create_controller(
    env: &ICoreWebView2Environment,
    hwnd: HWND,
) -> StdResult<ICoreWebView2Controller> {
    let env = env.clone();
    let (tx, rx) = mpsc::channel();

    CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| unsafe {
            env.CreateCoreWebView2Controller(hwnd, &handler)
                .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |result, controller| {
            result?;
            tx.send(controller.ok_or_else(|| windows::core::Error::from(E_POINTER)))
                .map_err(|_| windows::core::Error::from(E_POINTER))?;
            Ok(())
        }),
    )
    .map_err(map_webview2_error)?;

    rx.recv()
        .map_err(|_| WebViewError::WebView("Controller callback channel failed".to_string()))?
        .map_err(|err| WebViewError::WebView(format!("Controller creation failed: {err}")))
}

pub(crate) fn configure_controller(controller: &ICoreWebView2Controller) -> StdResult<()> {
    unsafe {
        controller
            .SetBounds(RECT {
                left: 0,
                top: 0,
                right: 1024,
                bottom: 768,
            })
            .map_err(|err| WebViewError::WebView(format!("SetBounds failed: {err}")))?;
        controller
            .SetIsVisible(true)
            .map_err(|err| WebViewError::WebView(format!("SetIsVisible failed: {err}")))?;
    }
    Ok(())
}

pub(crate) fn configure_settings(webview: &ICoreWebView2) -> StdResult<()> {
    unsafe {
        let settings = webview
            .Settings()
            .map_err(|err| WebViewError::WebView(format!("Settings failed: {err}")))?;
        settings
            .SetIsScriptEnabled(true)
            .map_err(|err| WebViewError::WebView(format!("SetIsScriptEnabled failed: {err}")))?;
        settings
            .SetAreDefaultScriptDialogsEnabled(false)
            .map_err(|err| {
                WebViewError::WebView(format!("SetAreDefaultScriptDialogsEnabled failed: {err}"))
            })?;
        settings.SetIsWebMessageEnabled(true).map_err(|err| {
            WebViewError::WebView(format!("SetIsWebMessageEnabled failed: {err}"))
        })?;
        settings
            .SetIsStatusBarEnabled(false)
            .map_err(|err| WebViewError::WebView(format!("SetIsStatusBarEnabled failed: {err}")))?;
    }
    Ok(())
}

pub(crate) fn install_document_scripts(webview: &ICoreWebView2) -> StdResult<()> {
    let script = r#"
        (function() {
            if (window.__LingXiaWindowsInjected) return;
            window.__LingXiaWindowsInjected = true;

            if (window.chrome && window.chrome.webview && !window.__LingXiaNativeMessageListener) {
                window.__LingXiaNativeMessageListener = true;
                window.chrome.webview.addEventListener('message', function(event) {
                    try {
                        var payload = typeof event.data === 'string' ? event.data : JSON.stringify(event.data);
                        if (typeof window.__LingXiaRecvMessage === 'function') {
                            window.__LingXiaRecvMessage(payload);
                        } else {
                            console.warn('[LingXia] __LingXiaRecvMessage not available');
                        }
                    } catch (e) {}
                });
            }

            window.LingXiaProxy = window.LingXiaProxy || {
                supportsMessagePort: function() { return false; },
                getPort: function() { return ''; },
                postMessage: function(message) {
                    window.chrome && window.chrome.webview && window.chrome.webview.postMessage(String(message));
                }
            };

            if (window.__LingXiaConsoleInjected) return;
            window.__LingXiaConsoleInjected = true;
            ['log', 'info', 'warn', 'error', 'debug'].forEach(function(level) {
                var original = console[level];
                console[level] = function() {
                    try {
                        var msg = Array.prototype.map.call(arguments, function(arg) {
                            return typeof arg === 'object' ? JSON.stringify(arg) : String(arg);
                        }).join(' ');
                        window.chrome && window.chrome.webview && window.chrome.webview.postMessage(JSON.stringify({
                            __lingxia_console__: true,
                            level: level,
                            message: msg
                        }));
                    } catch (e) {}
                    if (original) return original.apply(console, arguments);
                };
            });
        })();
    "#;

    let webview = webview.clone();
    let script = script.to_string();
    AddScriptToExecuteOnDocumentCreatedCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| unsafe {
            let script = CoTaskMemPWSTR::from(script.as_str());
            webview
                .AddScriptToExecuteOnDocumentCreated(*script.as_ref().as_pcwstr(), &handler)
                .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(|result, _id| result),
    )
    .map_err(map_webview2_error)?;

    Ok(())
}

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
        let _ = resp.send(response);
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
        let _ = start_resp.send(Err(WebViewError::WebView(format!(
            "CapturePreview failed: {err}"
        ))));
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
    let handler = ExecuteScriptCompletedHandler::create(Box::new(move |result, json| {
        let outcome = match result {
            Ok(()) => Ok(json),
            Err(err) => Err(WebViewScriptError::Platform(err.to_string())),
        };
        let _ = handler_resp.send(map(outcome));
        Ok(())
    }));

    let started = unsafe {
        let js = CoTaskMemPWSTR::from(js);
        webview.ExecuteScript(*js.as_ref().as_pcwstr(), &handler)
    };
    if let Err(err) = started {
        let _ = resp.send(map(Err(WebViewScriptError::Platform(err.to_string()))));
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
