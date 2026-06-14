//! WebView2 event handler registration (navigation, downloads,
//! messages, resource requests).

use super::*;

pub(crate) fn register_event_handlers(
    env: &ICoreWebView2Environment,
    webview: &ICoreWebView2,
    webtag: WebTag,
    registered_schemes: &[String],
    memory_pages: Arc<Mutex<HashMap<String, Vec<u8>>>>,
) -> StdResult<()> {
    let started_tag = webtag.clone();
    unsafe {
        let mut token = 0;
        webview
            .add_NavigationStarting(
                &NavigationStartingEventHandler::create(Box::new(move |_sender, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };

                    let mut uri = PWSTR::null();
                    args.Uri(&mut uri)?;
                    let uri = CoTaskMemPWSTR::from(uri).to_string();

                    if let Some(webview) = find_webview(&started_tag)
                        && matches!(webview.handle_navigation(&uri), NavigationPolicy::Cancel)
                    {
                        args.SetCancel(true)?;
                        return Ok(());
                    }

                    if let Some(delegate) = find_webview_delegate(&started_tag) {
                        delegate.on_page_started();
                    }
                    Ok(())
                })),
                &mut token,
            )
            .map_err(|err| {
                WebViewError::WebView(format!("add_NavigationStarting failed: {err}"))
            })?;
    }

    let finished_tag = webtag.clone();
    unsafe {
        let mut token = 0;
        webview
            .add_NavigationCompleted(
                &NavigationCompletedEventHandler::create(Box::new(move |_sender, _args| {
                    if let Some(delegate) = find_webview_delegate(&finished_tag) {
                        delegate.on_page_finished();
                    }
                    Ok(())
                })),
                &mut token,
            )
            .map_err(|err| {
                WebViewError::WebView(format!("add_NavigationCompleted failed: {err}"))
            })?;
    }

    let title_tag = webtag.clone();
    unsafe {
        let mut token = 0;
        webview
            .add_DocumentTitleChanged(
                &DocumentTitleChangedEventHandler::create(Box::new(move |sender, _args| {
                    let Some(sender) = sender else {
                        return Ok(());
                    };
                    let mut title = PWSTR::null();
                    sender.DocumentTitle(&mut title)?;
                    let title = CoTaskMemPWSTR::from(title).to_string();
                    if let Some(delegate) = find_webview_delegate(&title_tag) {
                        delegate.on_title_changed(&title);
                    }
                    Ok(())
                })),
                &mut token,
            )
            .map_err(|err| {
                WebViewError::WebView(format!("add_DocumentTitleChanged failed: {err}"))
            })?;
    }

    // Favicon change notifications need ICoreWebView2_15 (newer WebView2
    // runtimes); older runtimes simply do without favicons.
    let favicon_tag = webtag.clone();
    if let Ok(webview15) = webview.cast::<ICoreWebView2_15>() {
        let handler = FaviconChangedEventHandler::create(Box::new(move |sender, _args| {
            let Some(sender) = sender else {
                return Ok(());
            };
            let Ok(sender15) = sender.cast::<ICoreWebView2_15>() else {
                return Ok(());
            };
            let tag = favicon_tag.clone();
            unsafe {
                sender15.GetFavicon(
                    COREWEBVIEW2_FAVICON_IMAGE_FORMAT_PNG,
                    &GetFaviconCompletedHandler::create(Box::new(move |result, stream| {
                        if result.is_err() {
                            return Ok(());
                        }
                        // No stream / empty bytes = page has no favicon.
                        let png_bytes = stream
                            .as_ref()
                            .and_then(|stream| read_stream_to_end(stream).ok())
                            .unwrap_or_default();
                        if let Some(delegate) = find_webview_delegate(&tag) {
                            delegate.on_favicon_changed(png_bytes);
                        }
                        Ok(())
                    })),
                )?;
            }
            Ok(())
        }));
        let mut token = 0;
        if let Err(err) = unsafe { webview15.add_FaviconChanged(&handler, &mut token) } {
            // Favicons are cosmetic; never fail webview creation over them.
            log::warn!("add_FaviconChanged failed: {err}");
        }
    }

    let new_window_tag = webtag.clone();
    unsafe {
        let mut token = 0;
        webview
            .add_NewWindowRequested(
                &NewWindowRequestedEventHandler::create(Box::new(move |sender, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };

                    let uri = take_request_string(|slot| args.Uri(slot))?;
                    let Some(webview) = find_webview(&new_window_tag) else {
                        args.SetHandled(true)?;
                        return Ok(());
                    };

                    match webview.handle_new_window(&uri) {
                        NewWindowPolicy::LoadInSelf => {
                            if let Some(sender) = sender {
                                let uri = CoTaskMemPWSTR::from(uri.as_str());
                                sender.Navigate(*uri.as_ref().as_pcwstr())?;
                            }
                            args.SetHandled(true)?;
                        }
                        NewWindowPolicy::Cancel => {
                            args.SetHandled(true)?;
                        }
                    }
                    Ok(())
                })),
                &mut token,
            )
            .map_err(|err| {
                WebViewError::WebView(format!("add_NewWindowRequested failed: {err}"))
            })?;
    }

    let download_tag = webtag.clone();
    unsafe {
        let webview4: ICoreWebView2_4 = webview.cast().map_err(|err| {
            WebViewError::WebView(format!("WebView2_4 cast failed for downloads: {err}"))
        })?;
        let mut token = 0;
        webview4
            .add_DownloadStarting(
                &DownloadStartingEventHandler::create(Box::new(move |_sender, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };
                    let Some(webview) = find_webview(&download_tag) else {
                        return Ok(());
                    };
                    if !webview.has_download_handler() {
                        return Ok(());
                    }

                    let operation = args.DownloadOperation()?;
                    let request = download_request_from_operation(&operation)?;
                    webview.handle_download(request);
                    args.SetCancel(true)?;
                    Ok(())
                })),
                &mut token,
            )
            .map_err(|err| WebViewError::WebView(format!("add_DownloadStarting failed: {err}")))?;
    }

    let message_tag = webtag.clone();
    unsafe {
        let mut token = 0;
        webview
            .add_WebMessageReceived(
                &WebMessageReceivedEventHandler::create(Box::new(move |_sender, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };

                    let mut message = PWSTR::null();
                    args.TryGetWebMessageAsString(&mut message)?;
                    let payload = CoTaskMemPWSTR::from(message).to_string();

                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&payload) {
                        if json
                            .get("__lingxia_console__")
                            .and_then(|value| value.as_bool())
                            .unwrap_or(false)
                        {
                            if let (Some(level), Some(message)) = (
                                json.get("level").and_then(|value| value.as_str()),
                                json.get("message").and_then(|value| value.as_str()),
                            ) && let Some(delegate) = find_webview_delegate(&message_tag)
                            {
                                let level = match level {
                                    "error" => LogLevel::Error,
                                    "warn" => LogLevel::Warn,
                                    "debug" => LogLevel::Debug,
                                    "info" => LogLevel::Info,
                                    _ => LogLevel::Info,
                                };
                                delegate.log(level, message);
                            }
                            return Ok(());
                        }

                        // Native-component messages (window.NativeComponentBridge)
                        // are dispatched synchronously on this UI thread so
                        // mount/update/unmount ordering is preserved; handlers
                        // marshal their own Win32 work via
                        // UI layers should hop to their own window thread and never block here.
                        if json
                            .get("__lingxia_native_component__")
                            .and_then(|value| value.as_bool())
                            .unwrap_or(false)
                        {
                            if let Some(component_payload) =
                                json.get("payload").and_then(|value| value.as_str())
                                && let Some(delegate) = find_webview_delegate(&message_tag)
                            {
                                delegate.handle_native_component_message(component_payload);
                            }
                            return Ok(());
                        }
                    }

                    if let Some(delegate) = find_webview_delegate(&message_tag) {
                        let _ = thread::Builder::new()
                            .name(format!("lingxia-web-message-{}", message_tag.key()))
                            .spawn(move || delegate.handle_post_message(payload));
                    }
                    Ok(())
                })),
                &mut token,
            )
            .map_err(|err| {
                WebViewError::WebView(format!("add_WebMessageReceived failed: {err}"))
            })?;
    }

    for scheme in registered_request_schemes(registered_schemes) {
        let filter = format!("{scheme}://*");
        let filter = CoTaskMemPWSTR::from(filter.as_str());
        unsafe {
            webview
                .AddWebResourceRequestedFilter(
                    *filter.as_ref().as_pcwstr(),
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
                )
                .map_err(|err| {
                    WebViewError::WebView(format!(
                        "AddWebResourceRequestedFilter failed for {scheme}: {err}"
                    ))
                })?;
        }
    }

    let request_tag = webtag;
    let env = env.clone();
    let memory_pages = memory_pages.clone();
    let custom_schemes = webview2_custom_schemes(registered_schemes);
    unsafe {
        let mut token = 0;
        webview
            .add_WebResourceRequested(
                &WebResourceRequestedEventHandler::create(Box::new(move |_sender, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };

                    let request = args.Request()?;
                    let uri = take_request_string(|slot| request.Uri(slot))?;
                    let method = take_request_string(|slot| request.Method(slot))?;
                    if let Some(html) = find_memory_page(&memory_pages, &uri) {
                        let native_response = build_memory_html_response(&env, html)?;
                        args.SetResponse(&native_response)?;
                        return Ok(());
                    }

                    let body = request
                        .Content()
                        .ok()
                        .and_then(|stream| read_stream_to_end(&stream).ok())
                        .unwrap_or_default();

                    let mut http_request = Request::builder()
                        .method(method.as_str())
                        .uri(uri.as_str())
                        .body(body)
                        .map_err(http_error_to_win)?;
                    populate_request_headers(&request, http_request.headers_mut())?;

                    let scheme = request_scheme(&uri);
                    let response = find_webview(&request_tag)
                        .and_then(|webview| webview.handle_scheme_request(scheme, http_request));

                    let Some(response) = response else {
                        // PassThrough (or no webview found): leave the response
                        // unset so WebView2 default handling proceeds for real
                        // http/https requests. Only custom/app schemes, which
                        // the network stack cannot resolve, get a synthetic 404.
                        if custom_schemes.iter().any(|custom| custom == scheme) {
                            let native_response =
                                build_webview2_response(&env, not_found_response())?;
                            args.SetResponse(&native_response)?;
                        }
                        return Ok(());
                    };

                    let native_response = build_webview2_response(&env, response)?;
                    args.SetResponse(&native_response)?;
                    Ok(())
                })),
                &mut token,
            )
            .map_err(|err| {
                WebViewError::WebView(format!("add_WebResourceRequested failed: {err}"))
            })?;
    }

    Ok(())
}

pub(crate) fn download_request_from_operation(
    operation: &ICoreWebView2DownloadOperation,
) -> WinResult<DownloadRequest> {
    let url = take_request_string(|slot| unsafe { operation.Uri(slot) })?;
    let content_disposition = non_empty(take_request_string(|slot| unsafe {
        operation.ContentDisposition(slot)
    })?);
    let mime_type = non_empty(take_request_string(|slot| unsafe {
        operation.MimeType(slot)
    })?);
    let result_file_path = non_empty(take_request_string(|slot| unsafe {
        operation.ResultFilePath(slot)
    })?);
    let content_length = unsafe {
        let mut total = 0i64;
        operation.TotalBytesToReceive(&mut total)?;
        u64::try_from(total).ok().filter(|value| *value > 0)
    };
    let suggested_filename = result_file_path
        .as_ref()
        .and_then(|path| std::path::Path::new(path).file_name())
        .map(|name| name.to_string_lossy().to_string());

    Ok(DownloadRequest {
        url,
        user_agent: None,
        content_disposition,
        mime_type,
        content_length,
        suggested_filename,
        source_page_url: None,
        cookie: None,
    })
}

pub(crate) fn non_empty(value: String) -> Option<String> {
    let value = value.trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}

pub(crate) fn take_request_string(
    getter: impl FnOnce(*mut PWSTR) -> WinResult<()>,
) -> WinResult<String> {
    let mut value = PWSTR::null();
    getter(&mut value)?;
    Ok(CoTaskMemPWSTR::from(value).to_string())
}
